//! Preparing (downloads, natives, Java), building the command line, running the game and streaming its log.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use crate::core::events::{emit, Task};
use crate::core::fsutil::now_ms;
use crate::core::instances;
use crate::core::net::{download_all, DownloadItem};
use crate::core::paths;
use crate::core::settings::Settings;
use crate::core::types::{GameEvent, Instance, LoaderType, LogEvent, LogLine, RunView};
use crate::player::accounts::LaunchAuth;
use crate::{bail_tr, err_tr, tr};

use super::java::{cpu_for, find_java_for, hidden_command, host_cpu, install_java_for};
use super::loaders::{resolve_version, JavaPicker, ResolveContext};
use super::mojang::{asset_items, game_arch, os_name, plain,client_jar, logging_config, materialize_legacy_assets, resolve_libraries, rules_allow, ArgValue, Argument, VersionJson};

const LAUNCHER_NAME: &str = "ResinCraft";
const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One start of an instance. An instance can run more than once at a time, each with a different account (two
/// players on one PC); runs are numbered from 1 and run 1's id is the instance id, so a single run looks as before.
struct Run {
    slot: u32,
    account_id: String,
    username: String,
    /// Asks the watcher to stop the game; `None` while still preparing.
    kill: Option<mpsc::UnboundedSender<()>>,
}

/// Instance id → its runs (preparing or running).
static RUNNING: Mutex<Option<HashMap<String, Vec<Run>>>> = Mutex::new(None);

fn running<R>(f: impl FnOnce(&mut HashMap<String, Vec<Run>>) -> R) -> R {
    f(RUNNING.lock().unwrap().get_or_insert_with(HashMap::new))
}

fn run_id(instance: &str, slot: u32) -> String {
    if slot == 1 {
        instance.to_string()
    } else {
        format!("{instance}~{slot}")
    }
}

/// Instances with a game process running (not just preparing).
pub fn running_instances() -> Vec<String> {
    running(|m| m.iter().filter(|(_, runs)| runs.iter().any(|r| r.kill.is_some())).map(|(id, _)| id.clone()).collect())
}

/// Every run, preparing or running (the UI reads this after a reload).
pub fn runs() -> Vec<RunView> {
    running(|m| {
        let mut out: Vec<RunView> = m
            .iter()
            .flat_map(|(id, runs)| {
                runs.iter().map(|r| RunView {
                    instance_id: id.clone(),
                    run_id: run_id(id, r.slot),
                    account_id: r.account_id.clone(),
                    username: r.username.clone(),
                    state: if r.kill.is_some() { "running" } else { "preparing" },
                })
            })
            .collect();
        out.sort_by(|a, b| a.run_id.cmp(&b.run_id));
        out
    })
}

/// Stops every run of an instance, or only `run` (a run id) when given.
pub fn kill_instance(id: &str, run: Option<&str>) {
    running(|m| {
        for r in m.get(id).into_iter().flatten() {
            if run.is_none_or(|want| want == run_id(id, r.slot)) {
                if let Some(tx) = &r.kill {
                    let _ = tx.send(());
                }
            }
        }
    });
}

/// Takes the lowest free run slot for `auth`'s account, or fails if that account is already playing the instance.
fn reserve(id: &str, auth: &LaunchAuth) -> Result<u32> {
    running(|m| {
        let runs = m.entry(id.to_string()).or_default();
        if let Some(r) = runs.iter().find(|r| r.account_id == auth.account_id) {
            bail_tr!("launch.alreadyRunningAs", "{name} is already playing this instance. Pick another account to play it a second time.", name = r.username);
        }
        let slot = (1..).find(|n| !runs.iter().any(|r| r.slot == *n)).unwrap_or(1);
        runs.push(Run { slot, account_id: auth.account_id.clone(), username: auth.username.clone(), kill: None });
        Ok(slot)
    })
}

fn release(id: &str, slot: u32) {
    running(|m| {
        if let Some(runs) = m.get_mut(id) {
            runs.retain(|r| r.slot != slot);
            if runs.is_empty() {
                m.remove(id);
            }
        }
    });
}

/// Where a run unpacks native libraries. Run 1 uses the instance's own folder; later runs get their own, because
/// re-unpacking would delete DLLs the first game has loaded (Windows refuses, and the second launch fails).
fn natives_for(id: &str, slot: u32) -> PathBuf {
    if slot == 1 {
        paths::natives_dir(id)
    } else {
        paths::cache().join("natives").join(format!("{id}-{slot}"))
    }
}

struct RunInfo {
    instance_id: String,
    run_id: String,
    account_id: String,
    username: String,
}

fn game_event(run: &RunInfo, state: &'static str, code: Option<i32>, message: Option<String>) {
    emit(
        "game",
        GameEvent {
            instance_id: run.instance_id.clone(),
            run_id: run.run_id.clone(),
            account_id: run.account_id.clone(),
            username: run.username.clone(),
            state,
            code,
            message,
        },
    );
}

fn delimiter() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

// ---------- Preparation ----------

struct Prepared {
    version: VersionJson,
    java: String,
    classpath: Vec<PathBuf>,
    assets_dir: PathBuf,
    game_assets: PathBuf,
    asset_index: String,
    log_argument: Option<String>,
    natives: PathBuf,
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.filter(|s| !s.is_empty()).map(str::to_string)
}

/// A Java for `required` that runs as `cpu` (Adoptium's "x64" / "aarch64"; it only differs from this computer's
/// on an Apple Silicon Mac playing a version without arm64 natives).
async fn choose_java(inst: &Instance, settings: &Settings, required: u32, cpu: &str, task: &Arc<Task>) -> Result<String> {
    if let Some(custom) = non_empty(inst.java_path.as_deref()).or_else(|| non_empty(Some(&settings.java_path))) {
        return Ok(custom);
    }
    if let Some(found) = find_java_for(required, cpu).await? {
        return Ok(found.path);
    }
    if settings.auto_java {
        return Ok(install_java_for(required, cpu, task).await?.path);
    }
    bail_tr!(
        "launch.needJava",
        "Minecraft {version} needs Java {major}. Install it, or turn on automatic Java downloads in Settings.",
        version = inst.mc_version,
        major = required
    )
}

/// Like `net::download_all`, but files that can't be downloaded (no URL) must already exist.
async fn download_checked(items: Vec<DownloadItem>, concurrency: usize, progress: impl Fn(usize, usize) + Send + Sync) -> Result<()> {
    let (local, remote): (Vec<_>, Vec<_>) = items.into_iter().partition(|i| i.url.is_empty());
    for item in local {
        if tokio::fs::metadata(&item.dest).await.is_err() {
            let file = item.dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            bail_tr!("launch.fileMissing", "{file} is missing and can't be downloaded. Try reinstalling the mod loader.", file = file);
        }
    }
    download_all(remote, concurrency, progress).await
}

async fn prepare(inst: &Instance, settings: &Settings, natives: PathBuf, task: &Arc<Task>) -> Result<Prepared> {
    task.update(tr!("launch.readingVersion", "Reading version info…"), None);
    let picker: JavaPicker = {
        let (inst, settings, task) = (inst.clone(), settings.clone(), task.clone());
        Box::new(move |major| {
            let (inst, settings, task) = (inst.clone(), settings.clone(), task.clone());
            Box::pin(async move { choose_java(&inst, &settings, major, host_cpu(), &task).await })
        })
    };
    let ctx = ResolveContext { task: Some(task.clone()), concurrency: Some(settings.concurrency), java: Some(picker) };
    let mut version = resolve_version(&inst.mc_version, inst.loader, inst.loader_version.as_deref(), &ctx).await?;

    let libraries = resolve_libraries(&version);
    let arch = game_arch(&version);
    if arch == "arm64" && os_name() == "linux" && !libraries.iter().any(|l| l.extract) {
        bail_tr!(
            "launch.noArmLinux",
            "Minecraft {version} can't run on ARM Linux: its graphics library (LWJGL) has no ARM build. Minecraft 1.19 and newer work.",
            version = inst.mc_version
        );
    }
    if cpu_for(arch) != host_cpu() {
        need_rosetta(&inst.mc_version).await?;
    }
    let client = client_jar(&version)?;
    let logging = logging_config(&version);
    let assets = asset_items(&version).await?;

    let mut items = vec![client.clone()];
    items.extend(libraries.iter().map(|l| l.item.clone()));
    if let Some(l) = &logging {
        items.push(l.item.clone());
    }
    download_checked(items, settings.concurrency, task.counter(tr!("launch.downloadingLibraries", "Downloading libraries"))).await?;
    download_all(assets.items, settings.concurrency * 2, task.counter(tr!("launch.downloadingAssets", "Downloading assets"))).await?;
    let game_assets = materialize_legacy_assets(&assets.index, &assets.id, &paths::game_dir(&inst.id)).await?;

    task.update(tr!("launch.unpackingNatives", "Unpacking natives…"), None);
    let _ = tokio::fs::remove_dir_all(&natives).await;
    tokio::fs::create_dir_all(&natives).await?;
    let extract: Vec<(PathBuf, Vec<String>)> = libraries.iter().filter(|l| l.extract).map(|l| (l.item.dest.clone(), l.exclude.clone())).collect();
    {
        let natives = natives.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let re = regex::Regex::new(r"(?i)\.(dll|so|dylib|jnilib)$").unwrap();
            for (jar, exclude) in extract {
                let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?).map_err(|e| err_tr!("shell.openFailed", "Couldn't open {path}: {error}", path = jar.display().to_string(), error = e.to_string()))?;
                for i in 0..zip.len() {
                    let mut entry = zip.by_index(i)?;
                    let name = entry.name().to_string();
                    if entry.is_dir() || name.starts_with("META-INF/") || exclude.iter().any(|x| name.starts_with(x.as_str())) {
                        continue;
                    }
                    if !re.is_match(&name) {
                        continue;
                    }
                    let base = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string();
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    std::fs::write(natives.join(base), buf)?;
                }
            }
            Ok(())
        })
        .await??;
    }

    task.update(tr!("launch.checkingJava", "Checking Java…"), None);
    let java = choose_java(inst, settings, version.java_major(), cpu_for(arch), task).await?;

    // OptiFine on vanilla launches through launchwrapper; its jars go first on the classpath.
    let mut classpath: Vec<PathBuf> = Vec::new();
    if inst.loader == LoaderType::Vanilla && inst.optifine.is_some() {
        task.update(tr!("launch.settingUpOptifine", "Setting up OptiFine…"), None);
        classpath = super::optifine::apply_vanilla(inst, &mut version, &client.dest, &java, settings.concurrency).await?;
    }
    classpath.extend(libraries.iter().filter(|l| l.classpath).map(|l| l.item.dest.clone()));
    classpath.push(client.dest.clone());
    Ok(Prepared {
        java,
        classpath,
        assets_dir: paths::assets(),
        game_assets,
        asset_index: assets.id,
        log_argument: logging.map(|l| l.argument.replacen("${path}", &plain(&l.item.dest), 1)),
        version,
        natives,
    })
}

/// Apple Silicon: versions before 1.19 run as x64 through Rosetta 2, which macOS doesn't install by itself.
async fn need_rosetta(mc_version: &str) -> Result<()> {
    let ok = hidden_command("/usr/bin/arch").args(["-x86_64", "/usr/bin/true"]).status().await.map(|s| s.success()).unwrap_or(false);
    if !ok {
        bail_tr!(
            "launch.needRosetta",
            "Minecraft {version} runs through Rosetta 2 on Apple Silicon. Install it by running \"softwareupdate --install-rosetta\" in Terminal, then play again.",
            version = mc_version
        );
    }
    Ok(())
}

// ---------- Arguments ----------

fn expand(args: &[Argument], features: &BTreeMap<String, bool>) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Argument::Plain(s) => out.push(s.clone()),
            Argument::Ruled { rules, value } => {
                if rules_allow(rules.as_deref(), features) {
                    match value {
                        ArgValue::One(s) => out.push(s.clone()),
                        ArgValue::Many(v) => out.extend(v.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

/// Splits a user-typed JVM argument string, honouring double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""([^"]*)"|(\S+)"#).unwrap();
    re.captures_iter(s).map(|c| c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default()).collect()
}

/// Where the game goes once it's up, instead of the title screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Joins a server ("host:port").
    Server(String),
    /// Opens a singleplayer world (its folder in saves/). Minecraft 1.20+ only.
    World(String),
}

impl Target {
    fn server(&self) -> Option<&str> {
        match self {
            Target::Server(s) => Some(s),
            Target::World(_) => None,
        }
    }
    fn world(&self) -> Option<&str> {
        match self {
            Target::World(w) => Some(w),
            Target::Server(_) => None,
        }
    }
}

/// The rule features a launch turns on; quick play only when going straight to a server or world.
fn launch_features(target: Option<&Target>) -> BTreeMap<String, bool> {
    BTreeMap::from([
        ("has_custom_resolution".to_string(), true),
        ("is_demo_user".to_string(), false),
        ("is_quick_play_multiplayer".to_string(), target.and_then(Target::server).is_some()),
        ("is_quick_play_singleplayer".to_string(), target.and_then(Target::world).is_some()),
    ])
}

/// Splits "host", "host:port" and "[v6]:port" for `--server/--port` (the port defaults to 25565).
fn split_host_port(addr: &str) -> (String, String) {
    let addr = addr.trim();
    if let Some(rest) = addr.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').filter(|p| !p.is_empty()).unwrap_or("25565");
            return (host.to_string(), port.to_string());
        }
    }
    match addr.rsplit_once(':') {
        // More than one colon without brackets: a bare IPv6 address.
        Some((host, port)) if !host.contains(':') && !port.is_empty() => (host.to_string(), port.to_string()),
        _ => (addr.to_string(), "25565".to_string()),
    }
}

/// Adds the target to the expanded game arguments. 1.20+ already got `--quickPlayMultiplayer` or
/// `--quickPlaySingleplayer` from the version's own rules. Before it there is no quick play: servers take
/// `--server/--port` instead, and worlds can't be opened from outside the game.
fn with_target(mut game: Vec<String>, target: Option<&Target>, mc: &str) -> Result<Vec<String>> {
    match target {
        Some(Target::Server(addr)) if !game.iter().any(|a| a == "--quickPlayMultiplayer") => {
            let (host, port) = split_host_port(addr);
            game.extend(["--server".to_string(), host, "--port".to_string(), port]);
        }
        Some(Target::World(_)) if !game.iter().any(|a| a == "--quickPlaySingleplayer") => {
            bail_tr!("launch.worldNeedsNewer", "Opening a world straight away needs Minecraft 1.20 or newer; this instance is {mc}. Start the game and pick the world there.", mc = mc);
        }
        _ => {}
    }
    Ok(game)
}

/// `target` is where the game goes once it's up: a server (quick play on 1.20+, `--server` before) or a world.
fn build_args(inst: &Instance, settings: &Settings, p: &Prepared, auth: &LaunchAuth, target: Option<&Target>) -> Result<Vec<String>> {
    let v = &p.version;
    let dir = paths::game_dir(&inst.id);
    let uuid = auth.uuid.replace('-', "");
    let cp: Vec<String> = p.classpath.iter().map(|c| plain(c)).collect();
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", auth.username.clone()),
        ("auth_uuid", uuid.clone()),
        ("auth_access_token", auth.access_token.clone()),
        ("auth_session", format!("token:{}:{uuid}", auth.access_token)),
        ("auth_xuid", auth.xuid.clone().unwrap_or_else(|| "0".into())),
        ("clientid", "0".into()),
        ("user_type", auth.user_type.to_string()),
        ("user_properties", "{}".into()),
        ("version_name", v.id.clone()),
        ("version_type", v.kind.clone().unwrap_or_else(|| "release".into())),
        ("game_directory", plain(&dir)),
        ("assets_root", plain(&p.assets_dir)),
        ("game_assets", plain(&p.game_assets)),
        ("assets_index_name", p.asset_index.clone()),
        ("natives_directory", plain(&p.natives)),
        ("library_directory", plain(&paths::libraries())),
        ("classpath_separator", delimiter().into()),
        ("classpath", cp.join(delimiter())),
        ("launcher_name", LAUNCHER_NAME.into()),
        ("launcher_version", LAUNCHER_VERSION.into()),
        ("resolution_width", settings.game_width.to_string()),
        ("resolution_height", settings.game_height.to_string()),
        ("quickPlayMultiplayer", target.and_then(Target::server).unwrap_or_default().to_string()),
        ("quickPlaySingleplayer", target.and_then(Target::world).unwrap_or_default().to_string()),
    ]);
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    let sub = |s: &String| -> String { re.replace_all(s, |c: &regex::Captures| vars.get(&c[1]).cloned().unwrap_or_else(|| c[0].to_string())).into_owned() };
    let features = launch_features(target);

    let (jvm, game): (Vec<String>, Vec<String>) = match v.minecraft_arguments.as_deref().filter(|s| !s.is_empty()) {
        Some(legacy) => {
            let mut jvm = vec!["-Djava.library.path=${natives_directory}".to_string(), "-cp".into(), "${classpath}".into()];
            jvm.extend(expand(v.arguments.as_ref().map(|a| a.jvm.as_slice()).unwrap_or(&[]), &features));
            let mut game: Vec<String> = legacy.split(' ').map(str::to_string).collect();
            game.extend(expand(v.arguments.as_ref().map(|a| a.game.as_slice()).unwrap_or(&[]), &features));
            game.extend(["--width".into(), "${resolution_width}".into(), "--height".into(), "${resolution_height}".into()]);
            (jvm, game)
        }
        None => (
            expand(v.arguments.as_ref().map(|a| a.jvm.as_slice()).unwrap_or(&[]), &features),
            expand(v.arguments.as_ref().map(|a| a.game.as_slice()).unwrap_or(&[]), &features),
        ),
    };

    let game = with_target(game, target, &inst.mc_version)?;

    let main_class = v.main_class.clone().filter(|m| !m.is_empty()).ok_or_else(|| err_tr!("launch.noMainClass", "Version {version} has no main class", version = v.id))?;
    let memory = inst.memory_mb.unwrap_or(settings.memory_mb);
    let user_jvm = non_empty(inst.jvm_args.as_deref()).unwrap_or_else(|| settings.jvm_args.clone());
    let mut args = vec![format!("-Xmx{memory}M"), format!("-Xms{}M", memory.min(1024))];
    args.extend(split_args(&user_jvm));
    args.extend(jvm.iter().map(sub));
    if let Some(l) = &p.log_argument {
        args.push(l.clone());
    }
    args.push(main_class);
    args.extend(game.iter().map(sub));
    Ok(args)
}

// ---------- Logs ----------

fn level_of(text: &str, hint: Option<&str>) -> &'static str {
    use std::sync::OnceLock;
    static RES: OnceLock<[regex::Regex; 3]> = OnceLock::new();
    let [err, warn, debug] = RES.get_or_init(|| {
        [
            regex::Regex::new(r"\b(ERROR|FATAL|SEVERE)\b|EXCEPTION").unwrap(),
            regex::Regex::new(r"\bWARN(ING)?\b").unwrap(),
            regex::Regex::new(r"\b(DEBUG|TRACE)\b").unwrap(),
        ]
    });
    let t = hint.unwrap_or(text).to_uppercase();
    if err.is_match(&t) {
        "error"
    } else if warn.is_match(&t) {
        "warn"
    } else if debug.is_match(&t) {
        "debug"
    } else {
        "info"
    }
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

const EVENT_START: &str = "<log4j:Event";
const EVENT_END: &str = "</log4j:Event>";

/// Mojang's log4j config prints XML events on stdout; turn them back into readable lines.
pub struct LogParser {
    buf: String,
    /// Bytes of a UTF-8 character split across reads.
    partial: Vec<u8>,
    push: Box<dyn FnMut(LogLine) + Send>,
}

impl LogParser {
    pub fn new(push: impl FnMut(LogLine) + Send + 'static) -> Self {
        LogParser { buf: String::new(), partial: Vec::new(), push: Box::new(push) }
    }

    /// Decodes raw output bytes (lossy UTF-8, keeping split characters for the next read).
    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.partial.extend_from_slice(bytes);
        let data = std::mem::take(&mut self.partial);
        let mut text = String::new();
        let mut rest: &[u8] = &data;
        loop {
            match std::str::from_utf8(rest) {
                Ok(s) => {
                    text.push_str(s);
                    break;
                }
                Err(e) => {
                    let (good, bad) = rest.split_at(e.valid_up_to());
                    text.push_str(std::str::from_utf8(good).unwrap_or_default());
                    match e.error_len() {
                        Some(n) => {
                            text.push('\u{FFFD}');
                            rest = &bad[n..];
                        }
                        None => {
                            self.partial = bad.to_vec();
                            break;
                        }
                    }
                }
            }
        }
        self.feed(&text);
    }

    pub fn feed(&mut self, chunk: &str) {
        use std::sync::OnceLock;
        static RES: OnceLock<[regex::Regex; 5]> = OnceLock::new();
        let [level_re, thread_re, ts_re, msg_re, thrown_re] = RES.get_or_init(|| {
            [
                regex::Regex::new(r#"level="([^"]+)""#).unwrap(),
                regex::Regex::new(r#"thread="([^"]+)""#).unwrap(),
                regex::Regex::new(r#"timestamp="(\d+)""#).unwrap(),
                regex::Regex::new(r"(?s)<log4j:Message><!\[CDATA\[(.*?)\]\]></log4j:Message>").unwrap(),
                regex::Regex::new(r"(?s)<log4j:Throwable><!\[CDATA\[(.*?)\]\]></log4j:Throwable>").unwrap(),
            ]
        });
        self.buf.push_str(chunk);
        loop {
            let start = self.buf.find(EVENT_START);
            let nl = self.buf.find('\n');
            let plain_line = match (start, nl) {
                (None, _) => true,
                (Some(s), Some(n)) => n < s,
                (Some(_), None) => false,
            };
            if plain_line {
                let Some(nl) = nl else { return };
                let line = self.buf[..nl].trim_end_matches('\r').to_string();
                self.buf.drain(..=nl);
                if !line.trim().is_empty() {
                    let level = level_of(&line, None);
                    (self.push)(LogLine { text: line, level });
                }
                continue;
            }
            let start = start.unwrap_or(0);
            let Some(end) = self.buf[start..].find(EVENT_END).map(|e| e + start) else { return };
            let xml = self.buf[start..end].to_string();
            self.buf.drain(..end + EVENT_END.len());
            let cap = |re: &regex::Regex| re.captures(&xml).map(|c| c[1].to_string());
            let level = cap(level_re).unwrap_or_else(|| "INFO".into());
            let thread = cap(thread_re).unwrap_or_else(|| "main".into());
            let ts = cap(ts_re).and_then(|t| t.parse::<i64>().ok()).unwrap_or_else(now_ms);
            let msg = cap(msg_re).unwrap_or_default();
            let thrown = cap(thrown_re);
            let time = {
                use chrono::TimeZone;
                chrono::Local.timestamp_millis_opt(ts).single().map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default()
            };
            let lvl = level_of(&level, Some(&level));
            (self.push)(LogLine { text: format!("[{time}] [{}/{level}]: {}", unescape_xml(&thread), unescape_xml(&msg)), level: lvl });
            if let Some(thrown) = thrown {
                for t in thrown.trim_end().split('\n') {
                    (self.push)(LogLine { text: t.trim_end_matches('\r').to_string(), level: "error" });
                }
            }
        }
    }
}

// ---------- Launch ----------

#[derive(Default)]
pub struct LaunchHooks {
    pub on_started: Option<Box<dyn FnOnce() + Send>>,
    pub on_exit: Option<Box<dyn FnOnce(Option<i32>) + Send>>,
}

fn loader_label(inst: &Instance) -> String {
    if inst.loader == LoaderType::Vanilla {
        String::new()
    } else {
        format!(" + {} {}", inst.loader.as_str(), inst.loader_version.clone().unwrap_or_else(|| "undefined".into()))
    }
}

/// Prepares and starts the game. Returns once Java is running; the game is watched in the background.
pub async fn launch_instance(id: &str, settings: &Settings, auth: LaunchAuth, hooks: LaunchHooks) -> Result<()> {
    launch_instance_to(id, settings, auth, hooks, None).await
}

/// Like `launch_instance`, going to `target` (a server or a world) once the game is up.
/// The instance can already be running with other accounts; each run gets its own log and state (see `Run`).
pub async fn launch_instance_to(id: &str, settings: &Settings, auth: LaunchAuth, hooks: LaunchHooks, target: Option<&Target>) -> Result<()> {
    let inst = instances::get(id).await?;
    let slot = reserve(id, &auth)?;
    let run = RunInfo { instance_id: id.to_string(), run_id: run_id(id, slot), account_id: auth.account_id.clone(), username: auth.username.clone() };
    game_event(&run, "preparing", None, None);

    let title = if slot == 1 {
        tr!("launch.preparing", "Preparing {name}", name = inst.name)
    } else {
        tr!("launch.preparingAs", "Preparing {name} for {player}", name = inst.name, player = auth.username)
    };
    let natives = natives_for(id, slot);
    let prepared = Task::run(title, Some(id.to_string()), |t| {
        let (inst, settings, natives) = (inst.clone(), settings.clone(), natives.clone());
        async move { prepare(&inst, &settings, natives, &t).await }
    })
    .await
    .and_then(|p| build_args(&inst, settings, &p, &auth, target).map(|args| (p, args)));
    let (prepared, args) = match prepared {
        Ok(p) => p,
        Err(e) => {
            release(id, slot);
            game_event(&run, "stopped", None, None);
            return Err(e);
        }
    };

    let shown: Vec<&str> = args.iter().map(|a| if *a == auth.access_token { "<access token hidden>" } else { a.as_str() }).collect();

    // Batch log lines so a chatty game doesn't flood IPC.
    let pending: Arc<Mutex<Vec<LogLine>>> = Arc::new(Mutex::new(Vec::new()));
    let flush = {
        let (pending, id, run_id) = (pending.clone(), id.to_string(), run.run_id.clone());
        move || {
            let lines = std::mem::take(&mut *pending.lock().unwrap());
            if !lines.is_empty() {
                emit("log", LogEvent { instance_id: id.clone(), run_id: run_id.clone(), lines });
            }
        }
    };
    let push = {
        let pending = pending.clone();
        move |l: LogLine| pending.lock().unwrap().push(l)
    };
    push(LogLine { text: format!("ResinCraft is launching {} (Minecraft {}{})", inst.name, inst.mc_version, loader_label(&inst)), level: "info" });
    if slot > 1 {
        push(LogLine { text: format!("Playing as {} alongside another run of this instance (run {slot})", auth.username), level: "info" });
    }
    push(LogLine { text: format!("Java: {}", prepared.java), level: "info" });
    push(LogLine { text: format!("Command: {}", shown.join(" ")), level: "debug" });

    let game_dir = paths::game_dir(id);
    let _ = tokio::fs::create_dir_all(&game_dir).await;
    let mut cmd = hidden_command(&prepared.java);
    cmd.args(&args)
        .current_dir(&game_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            flush();
            release(id, slot);
            game_event(&run, "stopped", None, Some(e.to_string()));
            bail_tr!("launch.javaStartFailed", "Couldn't start Java ({java}): {error}", java = prepared.java, error = e.to_string());
        }
    };
    let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
    running(|m| {
        if let Some(r) = m.get_mut(id).and_then(|runs| runs.iter_mut().find(|r| r.slot == slot)) {
            r.kill = Some(kill_tx);
        }
    });
    let started = Instant::now();

    // Readers: stdout as-is, stderr with "info" promoted to "warn".
    let mut readers = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let push = push.clone();
        readers.push(tokio::spawn(async move {
            let mut parser = LogParser::new(push);
            let mut buf = vec![0u8; 16 * 1024];
            while let Ok(n) = out.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                parser.feed_bytes(&buf[..n]);
            }
            parser.feed("\n");
        }));
    }
    if let Some(mut err) = child.stderr.take() {
        let push = push.clone();
        readers.push(tokio::spawn(async move {
            let mut parser = LogParser::new(move |l: LogLine| push(LogLine { level: if l.level == "info" { "warn" } else { l.level }, ..l }));
            let mut buf = vec![0u8; 16 * 1024];
            while let Ok(n) = err.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                parser.feed_bytes(&buf[..n]);
            }
            parser.feed("\n");
        }));
    }
    let ticker = {
        let flush = flush.clone();
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_millis(120));
            loop {
                iv.tick().await;
                flush();
            }
        })
    };

    game_event(&run, "running", None, None);
    if let Some(f) = hooks.on_started {
        f();
    }
    let _ = instances::update(id, json!({ "lastPlayed": now_ms() })).await;

    let id = id.to_string();
    let on_exit = hooks.on_exit;
    tokio::spawn(async move {
        let mut killed = false;
        let status = loop {
            tokio::select! {
                s = child.wait() => break s,
                Some(()) = kill_rx.recv(), if !killed => {
                    killed = true;
                    terminate(&mut child);
                }
            }
        };
        release(&id, slot);
        // Let the readers drain what's left in the pipes.
        for r in readers {
            let _ = tokio::time::timeout(Duration::from_secs(5), r).await;
        }
        let code = status.as_ref().ok().and_then(|s| s.code());
        let signal = killed || code.is_none();
        let crashed = !signal && code.is_some_and(|c| c != 0);
        let shown_code = if killed {
            "SIGTERM".to_string()
        } else {
            code.map(|c| c.to_string()).unwrap_or_else(|| "null".into())
        };
        push(LogLine { text: format!("Game exited with code {shown_code}"), level: if crashed { "error" } else { "info" } });
        flush();
        ticker.abort();
        let secs = (started.elapsed().as_millis() as f64 / 1000.0).round() as i64;
        let playtime = instances::get(&id).await.map(|i| i.playtime).unwrap_or(inst.playtime);
        let _ = instances::update(&id, json!({ "playtime": playtime + secs })).await;
        let code = if signal { None } else { code };
        game_event(&run, if crashed { "crashed" } else { "stopped" }, code, None);
        if slot > 1 {
            let _ = tokio::fs::remove_dir_all(natives_for(&id, slot)).await;
        }
        if let Some(f) = on_exit {
            f(code);
        }
    });
    Ok(())
}

/// Asks the game to stop: SIGTERM on Unix (lets Minecraft save), TerminateProcess on Windows (as Node's kill does).
fn terminate(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        if std::process::Command::new("kill").arg("-TERM").arg(pid.to_string()).status().map(|s| s.success()).unwrap_or(false) {
            return;
        }
    }
    let _ = child.start_kill();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth(id: &str, name: &str) -> LaunchAuth {
        LaunchAuth { account_id: id.into(), username: name.into(), uuid: String::new(), access_token: "0".into(), user_type: "legacy", xuid: None }
    }

    /// The game arguments of a 1.20+ version JSON, with its quick play rules.
    fn modern_args() -> Vec<Argument> {
        serde_json::from_value(serde_json::json!([
            "--username", "${auth_player_name}",
            { "rules": [{ "action": "allow", "features": { "is_quick_play_multiplayer": true } }], "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"] },
            { "rules": [{ "action": "allow", "features": { "is_quick_play_singleplayer": true } }], "value": ["--quickPlaySingleplayer", "${quickPlaySingleplayer}"] }
        ]))
        .unwrap()
    }

    #[test]
    fn joining_on_new_versions_uses_quick_play() {
        let to = Target::Server("play.example.org:25570".into());
        let game = with_target(expand(&modern_args(), &launch_features(Some(&to))), Some(&to), "1.21.1").unwrap();
        assert_eq!(game, ["--username", "${auth_player_name}", "--quickPlayMultiplayer", "${quickPlayMultiplayer}"]);
        assert!(!game.contains(&"--server".to_string()));
    }

    #[test]
    fn no_server_means_no_join_arguments() {
        let game = with_target(expand(&modern_args(), &launch_features(None)), None, "1.21.1").unwrap();
        assert_eq!(game, ["--username", "${auth_player_name}"]);
    }

    #[test]
    fn worlds_open_with_quick_play() {
        let to = Target::World("New World (2)".into());
        let game = with_target(expand(&modern_args(), &launch_features(Some(&to))), Some(&to), "1.21.1").unwrap();
        assert_eq!(game, ["--username", "${auth_player_name}", "--quickPlaySingleplayer", "${quickPlaySingleplayer}"]);
        // Versions without quick play can't open a world from outside the game.
        let legacy = vec!["--username".to_string(), "${auth_player_name}".to_string()];
        assert!(with_target(legacy, Some(&to), "1.12.2").is_err());
    }

    #[test]
    fn joining_old_versions_uses_server_and_port() {
        let legacy = vec!["--username".to_string(), "${auth_player_name}".to_string()];
        let join = |addr: &str| with_target(legacy.clone(), Some(&Target::Server(addr.into())), "1.12.2").unwrap()[2..].to_vec();
        assert_eq!(join("play.example.org:25570"), ["--server", "play.example.org", "--port", "25570"]);
        assert_eq!(join("play.example.org"), ["--server", "play.example.org", "--port", "25565"]);
        assert_eq!(join("[2001:db8::1]:25580"), ["--server", "2001:db8::1", "--port", "25580"]);
        assert_eq!(join("[::1]"), ["--server", "::1", "--port", "25565"]);
        assert_eq!(join("2001:db8::1"), ["--server", "2001:db8::1", "--port", "25565"]);
    }

    #[test]
    fn runs_take_free_slots_one_per_account() {
        let inst = "test-runs-slots";
        assert_eq!(reserve(inst, &auth("a", "Alex")).unwrap(), 1);
        assert!(reserve(inst, &auth("a", "Alex")).is_err(), "the same account can't play one instance twice");
        assert_eq!(reserve(inst, &auth("b", "Steve")).unwrap(), 2);
        assert_eq!(run_id(inst, 1), inst);
        assert_eq!(run_id(inst, 2), format!("{inst}~2"));
        // Still preparing: listed as runs, but not as a running game.
        assert_eq!(runs().iter().filter(|r| r.instance_id == inst).count(), 2);
        assert!(!running_instances().contains(&inst.to_string()));
        release(inst, 1);
        assert_eq!(reserve(inst, &auth("c", "Kai")).unwrap(), 1, "a freed slot is reused");
        release(inst, 1);
        release(inst, 2);
        assert!(runs().iter().all(|r| r.instance_id != inst));
    }
}
