//! Play with friends without a server (`friends:*`): the host opens a world to LAN, the launcher sees the port in the
//! game log and offers an invite code; a friend pastes the code, this launcher tunnels a loopback port to the host's
//! world and starts the game into it. Nothing in the game changes, so any version and loader works, and Minecraft's
//! own account check still decides who gets in: the launcher moves bytes and never touches the login.
//!
//! Direct connections need nothing but the code. A host signed in with Microsoft also gets ResinCraft Relay as a
//! fallback for friends who can't reach them directly.

pub mod code;
pub mod guest;
pub mod host;
pub mod offline;
pub mod portmap;
pub mod secure;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use self::code::{Invite, Route, SECRET_LEN};
use self::host::Gate;
use self::portmap::{Mapping, Via};
use crate::core::events::emit;
use crate::core::fsutil::now_ms;
use crate::core::router::{done, json, Args};
use crate::core::text::Text;
use crate::core::types::ServerStatus;
use crate::server::relay::{self, FriendRelay, FriendTunnel};
use crate::{bail_tr, game};

/// How long a code lets new friends in. Those already playing stay until the host stops.
const CODE_LIFETIME_SECS: i64 = 4 * 3600;

/// A world open to LAN in a running game.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanWorld {
    pub run_id: String,
    pub instance_id: String,
    pub port: u16,
    /// Offline players can join it (friends/offline.rs).
    pub offline: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteView {
    pub run_id: String,
    pub instance_id: String,
    pub code: String,
    pub expires_at: i64,
    /// Friends in the tunnel now (a server list ping counts for a moment).
    pub connected: u32,
    /// The router forwards the port, through this.
    pub router: Option<Via>,
    pub public_address: Option<String>,
    pub port: u16,
    pub relay: bool,
    /// Why there's no relay fallback (no Microsoft account, no names left…).
    pub relay_note: Option<Text>,
    /// The relay's session ends in this many seconds unless continued.
    pub relay_ending: Option<u64>,
    /// The code ran out: friends still playing stay, nobody new gets in.
    pub expired: bool,
    /// Offline players can use the code.
    pub offline: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedView {
    /// What to type in the game's Direct Connection if it's already open.
    pub address: String,
    /// "direct", "lan" or "relay".
    pub route: &'static str,
    pub status: Option<ServerStatus>,
    /// The world lets offline players in.
    pub offline: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsState {
    pub worlds: Vec<LanWorld>,
    pub invite: Option<InviteView>,
    pub joined: Option<JoinedView>,
    /// Why the last invite ended by itself: "worldClosed" or "expired".
    pub ended: Option<String>,
}

struct Hosting {
    gate: Arc<Gate>,
    renewals: JoinHandle<()>,
    relay: Option<FriendTunnel>,
}

#[derive(Default)]
struct State {
    view: FriendsState,
    hosting: Option<Hosting>,
    joined: Option<guest::Joined>,
    /// The router mapping, for removing it once the renewals have stopped.
    mapping: Arc<Mutex<Option<Mapping>>>,
    /// Runs whose world lets offline players in, as their agent said.
    offline_runs: HashSet<String>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Mutex::default)
}

fn view() -> FriendsState {
    state().lock().unwrap().view.clone()
}

fn changed() {
    emit("friends", view());
}

fn edit_invite(f: impl FnOnce(&mut InviteView)) {
    if let Some(i) = state().lock().unwrap().view.invite.as_mut() {
        f(i);
    }
    changed();
}

/// Serving on a port. Vanilla, Fabric and Forge log it the same way; old versions say "Started on", 26.2 says
/// "Published LAN server on port". Chat lines are skipped so another player can't fake one.
fn lan_port_of(line: &str) -> Option<u16> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if line.contains("[CHAT]") {
        return None;
    }
    let re = RE.get_or_init(|| Regex::new(r"(?:^|: )(?:Started (?:serving )?on|Published LAN server on port) (\d{1,5})\s*$").unwrap());
    re.captures(line)?[1].parse().ok().filter(|p| *p > 0)
}

fn lan_closed(line: &str) -> bool {
    !line.contains("[CHAT]") && (line.trim_end().ends_with(": Stopping server") || line.trim_end() == "Stopping server" || line.contains(": Stopping singleplayer server"))
}

/// The agent's line, which Minecraft may log as "[STDOUT]: …". Chat can't fake it either.
fn agent_says(line: &str, what: &str) -> bool {
    !line.contains("[CHAT]") && line.trim_end().ends_with(what)
}

/// Every line of a game's log comes through here.
pub fn game_line(run_id: &str, instance_id: &str, line: &str) {
    if agent_says(line, offline::ON_LINE) {
        let mut s = state().lock().unwrap();
        s.offline_runs.insert(run_id.to_string());
        for w in s.view.worlds.iter_mut().filter(|w| w.run_id == run_id) {
            w.offline = true;
        }
    } else if let Some(port) = lan_port_of(line) {
        {
            let mut s = state().lock().unwrap();
            let offline = s.offline_runs.contains(run_id);
            s.view.worlds.retain(|w| w.run_id != run_id);
            s.view.worlds.push(LanWorld { run_id: run_id.to_string(), instance_id: instance_id.to_string(), port, offline });
        }
        changed();
    } else if lan_closed(line) {
        world_gone(run_id);
    }
}

pub fn game_exited(run_id: &str) {
    state().lock().unwrap().offline_runs.remove(run_id);
    world_gone(run_id);
}

fn world_gone(run_id: &str) {
    let (known, inviting) = {
        let mut s = state().lock().unwrap();
        let before = s.view.worlds.len();
        s.view.worlds.retain(|w| w.run_id != run_id);
        (before != s.view.worlds.len(), s.view.invite.as_ref().is_some_and(|i| i.run_id == run_id))
    };
    if inviting {
        tokio::spawn(end_invite(Some("worldClosed")));
    } else if known {
        changed();
    }
}

fn new_secret() -> [u8; SECRET_LEN] {
    let mut secret = [0u8; SECRET_LEN];
    secret.copy_from_slice(&uuid::Uuid::new_v4().as_bytes()[..SECRET_LEN]);
    // UUIDv4 keeps six bits fixed: top them up from a second one.
    for (s, x) in secret.iter_mut().zip(uuid::Uuid::new_v4().as_bytes()[4..].iter()) {
        *s ^= *x;
    }
    secret
}

/// Renews the router mapping until the invite ends, then removes it.
fn keep_mapping(gate: Arc<Gate>, slot: Arc<Mutex<Option<Mapping>>>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let lease = slot.lock().unwrap().as_ref().map(|m| m.lease);
            let Some(lease) = lease.filter(|l| *l > 0) else { break };
            tokio::select! {
                _ = gate.accepting.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs((lease as u64 / 2).max(30))) => {}
            }
            let current = slot.lock().unwrap().clone();
            let Some(mut m) = current else { break };
            if let Err(e) = portmap::renew(&mut m).await {
                eprintln!("[friends] couldn't renew the router mapping: {e:#}");
            }
            *slot.lock().unwrap() = Some(m);
        }
    })
}

/// Sets up everything an invite needs, in parallel: the router, the public address and the relay.
async fn start_invite(run_id: &str) -> Result<FriendsState> {
    let world = {
        let s = state().lock().unwrap();
        if s.view.invite.is_some() {
            return Ok(s.view.clone());
        }
        s.view.worlds.iter().find(|w| w.run_id == run_id).cloned()
    };
    let Some(world) = world else {
        bail_tr!("friends.noWorld", "Open your world to LAN in the game first.");
    };
    if tokio::net::TcpStream::connect(("127.0.0.1", world.port)).await.is_err() {
        world_gone(run_id);
        bail_tr!("friends.noWorld", "Open your world to LAN in the game first.");
    }
    let (port, listeners) = host::listen().await?;
    let relay_door = TcpListener::bind(("127.0.0.1", 0)).await?;
    let relay_port = relay_door.local_addr()?.port();
    let secret = new_secret();
    let gate = Gate::new(secret, world.port);
    let expires = now_ms() / 1000 + CODE_LIFETIME_SECS;

    let (mapped, public, tunnel) = tokio::join!(
        portmap::open(port),
        portmap::public_ipv4(),
        relay::open_for_friends(relay_port, |status| match status {
            FriendRelay::Online => edit_invite(|i| {
                i.relay = true;
                i.relay_ending = None;
            }),
            FriendRelay::Ending { secs_left } => edit_invite(|i| i.relay_ending = Some(secs_left)),
            FriendRelay::Down(why) => edit_invite(|i| {
                i.relay = false;
                i.relay_ending = None;
                i.relay_note = Some(why);
            }),
        })
    );
    let mapped = mapped.inspect_err(|e| eprintln!("[friends] no router mapping: {e:#}")).ok();
    let (tunnel, relay_note) = match tunnel {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(relay::as_text(&e))),
    };

    let mut routes = host::routes(port, mapped.as_ref().map(|m| m.external), public);
    if let Some(t) = &tunnel {
        routes.push(Route::Relay(t.name.clone()));
    }
    let code = Invite { secret, expires: expires as u32, routes, offline: world.offline }.encode();
    let public_address = mapped.as_ref().map(|m| m.external.to_string()).or_else(|| public.map(|ip| format!("{ip}:{port}")));
    let invite = InviteView {
        run_id: world.run_id.clone(),
        instance_id: world.instance_id.clone(),
        code,
        expires_at: expires * 1000,
        connected: 0,
        router: mapped.as_ref().map(|m| m.via),
        public_address,
        port,
        relay: tunnel.is_some(),
        relay_note,
        relay_ending: None,
        expired: false,
        offline: world.offline,
    };

    for l in listeners {
        tokio::spawn(host::serve(gate.clone(), l, false));
    }
    tokio::spawn(host::serve(gate.clone(), relay_door, true));

    let slot = Arc::new(Mutex::new(mapped));
    let renewals = keep_mapping(gate.clone(), slot.clone());
    let current = {
        let mut s = state().lock().unwrap();
        // Two presses at once: the first one stays, this one only cleans up.
        if s.view.invite.is_some() {
            None
        } else {
            s.view.invite = Some(invite);
            s.view.ended = None;
            s.mapping = slot.clone();
            s.hosting = Some(Hosting { gate: gate.clone(), renewals, relay: tunnel });
            Some(s.view.clone())
        }
    };
    let Some(current) = current else {
        gate.stop.cancel();
        return Ok(view());
    };
    watch_invite(gate, expires);
    changed();
    Ok(current)
}

/// Keeps the friend count current, ends the code at its time, and ends the invite when the last friend leaves after it.
fn watch_invite(gate: Arc<Gate>, expires: i64) {
    tokio::spawn(async move {
        let mut last = 0;
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = gate.stop.cancelled() => return,
                _ = tick.tick() => {}
            }
            let connected = gate.connected.load(Ordering::SeqCst);
            if connected != last {
                last = connected;
                edit_invite(|i| i.connected = connected);
            }
            if now_ms() / 1000 >= expires && !gate.accepting.is_cancelled() {
                gate.accepting.cancel();
                edit_invite(|i| i.expired = true);
                close_doors().await;
            }
            if gate.accepting.is_cancelled() && connected == 0 {
                end_invite(Some("expired")).await;
                return;
            }
        }
    });
}

/// Removes the router mapping and the relay name: what lets anyone new in.
async fn close_doors() {
    let (renewals, relay, mapping) = {
        let mut s = state().lock().unwrap();
        let hosting = s.hosting.as_mut();
        let (renewals, relay) = match hosting {
            Some(h) => (Some(h.renewals.abort_handle()), h.relay.take()),
            None => (None, None),
        };
        let mapping = s.mapping.lock().unwrap().take();
        (renewals, relay, mapping)
    };
    if let Some(r) = renewals {
        r.abort();
    }
    let closing = async {
        if let Some(m) = &mapping {
            if let Err(e) = portmap::close(m).await {
                eprintln!("[friends] couldn't remove the router mapping: {e:#}");
            }
        }
    };
    let leaving = async {
        if let Some(t) = relay {
            relay::close_for_friends(t).await;
        }
    };
    tokio::join!(closing, leaving);
}

async fn end_invite(why: Option<&str>) {
    let gate = {
        let mut s = state().lock().unwrap();
        if s.view.invite.take().is_some() {
            s.view.ended = why.map(str::to_string);
        }
        s.hosting.as_ref().map(|h| h.gate.clone())
    };
    if let Some(g) = gate {
        g.stop.cancel();
    }
    changed();
    close_doors().await;
    state().lock().unwrap().hosting = None;
}

fn leave() {
    let joined = {
        let mut s = state().lock().unwrap();
        s.view.joined = None;
        s.joined.take()
    };
    if let Some(j) = joined {
        j.stop.cancel();
    }
    changed();
}

/// Checks a code and opens the tunnel; the world's name, version and players come back through it.
async fn open_code(text: &str) -> Result<JoinedView> {
    let invite = Invite::decode(text)?;
    let offline = invite.offline;
    leave();
    let domain = relay::settings().map(|s| s.domain);
    let joined = guest::open(invite, domain).await?;
    let address = format!("127.0.0.1:{}", joined.local_port);
    let status = crate::player::servers::ping_server(&address, Duration::from_secs(8)).await.ok();
    let shown = JoinedView { address, route: guest::describe(&joined.route), status, offline };
    {
        let mut s = state().lock().unwrap();
        s.joined = Some(joined);
        s.view.joined = Some(shown.clone());
    }
    changed();
    Ok(shown)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinRequest {
    instance_id: String,
    #[serde(default)]
    account_id: Option<String>,
}

/// Starts the game into the open tunnel. A LAN world checks accounts with Mojang like an online server, so an offline
/// profile could only fail after the game has loaded: it's told now, unless the host let offline players in.
async fn join(req: JoinRequest) -> Result<()> {
    let Some((address, offline)) = state().lock().unwrap().view.joined.as_ref().map(|j| (j.address.clone(), j.offline)) else {
        bail_tr!("friends.notConnected", "Paste your friend's code first.");
    };
    let auth = crate::player::accounts::launch_auth_for(req.account_id.as_deref()).await?;
    if auth.user_type == "legacy" && !offline {
        bail_tr!(
            "friends.needsAccount",
            "Joining a friend's world needs a Microsoft account that owns Minecraft, because the world checks your account. Offline profiles can't join."
        );
    }
    game::launch_then(&req.instance_id, req.account_id.as_deref(), Some(game::launch::Target::Server(address)), Some(Box::new(leave))).await
}

/// Takes the router mapping and relay name down when the launcher closes.
pub async fn shutdown() {
    let _ = tokio::time::timeout(Duration::from_secs(5), end_invite(None)).await;
    leave();
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "friends:state" => json(view()),
        "friends:invite" => json(start_invite(&args.get::<String>(0)?).await?),
        "friends:stopInvite" => {
            end_invite(None).await;
            done()
        }
        "friends:relayContinue" => {
            if let Some(t) = state().lock().unwrap().hosting.as_ref().and_then(|h| h.relay.as_ref()) {
                t.continue_session();
            }
            edit_invite(|i| i.relay_ending = None);
            done()
        }
        "friends:open" => json(open_code(&args.get::<String>(0)?).await?),
        "friends:join" => {
            join(args.get(0)?).await?;
            done()
        }
        "friends:leave" => {
            leave();
            done()
        }
        _ => anyhow::bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_port_in_real_log_lines() {
        for line in [
            "[12:01:33] [Server thread/INFO]: Started serving on 51234",
            "[Server thread/INFO]: Started serving on 51234",
            "[12:01:33] [Server thread/INFO] [net.minecraft.server.MinecraftServer/]: Started serving on 51234",
            "[12:01:33] [Server thread/INFO] [minecraft/IntegratedServer]: Started on 51234",
            "Started serving on 51234",
            "[08:55:31] [Render thread/INFO]: Published LAN server on port 51234",
        ] {
            assert_eq!(lan_port_of(line), Some(51234), "{line}");
        }
    }

    #[test]
    fn chat_and_other_lines_are_not_lan_worlds() {
        for line in [
            "[12:01:33] [Render thread/INFO]: [System] [CHAT] <Bob> : Started serving on 1234",
            "[12:01:33] [Render thread/INFO]: [CHAT] Started serving on 1234",
            "[08:55:31] [Render thread/INFO]: [System] [CHAT] <Bob> : Published LAN server on port 1234",
            "[08:55:31] [Render thread/INFO]: [System] [CHAT] This world is now open to LAN. The port number is [62919]",
            "[12:01:33] [main/INFO]: Started serving on 99999",
            "Starting integrated minecraft server version 1.21.1",
            "Started on port 25565",
        ] {
            assert_eq!(lan_port_of(line), None, "{line}");
        }
    }

    #[test]
    fn hears_the_agent_but_not_chat() {
        assert!(agent_says("[09:40:01] [ResinCraft LAN offline/INFO]: [STDOUT]: [ResinCraft] LAN offline on", offline::ON_LINE));
        assert!(agent_says("[ResinCraft] LAN offline on", offline::ON_LINE));
        assert!(!agent_says("[09:40:01] [Render thread/INFO]: [System] [CHAT] <Bob> [ResinCraft] LAN offline on", offline::ON_LINE));
    }

    #[test]
    fn sees_the_world_close() {
        assert!(lan_closed("[12:05:00] [Server thread/INFO]: Stopping server"));
        assert!(lan_closed("[12:05:00] [Server thread/INFO]: Stopping singleplayer server as player logged out"));
        assert!(!lan_closed("[12:05:00] [Render thread/INFO]: [CHAT] <Bob> : Stopping server"));
    }

    #[test]
    fn secrets_differ_and_fill_every_byte() {
        let (a, b) = (new_secret(), new_secret());
        assert_ne!(a, b);
    }
}
