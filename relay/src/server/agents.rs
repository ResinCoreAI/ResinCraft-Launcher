//! The agent port: launchers connect (TLS + yamux), sign in with their Minecraft account, manage names and
//! bring one name online per connection. The first stream the launcher opens is the control stream.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_yamux::{Session, StreamHandle};
use tracing::{debug, info, warn};

use super::auth::{self, ProfileError};
use super::metrics::Metrics;
use super::{Account, Kick, Relay, Tunnel};
use crate::agent::{yamux_config, Io};
use crate::proto::{self, now_secs, AgentMsg, ErrorCode, RelayMsg, MAX_LINE, PING_SECS, PROTOCOL_VERSION};

/// Control messages one connection may send per minute.
const MESSAGES_PER_MINUTE: u32 = 240;

pub(crate) async fn serve(relay: Arc<Relay>, sock: TcpStream, peer: SocketAddr, tls: Option<tokio_rustls::TlsAcceptor>) {
    if relay.is_banned_ip(peer.ip()) {
        Metrics::inc(&relay.metrics.rejected_banned_total);
        return;
    }
    let Some(_ip) = relay.ip_guard(peer.ip(), true) else {
        Metrics::inc(&relay.metrics.rejected_limit_total);
        return;
    };
    let _ = sock.set_nodelay(true);
    let io: Box<dyn Io> = match tls {
        Some(acceptor) => match tokio::time::timeout(Duration::from_secs(10), acceptor.accept(sock)).await {
            Ok(Ok(s)) => Box::new(s),
            _ => {
                debug!(%peer, "agent TLS handshake failed");
                return;
            }
        },
        None => Box::new(sock),
    };
    Metrics::inc(&relay.metrics.agent_connections_total);
    relay.metrics.agent_connections_active.fetch_add(1, Ordering::Relaxed);

    let mut session = Session::new_server(io, yamux_config());
    let control = session.control();
    let (tx, rx) = oneshot::channel();
    let mut driver = tokio::spawn(async move {
        let mut first = Some(tx);
        // The launcher's first stream is the control stream; it has no business opening others.
        while let Some(Ok(stream)) = session.next().await {
            if let Some(tx) = first.take() {
                let _ = tx.send(stream);
            }
        }
    });
    if let Ok(Ok(ctrl)) = tokio::time::timeout(Duration::from_secs(10), rx).await {
        let mut agent = Agent {
            id: relay.next_id(),
            relay: relay.clone(),
            peer,
            control,
            account: None,
            online: None,
            kick: Kick::new(),
            conns: Arc::new(AtomicU32::new(0)),
            bytes_in: Arc::new(AtomicU64::new(0)),
            bytes_out: Arc::new(AtomicU64::new(0)),
            budget: (MESSAGES_PER_MINUTE, tokio::time::Instant::now()),
        };
        agent.run(ctrl).await;
        if let Some(name) = agent.online.take() {
            relay.go_offline(&name, agent.id);
            info!(name = %name, "tunnel offline");
            relay.event("offline", format!("{name} went offline"), None);
        }
        // Let yamux send the last message (an error, "restarting") before the connection goes; the launcher
        // hangs up once it has read it.
        if tokio::time::timeout(Duration::from_secs(3), &mut driver).await.is_err() {
            driver.abort();
        }
    }
    driver.abort();
    relay.metrics.agent_connections_active.fetch_sub(1, Ordering::Relaxed);
}

struct Agent {
    id: u64,
    relay: Arc<Relay>,
    peer: SocketAddr,
    control: tokio_yamux::Control,
    account: Option<Account>,
    online: Option<String>,
    kick: Arc<Kick>,
    conns: Arc<AtomicU32>,
    bytes_in: Arc<AtomicU64>,
    bytes_out: Arc<AtomicU64>,
    budget: (u32, tokio::time::Instant),
}

enum Step {
    Reply(RelayMsg),
    Close(RelayMsg),
}

fn error(code: ErrorCode) -> RelayMsg {
    RelayMsg::Error { code, message: message(code).to_string() }
}

/// English text for each code (the launcher shows its own, translated wording).
pub(crate) fn message(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::BadRequest => "unexpected request",
        ErrorCode::Version => "this launcher is too old for the relay; update ResinCraft Launcher",
        ErrorCode::AuthFailed => "Mojang didn't accept this Minecraft token",
        ErrorCode::NotOwned => "this account doesn't own Minecraft: Java Edition",
        ErrorCode::SessionExpired => "the relay session expired; sign in again",
        ErrorCode::Banned => "this account can't use the relay",
        ErrorCode::NameInvalid => "names are 3-24 lowercase letters, numbers and dashes",
        ErrorCode::NameReserved => "that name is reserved",
        ErrorCode::NameTaken => "that name belongs to another account",
        ErrorCode::TooManyNames => "this account has reserved as many names as it may",
        ErrorCode::TooManyTunnels => "this account has as many servers online as it may",
        ErrorCode::OverQuota => "this account used up this month's relay traffic",
        ErrorCode::RateLimited => "too many attempts; try again later",
        ErrorCode::Unavailable => "Minecraft's account service didn't answer",
        ErrorCode::Replaced => "this name came online somewhere else",
        ErrorCode::Kicked => "the relay's admin disconnected this server",
        ErrorCode::Released => "the relay's admin released or renamed this name",
        ErrorCode::ShuttingDown => "the relay is restarting",
        ErrorCode::Internal | ErrorCode::Unknown => "the relay ran into a problem",
    }
}

async fn send(w: &mut WriteHalf<StreamHandle>, msg: &RelayMsg) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(msg).map_err(std::io::Error::other)?;
    line.push(b'\n');
    w.write_all(&line).await?;
    w.flush().await
}

impl Agent {
    async fn run(&mut self, ctrl: StreamHandle) {
        let (r, mut w) = tokio::io::split(ctrl);
        let mut lines = FramedRead::new(r, LinesCodec::new_with_max_length(MAX_LINE));
        let mut shutdown = self.relay.shutdown_rx();

        let Ok(Some(Ok(line))) = tokio::time::timeout(Duration::from_secs(10), lines.next()).await else { return };
        match serde_json::from_str::<AgentMsg>(&line) {
            Ok(AgentMsg::Hello { version, client }) if version == PROTOCOL_VERSION => {
                debug!(client = %client.chars().take(64).collect::<String>(), "launcher connected");
                let welcome = RelayMsg::Welcome { version: PROTOCOL_VERSION, base_domain: self.relay.cfg.base_domain.clone() };
                if send(&mut w, &welcome).await.is_err() {
                    return;
                }
            }
            Ok(AgentMsg::Hello { .. }) => {
                let _ = send(&mut w, &error(ErrorCode::Version)).await;
                return;
            }
            _ => {
                let _ = send(&mut w, &error(ErrorCode::BadRequest)).await;
                return;
            }
        }
        if *shutdown.borrow() {
            let _ = send(&mut w, &RelayMsg::Shutdown { reconnect_ms: 3000 }).await;
            return;
        }

        let silence = Duration::from_secs(PING_SECS * 3);
        let mut deadline = tokio::time::Instant::now() + silence;
        let mut usage = tokio::time::interval(Duration::from_secs(60));
        usage.tick().await;
        let kick = self.kick.clone();
        loop {
            let reply = tokio::select! {
                line = lines.next() => {
                    deadline = tokio::time::Instant::now() + silence;
                    let Some(Ok(line)) = line else { return };
                    if !self.within_budget() {
                        let _ = send(&mut w, &error(ErrorCode::RateLimited)).await;
                        return;
                    }
                    let Ok(msg) = serde_json::from_str::<AgentMsg>(&line) else {
                        let _ = send(&mut w, &error(ErrorCode::BadRequest)).await;
                        return;
                    };
                    match self.handle(msg).await {
                        Step::Reply(m) => m,
                        Step::Close(m) => {
                            let _ = send(&mut w, &m).await;
                            return;
                        }
                    }
                }
                _ = usage.tick(), if self.online.is_some() => {
                    RelayMsg::Usage { usage: self.usage() }
                }
                reason = kick.wait() => {
                    let _ = send(&mut w, &error(reason)).await;
                    return;
                }
                _ = shutdown.changed() => {
                    let _ = send(&mut w, &RelayMsg::Shutdown { reconnect_ms: 3000 }).await;
                    return;
                }
                _ = tokio::time::sleep_until(deadline) => {
                    debug!("launcher went silent");
                    return;
                }
            };
            if send(&mut w, &reply).await.is_err() {
                return;
            }
        }
    }

    fn within_budget(&mut self) -> bool {
        let (left, since) = &mut self.budget;
        if since.elapsed() >= Duration::from_secs(60) {
            *left = MESSAGES_PER_MINUTE;
            *since = tokio::time::Instant::now();
        }
        if *left == 0 {
            return false;
        }
        *left -= 1;
        true
    }

    fn usage(&self) -> proto::Usage {
        let uuid = self.account.as_ref().map(|a| a.uuid.as_str()).unwrap_or_default();
        self.relay.usage(uuid, self.conns.load(Ordering::Relaxed))
    }

    async fn handle(&mut self, msg: AgentMsg) -> Step {
        let relay = self.relay.clone();
        match msg {
            AgentMsg::Hello { .. } => Step::Close(error(ErrorCode::BadRequest)),
            AgentMsg::Ping { seq } => Step::Reply(RelayMsg::Pong { seq }),
            AgentMsg::Login { minecraft_token } => {
                if self.account.is_some() {
                    return Step::Reply(error(ErrorCode::BadRequest));
                }
                if !relay.login_allowed(self.peer.ip()) {
                    relay.event("auth-fail", "Too many sign-ins from one address", Some(self.peer.ip()));
                    return Step::Reply(error(ErrorCode::RateLimited));
                }
                match auth::verify(&relay.http, &relay.cfg.auth.profile_url, &minecraft_token).await {
                    Ok(profile) => {
                        if relay.is_banned_uuid(&profile.uuid) {
                            Metrics::inc(&relay.metrics.rejected_banned_total);
                            relay.event("auth-fail", format!("Banned account {} ({}) tried to sign in", profile.name, profile.uuid), Some(self.peer.ip()));
                            return Step::Close(error(ErrorCode::Banned));
                        }
                        Metrics::inc(&relay.metrics.logins_total);
                        let session = relay.new_session(&profile.uuid, &profile.name);
                        info!(account = %profile.name, "signed in with Minecraft");
                        relay.event("signin", format!("{} ({}) signed in with Minecraft", profile.name, profile.uuid), Some(self.peer.ip()));
                        self.account = Some(Account { uuid: profile.uuid.clone(), name: profile.name.clone() });
                        Step::Reply(RelayMsg::Authed { uuid: profile.uuid, username: profile.name, session: Some(session) })
                    }
                    Err(e) => {
                        Metrics::inc(&relay.metrics.login_failures_total);
                        let why = match &e {
                            ProfileError::Unauthorized => "Mojang didn't accept the token".to_string(),
                            ProfileError::NoProfile => "the account doesn't own Minecraft".to_string(),
                            ProfileError::Unavailable(w) => format!("Mojang didn't answer ({w})"),
                        };
                        relay.event("auth-fail", format!("Sign-in failed: {why}"), Some(self.peer.ip()));
                        Step::Reply(error(match e {
                            ProfileError::Unauthorized => ErrorCode::AuthFailed,
                            ProfileError::NoProfile => ErrorCode::NotOwned,
                            ProfileError::Unavailable(why) => {
                                warn!(error = %why, "Mojang's profile check failed");
                                ErrorCode::Unavailable
                            }
                        }))
                    }
                }
            }
            AgentMsg::Resume { session } => {
                if self.account.is_some() {
                    return Step::Reply(error(ErrorCode::BadRequest));
                }
                match relay.resume(&session) {
                    Some(acc) if relay.is_banned_uuid(&acc.uuid) => {
                        relay.event("auth-fail", format!("Banned account {} ({}) tried to reconnect", acc.name, acc.uuid), Some(self.peer.ip()));
                        Step::Close(error(ErrorCode::Banned))
                    }
                    Some(acc) => {
                        Metrics::inc(&relay.metrics.resumes_total);
                        let reply = RelayMsg::Authed { uuid: acc.uuid.clone(), username: acc.name.clone(), session: None };
                        self.account = Some(acc);
                        Step::Reply(reply)
                    }
                    None => Step::Reply(error(ErrorCode::SessionExpired)),
                }
            }
            other => {
                let Some(acc) = self.account.clone() else { return Step::Reply(error(ErrorCode::BadRequest)) };
                let base = &relay.cfg.base_domain;
                match other {
                    AgentMsg::Check { name } => Step::Reply(relay.check(&acc.uuid, &name)),
                    AgentMsg::Reserve { name, release } => match relay.reserve(&acc, &name, release.as_deref()) {
                        Ok(()) => {
                            relay.event("name", format!("{} reserved {name}", acc.name), None);
                            Step::Reply(RelayMsg::Reserved { address: proto::address(&name, base), name })
                        }
                        Err(code) => Step::Reply(error(code)),
                    },
                    AgentMsg::Release { name } => match relay.release(&acc, &name) {
                        Ok(()) => Step::Reply(RelayMsg::Released { name }),
                        Err(code) => Step::Reply(error(code)),
                    },
                    AgentMsg::Online { name } => {
                        if self.online.is_some() {
                            return Step::Reply(error(ErrorCode::BadRequest));
                        }
                        let tunnel = Tunnel {
                            owner: acc.uuid.clone(),
                            owner_name: acc.name.clone(),
                            agent: self.id,
                            control: self.control.clone(),
                            conns: self.conns.clone(),
                            bytes_in: self.bytes_in.clone(),
                            bytes_out: self.bytes_out.clone(),
                            kick: self.kick.clone(),
                            since: now_secs(),
                        };
                        match relay.go_online(&acc, &name, tunnel) {
                            Ok(()) => {
                                info!(name = %name, account = %acc.name, "tunnel online");
                                relay.event("online", format!("{name} came online ({})", acc.name), Some(self.peer.ip()));
                                self.online = Some(name.clone());
                                Step::Reply(RelayMsg::Online { address: proto::address(&name, base), name, limits: relay.limits(), usage: self.usage() })
                            }
                            Err(code) => Step::Reply(error(code)),
                        }
                    }
                    _ => Step::Reply(error(ErrorCode::BadRequest)),
                }
            }
        }
    }
}
