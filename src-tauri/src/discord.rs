//! Discord Rich Presence: friends see which instance (or modpack) is being played, its Minecraft version and loader,
//! for how long, and whether it went straight into a world or onto a server.
//!
//! Talks to the local Discord client over its IPC socket (`\\.\pipe\discord-ipc-N` on Windows, a Unix socket in the
//! temp folder elsewhere): frames of `opcode: u32 LE, length: u32 LE, JSON`, a handshake with the app's Client ID,
//! then `SET_ACTIVITY` commands. A worker thread owns the connection; when Discord isn't running it stays quiet and
//! tries again every so often. The app ID comes from `RESINCRAFT_DISCORD_APP_ID` at build time (see build.rs);
//! a build without one never connects.
//!
//! Presence text is English: friends read it in their own Discord, whatever language this launcher uses.

use std::io::{self, Read, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};

use crate::core::fsutil::now_ms;
use crate::core::settings::{self, Settings};
use crate::core::types::{Instance, LoaderType};
use crate::game::launch::Target;

/// The Rich Presence asset (uploaded under the app's Art Assets) shown when there's no modpack icon.
pub const LOGO: &str = "logo";
const LAUNCHER: &str = "ResinCraft Launcher";
/// How often a shown presence is sent again: it reconnects after Discord restarts and notices a closed pipe.
const RESEND: Duration = Duration::from_secs(30);

/// The Discord application ID this build talks to, if it has one.
pub fn app_id() -> Option<String> {
    option_env!("RESINCRAFT_DISCORD_APP_ID")
        .map(str::to_string)
        .or_else(|| std::env::var("RESINCRAFT_DISCORD_APP_ID").ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
}

/// This build can show Rich Presence (an app ID is built in).
pub fn available() -> bool {
    app_id().is_some()
}

// ---------- What to show ----------

/// A running game, as presence shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    pub run_id: String,
    /// The modpack's name for a modpack instance, else the instance's.
    pub name: String,
    pub mc_version: String,
    /// "Fabric 0.16.5"; None for vanilla.
    pub loader: Option<String>,
    /// An https icon (the modpack's); a custom icon is a data URL Discord can't show.
    pub icon: Option<String>,
    pub target: Option<Target>,
    pub started_ms: i64,
}

impl Playing {
    pub fn new(run_id: &str, inst: &Instance, target: Option<&Target>, started_ms: i64) -> Playing {
        let web = |u: &Option<String>| u.clone().filter(|u| u.starts_with("https://") && u.len() <= 256);
        Playing {
            run_id: run_id.to_string(),
            name: inst.modpack.as_ref().map(|m| m.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| inst.name.clone()),
            mc_version: inst.mc_version.clone(),
            loader: (inst.loader != LoaderType::Vanilla).then(|| match inst.loader_version.as_deref().filter(|v| !v.is_empty()) {
                Some(v) => format!("{} {v}", inst.loader.label()),
                None => inst.loader.label().to_string(),
            }),
            icon: web(&inst.icon_url).or_else(|| web(&inst.pack_icon_url)),
            target: target.cloned(),
            started_ms,
        }
    }

    fn version_line(&self) -> String {
        match &self.loader {
            Some(l) => format!("Minecraft {} · {l}", self.mc_version),
            None => format!("Minecraft {}", self.mc_version),
        }
    }
}

/// The settings that shape the presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub enabled: bool,
    pub show_server: bool,
    pub in_launcher: bool,
}

impl Options {
    pub fn from_settings(s: &Settings) -> Options {
        Options { enabled: s.discord_presence, show_server: s.discord_show_server, in_launcher: s.discord_in_launcher }
    }
}

/// Discord wants 2 to 128 characters in its text fields.
fn field(s: &str) -> String {
    let s = s.trim();
    let mut out: String = s.chars().take(128).collect();
    if s.chars().count() > 128 {
        out = out.chars().take(127).collect::<String>() + "…";
    }
    while out.chars().count() < 2 {
        out.push('\u{00A0}');
    }
    out
}

/// "host:25565" reads as just "host".
fn shown_address(addr: &str) -> &str {
    addr.trim().strip_suffix(":25565").unwrap_or(addr.trim())
}

/// The `activity` for `SET_ACTIVITY`: the newest game when one runs, else "In the launcher" if wanted, else none
/// (which clears the presence).
pub fn activity(playing: Option<&Playing>, opts: Options, launcher_since_ms: i64) -> Option<Value> {
    if !opts.enabled {
        return None;
    }
    let Some(p) = playing else {
        return opts.in_launcher.then(|| {
            json!({
                "details": field("In the launcher"),
                "timestamps": { "start": launcher_since_ms },
                "assets": { "large_image": LOGO, "large_text": LAUNCHER },
            })
        });
    };
    let version = p.version_line();
    let state = match &p.target {
        Some(Target::World(_)) => "Singleplayer".to_string(),
        Some(Target::Server(addr)) if opts.show_server => format!("Server: {}", shown_address(addr)),
        Some(Target::Server(_)) => "Multiplayer".to_string(),
        None => version.clone(),
    };
    let assets = match &p.icon {
        Some(icon) => json!({ "large_image": icon, "large_text": field(&version), "small_image": LOGO, "small_text": LAUNCHER }),
        None => json!({ "large_image": LOGO, "large_text": field(&version) }),
    };
    Some(json!({
        "details": field(&p.name),
        "state": field(&state),
        "timestamps": { "start": p.started_ms },
        "assets": assets,
    }))
}

// ---------- IPC ----------

pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;

/// One IPC frame: opcode and length (little-endian u32s), then the JSON payload.
pub fn encode(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

pub fn read_frame(r: &mut impl Read) -> io::Result<(u32, Value)> {
    let mut head = [0u8; 8];
    r.read_exact(&mut head)?;
    let op = u32::from_le_bytes(head[..4].try_into().unwrap());
    let len = u32::from_le_bytes(head[4..].try_into().unwrap()) as usize;
    if len > 1 << 20 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Discord IPC frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

pub fn handshake(app_id: &str) -> Vec<u8> {
    encode(OP_HANDSHAKE, &json!({ "v": 1, "client_id": app_id }))
}

pub fn set_activity(activity: Option<&Value>, pid: u32, nonce: &str) -> Vec<u8> {
    // Without an activity, Discord clears the presence.
    let mut args = json!({ "pid": pid });
    if let Some(a) = activity {
        args["activity"] = a.clone();
    }
    encode(OP_FRAME, &json!({ "cmd": "SET_ACTIVITY", "args": args, "nonce": nonce }))
}

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

/// Where Discord (or one of its other builds: PTB, Canary, Flatpak, Snap) listens.
fn socket_paths() -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    {
        (0..10).map(|i| format!(r"\\.\pipe\discord-ipc-{i}").into()).collect()
    }
    #[cfg(unix)]
    {
        let mut dirs: Vec<std::path::PathBuf> =
            ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().filter_map(|k| std::env::var_os(k)).map(Into::into).collect();
        dirs.push("/tmp".into());
        dirs.dedup();
        let subs = ["", "app/com.discordapp.Discord", "snap.discord", ".flatpak/dev.vencord.Vesktop/xdg-run"];
        dirs.iter().flat_map(|d| subs.iter().flat_map(move |s| (0..10).map(move |i| d.join(s).join(format!("discord-ipc-{i}"))))).collect()
    }
}

fn open(path: &std::path::Path) -> io::Result<Box<dyn Pipe>> {
    #[cfg(windows)]
    {
        Ok(Box::new(std::fs::OpenOptions::new().read(true).write(true).open(path)?))
    }
    #[cfg(unix)]
    {
        let s = std::os::unix::net::UnixStream::connect(path)?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        s.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(Box::new(s))
    }
}

/// A connection to the local Discord client.
pub struct Client {
    pipe: Box<dyn Pipe>,
}

impl Client {
    /// Connects to the first Discord that accepts `app_id`; fails when none is running.
    pub fn connect(app_id: &str) -> io::Result<Client> {
        let mut last = io::Error::new(io::ErrorKind::NotFound, "Discord isn't running");
        for path in socket_paths() {
            let Ok(pipe) = open(&path) else { continue };
            let mut c = Client { pipe };
            match c.send(&handshake(app_id)).and_then(|_| c.reply(None)) {
                Ok(_) => return Ok(c),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        self.pipe.write_all(frame)?;
        self.pipe.flush()
    }

    /// Reads until the answer to `nonce` (or, for the handshake, the READY event), answering pings on the way.
    fn reply(&mut self, nonce: Option<&str>) -> io::Result<Value> {
        for _ in 0..16 {
            let (op, v) = read_frame(&mut self.pipe)?;
            match op {
                OP_CLOSE => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!("Discord closed the connection: {v}"))),
                OP_PING => self.send(&encode(OP_PONG, &v))?,
                OP_FRAME if nonce.is_none_or(|n| v["nonce"] == n) => return Ok(v),
                _ => {}
            }
        }
        Err(io::Error::new(io::ErrorKind::InvalidData, "no answer from Discord"))
    }

    /// Shows `activity`, or clears the presence with None. Discord's answer is returned (an `ERROR` event names a bad field).
    pub fn set_activity(&mut self, activity: Option<&Value>) -> io::Result<Value> {
        let nonce = uuid::Uuid::new_v4().to_string();
        self.send(&set_activity(activity, std::process::id(), &nonce))?;
        self.reply(Some(&nonce))
    }

    pub fn close(mut self) {
        let _ = self.send(&encode(OP_CLOSE, &json!({})));
    }
}

// ---------- The worker ----------

enum Msg {
    Show(Option<Value>),
    Stop(mpsc::Sender<()>),
}

static WORKER: Mutex<Option<mpsc::Sender<Msg>>> = Mutex::new(None);
/// Running games, oldest first; the newest is shown.
static PLAYING: Mutex<Vec<Playing>> = Mutex::new(Vec::new());
static LAUNCHER_SINCE: OnceLock<i64> = OnceLock::new();

/// Keeps Discord showing the latest wanted activity. Quiet when Discord isn't there; tries again every `RESEND`.
fn worker(app_id: String, rx: mpsc::Receiver<Msg>) {
    let mut conn: Option<Client> = None;
    let mut want: Option<Value> = None;
    let mut dirty = false;
    loop {
        match rx.recv_timeout(RESEND) {
            Ok(Msg::Show(a)) => {
                want = a;
                // Games starting and stopping together become one update (Discord allows 5 per 20 s).
                while let Ok(m) = rx.recv_timeout(Duration::from_millis(300)) {
                    match m {
                        Msg::Show(a) => want = a,
                        Msg::Stop(done) => return stop(conn, done),
                    }
                }
                dirty = true;
            }
            Ok(Msg::Stop(done)) => return stop(conn, done),
            Err(RecvTimeoutError::Timeout) => dirty |= want.is_some(),
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if !dirty {
            continue;
        }
        if conn.is_none() {
            // Nothing to show and no connection: Discord already dropped whatever we showed.
            if want.is_none() {
                dirty = false;
                continue;
            }
            conn = Client::connect(&app_id).ok();
        }
        let Some(c) = conn.as_mut() else { continue };
        match c.set_activity(want.as_ref()) {
            Ok(answer) => {
                dirty = false;
                if answer["evt"] == "ERROR" {
                    eprintln!("[discord] {}", answer["data"]);
                }
            }
            Err(_) => conn = None,
        }
    }
}

fn stop(conn: Option<Client>, done: mpsc::Sender<()>) {
    if let Some(mut c) = conn {
        let _ = c.set_activity(None);
        c.close();
    }
    let _ = done.send(());
}

fn show(activity: Option<Value>) {
    let Some(id) = app_id() else { return };
    let mut worker = WORKER.lock().unwrap();
    if worker.is_none() {
        if activity.is_none() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        if std::thread::Builder::new().name("discord".into()).spawn(move || worker_main(id, rx)).is_err() {
            return;
        }
        *worker = Some(tx);
    }
    if let Some(tx) = worker.as_ref() {
        if tx.send(Msg::Show(activity)).is_err() {
            *worker = None;
        }
    }
}

fn worker_main(id: String, rx: mpsc::Receiver<Msg>) {
    worker(id, rx);
    *WORKER.lock().unwrap() = None;
}

/// Shows what's going on now with the current settings. Call after the settings change.
pub async fn refresh() {
    if !available() {
        return;
    }
    let opts = Options::from_settings(&settings::get().await);
    let since = *LAUNCHER_SINCE.get_or_init(now_ms);
    let newest = PLAYING.lock().unwrap().last().cloned();
    show(activity(newest.as_ref(), opts, since));
}

/// The launcher opened (shows "In the launcher" when that's turned on).
pub async fn init() {
    LAUNCHER_SINCE.get_or_init(now_ms);
    refresh().await;
}

/// A game is up.
pub async fn started(run_id: &str, inst: &Instance, target: Option<&Target>) {
    {
        let mut playing = PLAYING.lock().unwrap();
        playing.retain(|p| p.run_id != run_id);
        playing.push(Playing::new(run_id, inst, target, now_ms()));
    }
    refresh().await;
}

/// A game ended.
pub async fn stopped(run_id: &str) {
    PLAYING.lock().unwrap().retain(|p| p.run_id != run_id);
    refresh().await;
}

/// The launcher is closing: clear the presence (waits up to two seconds for Discord).
pub fn shutdown() {
    let Some(tx) = WORKER.lock().unwrap().take() else { return };
    let (done_tx, done_rx) = mpsc::channel();
    if tx.send(Msg::Stop(done_tx)).is_ok() {
        let _ = done_rx.recv_timeout(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ModpackRef;

    fn instance() -> Instance {
        serde_json::from_value(json!({
            "id": "abc", "name": "My Fabric world", "mcVersion": "1.21.1", "loader": "fabric", "loaderVersion": "0.16.5",
            "color": "#fff", "createdAt": 0
        }))
        .unwrap()
    }

    const ON: Options = Options { enabled: true, show_server: false, in_launcher: false };

    #[test]
    fn a_game_shows_name_version_loader_and_start() {
        let p = Playing::new("abc", &instance(), None, 1_700_000_000_000);
        let a = activity(Some(&p), ON, 0).unwrap();
        assert_eq!(a["details"], "My Fabric world");
        assert_eq!(a["state"], "Minecraft 1.21.1 · Fabric 0.16.5");
        assert_eq!(a["timestamps"]["start"], 1_700_000_000_000i64);
        assert_eq!(a["assets"], json!({ "large_image": "logo", "large_text": "Minecraft 1.21.1 · Fabric 0.16.5" }));
    }

    #[test]
    fn vanilla_has_no_loader() {
        let mut inst = instance();
        inst.loader = LoaderType::Vanilla;
        inst.loader_version = None;
        let a = activity(Some(&Playing::new("abc", &inst, None, 0)), ON, 0).unwrap();
        assert_eq!(a["state"], "Minecraft 1.21.1");
    }

    #[test]
    fn modpacks_show_their_name_and_web_icon() {
        let mut inst = instance();
        inst.modpack = Some(ModpackRef { project_id: Some("1".into()), version_id: None, name: "All the Mods 10".into() });
        inst.icon_url = Some("https://cdn.modrinth.com/data/abc/icon.png".into());
        let a = activity(Some(&Playing::new("abc", &inst, None, 0)), ON, 0).unwrap();
        assert_eq!(a["details"], "All the Mods 10");
        assert_eq!(a["assets"]["large_image"], "https://cdn.modrinth.com/data/abc/icon.png");
        assert_eq!(a["assets"]["small_image"], "logo");
        // A custom icon is a data URL: back to the modpack's own icon, or the logo.
        inst.icon_url = Some("data:image/png;base64,AAAA".into());
        assert_eq!(Playing::new("abc", &inst, None, 0).icon, None);
        inst.pack_icon_url = Some("https://media.forgecdn.net/avatars/1.png".into());
        assert_eq!(Playing::new("abc", &inst, None, 0).icon.as_deref(), Some("https://media.forgecdn.net/avatars/1.png"));
    }

    #[test]
    fn targets_show_singleplayer_or_server_only_when_allowed() {
        let state = |target: Target, show_server: bool| {
            let p = Playing::new("abc", &instance(), Some(&target), 0);
            activity(Some(&p), Options { show_server, ..ON }, 0).unwrap()["state"].clone()
        };
        assert_eq!(state(Target::World("New World".into()), true), "Singleplayer");
        assert_eq!(state(Target::Server("play.example.org:25565".into()), false), "Multiplayer");
        assert_eq!(state(Target::Server("play.example.org:25565".into()), true), "Server: play.example.org");
        assert_eq!(state(Target::Server("play.example.org:25570".into()), true), "Server: play.example.org:25570");
    }

    #[test]
    fn launcher_presence_only_when_wanted_and_off_clears() {
        assert_eq!(activity(None, ON, 5), None);
        let a = activity(None, Options { in_launcher: true, ..ON }, 5).unwrap();
        assert_eq!(a["details"], "In the launcher");
        assert_eq!(a["timestamps"]["start"], 5);
        let p = Playing::new("abc", &instance(), None, 0);
        assert_eq!(activity(Some(&p), Options { enabled: false, ..ON }, 0), None);
    }

    #[test]
    fn text_fields_fit_discord_limits() {
        assert_eq!(field("A").chars().count(), 2);
        let long = field(&"x".repeat(300));
        assert_eq!(long.chars().count(), 128);
        assert!(long.ends_with('…'));
        assert_eq!(field("  Pack  "), "Pack");
    }

    #[test]
    fn frames_round_trip() {
        let bytes = handshake("1234567890");
        assert_eq!(&bytes[..4], &0u32.to_le_bytes());
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize, bytes.len() - 8);
        let (op, v) = read_frame(&mut &bytes[..]).unwrap();
        assert_eq!((op, v), (OP_HANDSHAKE, json!({ "v": 1, "client_id": "1234567890" })));

        let a = json!({ "details": "Pack" });
        let (op, v) = read_frame(&mut &set_activity(Some(&a), 42, "n1")[..]).unwrap();
        assert_eq!(op, OP_FRAME);
        assert_eq!(v, json!({ "cmd": "SET_ACTIVITY", "args": { "pid": 42, "activity": { "details": "Pack" } }, "nonce": "n1" }));
        // Clearing sends no activity at all.
        let (_, v) = read_frame(&mut &set_activity(None, 42, "n2")[..]).unwrap();
        assert_eq!(v["args"], json!({ "pid": 42 }));
    }

    #[test]
    fn a_fake_discord_gets_handshake_and_activity() {
        // Discord's side of a conversation: READY, then the answer to SET_ACTIVITY (its nonce is echoed below).
        struct Fake {
            input: Vec<u8>,
            output: io::Cursor<Vec<u8>>,
        }
        impl Read for Fake {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.output.position() as usize == self.output.get_ref().len() {
                    // Answer the latest command with its nonce.
                    let mut r = &self.input[..];
                    let mut nonce = Value::Null;
                    while let Ok((_, v)) = read_frame(&mut r) {
                        nonce = v["nonce"].clone();
                    }
                    let mut out = encode(OP_PING, &json!({}));
                    out.extend(encode(OP_FRAME, &json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": nonce })));
                    self.output = io::Cursor::new(out);
                }
                self.output.read(buf)
            }
        }
        impl Write for Fake {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.input.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let ready = encode(OP_FRAME, &json!({ "cmd": "DISPATCH", "evt": "READY" }));
        let mut c = Client { pipe: Box::new(Fake { input: Vec::new(), output: io::Cursor::new(ready) }) };
        assert_eq!(c.reply(None).unwrap()["evt"], "READY");
        let answer = c.set_activity(Some(&json!({ "details": "Pack" }))).unwrap();
        assert_eq!(answer["cmd"], "SET_ACTIVITY");
    }
}
