//! The command list behind the server console's suggestions (`server:commands`). Three sources, labelled by where a
//! command comes from:
//!
//! 1. The running server. Mods register commands in code, so only it can name them. Bukkit servers answer `bukkit:help`
//!    (plain `help` can belong to a plugin, EssentialsX's does) with every command, description and namespace in one
//!    burst to the console. Vanilla, Fabric, Quilt, Forge and NeoForge answer `help` with one `/name usage` line per
//!    command. Both go out at most once a run on their own, plus Refresh, and only with nobody online. The reply is
//!    hidden from the console, its log and the remote view, but the server writes it to its own logs/latest.log.
//!    Hybrids, Sponge, Minecraft before 1.13 (paged help) and proxies (Velocity and BungeeCord have no `help`) are
//!    never asked.
//! 2. `commands:` in plugin.yml of the jars in plugins/, which also work with the server stopped.
//! 3. A short built-in list per kind, only until the server has answered: its answer is the truth, built-ins it
//!    doesn't name are dropped.
//!
//! The last answer is kept in the server's folder. A stopped server shows it with whatever plugin.yml adds.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::content::index::{open_zip, read_zip_text};
use crate::content::jarmeta;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::router::json;
use crate::core::types::ServerKind;
use crate::core::Args;

use super::{runtime, store};

const CACHE_FILE: &str = "resincraft-commands.json";
/// Late plugins log for a while after "Done"; ask after them.
const ASK_AFTER_READY: Duration = Duration::from_secs(12);
/// How often the first question looks again while players are on.
const ASK_RETRY: Duration = Duration::from_secs(30);
/// The reply is over once this passes without a line of it.
const QUIET: Duration = Duration::from_millis(1500);
/// An unanswered question is dropped after this.
const GIVE_UP: Duration = Duration::from_secs(15);
const REFRESH_EVERY: Duration = Duration::from_secs(30);
/// A reply to what someone just typed could still be on its way; ours would be taken for it.
const TYPED_GAP: Duration = Duration::from_secs(3);
const WATCH_EVERY: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleCommand {
    /// Without the slash; `luckperms:lp` when that's how the server lists it.
    pub name: String,
    /// The plugin or mod's name, or the game's. None when it can't be told; the UI says "other".
    pub source: Option<String>,
    /// minecraft, server, plugin, mod or proxy.
    pub source_kind: String,
    pub usage: Option<String>,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    /// From our short list, not from this server or its files.
    pub built_in: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandList {
    pub commands: Vec<ConsoleCommand>,
    /// When the server last answered (Unix ms), from this run or an earlier one.
    pub live_at: Option<i64>,
    /// The answer is from the run going on now.
    pub current: bool,
    /// A question is out or about to be: look again soon.
    pub asking: bool,
    /// This kind of server can be asked.
    pub can_ask: bool,
    /// Why Refresh didn't ask: stopped, players, wait or unsupported.
    pub refused: Option<&'static str>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Live {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// The last answer, on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cache {
    kind: String,
    at: i64,
    commands: Vec<Live>,
}

fn kind_name(kind: ServerKind) -> String {
    format!("{kind:?}").to_lowercase()
}

// ---------- Which servers can be asked ----------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    Bukkit,
    Brigadier,
}

impl Format {
    fn command(self) -> &'static str {
        match self {
            Format::Bukkit => "bukkit:help",
            Format::Brigadier => "help",
        }
    }
}

fn format_of(kind: ServerKind, mc_version: &str) -> Option<Format> {
    if !brigadier_era(mc_version) {
        return None;
    }
    use ServerKind::*;
    match kind {
        _ if kind.bukkit() => Some(Format::Bukkit),
        Vanilla | Fabric | Quilt | Forge | Neoforge => Some(Format::Brigadier),
        _ => None,
    }
}

/// 1.13 or later (`26.2` is later).
fn brigadier_era(version: &str) -> bool {
    version.strip_prefix("1.").is_none_or(|rest| rest.split(['.', '-']).next().and_then(|m| m.parse::<u32>().ok()).is_some_and(|m| m >= 13))
}

// ---------- Reading the reply ----------

fn reply_res() -> &'static [Regex; 5] {
    static RE: OnceLock<[Regex; 5]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            Regex::new(r"^(?:\[[^\]]*\]:?\s*)+").unwrap(),
            Regex::new(r"^-+ Help: Index\b").unwrap(),
            Regex::new(r"^[\w .-]+:(?: |$)").unwrap(),
            Regex::new(r"^/(\S+?):(?:\s+(.*))?$").unwrap(),
            Regex::new(r"^/([A-Za-z0-9_.:?-]+)(?:\s+(.*))?$").unwrap(),
        ]
    })
}

/// A console line without its log prefix or colour codes.
fn body(line: &str) -> String {
    let plain = strip_colours(line);
    reply_res()[0].replace(&plain, "").trim().to_string()
}

fn strip_colours(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"§[0-9a-fk-orxA-FK-ORX]").unwrap()).replace_all(text, "").into_owned()
}

fn text_of(c: Option<regex::Match>) -> Option<String> {
    c.map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty())
}

/// "/name: description" (the name may hold a namespace's colon).
fn bukkit_line(body: &str) -> Option<Live> {
    let c = reply_res()[3].captures(body)?;
    Some(Live { name: c[1].to_string(), usage: None, description: text_of(c.get(2)) })
}

/// "/name usage".
fn brigadier_line(body: &str) -> Option<Live> {
    let c = reply_res()[4].captures(body)?;
    Some(Live { name: c[1].to_string(), usage: text_of(c.get(2)), description: None })
}

/// One reply being read off the console.
struct Capture {
    format: Format,
    asked: Instant,
    /// Its first line has come.
    started: bool,
    /// Past the topics of Bukkit's index.
    listing: bool,
    last: Instant,
    /// Someone typed a command: this reply ends here, so theirs isn't swallowed.
    closed: bool,
    found: Vec<Live>,
}

impl Capture {
    fn new(format: Format, now: Instant) -> Self {
        Capture { format, asked: now, started: false, listing: false, last: now, closed: false, found: Vec::new() }
    }

    fn over(&self, now: Instant) -> bool {
        self.closed || if self.started { now.duration_since(self.last) >= QUIET } else { now.duration_since(self.asked) >= GIVE_UP }
    }

    /// True when the line is part of the reply, which keeps it off the console. Anything else keeps going through,
    /// even in the middle of the reply: another thread may log there.
    fn feed(&mut self, line: &str, now: Instant) -> bool {
        if self.over(now) {
            return false;
        }
        let body = body(line);
        let mine = match self.format {
            Format::Brigadier => brigadier_line(&body).map(|l| self.found.push(l)).is_some(),
            Format::Bukkit if !self.started => reply_res()[1].is_match(&body),
            Format::Bukkit => {
                if let Some(l) = bukkit_line(&body) {
                    self.listing = true;
                    self.found.push(l);
                    true
                } else {
                    !self.listing && (body.starts_with("Use /help") || reply_res()[2].is_match(&body))
                }
            }
        };
        if mine {
            self.started = true;
            self.last = now;
        }
        mine
    }
}

// ---------- Asking ----------

#[derive(Default)]
struct State {
    run: u64,
    capture: Option<Capture>,
    /// The first question of this run is still to go.
    waiting: bool,
    asked: Option<Instant>,
    answered: bool,
    typed: Option<Instant>,
}

static STATES: LazyLock<Mutex<HashMap<String, State>>> = LazyLock::new(Default::default);

/// The server's state for this run, fresh when it's a new one.
fn state<'a>(all: &'a mut HashMap<String, State>, id: &str, run: u64) -> &'a mut State {
    let s = all.entry(id.to_string()).or_default();
    if s.run != run {
        *s = State { run, ..Default::default() };
    }
    s
}

/// Takes the reply to our own question off the console; true when the line is hidden.
pub fn hide_line(id: &str, text: &str) -> bool {
    let mut all = STATES.lock().unwrap();
    all.get_mut(id).and_then(|s| s.capture.as_mut()).is_some_and(|c| c.feed(text, Instant::now()))
}

/// A command someone sent to the console: `help` from a person shows normally.
pub fn note_typed(id: &str) {
    let mut all = STATES.lock().unwrap();
    if let Some(s) = all.get_mut(id) {
        s.typed = Some(Instant::now());
        if let Some(c) = s.capture.as_mut() {
            c.closed = true;
        }
    }
}

fn players_on(id: &str) -> bool {
    runtime::online(id).unwrap_or(0) > 0 || !runtime::players(id).is_empty()
}

/// The server is up: ask once, when it has settled and nobody's on.
pub fn on_ready(id: &str) {
    let Some(run) = runtime::server_run(id) else { return };
    let id = id.to_string();
    tokio::spawn(async move {
        let Ok(meta) = store::get(&id).await else { return };
        let Some(format) = format_of(meta.kind, &meta.mc_version) else { return };
        state(&mut STATES.lock().unwrap(), &id, run).waiting = true;
        tokio::time::sleep(ASK_AFTER_READY).await;
        loop {
            if runtime::server_run(&id) != Some(run) {
                return;
            }
            if runtime::state(&id) == "running" && !players_on(&id) && begin(&id, run, format, Instant::now()) {
                return;
            }
            tokio::time::sleep(ASK_RETRY).await;
        }
    });
}

/// Sends the question unless one is out or someone just typed; true when this ended the wait for the first one.
fn begin(id: &str, run: u64, format: Format, now: Instant) -> bool {
    let mut all = STATES.lock().unwrap();
    let s = state(&mut all, id, run);
    let settled = s.typed.is_none_or(|t| now.duration_since(t) >= TYPED_GAP);
    if s.capture.is_some() || !settled {
        return false;
    }
    s.capture = Some(Capture::new(format, now));
    s.asked = Some(now);
    s.waiting = false;
    drop(all);
    if runtime::quiet_command(id, format.command()).is_err() {
        STATES.lock().unwrap().get_mut(id).map(|s| s.capture = None);
        return true;
    }
    tokio::spawn(watch(id.to_string(), run));
    true
}

/// Waits out the reply and keeps it.
async fn watch(id: String, run: u64) {
    loop {
        tokio::time::sleep(WATCH_EVERY).await;
        let done = {
            let mut all = STATES.lock().unwrap();
            let Some(s) = all.get_mut(&id).filter(|s| s.run == run) else { return };
            match s.capture.as_ref().is_some_and(|c| c.over(Instant::now()) || runtime::server_run(&id) != Some(run)) {
                true => s.capture.take(),
                false => None,
            }
        };
        let Some(c) = done else { continue };
        if c.found.len() < MIN_ANSWER {
            return;
        }
        let Ok(meta) = store::get(&id).await else { return };
        let Ok(dir) = store::dir(&id) else { return };
        let cache = Cache { kind: kind_name(meta.kind), at: now_ms(), commands: c.found };
        if write_json(&dir.join(CACHE_FILE), &cache).await.is_ok() {
            if let Some(s) = STATES.lock().unwrap().get_mut(&id).filter(|s| s.run == run) {
                s.answered = true;
            }
        }
        return;
    }
}

/// Fewer lines than this aren't a list of commands: something else looked like the reply.
const MIN_ANSWER: usize = 5;

// ---------- plugin.yml ----------

#[derive(Debug, Clone, Default, PartialEq)]
struct YmlCommand {
    name: String,
    usage: Option<String>,
    description: Option<String>,
    aliases: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct PluginCommands {
    plugin: String,
    commands: Vec<YmlCommand>,
}

/// `key: value`, with `key:` giving an empty value. None for lines without a key.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let at = if line.ends_with(':') { line.len() - 1 } else { line.find(": ")? };
    Some((line[..at].trim(), line[at + 1..].trim()))
}

/// A scalar without its quotes or trailing comment.
fn scalar(value: &str) -> String {
    let v = value.trim();
    for q in ['"', '\''] {
        if let Some(rest) = v.strip_prefix(q) {
            let inner = rest.rfind(q).map_or(rest, |end| &rest[..end]);
            return if q == '"' { inner.replace("\\n", "\n").replace("\\\"", "\"") } else { inner.replace("''", "'") };
        }
    }
    v.split(" #").next().unwrap_or_default().trim().to_string()
}

fn alias_list(value: &str, into: &mut Vec<String>) {
    let v = value.trim();
    let inner = v.strip_prefix('[').map_or(v, |r| r.split(']').next().unwrap_or_default());
    into.extend(inner.split(',').map(scalar).filter(|a| !a.is_empty()));
}

/// Text a jar left unfilled, as nothing: `${usage}`.
fn tidy(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let plain = strip_colours(text);
    let plain = RE.get_or_init(|| Regex::new(r"&[0-9a-fk-or]").unwrap()).replace_all(&plain, "");
    Some(plain.trim().to_string()).filter(|t| !t.is_empty() && !t.contains("${"))
}

/// Usage only says something past the command itself.
fn tidy_usage(usage: &str, name: &str) -> Option<String> {
    let filled = usage.replace("<command>", name);
    tidy(&filled).filter(|u| !u.trim_start_matches('/').eq_ignore_ascii_case(name))
}

/// The `commands:` map of a plugin.yml. It's hand-read like the rest of depcheck's YAML: names at one indent,
/// properties deeper, `aliases` as a flow list, a `- x` list or one name, text over several lines joined.
fn parse_commands(text: &str) -> Vec<YmlCommand> {
    let mut lines = text.lines();
    if !lines.any(|l| key_value(l.trim_end()).is_some_and(|(k, v)| !l.starts_with([' ', '\t']) && k == "commands" && scalar(v).is_empty())) {
        return Vec::new();
    }
    let mut out: Vec<YmlCommand> = Vec::new();
    let (mut base, mut props, mut key) = (None, None, "");
    for line in lines {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        if indent == 0 {
            break;
        }
        if indent <= *base.get_or_insert(indent) {
            if let Some((name, _)) = key_value(t) {
                out.push(YmlCommand { name: scalar(name), ..Default::default() });
                (props, key) = (None, "");
            }
            continue;
        }
        let Some(cmd) = out.last_mut() else { continue };
        if indent == *props.get_or_insert(indent) && !t.starts_with('-') {
            if let Some((k, v)) = key_value(t) {
                key = k;
                match k {
                    "aliases" => alias_list(v, &mut cmd.aliases),
                    "usage" if !v.starts_with(['|', '>']) => cmd.usage = Some(scalar(v)),
                    "description" if !v.starts_with(['|', '>']) => cmd.description = Some(scalar(v)),
                    _ => {}
                }
                continue;
            }
        }
        match key {
            "aliases" => alias_list(t.trim_start_matches('-'), &mut cmd.aliases),
            "usage" => cmd.usage = Some(cmd.usage.take().map_or(t.to_string(), |u| format!("{u}\n{t}"))),
            "description" => cmd.description = Some(cmd.description.take().map_or(t.to_string(), |d| format!("{d} {t}"))),
            _ => {}
        }
    }
    for c in &mut out {
        c.usage = c.usage.as_deref().and_then(|u| tidy_usage(u, &c.name));
        c.description = c.description.as_deref().and_then(tidy);
    }
    out.retain(|c| !c.name.is_empty());
    out
}

/// plugin.yml's top-level `name`.
fn plugin_name(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix("name:")).map(scalar).filter(|n| !n.is_empty())
}

fn read_plugin(path: &Path) -> Option<PluginCommands> {
    let mut zip = open_zip(path).ok()?;
    let mut name = None;
    let mut commands = Vec::new();
    for file in ["plugin.yml", "paper-plugin.yml"] {
        let Some(text) = read_zip_text(&mut zip, file).ok().flatten() else { continue };
        name = name.or_else(|| plugin_name(&text));
        if commands.is_empty() {
            commands = parse_commands(&text);
        }
    }
    Some(PluginCommands { plugin: name?, commands })
}

fn jars(folder: &Path) -> Vec<std::path::PathBuf> {
    let Ok(dir) = std::fs::read_dir(folder) else { return Vec::new() };
    dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar"))).collect()
}

fn scan_plugins(dir: &Path) -> Vec<PluginCommands> {
    let mut all: Vec<PluginCommands> = jars(&dir.join("plugins")).iter().filter_map(|p| read_plugin(p)).collect();
    all.sort_by(|a, b| a.plugin.cmp(&b.plugin));
    all
}

fn scan_mods(dir: &Path) -> Vec<ModInfo> {
    let mut all: Vec<ModInfo> = jars(&dir.join("mods"))
        .iter()
        .map(|p| jarmeta::read_jar(p))
        .filter(|m| !m.ids.is_empty())
        .map(|m| ModInfo { name: m.name.clone().unwrap_or_else(|| m.ids[0].clone()), ids: m.ids })
        .collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    all
}

// ---------- Built-ins ----------

/// Everything vanilla servers can have, to tell the game's commands from mods' in a `help` reply (which doesn't say).
const VANILLA: &[&str] = &[
    "advancement", "attribute", "ban", "ban-ip", "banlist", "bossbar", "clear", "clone", "damage", "data", "datapack", "debug", "defaultgamemode", "deop", "dialog",
    "difficulty", "effect", "enchant", "execute", "experience", "fetchprofile", "fill", "fillbiome", "forceload", "function", "gamemode", "gamerule", "give", "help",
    "item", "jfr", "kick", "kill", "list", "locate", "locatebiome", "locate", "loot", "me", "msg", "op", "pardon", "pardon-ip", "particle", "perf", "place",
    "placefeature", "playsound", "publish", "random", "recipe", "reload", "replaceitem", "return", "ride", "rotate", "save-all", "save-off", "save-on", "say",
    "schedule", "scoreboard", "seed", "setblock", "setidletimeout", "setworldspawn", "spawnpoint", "spectate", "spreadplayers", "stop", "stopsound", "stopwatch",
    "summon", "swing", "tag", "team", "teammsg", "teleport", "tell", "tellraw", "test", "tick", "time", "title", "tm", "tp", "transfer", "trigger", "version", "w",
    "waypoint", "weather", "whitelist", "worldborder", "xp",
];

/// In every Minecraft server since 1.8.
const MINECRAFT_ALWAYS: &[&str] = &[
    "ban", "ban-ip", "banlist", "clear", "deop", "difficulty", "effect", "enchant", "fill", "gamemode", "gamerule", "give", "help", "kick", "kill", "list", "me", "msg",
    "op", "pardon", "pardon-ip", "particle", "playsound", "save-all", "save-off", "save-on", "say", "scoreboard", "seed", "setblock", "setworldspawn", "spawnpoint",
    "stop", "summon", "tell", "time", "title", "tp", "weather", "whitelist", "worldborder", "xp",
];
const MINECRAFT_SINCE_1_13: &[&str] = &["advancement", "bossbar", "data", "datapack", "execute", "experience", "function", "locate", "reload", "teleport"];
const BEDROCK: &[&str] = &["clear", "deop", "difficulty", "effect", "gamemode", "gamerule", "give", "help", "kick", "kill", "list", "op", "reload", "say", "setworldspawn", "stop", "summon", "tell", "time", "title", "tp", "weather"];
const VELOCITY: &[&str] = &["glist", "send", "shutdown", "velocity"];
const BUNGEE: &[&str] = &["alert", "bungee", "end", "find", "glist", "greload", "ip", "perms", "send"];

fn built_ins(kind: ServerKind, mc_version: &str) -> Vec<ConsoleCommand> {
    use ServerKind::*;
    let of = |names: &[&str], source: &str, source_kind: &str| -> Vec<ConsoleCommand> {
        names.iter().map(|n| ConsoleCommand { name: n.to_string(), source: Some(source.to_string()), source_kind: source_kind.to_string(), built_in: true, ..Default::default() }).collect()
    };
    match kind {
        Velocity => of(VELOCITY, kind.label(), "proxy"),
        Bungeecord | Waterfall | Travertine => of(BUNGEE, kind.label(), "proxy"),
        Bedrock => of(BEDROCK, "Minecraft", "minecraft"),
        _ if kind.bukkit() || kind.hybrid() || matches!(kind, Vanilla | Sponge | Fabric | Quilt | Forge | Neoforge) => {
            let mut all = of(MINECRAFT_ALWAYS, "Minecraft", "minecraft");
            if brigadier_era(mc_version) {
                all.extend(of(MINECRAFT_SINCE_1_13, "Minecraft", "minecraft"));
            }
            if kind.bukkit() {
                all.extend(of(&["plugins", "version"], kind.label(), "server"));
            }
            all
        }
        _ => Vec::new(),
    }
}

// ---------- Combining ----------

#[derive(Debug, Clone, Default, PartialEq)]
struct ModInfo {
    ids: Vec<String>,
    name: String,
}

/// Lowercase with only letters and digits, so `Just Enough Items` meets `justenoughitems`.
fn squeezed(text: &str) -> String {
    text.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// The mod a command belongs to when its name says so: the mod's id or name, or the id and more (`ftbquestsx`). A
/// name that merely resembles a mod stays unlabelled: no source beats the wrong one.
fn mod_of<'a>(name: &str, mods: &'a [ModInfo]) -> Option<&'a ModInfo> {
    let name = squeezed(name);
    mods.iter()
        .find(|m| m.ids.iter().any(|i| squeezed(i) == name) || squeezed(&m.name) == name)
        .or_else(|| mods.iter().filter(|m| m.ids.iter().any(|i| i.len() >= 4 && name.starts_with(&squeezed(i)))).max_by_key(|m| m.ids.iter().map(String::len).max()))
}

fn labelled(cmd: &mut ConsoleCommand, source: &str, kind: &str) {
    cmd.source = Some(source.to_string());
    cmd.source_kind = kind.to_string();
}

/// What a namespace in a Bukkit server's list stands for.
fn label_namespace(cmd: &mut ConsoleCommand, ns: &str, kind: ServerKind, plugins: &[PluginCommands]) {
    match ns {
        "minecraft" => labelled(cmd, "Minecraft", "minecraft"),
        "bukkit" => labelled(cmd, "Bukkit", "server"),
        "spigot" => labelled(cmd, "Spigot", "server"),
        "paper" => labelled(cmd, "Paper", "server"),
        n if n == kind_name(kind) => labelled(cmd, kind.label(), "server"),
        n => labelled(cmd, plugins.iter().find(|p| p.plugin.eq_ignore_ascii_case(n)).map_or(n, |p| p.plugin.as_str()), "plugin"),
    }
}

fn split_namespace(name: &str) -> (Option<&str>, &str) {
    name.split_once(':').map_or((None, name), |(ns, plain)| (Some(ns), plain))
}

/// All sources into one list, each command once, plain names before namespaced ones.
fn merge(kind: ServerKind, mc_version: &str, live: Option<&[Live]>, current: bool, plugins: &[PluginCommands], mods: &[ModInfo]) -> Vec<ConsoleCommand> {
    const MOJANG: &str = "A Mojang provided command.";
    let mut owner: HashMap<String, (&PluginCommands, &YmlCommand)> = HashMap::new();
    let mut alias_of: HashMap<String, &YmlCommand> = HashMap::new();
    for p in plugins {
        for c in &p.commands {
            owner.entry(c.name.to_lowercase()).or_insert((p, c));
            for a in &c.aliases {
                alias_of.entry(a.to_lowercase()).or_insert(c);
            }
        }
    }
    let mut all: BTreeMap<String, ConsoleCommand> = BTreeMap::new();
    let mut aliases: Vec<(String, String)> = Vec::new();

    // Which namespaces list each plain name, the game's last.
    let mut namespaces: HashMap<String, Vec<&str>> = HashMap::new();
    for (ns, plain) in live.unwrap_or_default().iter().filter_map(|l| Some((split_namespace(&l.name).0?, split_namespace(&l.name).1))) {
        let list = namespaces.entry(plain.to_lowercase()).or_default();
        if ns == "minecraft" {
            list.push(ns);
        } else {
            list.insert(0, ns);
        }
    }
    let bukkit = kind.bukkit();
    for l in live.unwrap_or_default() {
        let (ns, plain) = split_namespace(&l.name);
        let lower = plain.to_lowercase();
        // `/tp -> teleport`.
        if let Some(target) = l.usage.as_deref().and_then(|u| u.strip_prefix("-> ")) {
            aliases.push((target.trim().to_lowercase(), l.name.clone()));
            continue;
        }
        if !owner.contains_key(&lower) {
            if let Some(c) = alias_of.get(&lower) {
                if ns.is_none() {
                    aliases.push((c.name.to_lowercase(), l.name.clone()));
                }
                continue;
            }
        }
        let mojang = l.description.as_deref() == Some(MOJANG);
        let mut cmd = ConsoleCommand {
            name: l.name.clone(),
            source_kind: "other".to_string(),
            usage: l.usage.clone(),
            description: l.description.clone().filter(|_| !mojang),
            ..Default::default()
        };
        if let Some(ns) = ns {
            label_namespace(&mut cmd, ns, kind, plugins);
        } else if let Some((p, _)) = owner.get(&lower) {
            labelled(&mut cmd, &p.plugin, "plugin");
        } else if bukkit {
            match namespaces.get(&lower) {
                _ if mojang => labelled(&mut cmd, "Minecraft", "minecraft"),
                Some(found) => label_namespace(&mut cmd, found[0], kind, plugins),
                None => labelled(&mut cmd, kind.label(), "server"),
            }
        } else if VANILLA.contains(&lower.as_str()) {
            labelled(&mut cmd, "Minecraft", "minecraft");
        } else if lower == kind_name(kind) {
            labelled(&mut cmd, kind.label(), "server");
        } else if let Some(m) = mod_of(&lower, mods) {
            labelled(&mut cmd, &m.name, "mod");
        } else if !kind.modded() {
            labelled(&mut cmd, kind.label(), "server");
        }
        all.insert(l.name.to_lowercase(), cmd);
    }

    // plugin.yml says more about what the server listed, and alone names what a stopped server's list doesn't have
    // yet. With this run's answer in, a command it left out isn't there, whatever the jar says.
    for p in plugins {
        for c in &p.commands {
            let key = c.name.to_lowercase();
            if current && !all.contains_key(&key) {
                continue;
            }
            let cmd = all.entry(key).or_insert_with(|| ConsoleCommand { name: c.name.clone(), ..Default::default() });
            labelled(cmd, &p.plugin, "plugin");
            cmd.usage = c.usage.clone().or(cmd.usage.take());
            cmd.description = c.description.clone().or(cmd.description.take());
            cmd.aliases.extend(c.aliases.iter().cloned());
        }
    }
    if live.is_none() {
        for b in built_ins(kind, mc_version) {
            all.entry(b.name.to_lowercase()).or_insert(b);
        }
    }
    for (target, alias) in aliases {
        if let Some(cmd) = all.get_mut(&target) {
            cmd.aliases.push(alias);
        }
    }
    let mut out: Vec<ConsoleCommand> = all.into_values().collect();
    for c in &mut out {
        c.aliases.sort();
        c.aliases.dedup();
    }
    out.sort_by_key(|c| c.name.contains(':'));
    out
}

// ---------- Requests ----------

pub async fn list(id: &str) -> Result<CommandList> {
    let meta = store::get(id).await?;
    let dir = store::dir(id)?;
    let cache = read_json::<Cache>(&dir.join(CACHE_FILE)).await.filter(|c| c.kind == kind_name(meta.kind));
    let run = runtime::server_run(id);
    let (current, waiting, capturing) = {
        let all = STATES.lock().unwrap();
        all.get(id).filter(|s| Some(s.run) == run).map_or((false, false, false), |s| (s.answered, s.waiting, s.capture.is_some()))
    };
    let (kind, has_plugins, has_mods) = (meta.kind, meta.kind.bukkit() || meta.kind.hybrid(), meta.kind.modded() && cache.is_some());
    let (plugins, mods) = tokio::task::spawn_blocking(move || (if has_plugins { scan_plugins(&dir) } else { Vec::new() }, if has_mods { scan_mods(&dir) } else { Vec::new() })).await?;
    Ok(CommandList {
        commands: merge(kind, &meta.mc_version, cache.as_ref().map(|c| c.commands.as_slice()), current, &plugins, &mods),
        live_at: cache.as_ref().map(|c| c.at),
        current,
        // While players keep it waiting nothing is on its way, and the screen needn't keep looking.
        asking: capturing || (waiting && !players_on(id)),
        can_ask: format_of(kind, &meta.mc_version).is_some(),
        refused: None,
    })
}

/// Why this can't ask now, if so.
fn refusal(id: &str, format: Option<Format>, now: Instant) -> Option<&'static str> {
    if format.is_none() {
        return Some("unsupported");
    }
    let Some(run) = runtime::server_run(id).filter(|_| runtime::state(id) == "running") else { return Some("stopped") };
    if players_on(id) {
        return Some("players");
    }
    let all = STATES.lock().unwrap();
    let s = all.get(id).filter(|s| s.run == run);
    let soon = |t: Option<Instant>, gap: Duration| t.is_some_and(|t| now.duration_since(t) < gap);
    s.is_some_and(|s| s.capture.is_some() || soon(s.asked, REFRESH_EVERY) || soon(s.typed, TYPED_GAP)).then_some("wait")
}

pub async fn refresh(id: &str) -> Result<CommandList> {
    let meta = store::get(id).await?;
    let format = format_of(meta.kind, &meta.mc_version);
    let refused = refusal(id, format, Instant::now());
    if let (None, Some(format), Some(run)) = (refused, format, runtime::server_run(id)) {
        begin(id, run, format, Instant::now());
    }
    Ok(CommandList { refused, ..list(id).await? })
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    let id = args.get::<String>(0)?;
    match channel {
        "server:commandsRefresh" => json(refresh(&id).await?),
        _ => json(list(&id).await?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(c: &Capture, ms: u64) -> Instant {
        c.asked + Duration::from_millis(ms)
    }

    fn live(name: &str, usage: Option<&str>, description: Option<&str>) -> Live {
        Live { name: name.to_string(), usage: usage.map(str::to_string), description: description.map(str::to_string) }
    }

    /// As Paper 26.2 really prints it to the console.
    const PAPER: &[&str] = &[
        "[21:07:13 INFO]: --------- Help: Index ---------------------------------",
        "[21:07:13 INFO]: Use /help [n] to get page n of help.",
        "[21:07:13 INFO]: Aliases: Lists command aliases",
        "[21:07:13 INFO]: PlugManX: All commands for PlugManX",
        "[21:07:13 INFO]: /about: Gets the version of this server including any plugins in use",
        "[21:07:13 INFO]: /bukkit:about: Gets the version of this server including any plugins in use",
        "[21:07:13 INFO]: /ban: A Mojang provided command.",
        "[21:07:13 INFO]: /minecraft:ban: A Mojang provided command.",
        "[21:07:13 INFO]: /plm:",
        "[21:07:13 INFO]: /plugman:",
        "[21:07:13 INFO]: /plugmanx:plm:",
        "[21:07:13 INFO]: /plugmanx:plugman:",
        "[21:07:13 INFO]: /restart: Restarts the server",
        "[21:07:13 INFO]: /spark:",
        "[21:07:13 INFO]: /paper:spark:",
        "[21:07:13 INFO]: /viaversion: Shows ViaVersion Version and more.",
    ];

    fn paper_reply() -> Vec<Live> {
        let mut c = Capture::new(Format::Bukkit, Instant::now());
        let now = c.asked;
        assert!(PAPER.iter().all(|l| c.feed(l, now)), "every line of the reply is hidden");
        c.found
    }

    fn plugins() -> Vec<PluginCommands> {
        vec![
            PluginCommands {
                plugin: "PlugManX".into(),
                commands: vec![YmlCommand { name: "plugman".into(), usage: Some("/plugman list".into()), description: Some("Manage plugins.".into()), aliases: vec!["plm".into()] }],
            },
            PluginCommands { plugin: "ViaVersion".into(), commands: vec![YmlCommand { name: "viaversion".into(), aliases: vec!["viaver".into()], ..Default::default() }] },
        ]
    }

    fn find<'a>(list: &'a [ConsoleCommand], name: &str) -> &'a ConsoleCommand {
        list.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} missing"))
    }

    #[test]
    fn reads_a_bukkit_reply_and_nothing_else() {
        let found = paper_reply();
        assert_eq!(found.len(), 12);
        assert_eq!(found[0], live("about", None, Some("Gets the version of this server including any plugins in use")));
        assert_eq!(found[1].name, "bukkit:about");
        assert_eq!(found[4], live("plm", None, None));
        assert_eq!(found[6].name, "plugmanx:plm");
    }

    #[test]
    fn bukkit_lines_show_until_the_header_comes() {
        let mut c = Capture::new(Format::Bukkit, Instant::now());
        let now = c.asked;
        assert!(!c.feed("[21:07:13 INFO]: /about: looks like one", now));
        assert!(!c.feed("[21:07:13 INFO]: Aliases: looks like one", now));
        assert!(c.found.is_empty());
    }

    #[test]
    fn another_threads_line_in_the_middle_still_shows() {
        let mut c = Capture::new(Format::Bukkit, Instant::now());
        let now = c.asked;
        assert!(c.feed(PAPER[0], now));
        assert!(!c.feed("[21:07:13 INFO]: [ViaVersion] Finished mapping loading", now), "not a topic");
        assert!(c.feed(PAPER[4], now));
        assert!(!c.feed("[21:07:13 WARN]: Can't keep up!", now));
        assert!(!c.feed("[21:07:13 INFO]: Aliases: a topic after the list isn't one", now));
        assert!(c.feed(PAPER[6], now));
        assert_eq!(c.found.len(), 2);
    }

    #[test]
    fn colour_codes_and_ansi_free_lines_read_alike() {
        let mut c = Capture::new(Format::Bukkit, Instant::now());
        let now = c.asked;
        assert!(c.feed("[21:07:13 INFO]: §e--------- §fHelp: Index (1/1) §e------", now));
        assert!(c.feed("[21:07:13 INFO]: §6/tps: §fGets the current ticks per second", now));
        assert_eq!(c.found, [live("tps", None, Some("Gets the current ticks per second"))]);
    }

    #[test]
    fn reads_a_brigadier_reply_from_each_log_format() {
        let mut c = Capture::new(Format::Brigadier, Instant::now());
        let now = c.asked;
        assert!(c.feed("[21:07:13] [Server thread/INFO] [minecraft/MinecraftServer]: /advancement (grant|revoke)", now));
        assert!(c.feed("[21:07:13] [Server thread/INFO]: /tp -> teleport", now));
        assert!(c.feed("[21:07:13 INFO]: /ftbquests", now));
        assert!(!c.feed("[21:07:13] [Server thread/INFO]: <Steve> /ftbquests", now));
        assert!(!c.feed("[21:07:13] [Server thread/INFO]: Saved the game", now));
        assert_eq!(c.found[0], live("advancement", Some("(grant|revoke)"), None));
        assert_eq!(c.found[1], live("tp", Some("-> teleport"), None));
        assert_eq!(c.found.len(), 3);
    }

    #[test]
    fn the_reply_ends_when_it_goes_quiet_so_a_later_help_shows() {
        let mut c = Capture::new(Format::Brigadier, Instant::now());
        assert!(c.feed("/a", at(&c, 0)));
        assert!(!c.over(at(&c, 1000)));
        assert!(c.feed("/b", at(&c, 1000)));
        assert!(c.over(at(&c, 2600)));
        assert!(!c.feed("/c", at(&c, 2600)));
    }

    #[test]
    fn typing_ends_the_reply_at_once() {
        let mut c = Capture::new(Format::Brigadier, Instant::now());
        assert!(c.feed("/a", at(&c, 0)));
        c.closed = true;
        assert!(!c.feed("/b", at(&c, 1)));
    }

    #[test]
    fn an_unanswered_question_is_dropped() {
        let c = Capture::new(Format::Bukkit, Instant::now());
        assert!(!c.over(at(&c, 14_000)));
        assert!(c.over(at(&c, 15_000)));
    }

    #[test]
    fn only_known_kinds_are_asked() {
        use ServerKind::*;
        for k in [Paper, Purpur, Spigot, Craftbukkit, Leaf, Pufferfish, Folia] {
            assert_eq!(format_of(k, "1.21.4"), Some(Format::Bukkit), "{k:?}");
        }
        for k in [Vanilla, Fabric, Quilt, Forge, Neoforge] {
            assert_eq!(format_of(k, "1.20.1"), Some(Format::Brigadier), "{k:?}");
        }
        for k in [Velocity, Bungeecord, Bedrock, Arclight, Mohist, Sponge, Spongeforge, Cuberite, Geyser, Pocketmine] {
            assert_eq!(format_of(k, "1.20.1"), None, "{k:?}");
        }
        assert_eq!(format_of(Paper, "26.2"), Some(Format::Bukkit));
        assert_eq!(format_of(Forge, "1.12.2"), None, "paged help");
        assert_eq!(format_of(Paper, "1.8.8"), None);
    }

    #[test]
    fn reads_plugin_yml_commands() {
        let yml = "name: Demo\nmain: a.B\ncommands:\n  demo:\n    description: Does things # really\n    usage: /<command> [x]\n    aliases: [d, 'de']\n  'other':\n    aliases: o\n  third:\n    aliases:\n      - t1\n      - t2\n    permission: demo.x\n  bare:\n  long:\n    usage: |\n      /long a\n      /long b\n    description: >-\n      Two\n      lines\npermissions:\n  demo.x:\n    default: op\n";
        let c = parse_commands(yml);
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["demo", "other", "third", "bare", "long"]);
        assert_eq!(c[0].description.as_deref(), Some("Does things"));
        assert_eq!(c[0].usage.as_deref(), Some("/demo [x]"));
        assert_eq!(c[0].aliases, ["d", "de"]);
        assert_eq!(c[1].aliases, ["o"]);
        assert_eq!(c[2].aliases, ["t1", "t2"]);
        assert_eq!(c[3], YmlCommand { name: "bare".into(), ..Default::default() });
        assert_eq!(c[4].usage.as_deref(), Some("/long a\n/long b"));
        assert_eq!(c[4].description.as_deref(), Some("Two lines"));
    }

    #[test]
    fn reads_real_plugin_yml_shapes() {
        // PlugManX: a plain scalar carried over lines.
        let plugman = "name: PlugManX\ncommands:\n  plugman:\n    aliases: [ plm ]\n    description: Manage plugins.\n    permission: plugman.help\n    usage: /plugman (help|dump)\n      /plugman list [-v]\npermissions:\n  plugman.admin:\n    description: x\n";
        let c = parse_commands(plugman);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].aliases, ["plm"]);
        assert_eq!(c[0].usage.as_deref(), Some("/plugman (help|dump)\n/plugman list [-v]"));
        // CRLF, a comment on the name line, and no commands at all.
        let crlf = "name: X\r\ncommands:\r\n  viaversion:\r\n    permission: a # why\r\n    aliases: [ viaver, vvbukkit ]\r\n";
        assert_eq!(parse_commands(crlf)[0].aliases, ["viaver", "vvbukkit"]);
        assert!(parse_commands("name: X\ncommands: {}\n").is_empty());
        assert!(parse_commands("name: X\nmain: y\n").is_empty());
        assert!(parse_commands("name: X\npermissions:\n  commands:\n    a:\n      description: no\n").is_empty());
        assert_eq!(plugin_name("main: a\nname: \"My Plugin\"\nversion: 1\n").as_deref(), Some("My Plugin"));
    }

    #[test]
    fn useless_usage_and_placeholders_go() {
        assert_eq!(tidy_usage("/<command>", "demo"), None);
        assert_eq!(tidy_usage("/demo", "demo"), None);
        assert_eq!(tidy_usage("${usage}", "demo"), None);
        assert_eq!(tidy_usage("&c/<command> <a>", "demo").as_deref(), Some("/demo <a>"));
    }

    #[test]
    fn a_paper_answer_is_labelled_by_plugin_and_namespace() {
        let found = paper_reply();
        let list = merge(ServerKind::Paper, "26.2", Some(&found), true, &plugins(), &[]);
        assert_eq!(find(&list, "ban").source.as_deref(), Some("Minecraft"));
        assert_eq!(find(&list, "ban").description, None, "the stock line says nothing");
        assert_eq!(find(&list, "about").source.as_deref(), Some("Bukkit"));
        assert_eq!(find(&list, "restart").source.as_deref(), Some("Paper"));
        assert_eq!(find(&list, "spark").source.as_deref(), Some("Paper"));
        let plugman = find(&list, "plugman");
        assert_eq!((plugman.source.as_deref(), plugman.source_kind.as_str()), (Some("PlugManX"), "plugin"));
        assert_eq!(plugman.usage.as_deref(), Some("/plugman list"));
        assert_eq!(plugman.aliases, ["plm"]);
        assert_eq!(find(&list, "plugmanx:plugman").source.as_deref(), Some("PlugManX"));
        assert!(!list.iter().any(|c| c.name == "plm" || c.name == "plugmanx:plm"), "an alias isn't a command of its own");
        assert_eq!(find(&list, "viaversion").description.as_deref(), Some("Shows ViaVersion Version and more."));
    }

    #[test]
    fn namespaced_names_rank_below_plain_ones() {
        let list = merge(ServerKind::Paper, "26.2", Some(&paper_reply()), true, &plugins(), &[]);
        let first = list.iter().position(|c| c.name.contains(':')).unwrap();
        assert!(list[first..].iter().all(|c| c.name.contains(':')));
    }

    #[test]
    fn each_command_appears_once() {
        let list = merge(ServerKind::Paper, "26.2", Some(&paper_reply()), false, &plugins(), &[]);
        let mut names: Vec<String> = list.iter().map(|c| c.name.to_lowercase()).collect();
        names.sort();
        let all = names.len();
        names.dedup();
        assert_eq!(names.len(), all);
    }

    #[test]
    fn a_stopped_server_shows_what_its_files_say() {
        let list = merge(ServerKind::Paper, "26.2", None, false, &plugins(), &[]);
        assert_eq!(find(&list, "plugman").source.as_deref(), Some("PlugManX"));
        assert!(find(&list, "give").built_in);
        assert!(!find(&list, "plugman").built_in);
        assert!(find(&list, "plugins").built_in);
    }

    #[test]
    fn an_answer_replaces_built_ins_and_this_runs_answer_drops_absent_plugins() {
        let found = vec![live("list", None, None)];
        let list = merge(ServerKind::Paper, "26.2", Some(&found), true, &plugins(), &[]);
        assert_eq!(list.len(), 1);
        // A saved answer from an earlier run, with a plugin added since.
        let list = merge(ServerKind::Paper, "26.2", Some(&found), false, &plugins(), &[]);
        assert!(!list.iter().any(|c| c.built_in));
        assert_eq!(find(&list, "plugman").source.as_deref(), Some("PlugManX"));
    }

    #[test]
    fn a_forge_answer_is_labelled_by_game_and_mod() {
        let found = vec![
            live("advancement", Some("(grant|revoke)"), None),
            live("forge", Some("(tps|track|entity|generate|dimensions|mods|tags)"), None),
            live("ftbquests", Some("(editing_mode|reload)"), None),
            live("just-enough-items", None, None),
            live("wand", Some("(a|b)"), None),
            live("teleport", Some("(<location>|<destination>|<targets>)"), None),
            live("tp", Some("-> teleport"), None),
        ];
        let mods = [
            ModInfo { ids: vec!["ftbquests".into()], name: "FTB Quests".into() },
            ModInfo { ids: vec!["jei".into()], name: "Just Enough Items".into() },
            ModInfo { ids: vec!["wands".into()], name: "Wands".into() },
        ];
        let list = merge(ServerKind::Forge, "1.20.1", Some(&found), true, &[], &mods);
        assert_eq!(find(&list, "advancement").source.as_deref(), Some("Minecraft"));
        assert_eq!(find(&list, "advancement").usage.as_deref(), Some("(grant|revoke)"));
        assert_eq!(find(&list, "forge").source.as_deref(), Some("Forge"));
        assert_eq!((find(&list, "ftbquests").source.as_deref(), find(&list, "ftbquests").source_kind.as_str()), (Some("FTB Quests"), "mod"));
        assert_eq!(find(&list, "just-enough-items").source.as_deref(), Some("Just Enough Items"));
        assert_eq!(find(&list, "teleport").aliases, ["tp"]);
        assert!(!list.iter().any(|c| c.name == "tp"));
        let unknown = find(&list, "wand");
        assert_eq!((unknown.source.as_deref(), unknown.source_kind.as_str()), (None, "other"), "close to a mod's name isn't that mod");
    }

    #[test]
    fn built_ins_fit_the_kind() {
        let names = |k, v| built_ins(k, v).into_iter().map(|c| c.name).collect::<Vec<_>>();
        let has = |k, v, n: &str| names(k, v).iter().any(|c| c == n);
        assert!(has(ServerKind::Velocity, "3.5.1", "shutdown") && has(ServerKind::Velocity, "3.5.1", "glist"));
        assert!(!has(ServerKind::Velocity, "3.5.1", "stop") && !has(ServerKind::Velocity, "3.5.1", "give"));
        assert!(has(ServerKind::Bungeecord, "1.21", "end") && !has(ServerKind::Bungeecord, "1.21", "shutdown"));
        assert!(has(ServerKind::Bedrock, "1.21", "gamerule") && !has(ServerKind::Bedrock, "1.21", "whitelist"));
        assert!(has(ServerKind::Paper, "1.21", "plugins") && !has(ServerKind::Fabric, "1.21", "plugins"));
        assert!(has(ServerKind::Forge, "1.20.1", "execute") && !has(ServerKind::Forge, "1.12.2", "execute"));
        assert!(names(ServerKind::Cuberite, "1.12.2").is_empty() && names(ServerKind::Pocketmine, "5").is_empty() && names(ServerKind::Waterdogpe, "2").is_empty());
        for b in MINECRAFT_ALWAYS.iter().chain(MINECRAFT_SINCE_1_13) {
            assert!(VANILLA.contains(b), "{b}");
        }
    }

    #[test]
    fn a_cache_survives_the_trip_to_disk() {
        let c = Cache { kind: "paper".into(), at: 5, commands: vec![live("a", None, Some("x"))] };
        let back: Cache = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.commands, c.commands);
    }
}
