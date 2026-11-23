//! `update:*`: finding a newer ResinCraft and installing it in place, with Tauri's updater plugin.
//!
//! The update file is the release's NSIS installer itself (`bundle.createUpdaterArtifacts`), signed with the key
//! whose public half is in tauri.conf.json. It is checked against that key after downloading, then runs in
//! passive mode with `/UPDATE`: it replaces the program files only, so everything in %APPDATA%\ResinCraft
//! Launcher (instances, accounts, settings) stays, and it starts the new version when it's done. On macOS the
//! update is the `.app.tar.gz`, on Linux the `.AppImage` (or the `.deb` for .deb installs); both replace only
//! the app, and the app restarts itself.
//!
//! Where updates come from (tauri.conf.json `plugins.updater.endpoints`): latest.json on the repository's `updates`
//! branch, which .github/workflows/publish-update.yml writes when a release is published, so publishing a release's
//! draft is what offers it to players. (Launcher 0.3.0 read the newest release's own latest.json instead.) Test
//! builds point elsewhere with `tauri build --config` (see README, "Updates").
//!
//! Two invite-only channels live on the relay, each with its own list and its own build, and neither gives the other:
//! - Dev, for developers (an admin adds them): the Dev build (`/api/dev/feed`, `/dev/<target>`), what they install in
//!   their own launcher. `release-dev.bat` uploads it.
//! - Test, for testers a developer invites: the Test build (`/api/dev/test/feed`, `/dev/test/<target>`).
//! Launcher admins have both. The relay serves the feeds and update files only with the account's relay session.
//! Dev and Test users also get Beta's releases when they're newer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt};

use crate::core::events::{emit, Task};
use crate::core::fsutil::{read_json, write_json};
use crate::core::router::{done, json};
use crate::core::text::Text;
use crate::core::{ctx, instances, paths, Args};
use crate::{bail_tr, tr};

const RELEASES: &str = "https://github.com/ResinCoreAI/ResinCraft-Launcher/releases";
/// The Beta channel's feed (Settings → Updates): pre-releases and stable releases, written by publish-update.yml.
const STABLE_FEED: &str = "https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/updates/latest.json";
const BETA_FEED: &str = "https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/updates/beta.json";

/// How long the relay's word on the Dev channel is kept before Settings asks again.
const DEV_ACCESS_TTL_MS: u64 = 10 * 60_000;

/// What the UI shows in Settings → Updates and in the update banner (also sent as the `update` event).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// The running version.
    pub current: String,
    /// "idle" | "checking" | "downloading" | "waiting" | "installing"
    pub state: &'static str,
    /// When the last check finished (ms since 1970), successful or not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<u64>,
    /// A newer version that can be installed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<Available>,
    /// Why the last check or install didn't work, in words for players.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Text>,
    /// While waiting to install: what is still running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_for: Option<Blockers>,
    /// Installed from the Microsoft Store (an MSIX package): the Store updates it, so ResinCraft never looks.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub store: bool,
    /// Whether the signed-in account may use the Dev channel (the relay's admin invited it), once the relay said:
    /// Settings offers it. A launcher left on Dev without it keeps the setting and gets Beta's updates meanwhile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<bool>,
    /// The same for the Test channel: a tester's invitation (separate from Dev, which is for developers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<bool>,
}

/// Whether this copy runs from an MSIX package (the Microsoft Store), which Windows updates itself.
pub fn packaged() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
        use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
        let mut len = 0u32;
        // Unpackaged processes get APPMODEL_ERROR_NO_PACKAGE; packaged ones "buffer too small".
        unsafe { GetCurrentPackageFullName(&mut len, windows::core::PWSTR::null()) != APPMODEL_ERROR_NO_PACKAGE }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Release date (RFC 3339), when the update server gave one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// The release page with the full notes.
    pub notes_url: String,
    /// The version already installed, offered again by Reinstall.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reinstall: bool,
    /// A Dev build from the relay (it has no release page: `notes_url` is empty).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dev: bool,
}

/// Games and hosted servers that would be cut off by the restart (their names, for the warning).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Blockers {
    pub games: Vec<String>,
    pub servers: Vec<String>,
}

impl Blockers {
    pub fn is_empty(&self) -> bool {
        self.games.is_empty() && self.servers.is_empty()
    }
}

#[derive(Default)]
struct State {
    status: UpdateStatus,
    /// The update found by the last check, kept for `update:install`.
    update: Option<Update>,
    /// A check or an install is running.
    busy: bool,
    /// When the relay last answered about the Dev channel (ms since 1970).
    dev_asked: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Saved between runs so Settings can say when ResinCraft last looked.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Saved {
    last_checked: Option<u64>,
}

fn state_file() -> std::path::PathBuf {
    paths::root().join("update.json")
}

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut guard = STATE.lock().unwrap();
    f(guard.get_or_insert_with(State::default))
}

/// Marks a check or install as running; false when one already is.
fn begin() -> bool {
    with(|st| !std::mem::replace(&mut st.busy, true))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The version this copy was built as: the bundle's version (a Dev/Test/Beta build sets it at build time), not
/// Cargo.toml's, which stays the last release.
pub(crate) fn current_version() -> String {
    ctx::app().map(|a| a.package_info().version.to_string()).unwrap_or_else(|| env!("CARGO_PKG_VERSION").into())
}

/// Changes the status and tells the UI.
fn publish(f: impl FnOnce(&mut UpdateStatus)) -> UpdateStatus {
    let s = with(|st| {
        f(&mut st.status);
        st.status.clone()
    });
    emit("update", s.clone());
    s
}

async fn status() -> UpdateStatus {
    let saved: Saved = read_json(&state_file()).await.unwrap_or_default();
    with(|st| {
        st.status.current = current_version();
        st.status.store = packaged();
        if st.status.state.is_empty() {
            st.status.state = "idle";
        }
        if st.status.last_checked.is_none() {
            st.status.last_checked = saved.last_checked;
        }
        st.status.clone()
    })
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "update:status" => json(status().await),
        // Never fails because of the network: problems come back as `error` in the status.
        "update:check" => json(check().await?),
        // Settings → Updates → Reinstall: offers the newest release even when it's the one installed, then the
        // UI installs it like an update (signature checked, same installer type).
        "update:checkReinstall" => json(check_with(true).await?),
        "update:blockers" => json(blockers().await),
        // Settings → Updates: asks the relay whether this account may use the Dev channel (at most every 10 minutes).
        "update:devAccess" => {
            if with(|st| now_ms().saturating_sub(st.dev_asked) > DEV_ACCESS_TTL_MS) {
                ask_dev_access().await;
            }
            json(status().await)
        }
        // `true`: download now and install once every game and hosted server has stopped. "close": download, then
        // stop the hosted servers (saving their worlds) and close the games, and install.
        "update:install" => {
            let mode = match args.raw(0) {
                Value::String(s) if s == "close" => Mode::Close,
                Value::Bool(true) => Mode::Wait,
                _ => Mode::Now,
            };
            install(mode).await?;
            done()
        }
        // An update waiting for games and servers: stop the servers (saving their worlds) and close the games now.
        "update:closeRunning" => {
            close_running().await;
            done()
        }
        "update:cancel" => {
            CANCEL.store(true, Ordering::SeqCst);
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}

/// Asks the update server whether a newer version exists.
pub async fn check() -> Result<UpdateStatus> {
    check_with(false).await
}

/// `reinstall`: the newest release counts as an update even when it's the version running.
async fn check_with(reinstall: bool) -> Result<UpdateStatus> {
    let Some(app) = ctx::app() else { bail_tr!("update.noApp", "Updates only work in the installed app.") };
    if packaged() {
        return Ok(status().await);
    }
    if !begin() {
        return Ok(status().await);
    }
    status().await;
    publish(|s| s.state = "checking");

    let channel = crate::core::settings::get().await.update_channel;
    // Dev and Test: that relay build next to Beta's feed; an account that isn't on its list (any more) gets Beta.
    let relay_channel = channel == "dev" || channel == "test";
    let dev = if relay_channel { ask_dev_access().await } else { DevAccess::No };
    let mut builder = app.updater_builder().timeout(Duration::from_secs(30));
    if channel == "beta" || relay_channel {
        // The stable feed second: the updater tries the next address when one fails (no beta.json yet).
        builder = builder.endpoints(vec![BETA_FEED.parse().expect("beta feed address"), STABLE_FEED.parse().expect("stable feed address")])?;
    }
    if reinstall {
        builder = builder.version_comparator(|current, remote| remote.version >= current);
    }
    let result = match builder.build() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let mut from_dev = None;
    let feed = match dev {
        DevAccess::Yes { dev_feed, test_feed, session, .. } => if channel == "test" { test_feed } else { dev_feed }.map(|f| (f, session)),
        _ => None,
    };
    let result = match feed {
        Some((feed, session)) => {
            let found = check_dev(&app, &feed, &session, reinstall).await;
            from_dev = found.as_ref().ok().and_then(|u| u.as_ref()).map(|u| u.version.clone());
            newest(result, found)
        }
        None => result,
    };
    let checked = now_ms();
    let _ = write_json(&state_file(), &Saved { last_checked: Some(checked) }).await;

    let (update, error) = match result {
        Ok(update) => (update, None),
        Err(e) => {
            eprintln!("[update] check failed: {e}");
            (None, Some(calm(&e)))
        }
    };
    let available = update.as_ref().map(|u| {
        let dev = from_dev.as_deref() == Some(u.version.as_str());
        Available {
            version: u.version.clone(),
            notes: u.body.clone().filter(|b| !b.trim().is_empty()),
            date: u.raw_json.get("pub_date").and_then(Value::as_str).map(str::to_string),
            notes_url: if dev { String::new() } else { format!("{RELEASES}/tag/v{}", u.version.trim_start_matches('v')) },
            reinstall: u.version.trim_start_matches('v') == current_version(),
            dev,
        }
    });
    with(|st| {
        st.busy = false;
        // A failed check keeps what an earlier one found.
        if error.is_none() || update.is_some() {
            st.update = update;
        }
    });
    Ok(publish(|s| {
        s.state = "idle";
        s.last_checked = Some(checked);
        if error.is_none() {
            s.available = available;
        }
        s.error = error;
    }))
}

/// What the relay says about this launcher's account and its two invite-only channels.
enum DevAccess {
    /// On at least one of the relay's lists (or a launcher admin), with the relay session that opens the feeds (and
    /// their downloads). `dev_feed`: a developer's Dev build. `test_feed`: a tester's Test build (a separate
    /// invitation; neither gives the other). `developer`: the account may also manage both from the Admin page.
    Yes { dev_feed: Option<String>, test_feed: Option<String>, session: String, developer: bool },
    /// Not invited, no Microsoft account, a relay without the Dev channel, or a build without the relay.
    No,
    /// The relay couldn't be asked (offline…): Settings keeps what it last heard.
    Unknown,
}

/// Asks the relay (signing the active Microsoft account in when it has no relay session yet) and tells Settings.
async fn ask_dev_access() -> DevAccess {
    let access = dev_access().await;
    if !matches!(access, DevAccess::Unknown) {
        let (dev, test) = match &access {
            DevAccess::Yes { dev_feed, test_feed, .. } => (dev_feed.is_some(), test_feed.is_some()),
            _ => (false, false),
        };
        let changed = with(|st| {
            st.dev_asked = now_ms();
            st.status.dev != Some(dev) || st.status.test != Some(test)
        });
        if changed {
            publish(|s| {
                s.dev = Some(dev);
                s.test = Some(test);
            });
        }
    }
    access
}

/// Whether the active account is one of the relay's developers (the Admin page shows them its Dev section).
pub(crate) async fn is_developer() -> bool {
    matches!(ask_dev_access().await, DevAccess::Yes { developer: true, .. })
}

async fn dev_access() -> DevAccess {
    use crate::server::relay;
    if relay::settings().is_none() {
        return DevAccess::No;
    }
    let Some(account) = crate::player::accounts::microsoft_account(None).await else { return DevAccess::No };
    for retry in [false, true] {
        let (session, domain) = match relay::session_token(&account).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[update] no relay session for the Dev channel: {e:#}");
                return DevAccess::Unknown;
            }
        };
        let res = ctx::http().get(format!("https://{domain}/api/dev/access")).bearer_auth(&session).timeout(Duration::from_secs(15)).send().await;
        match res.map(|r| (r.status().as_u16(), r)) {
            // The saved session ran out: sign in again, once.
            Ok((401, _)) if !retry => relay::forget_session(&account).await,
            Ok((200, r)) => {
                let v = r.json::<Value>().await.unwrap_or(Value::Null);
                let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
                // A relay from before the Test channel doesn't send `test`.
                let dev_feed = flag("dev").then(|| format!("https://{domain}/api/dev/feed"));
                let test_feed = flag("test").then(|| format!("https://{domain}/api/dev/test/feed"));
                return if dev_feed.is_some() || test_feed.is_some() || flag("developer") {
                    DevAccess::Yes { dev_feed, test_feed, session, developer: flag("developer") }
                } else {
                    DevAccess::No
                };
            }
            // A relay without the Dev channel (older), or a banned account.
            Ok((403 | 404, _)) => return DevAccess::No,
            Ok((code, _)) => {
                eprintln!("[update] the relay answered HTTP {code} about the Dev channel");
                return DevAccess::Unknown;
            }
            Err(e) => {
                eprintln!("[update] couldn't ask the relay about the Dev channel: {}", e.without_url());
                return DevAccess::Unknown;
            }
        }
    }
    DevAccess::Unknown
}

/// The relay's Dev build, when it's newer than the running version (or, `reinstall`, the same).
async fn check_dev(app: &tauri::AppHandle, feed: &str, session: &str, reinstall: bool) -> Result<Option<Update>, UpdaterError> {
    let Ok(url) = feed.parse() else { return Ok(None) };
    let mut builder = app.updater_builder().timeout(Duration::from_secs(30)).endpoints(vec![url])?.header("Authorization", format!("Bearer {session}"))?;
    if reinstall {
        builder = builder.version_comparator(|current, remote| remote.version >= current);
    }
    builder.build()?.check().await
}

/// The newer of Beta's update and the Dev build. A Dev problem (no build for this platform, the relay offline) never
/// hides Beta's update; Beta's problem only shows when there's no Dev build either.
fn newest(beta: Result<Option<Update>, UpdaterError>, dev: Result<Option<Update>, UpdaterError>) -> Result<Option<Update>, UpdaterError> {
    let version = |u: &Update| semver::Version::parse(u.version.trim_start_matches('v')).ok();
    match (beta, dev) {
        (Ok(Some(b)), Ok(Some(d))) => Ok(Some(if version(&d) > version(&b) { d } else { b })),
        (Ok(b), Ok(None)) => Ok(b),
        (Ok(None), Ok(d)) => Ok(d),
        (Ok(b), Err(e)) => {
            eprintln!("[update] Dev check failed: {e}");
            Ok(b)
        }
        (Err(_), Ok(Some(d))) => Ok(Some(d)),
        (Err(e), _) => Err(e),
    }
}

/// Names of the games and hosted servers that are running.
pub async fn blockers() -> Blockers {
    let mut out = Blockers::default();
    for id in crate::game::running_ids() {
        out.games.push(instances::get(&id).await.map(|i| i.name).unwrap_or(id));
    }
    for id in crate::server::runtime::running_ids() {
        out.servers.push(crate::server::store::get(&id).await.map(|s| s.name).unwrap_or(id));
    }
    out
}

/// When to install a downloaded update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Right away (nothing may be running).
    Now,
    /// Once every game and hosted server has stopped.
    Wait,
    /// After stopping the hosted servers and closing the games.
    Close,
}

/// Downloads the update found by the last check and installs it. Returns once the download has started; the
/// task dock shows progress. On Windows the app closes when the installer starts, and the installer opens the
/// new version.
async fn install(mode: Mode) -> Result<()> {
    if ctx::app().is_none() {
        bail_tr!("update.noApp", "Updates only work in the installed app.");
    }
    if packaged() {
        bail_tr!("update.store", "The Microsoft Store updates this copy of ResinCraft.");
    }
    // A dev build would replace the installed copy rather than itself.
    if cfg!(debug_assertions) {
        bail_tr!("update.devBuild", "Development builds don't install updates. Use the installed ResinCraft for that.");
    }
    let update = with(|st| st.update.clone());
    let Some(update) = update else {
        bail_tr!("update.nothing", "There's no update to install. Check for updates first.");
    };
    if mode == Mode::Now && !blockers().await.is_empty() {
        bail_tr!("update.stillRunning", "Close Minecraft and stop your hosted servers before updating.");
    }
    if !begin() {
        bail_tr!("update.busy", "ResinCraft is already checking for or installing an update.");
    }
    CANCEL.store(false, Ordering::SeqCst);
    let version = update.version.clone();
    let reinstall = version.trim_start_matches('v') == current_version();
    tauri::async_runtime::spawn(async move {
        let title = if reinstall {
            tr!("update.task.reinstall", "Reinstalling ResinCraft {version}", version = version)
        } else {
            tr!("update.task.title", "Updating ResinCraft to {version}", version = version)
        };
        let result = Task::run(title, None, |task| run_install(update, mode == Mode::Close, task)).await;
        with(|st| st.busy = false);
        if let Err(e) = result {
            eprintln!("[update] install failed: {e:#}");
            let error = crate::core::text::text_of(&e).cloned().unwrap_or_else(|| calm_any(&e));
            // A file that fails the signature check is not offered again (a network problem is: try again later).
            let refused = matches!(error.key.as_str(), "update.badSignature" | "update.notForThisPc");
            if refused {
                with(|st| st.update = None);
            }
            publish(|s| {
                s.state = "idle";
                s.waiting_for = None;
                if refused {
                    s.available = None;
                }
                s.error = Some(error);
            });
        }
    });
    Ok(())
}

async fn run_install(update: Update, close_running: bool, task: std::sync::Arc<Task>) -> Result<()> {
    publish(|s| {
        s.state = "downloading";
        s.error = None;
    });
    let downloading = tr!("update.task.downloading", "Downloading the update");
    task.update(downloading.clone(), Some(0.0));
    let mut received: u64 = 0;
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                if let Some(total) = total.filter(|t| *t > 0) {
                    task.update(downloading.clone(), Some((received as f64 / total as f64).min(0.99)));
                }
            },
            || {},
        )
        .await
        .map_err(|e| anyhow::Error::from(calm(&e)))?;

    if close_running {
        task.update(tr!("update.task.closing", "Stopping hosted servers and closing Minecraft"), None);
        self::close_running().await;
    }

    // Wait for games and servers (the restart would cut them off), checking every couple of seconds.
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            publish(|s| {
                s.state = "idle";
                s.waiting_for = None;
            });
            bail_tr!("update.cancelled", "Update cancelled. It will be offered again later.");
        }
        let running = blockers().await;
        if running.is_empty() {
            break;
        }
        task.update(tr!("update.task.waiting", "Waiting for Minecraft and hosted servers to stop"), None);
        publish(|s| {
            s.state = "waiting";
            s.waiting_for = Some(running);
        });
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    publish(|s| {
        s.state = "installing";
        s.waiting_for = None;
    });
    task.update(tr!("update.task.installing", "Installing. ResinCraft will restart by itself."), Some(1.0));
    // Let the UI show it before the window closes.
    tokio::time::sleep(Duration::from_millis(800)).await;
    // Windows: starts the installer and exits the app (the installer opens the new version). Elsewhere it
    // returns once the new files are in place, and the app restarts itself.
    update.install(&bytes).map_err(|e| anyhow::Error::from(calm(&e)))?;
    if let Some(app) = ctx::app() {
        app.restart()
    }
    Ok(())
}

/// Stops every hosted server (`stop`, which saves its world) and closes every running game.
async fn close_running() {
    crate::server::runtime::shutdown_all(Duration::from_secs(60)).await;
    for id in crate::game::running_ids() {
        crate::game::launch::kill_instance(&id, None);
    }
}

/// A message for players instead of the updater's technical one.
fn calm(e: &UpdaterError) -> Text {
    match e {
        UpdaterError::Reqwest(err) if err.is_timeout() || err.is_connect() || err.is_request() => offline(),
        UpdaterError::Reqwest(err) if err.status().is_some_and(|s| s.as_u16() == 403 || s.as_u16() == 429) => busy(),
        UpdaterError::Reqwest(_) | UpdaterError::Io(_) => offline(),
        UpdaterError::Network(msg) if msg.contains("403") || msg.contains("429") => busy(),
        UpdaterError::Network(_) => offline(),
        UpdaterError::Minisign(_)
        | UpdaterError::Base64(_)
        | UpdaterError::SignatureUtf8(_)
        | UpdaterError::SignedVersionMismatch { .. } => tr!(
            "update.badSignature",
            "The downloaded update isn't signed by Puirkle, so it wasn't installed. Your current version keeps working."
        ),
        // Every update address answered, but none with update information (e.g. an error page).
        UpdaterError::ReleaseNotFound => {
            tr!("update.noInfo", "Couldn't get update information right now. ResinCraft will try again later.")
        }
        UpdaterError::TargetNotFound(_) | UpdaterError::TargetsNotFound(_) => {
            tr!("update.notForThisPc", "The new version isn't available for this kind of computer yet.")
        }
        other => tr!("update.failed", "The update didn't work this time ({reason}). ResinCraft will try again later.", reason = other.to_string()),
    }
}

fn calm_any(e: &anyhow::Error) -> Text {
    tr!("update.failed", "The update didn't work this time ({reason}). ResinCraft will try again later.", reason = e.to_string())
}

fn offline() -> Text {
    tr!("update.offline", "Couldn't reach the update server. Check your internet connection; ResinCraft will try again later.")
}

fn busy() -> Text {
    tr!("update.rateLimited", "The update server is busy right now. ResinCraft will try again later.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_errors_read_calmly() {
        assert_eq!(calm(&UpdaterError::Network("Download request failed with status: 429 Too Many Requests".into())).key, "update.rateLimited");
        assert_eq!(calm(&UpdaterError::Network("Download request failed with status: 500".into())).key, "update.offline");
        assert_eq!(calm(&UpdaterError::TargetNotFound("windows-x86_64".into())).key, "update.notForThisPc");
        let bad = UpdaterError::SignedVersionMismatch { signed: "0.2.0".into(), announced: "0.3.0".into() };
        assert_eq!(calm(&bad).key, "update.badSignature");
        assert_eq!(calm(&UpdaterError::ReleaseNotFound).key, "update.noInfo");
        assert_eq!(calm(&UpdaterError::EmptyEndpoints).key, "update.failed");
    }

    #[test]
    fn blockers_empty_only_when_nothing_runs() {
        assert!(Blockers::default().is_empty());
        assert!(!Blockers { games: vec!["Cozy Survival".into()], servers: vec![] }.is_empty());
        assert!(!Blockers { games: vec![], servers: vec!["Friends SMP".into()] }.is_empty());
    }
}
