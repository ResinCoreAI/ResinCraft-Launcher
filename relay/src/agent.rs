//! The launcher's side of a tunnel: one connection to the relay's agent port (TLS + yamux), sign-in, the name
//! picker's requests, and serving players once a name is online. Reconnecting and choosing credentials are up
//! to the caller (the launcher keeps the relay's session token; this module never stores anything).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};

use crate::proto::{
    self, AgentMsg, DataHeader, ErrorCode, Limits, RelayMsg, ServiceState, Usage, ALPN, BEDROCK_PROTOCOL_VERSION, MAX_LINE, PING_SECS,
    PROTOCOL_VERSION, TOPUP_PROTOCOL_VERSION,
};

/// The agent port when a relay URL doesn't name one.
pub const DEFAULT_AGENT_PORT: u16 = 7000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

/// Where the relay is and how to trust it.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// "tls://relay.example.net:7000", "relay.example.net" (TLS, port 7000) or "tcp://127.0.0.1:7000"
    /// (plain TCP, for local testing or a relay behind a tunnel you trust).
    pub url: String,
    /// Extra root certificates (PEM) to trust besides the public web roots, e.g. a test CA.
    pub extra_roots_pem: Option<Vec<u8>>,
    /// Shown in the relay's logs, e.g. "ResinCraft Launcher 0.2.0".
    pub client: String,
}

/// Why something didn't work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    /// The relay said no (`code` decides what the player is told).
    Refused(ErrorCode, String),
    /// The relay couldn't be reached, didn't answer or the connection dropped.
    Network(String),
    /// The relay sent something unexpected.
    Protocol(String),
    /// The relay URL can't be used.
    Config(String),
}

impl AgentError {
    /// Whether trying again later could help.
    pub fn retryable(&self) -> bool {
        match self {
            AgentError::Refused(code, _) => code.retryable(),
            AgentError::Network(_) | AgentError::Protocol(_) => true,
            AgentError::Config(_) => false,
        }
    }

    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            AgentError::Refused(code, _) => Some(*code),
            _ => None,
        }
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::Refused(_, msg) => write!(f, "the relay refused: {msg}"),
            AgentError::Network(msg) => write!(f, "{msg}"),
            AgentError::Protocol(msg) => write!(f, "unexpected answer from the relay: {msg}"),
            AgentError::Config(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AgentError {}

fn net(e: impl std::fmt::Display) -> AgentError {
    AgentError::Network(e.to_string())
}

/// How to sign in.
#[derive(Clone)]
pub enum Credentials {
    /// The relay's session token from an earlier login.
    Session(String),
    /// A Minecraft services access token (checked with Mojang once, never stored by the relay).
    Minecraft(String),
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Credentials::Session(_) => "Session(…)",
            Credentials::Minecraft(_) => "Minecraft(…)",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authed {
    pub uuid: String,
    pub username: String,
    /// A new session token to keep (after a Minecraft-token login).
    pub session: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameStatus {
    pub name: String,
    pub available: bool,
    pub mine: bool,
    pub reason: Option<ErrorCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Online {
    pub name: String,
    pub address: String,
    pub limits: Limits,
    pub usage: Usage,
    /// Seconds until the session ends (None: no time limit).
    pub secs_left: Option<u64>,
    /// A Bedrock tunnel's UDP port on the relay (players add `address` with this port).
    pub bedrock_port: Option<u16>,
}

/// Something that happened while serving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Usage(Usage),
    /// The session ends in `secs_left` seconds unless the host continues ([`Command::Continue`]).
    Warning { secs_left: u64 },
    /// The session was extended (the host continued, or the relay's admin extended it).
    Extended { secs_left: u64, continues: u32 },
    /// The relay's admin flipped the switch (paused: this tunnel goes on until its session ends).
    Service { state: ServiceState, message: Option<String> },
    /// A top-up: the answer to [`Command::TopUp`] (`refused` says why there's none), or the relay's admin added
    /// data (`by_admin`). `usage` is the account's data after it.
    TopUp { added_bytes: u64, by_admin: bool, refused: Option<ErrorCode>, usage: Usage },
}

/// The answer to a top-up asked for outside [`Connection::serve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopUp {
    pub added_bytes: u64,
    pub usage: Usage,
}

/// What the launcher can ask for while serving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Another full session (once the end is near).
    Continue,
    /// More data for this month (once little is left); answered with [`Event::TopUp`]. Relays older than
    /// protocol 4 don't have top-ups: the answer is a refusal right away.
    TopUp,
}

enum Target {
    Tls { host: String, port: u16 },
    Plain { host: String, port: u16 },
}

fn parse_url(url: &str) -> Result<Target, AgentError> {
    let url = url.trim();
    let (tls, rest) = match url.split_once("://") {
        Some(("tls" | "relay", rest)) => (true, rest),
        Some(("tcp", rest)) => (false, rest),
        Some((scheme, _)) => return Err(AgentError::Config(format!("unsupported relay URL scheme {scheme}://"))),
        None => (true, url),
    };
    let rest = rest.trim_end_matches('/');
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') || h.ends_with(']') => {
            (h.trim_matches(['[', ']']).to_string(), p.parse().map_err(|_| AgentError::Config(format!("bad port in relay URL {url}")))?)
        }
        _ => (rest.trim_matches(['[', ']']).to_string(), DEFAULT_AGENT_PORT),
    };
    if host.is_empty() {
        return Err(AgentError::Config(format!("no host in relay URL {url}")));
    }
    Ok(if tls { Target::Tls { host, port } } else { Target::Plain { host, port } })
}

fn client_tls(extra_roots_pem: Option<&[u8]>) -> Result<Arc<rustls::ClientConfig>, AgentError> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::CertificateDer;
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(pem) = extra_roots_pem {
        for cert in CertificateDer::pem_slice_iter(pem) {
            let cert = cert.map_err(|e| AgentError::Config(format!("bad extra root certificate: {e}")))?;
            roots.add(cert).map_err(|e| AgentError::Config(format!("bad extra root certificate: {e}")))?;
        }
    }
    // An explicit provider: the launcher links crates that enable other rustls providers too.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| AgentError::Config(e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    Ok(Arc::new(cfg))
}

/// yamux settings both sides use: a bigger window than the 256 KB default so chunk bursts flow on slower links.
pub fn yamux_config() -> YamuxConfig {
    YamuxConfig { max_stream_window_size: 1024 * 1024, ..YamuxConfig::default() }
}

/// Any byte stream yamux can run on.
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// An open control connection to the relay.
pub struct Connection {
    lines: FramedRead<ReadHalf<StreamHandle>, LinesCodec>,
    writer: WriteHalf<StreamHandle>,
    inbound: mpsc::Receiver<StreamHandle>,
    driver: JoinHandle<()>,
    /// The relay's base domain ("rscraft.resincore.tech").
    pub base_domain: String,
    /// The relay's protocol version (3 and later can carry Bedrock servers).
    pub relay_version: u32,
    seq: u64,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.driver.abort();
    }
}

impl Connection {
    /// Connects and says hello.
    pub async fn connect(cfg: &AgentConfig) -> Result<Connection, AgentError> {
        let target = parse_url(&cfg.url)?;
        let (host, port) = match &target {
            Target::Tls { host, port } | Target::Plain { host, port } => (host.clone(), *port),
        };
        let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host.as_str(), port)))
            .await
            .map_err(|_| net(format!("{host}:{port} didn't answer")))?
            .map_err(|e| net(format!("couldn't connect to {host}:{port}: {e}")))?;
        let _ = tcp.set_nodelay(true);
        let io: Box<dyn Io> = match target {
            Target::Plain { .. } => Box::new(tcp),
            Target::Tls { host, .. } => {
                let connector = tokio_rustls::TlsConnector::from(client_tls(cfg.extra_roots_pem.as_deref())?);
                let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| AgentError::Config(format!("bad relay host {host}")))?;
                let tls = tokio::time::timeout(CONNECT_TIMEOUT, connector.connect(name, tcp))
                    .await
                    .map_err(|_| net(format!("{host} didn't finish the TLS handshake")))?
                    .map_err(|e| net(format!("TLS with {host} failed: {e}")))?;
                Box::new(tls)
            }
        };
        Self::over(io, &cfg.client).await
    }

    /// Runs the protocol over an already open byte stream (tests use in-memory pipes).
    pub async fn over(io: Box<dyn Io>, client: &str) -> Result<Connection, AgentError> {
        let mut session = Session::new_client(io, yamux_config());
        let mut control = session.control();
        let (tx, rx) = mpsc::channel(64);
        let driver = tokio::spawn(async move {
            while let Some(Ok(stream)) = session.next().await {
                if tx.send(stream).await.is_err() {
                    break;
                }
            }
        });
        let stream = match tokio::time::timeout(CONNECT_TIMEOUT, control.open_stream()).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                driver.abort();
                return Err(net(format!("the relay connection closed ({e:?})")));
            }
            Err(_) => {
                driver.abort();
                return Err(net("the relay didn't answer"));
            }
        };
        let (r, w) = tokio::io::split(stream);
        let mut conn = Connection {
            lines: FramedRead::new(r, LinesCodec::new_with_max_length(MAX_LINE)),
            writer: w,
            inbound: rx,
            driver,
            base_domain: String::new(),
            relay_version: 0,
            seq: 0,
        };
        conn.send(&AgentMsg::Hello { version: PROTOCOL_VERSION, client: client.to_string() }).await?;
        match conn.recv().await? {
            RelayMsg::Welcome { base_domain, version } => {
                conn.base_domain = base_domain;
                conn.relay_version = version;
            }
            other => return Err(unexpected(&other)),
        }
        Ok(conn)
    }

    async fn send(&mut self, msg: &AgentMsg) -> Result<(), AgentError> {
        let mut line = serde_json::to_vec(msg).map_err(|e| AgentError::Protocol(e.to_string()))?;
        line.push(b'\n');
        self.writer.write_all(&line).await.map_err(|e| net(format!("the relay connection dropped: {e}")))?;
        self.writer.flush().await.map_err(|e| net(format!("the relay connection dropped: {e}")))
    }

    /// The next message that answers a request (usage reports and pongs are skipped; errors become Err).
    async fn recv(&mut self) -> Result<RelayMsg, AgentError> {
        loop {
            let line = tokio::time::timeout(REPLY_TIMEOUT, self.lines.next()).await.map_err(|_| net("the relay didn't answer"))?;
            let msg = parse_line(line)?;
            match msg {
                RelayMsg::Usage { .. }
                | RelayMsg::Pong { .. }
                | RelayMsg::Service { .. }
                | RelayMsg::SessionWarning { .. }
                | RelayMsg::SessionExtended { .. }
                | RelayMsg::TopUp { by_admin: true, .. } => continue,
                RelayMsg::Error { code, message } => return Err(AgentError::Refused(code, message)),
                RelayMsg::Shutdown { .. } => return Err(AgentError::Refused(ErrorCode::ShuttingDown, "the relay is restarting".into())),
                other => return Ok(other),
            }
        }
    }

    pub async fn auth(&mut self, creds: Credentials) -> Result<Authed, AgentError> {
        let msg = match creds {
            Credentials::Session(session) => AgentMsg::Resume { session },
            Credentials::Minecraft(minecraft_token) => AgentMsg::Login { minecraft_token },
        };
        self.send(&msg).await?;
        match self.recv().await? {
            RelayMsg::Authed { uuid, username, session } => Ok(Authed { uuid, username, session }),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn check(&mut self, name: &str) -> Result<NameStatus, AgentError> {
        self.send(&AgentMsg::Check { name: name.to_string() }).await?;
        match self.recv().await? {
            RelayMsg::NameStatus { name, available, mine, reason } => Ok(NameStatus { name, available, mine, reason }),
            other => Err(unexpected(&other)),
        }
    }

    /// Reserves `name` (freeing `release` when given); returns the address players type.
    pub async fn reserve(&mut self, name: &str, release: Option<&str>) -> Result<String, AgentError> {
        self.send(&AgentMsg::Reserve { name: name.to_string(), release: release.map(str::to_string) }).await?;
        match self.recv().await? {
            RelayMsg::Reserved { address, .. } => Ok(address),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn release(&mut self, name: &str) -> Result<(), AgentError> {
        self.send(&AgentMsg::Release { name: name.to_string() }).await?;
        match self.recv().await? {
            RelayMsg::Released { .. } => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// Whether this relay has data top-ups.
    pub fn has_top_ups(&self) -> bool {
        self.relay_version >= TOPUP_PROTOCOL_VERSION
    }

    /// Asks for more of this month's data (after running out, before bringing a name online again).
    pub async fn top_up(&mut self) -> Result<TopUp, AgentError> {
        if !self.has_top_ups() {
            return Err(AgentError::Refused(ErrorCode::TopUpLimit, "this relay has no top-ups".into()));
        }
        self.send(&AgentMsg::TopUp).await?;
        match self.recv().await? {
            RelayMsg::TopUp { refused: Some(code), .. } => Err(AgentError::Refused(code, crate::proto::top_up_refusal(code).into())),
            RelayMsg::TopUp { added_bytes, usage, .. } => Ok(TopUp { added_bytes, usage }),
            other => Err(unexpected(&other)),
        }
    }

    /// Brings `name` online on this connection. Then call [`Connection::serve`].
    pub async fn online(&mut self, name: &str) -> Result<Online, AgentError> {
        self.online_as(name, false).await
    }

    /// Brings `name` online as a Bedrock server: the relay gives it a UDP port of its own. Then call
    /// [`Connection::serve`] with the Bedrock server's UDP address.
    pub async fn online_bedrock(&mut self, name: &str) -> Result<Online, AgentError> {
        if self.relay_version < BEDROCK_PROTOCOL_VERSION {
            return Err(AgentError::Refused(ErrorCode::BedrockUnavailable, "this relay doesn't carry Bedrock servers yet".into()));
        }
        let online = self.online_as(name, true).await?;
        if online.bedrock_port.is_none() {
            return Err(AgentError::Protocol("the relay brought the name online without a Bedrock port".into()));
        }
        Ok(online)
    }

    async fn online_as(&mut self, name: &str, bedrock: bool) -> Result<Online, AgentError> {
        self.send(&AgentMsg::Online { name: name.to_string(), bedrock }).await?;
        match self.recv().await? {
            RelayMsg::Online { name, address, limits, usage, secs_left, bedrock_port } => Ok(Online { name, address, limits, usage, secs_left, bedrock_port }),
            other => Err(unexpected(&other)),
        }
    }

    /// Joins every player the relay sends to the server at `local`, until the connection ends; returns why it
    /// ended (always an error: the relay restarting, the connection dropping, the name taken over elsewhere…).
    pub async fn serve(self, local: SocketAddr, on_event: impl FnMut(Event) + Send) -> AgentError {
        let (_keep, mut commands) = mpsc::unbounded_channel();
        self.serve_with(local, on_event, &mut commands).await
    }

    /// Like [`Connection::serve`], also sending what arrives on `commands` (Continue) to the relay.
    pub async fn serve_with(
        mut self,
        local: SocketAddr,
        mut on_event: impl FnMut(Event) + Send,
        commands: &mut mpsc::UnboundedReceiver<Command>,
    ) -> AgentError {
        let mut commands_open = true;
        let mut ping = tokio::time::interval(Duration::from_secs(PING_SECS));
        ping.tick().await;
        let silence = Duration::from_secs(PING_SECS * 3);
        let mut deadline = tokio::time::Instant::now() + silence;
        loop {
            tokio::select! {
                // Messages first: the relay's last words (an error, "restarting") arrive just before it hangs up.
                biased;
                line = self.lines.next() => {
                    deadline = tokio::time::Instant::now() + silence;
                    match parse_line(line) {
                        Ok(RelayMsg::Usage { usage }) => on_event(Event::Usage(usage)),
                        Ok(RelayMsg::SessionWarning { secs_left }) => on_event(Event::Warning { secs_left }),
                        Ok(RelayMsg::SessionExtended { secs_left, continues }) => on_event(Event::Extended { secs_left, continues }),
                        Ok(RelayMsg::Service { state, message }) => on_event(Event::Service { state, message }),
                        Ok(RelayMsg::TopUp { added_bytes, by_admin, refused, usage }) => on_event(Event::TopUp { added_bytes, by_admin, refused, usage }),
                        Ok(RelayMsg::Pong { .. }) => {}
                        Ok(RelayMsg::Error { code, message }) => return AgentError::Refused(code, message),
                        Ok(RelayMsg::Shutdown { .. }) => return AgentError::Refused(ErrorCode::ShuttingDown, "the relay is restarting".into()),
                        Ok(_) => {}
                        Err(e) => return e,
                    }
                }
                stream = self.inbound.recv() => match stream {
                    Some(stream) => {
                        tokio::spawn(forward(stream, local));
                    }
                    None => return net("the relay connection dropped"),
                },
                cmd = commands.recv(), if commands_open => match cmd {
                    Some(Command::Continue) => {
                        if let Err(e) = self.send(&AgentMsg::Continue).await {
                            return e;
                        }
                    }
                    Some(Command::TopUp) if !self.has_top_ups() => {
                        on_event(Event::TopUp { added_bytes: 0, by_admin: false, refused: Some(ErrorCode::TopUpLimit), usage: Usage::default() });
                    }
                    Some(Command::TopUp) => {
                        if let Err(e) = self.send(&AgentMsg::TopUp).await {
                            return e;
                        }
                    }
                    None => commands_open = false,
                },
                _ = ping.tick() => {
                    self.seq += 1;
                    let seq = self.seq;
                    if let Err(e) = self.send(&AgentMsg::Ping { seq }).await {
                        return e;
                    }
                }
                _ = tokio::time::sleep_until(deadline) => return net("the relay stopped answering"),
            }
        }
    }
}

fn parse_line(line: Option<Result<String, tokio_util::codec::LinesCodecError>>) -> Result<RelayMsg, AgentError> {
    match line {
        Some(Ok(text)) => serde_json::from_str(&text).map_err(|e| AgentError::Protocol(e.to_string())),
        Some(Err(e)) => Err(net(format!("the relay connection dropped: {e}"))),
        None => Err(net("the relay closed the connection")),
    }
}

fn unexpected(msg: &RelayMsg) -> AgentError {
    AgentError::Protocol(format!("{msg:?}"))
}

/// One player: read the stream's header line, then join the rest to the local server.
async fn forward(stream: StreamHandle, local: SocketAddr) {
    let mut stream = BufReader::new(stream);
    let mut line = Vec::new();
    let header = tokio::time::timeout(Duration::from_secs(10), (&mut stream).take(4096).read_until(b'\n', &mut line)).await;
    if !matches!(header, Ok(Ok(n)) if n > 0 && line.ends_with(b"\n")) {
        return;
    }
    let header: DataHeader = serde_json::from_slice(&line).unwrap_or_default();
    if header.udp {
        return forward_udp(stream, local).await;
    }
    let Ok(mut server) = TcpStream::connect(local).await else {
        // Nothing listens locally (the server is restarting): the player gets disconnected.
        let _ = stream.get_mut().shutdown().await;
        return;
    };
    let _ = server.set_nodelay(true);
    let _ = tokio::io::copy_bidirectional(&mut stream, &mut server).await;
}

/// One Bedrock player: datagrams from the stream go to the local server from a socket of their own (so the
/// server tells players apart by port), and its answers go back as frames.
async fn forward_udp(stream: BufReader<StreamHandle>, local: SocketAddr) {
    let any: SocketAddr = if local.is_ipv4() { ([127, 0, 0, 1], 0).into() } else { (std::net::Ipv6Addr::LOCALHOST, 0).into() };
    let Ok(sock) = UdpSocket::bind(any).await else { return };
    if sock.connect(local).await.is_err() {
        return;
    }
    let (mut from_relay, mut to_relay) = tokio::io::split(stream);
    let up = async {
        let mut packet = Vec::with_capacity(1600);
        while let Ok(true) = proto::read_datagram(&mut from_relay, &mut packet).await {
            // Nothing listening yet (the server is starting): the player's client tries again.
            let _ = sock.send(&packet).await;
        }
    };
    let down = async {
        let mut buf = vec![0u8; 65_536];
        let mut frame = Vec::with_capacity(1600);
        loop {
            let n = match sock.recv(&mut buf).await {
                Ok(n) => n,
                // Windows reports "port unreachable" from an earlier send here.
                Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionRefused) => continue,
                Err(_) => break,
            };
            frame.clear();
            proto::write_datagram(&mut frame, &buf[..n]);
            if to_relay.write_all(&frame).await.is_err() {
                break;
            }
        }
    };
    // The relay closes the stream when the player goes quiet.
    tokio::select! {
        _ = up => {}
        _ = down => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_relay_urls() {
        let ok = |url: &str| match parse_url(url).unwrap() {
            Target::Tls { host, port } => ("tls", host, port),
            Target::Plain { host, port } => ("tcp", host, port),
        };
        assert_eq!(ok("tls://rscraft.resincore.tech:7000"), ("tls", "rscraft.resincore.tech".into(), 7000));
        assert_eq!(ok("rscraft.resincore.tech"), ("tls", "rscraft.resincore.tech".into(), DEFAULT_AGENT_PORT));
        assert_eq!(ok("rscraft.resincore.tech:443/"), ("tls", "rscraft.resincore.tech".into(), 443));
        assert_eq!(ok("tcp://127.0.0.1:7100"), ("tcp", "127.0.0.1".into(), 7100));
        assert_eq!(ok("tcp://[::1]:7100"), ("tcp", "::1".into(), 7100));
        assert!(parse_url("http://rscraft.resincore.tech").is_err());
        assert!(parse_url("tls://:7000").is_err());
        assert!(parse_url("tls://rscraft.resincore.tech:abc").is_err());
    }
}
