//! Horizon (playhorizon.dev), a Rust rewrite of the Java Edition client, as an instance. Horizon's Launch API
//! (docs/api/launch.md in its repository) says how another launcher starts it: ask the build what it takes
//! (`--launch-api`), then run `horizon-client` with a folder per instance and the account's session on its standard
//! input. Horizon ships only its rolling `nightly` release and deletes dated builds after a week, so this follows
//! `nightly` and keeps the copy it has when GitHub can't be reached or an update fails. Each build is prepared once,
//! by its own `horizon-prepare`, from Mojang's files in the launcher's folders (they're laid out like the official
//! launcher's), and shared by every Horizon instance. Hosted Horizon servers keep their own copy (installers.rs).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::content::github;
use crate::core::events::Task;
use crate::core::net::{download_all, fetch_text, DownloadItem, Hash};
use crate::core::paths;
use crate::core::types::Instance;
use crate::err_tr;
use crate::player::accounts::LaunchAuth;
use crate::server::installers::{java_for, make_executable, run_tool, unpack_tar, unpack_zip};

use super::java::hidden_command;
use super::launch::Target;
use super::mojang::{asset_items, client_jar, get_vanilla_version, plain, resolve_libraries};

const REPO: &str = "horizon-rust/horizon";
/// The one channel Horizon ships.
pub const NIGHTLY: &str = "nightly";
/// Written last, once a build is prepared: only such a build is used.
const CURRENT_FILE: &str = "current.txt";
const LOCK_FILE: &str = "research-lock.json";
/// The options ResinCraft can't start the game without.
const REQUIRED_OPTIONS: [&str; 4] = ["--resources", "--game-dir", "--home", "--session-stdin"];

/// A prepared build.
#[derive(Debug, Clone)]
pub struct Build {
    pub id: String,
    /// The folder holding the programs.
    pub root: PathBuf,
    /// The Minecraft version this build is pinned to.
    pub mc: String,
}

impl Build {
    fn client(&self) -> PathBuf {
        self.root.join(if cfg!(windows) { "horizon-client.exe" } else { "horizon-client" })
    }
    fn resources(&self) -> PathBuf {
        resources(&self.root, &self.mc)
    }
}

pub fn resources(root: &Path, mc: &str) -> PathBuf {
    root.join(".local").join("horizon-client").join(mc)
}

fn builds_dir() -> PathBuf {
    paths::root().join("horizon-builds")
}

/// A hosted server's archive and game program. Windows and macOS have only the full game (it runs without a window as
/// a server); Linux has a build without its sound and controller libraries.
pub fn server_archive(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    match (os, arch) {
        ("windows", "x86_64") => Some(("horizon-windows-x64.zip", "horizon-client.exe")),
        ("linux", "x86_64") => Some(("horizon-server-linux-x86_64.tar.gz", "horizon-engine")),
        ("macos", "aarch64") => Some(("horizon-macos-arm64.zip", "horizon-client")),
        _ => None,
    }
}

/// The game's archive, for playing.
pub fn client_archive(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("horizon-windows-x64.zip"),
        ("linux", "x86_64") => Some("horizon-linux-x86_64.tar.gz"),
        ("macos", "aarch64") => Some("horizon-macos-arm64.zip"),
        _ => None,
    }
}

/// "Nightly 2026-10-31 · 9d979ca" is nightly-2026-10-31-9d979ca, as Horizon's own tools name it. The `nightly` tag
/// moves, so the title is what tells two builds apart.
pub fn build_id(title: &str) -> Option<String> {
    let rest = title.strip_prefix("Nightly ")?;
    let (date, commit) = rest.split_once('·')?;
    let (date, commit) = (date.trim(), commit.trim());
    (!date.is_empty() && !commit.is_empty() && date.chars().all(|c| c.is_ascii_digit() || c == '-') && commit.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("nightly-{date}-{commit}"))
}

/// The Minecraft version a build is pinned to, from its research-lock.json.
pub fn mc_in_lock(lock: &str) -> Option<String> {
    serde_json::from_str::<Value>(lock).ok()?.pointer("/minecraft/version")?.as_str().map(str::to_string)
}

/// The rolling release and its build id.
pub async fn latest() -> Result<(github::Release, String)> {
    let release = github::release_of(REPO, NIGHTLY).await?;
    let id = build_id(&release.title()).ok_or_else(|| anyhow!("Couldn't read which Horizon build {} is", release.title()))?;
    Ok((release, id))
}

async fn installed() -> Option<Build> {
    let id = tokio::fs::read_to_string(builds_dir().join(CURRENT_FILE)).await.ok()?.trim().to_string();
    let root = find_root(&builds_dir().join(&id)).await?;
    let mc = mc_in_lock(&tokio::fs::read_to_string(root.join(LOCK_FILE)).await.ok()?)?;
    Some(Build { id, root, mc })
}

/// The Minecraft version Horizon plays now: the installed build's, else the one the rolling release pins.
pub async fn minecraft() -> Result<String> {
    if let Some(b) = installed().await {
        return Ok(b.mc);
    }
    let lock = fetch_text(&format!("https://raw.githubusercontent.com/{REPO}/{NIGHTLY}/{LOCK_FILE}")).await?;
    mc_in_lock(&lock).ok_or_else(|| anyhow!("Couldn't read which Minecraft version Horizon needs"))
}

async fn find_root(dir: &Path) -> Option<PathBuf> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut locks = Vec::new();
        crate::server::installers::find_files(&dir, LOCK_FILE, 2, &mut locks);
        locks.into_iter().next().and_then(|l| l.parent().map(Path::to_path_buf))
    })
    .await
    .ok()
    .flatten()
}

/// Downloads a release's archive into `dir` and unpacks it there.
pub async fn fetch(release: &github::Release, name: &str, dir: &Path, task: &Arc<Task>) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let asset = release.assets.iter().find(|a| a.name == name).ok_or_else(|| anyhow!("Horizon's newest build has no {name}"))?;
    let file = dir.join(name);
    let item = DownloadItem::new(&asset.url, &file).hash(asset.sha256().map(Hash::sha256)).size(Some(asset.size));
    download_all(vec![item], 1, task.counter("Downloading Horizon")).await?;
    task.update("Unpacking Horizon…", None);
    if name.ends_with(".zip") {
        unpack_zip(&file, dir, "Horizon").await?;
    } else {
        unpack_tar(&file, dir).await?;
    }
    let _ = tokio::fs::remove_file(&file).await;
    Ok(())
}

/// Runs an unpacked build's horizon-prepare on the Minecraft version it's pinned to, from Mojang's files in the
/// launcher's own folders. Returns the build's folder and that version.
pub async fn prepare(dir: &Path, owner: Option<String>, task: &Arc<Task>) -> Result<(PathBuf, String)> {
    let root = find_root(dir).await.ok_or_else(|| anyhow!("Horizon's download has no {LOCK_FILE}"))?;
    let mc = mc_in_lock(&tokio::fs::read_to_string(root.join(LOCK_FILE)).await?).ok_or_else(|| anyhow!("Couldn't read which Minecraft version Horizon needs"))?;

    task.update(format!("Downloading Minecraft {mc}…"), None);
    let vanilla = get_vanilla_version(&mc).await?;
    let mut items = vec![client_jar(&vanilla)?];
    items.extend(resolve_libraries(&vanilla).into_iter().map(|l| l.item));
    download_all(items, 8, task.counter("Downloading Minecraft")).await?;
    download_all(asset_items(&vanilla).await?.items, 16, task.counter("Downloading assets")).await?;
    let server_jar = paths::cache().join("server-installers").join(format!("horizon-server-{mc}.jar"));
    crate::server::installers::vanilla_jar(&mc, &server_jar, task).await?;

    let java = java_for(vanilla.java_major(), owner, &mc).await?;
    let java_home = Path::new(&java).parent().and_then(Path::parent).map(Path::to_path_buf);
    let tool = root.join(if cfg!(windows) { "horizon-prepare.exe" } else { "horizon-prepare" });
    make_executable(&tool).await?;
    let args = vec!["--prepare".to_string(), "--server-jar".into(), plain(&server_jar), "--minecraft-dir".into(), plain(&paths::root()), "--root".into(), plain(&root)];
    run_tool(&plain(&tool), &args, &root, task, "Horizon", java_home.as_deref().map(|h| ("JAVA_HOME", h))).await?;
    let _ = tokio::fs::remove_file(&server_jar).await;
    if !resources(&root, &mc).is_dir() {
        return Err(anyhow!("Preparing Horizon finished without its resources"));
    }
    Ok((root, mc))
}

async fn install(release: &github::Release, id: &str, owner: Option<String>, task: &Arc<Task>) -> Result<Build> {
    let name = client_archive(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| anyhow!("Horizon has no build for this computer"))?;
    let dir = builds_dir().join(id);
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let done: Result<(PathBuf, String)> = async {
        fetch(release, name, &dir, task).await?;
        prepare(&dir, owner, task).await
    }
    .await;
    let (root, mc) = match done {
        Ok(d) => d,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(e);
        }
    };
    make_executable(&root.join("horizon-client")).await?;
    tokio::fs::write(builds_dir().join(CURRENT_FILE), id).await?;
    Ok(Build { id: id.to_string(), root, mc })
}

/// The newest build, installed if it has to be. Whatever goes wrong while updating, the copy that's there stays in use.
pub async fn ensure_build(owner: Option<String>, task: &Arc<Task>) -> Result<Build> {
    let have = installed().await;
    let (release, id) = match latest().await {
        Ok(l) => l,
        Err(e) => return have.ok_or(e),
    };
    if let Some(b) = have.as_ref().filter(|b| b.id == id) {
        return Ok(b.clone());
    }
    match install(&release, &id, owner, task).await {
        Ok(new) => {
            // A game that's still running holds its files: that copy goes next time.
            if let Some(old) = &have {
                let _ = tokio::fs::remove_dir_all(builds_dir().join(&old.id)).await;
            }
            Ok(new)
        }
        Err(e) => match have {
            Some(old) => {
                eprintln!("[horizon] keeping {}: {e:#}", old.id);
                Ok(old)
            }
            None => Err(e),
        },
    }
}

/// Forgets the installed builds, so the next start installs the newest again.
pub async fn forget_builds() {
    let _ = tokio::fs::remove_dir_all(builds_dir()).await;
}

#[derive(Deserialize)]
struct ApiOption {
    name: String,
}
#[derive(Deserialize)]
struct ApiReply {
    launch_api: String,
    #[serde(default)]
    options: Vec<ApiOption>,
}

/// What a build says it takes, or why ResinCraft can't start it.
pub fn read_launch_api(reply: &str) -> Result<Vec<String>, String> {
    let api: ApiReply = serde_json::from_str(reply.trim()).map_err(|_| "its --launch-api reply couldn't be read".to_string())?;
    let options: Vec<String> = api.options.into_iter().map(|o| o.name).collect();
    match REQUIRED_OPTIONS.iter().find(|o| !options.iter().any(|have| have == *o)) {
        Some(missing) => Err(format!("Launch API {} doesn't offer {missing}", api.launch_api)),
        None => Ok(options),
    }
}

fn launch_api_error(why: &str) -> anyhow::Error {
    err_tr!("horizon.launchApi", "This Horizon build can't be started from ResinCraft ({why}). Open Horizon's own launcher from the Horizon page instead.", why = why)
}

/// Asks the build what it takes, as the Launch API says to before every start.
pub async fn launch_options(exe: &Path) -> Result<Vec<String>> {
    let mut cmd = hidden_command(&plain(exe));
    cmd.arg("--launch-api").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await.map_err(|_| launch_api_error("--launch-api didn't answer"))?;
    let out = out.map_err(|e| launch_api_error(&e.to_string()))?;
    if !out.status.success() {
        return Err(launch_api_error(&format!("--launch-api exited with {}", out.status)));
    }
    read_launch_api(&String::from_utf8_lossy(&out.stdout)).map_err(|why| launch_api_error(&why))
}

/// The game's arguments. Options a build doesn't list are left out: it would refuse them.
pub fn game_args(options: &[String], resources: &Path, game_dir: &Path, width: u32, height: u32, target: Option<&Target>) -> Vec<String> {
    let has = |name: &str| options.iter().any(|o| o == name);
    let mut args = vec!["--resources".to_string(), plain(resources), "--game-dir".into(), plain(game_dir), "--home".into(), plain(game_dir)];
    for (name, value) in [("--width", width), ("--height", height)] {
        if has(name) {
            args.extend([name.to_string(), value.to_string()]);
        }
    }
    match target {
        Some(Target::Server(address)) if has("--quickPlayMultiplayer") => args.extend(["--quickPlayMultiplayer".to_string(), address.clone()]),
        Some(Target::World(world)) if has("--quickPlaySingleplayer") => args.extend(["--quickPlaySingleplayer".to_string(), world.clone()]),
        _ => {}
    }
    args.push("--session-stdin".to_string());
    args
}

/// The account the game joins online-mode servers with, for its standard input: never on its command line.
pub fn session_json(auth: &LaunchAuth) -> String {
    let mut session = json!({ "name": auth.username, "uuid": auth.uuid, "access_token": auth.access_token, "user_type": "msa" });
    if let Some(xuid) = &auth.xuid {
        session["xuid"] = json!(xuid);
    }
    session.to_string()
}

/// Horizon's rule for every launcher: it starts only for a signed-in Microsoft account that owns Java Edition.
/// `launch_auth_for` has already turned away accounts that don't own it.
pub fn check_owner(auth: &LaunchAuth) -> Result<()> {
    if auth.user_type == "msa" {
        return Ok(());
    }
    Err(err_tr!("horizon.needsOwner", "Horizon only starts for a Microsoft account that owns Minecraft: Java Edition. {name} is an offline account: switch to your Microsoft account to play.", name = auth.username))
}

/// The newest build for an instance, and how to start it. The instance follows the Minecraft version of the build.
pub async fn prepare_launch(inst: &Instance, width: u32, height: u32, target: Option<&Target>, task: &Arc<Task>) -> Result<(String, Vec<String>)> {
    let build = ensure_build(Some(inst.id.clone()), task).await?;
    if build.mc != inst.mc_version {
        crate::core::instances::update(&inst.id, json!({ "mcVersion": build.mc })).await?;
    }
    let exe = build.client();
    let options = launch_options(&exe).await?;
    let game_dir = paths::game_dir(&inst.id);
    tokio::fs::create_dir_all(&game_dir).await.context("Couldn't make the game folder")?;
    Ok((plain(&exe), game_args(&options, &build.resources(), &game_dir, width, height, target)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth(user_type: &'static str, xuid: Option<&str>) -> LaunchAuth {
        LaunchAuth { account_id: "a".into(), username: "Alex".into(), uuid: "069a79f444e94726a5befca90e38aaf5".into(), access_token: "t".into(), user_type, xuid: xuid.map(str::to_string) }
    }

    #[test]
    fn nightly_titles_name_a_build() {
        assert_eq!(build_id("Nightly 2026-10-31 · 9d979ca").as_deref(), Some("nightly-2026-10-31-9d979ca"));
        assert_eq!(build_id("nightly"), None);
        assert_eq!(build_id("Nightly ../evil · 9d979ca"), None);
    }

    #[test]
    fn archives_per_computer() {
        assert_eq!(server_archive("windows", "x86_64"), Some(("horizon-windows-x64.zip", "horizon-client.exe")));
        assert_eq!(server_archive("linux", "x86_64"), Some(("horizon-server-linux-x86_64.tar.gz", "horizon-engine")));
        assert_eq!(server_archive("linux", "aarch64"), None);
        assert_eq!(client_archive("linux", "x86_64"), Some("horizon-linux-x86_64.tar.gz"));
        assert_eq!(client_archive("macos", "x86_64"), None);
    }

    #[test]
    fn lock_names_its_minecraft() {
        assert_eq!(mc_in_lock(r#"{"minecraft":{"version":"26.3","manifest_sha1":"x"}}"#).as_deref(), Some("26.3"));
        assert_eq!(mc_in_lock("{}"), None);
    }

    #[test]
    fn launch_api_reply_needs_what_we_pass() {
        let good = r#"{"launch_api":"0.1.0","future":1,"options":[{"name":"--resources","value":"DIR"},{"name":"--game-dir"},{"name":"--home"},{"name":"--session-stdin","value":null}]}"#;
        assert_eq!(read_launch_api(good).unwrap().len(), 4);
        let missing = r#"{"launch_api":"0.2.0","options":[{"name":"--resources"},{"name":"--game-dir"},{"name":"--home"}]}"#;
        assert_eq!(read_launch_api(missing).unwrap_err(), "Launch API 0.2.0 doesn't offer --session-stdin");
        assert!(read_launch_api("Horizon v1").is_err());
    }

    #[test]
    fn game_args_leave_out_what_a_build_refuses() {
        let basic: Vec<String> = REQUIRED_OPTIONS.iter().map(|o| o.to_string()).collect();
        let args = game_args(&basic, Path::new("res"), Path::new("game"), 854, 480, Some(&Target::Server("mc.test:25565".into())));
        assert_eq!(args, ["--resources", "res", "--game-dir", "game", "--home", "game", "--session-stdin"]);
        let mut all = basic;
        all.extend(["--width", "--height", "--quickPlayMultiplayer", "--quickPlaySingleplayer"].map(str::to_string));
        let args = game_args(&all, Path::new("res"), Path::new("game"), 854, 480, Some(&Target::Server("mc.test:25565".into())));
        assert!(args.windows(2).any(|w| w == ["--quickPlayMultiplayer", "mc.test:25565"]));
        assert!(args.windows(2).any(|w| w == ["--width", "854"]));
        let args = game_args(&all, Path::new("res"), Path::new("game"), 854, 480, Some(&Target::World("My World".into())));
        assert!(args.windows(2).any(|w| w == ["--quickPlaySingleplayer", "My World"]));
    }

    #[test]
    fn session_is_the_documented_json() {
        let v: Value = serde_json::from_str(&session_json(&auth("msa", Some("123")))).unwrap();
        assert_eq!((v["name"].as_str(), v["access_token"].as_str(), v["user_type"].as_str(), v["xuid"].as_str()), (Some("Alex"), Some("t"), Some("msa"), Some("123")));
        let v: Value = serde_json::from_str(&session_json(&auth("msa", None))).unwrap();
        assert!(v.get("xuid").is_none());
    }

    #[test]
    fn offline_accounts_are_refused() {
        assert!(check_owner(&auth("msa", None)).is_ok());
        assert!(check_owner(&auth("legacy", None)).is_err());
    }
}
