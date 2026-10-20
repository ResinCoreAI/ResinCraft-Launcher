//! Settings → Storage: where instances, hosted servers and the shared game files (libraries, assets, versions,
//! Java, caches) are kept, the drives to suggest, the checks a new folder must pass, and moving everything there.
//!
//! Everything under `paths::games()` moves as one: an instance can't start without its libraries and Java, so
//! splitting them over drives would only make a missing drive break games in confusing ways. Settings, accounts
//! and the launcher's other small files stay in the launcher folder (`paths::root()`, in %APPDATA%), which also
//! remembers the choice (`games-folder.json`).
//!
//! A move never leaves an instance half in each place: on the same drive the folders are renamed (undone if one
//! fails); onto another drive everything is copied into `<new>/.resin-moving` first, put in place only once every
//! file arrived, and the old copies are removed only after the launcher switched over. A failed or cancelled copy
//! removes the staging folder and leaves everything as it was.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::core::events::{self, Task};
use crate::core::router::json;
use crate::core::text::Text;
use crate::core::{fsutil, paths, settings, shell, Args};
use crate::{bail_tr, err_tr, tr};

/// A move is running: games and servers can't start, and another move waits.
static MOVING: AtomicBool = AtomicBool::new(false);

/// Where the copies wait (on the new drive) until every file arrived.
const STAGING: &str = ".resin-moving";

/// Longer paths leave too little room for Minecraft's own deep folders (Windows' 260-character limit).
const MAX_PATH_LEN: usize = 120;

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "storage:info" => json(info().await),
        "storage:drives" => json(tokio::task::spawn_blocking(drives).await?),
        "storage:check" => json(check(&args.get::<String>(0)?).await?),
        "storage:pick" => {
            // The UI sends the title in the player's language.
            let title = args.get::<Option<String>>(0)?.unwrap_or_else(|| "Where should ResinCraft keep your instances?".into());
            json(shell::pick_folder(&title).await?.map(|p| p.to_string_lossy().into_owned()))
        }
        "storage:change" => json(change(&args.get::<String>(0)?, args.get::<Option<bool>>(1)?.unwrap_or(true)).await?),
        _ => anyhow::bail!("Unknown request: {channel}"),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub folder: String,
    /// The launcher's own folder, which is also the default games folder.
    pub default_folder: String,
    pub is_default: bool,
    /// The drive (or mount point) the folder is on, e.g. `D:\`.
    pub drive: String,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    /// The folder can't be found (an unplugged drive).
    pub missing: bool,
    pub moving: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub root: String,
    pub label: String,
    /// `<drive>\ResinCraft`, or the current folder when it is on this drive.
    pub suggestion: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub removable: bool,
    /// The current games folder is on this drive.
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCheck {
    /// The folder as it will be used (cleaned up).
    pub folder: String,
    /// Why the folder can't be used at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<Text>,
    /// Why the current instances can't be moved there (starting empty there may still work).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_problem: Option<Text>,
    /// Why nothing can change right now (a game is running…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Text>,
    /// Bytes to copy (0 when the folder is on the same drive: the folders are only renamed).
    pub need_bytes: u64,
    pub free_bytes: Option<u64>,
    pub same_drive: bool,
    pub is_default: bool,
    /// There are instances, servers or game files to move.
    pub has_files: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeResult {
    pub info: StorageInfo,
    /// Some old files couldn't be removed (a file in use); they are copies, safe to delete by hand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leftovers: Option<String>,
}

pub fn ensure_not_moving() -> Result<()> {
    if MOVING.load(Ordering::SeqCst) {
        bail_tr!("storage.moving", "Your instances are being moved to another folder. Wait for that to finish.");
    }
    Ok(())
}

pub async fn info() -> StorageInfo {
    let folder = paths::games();
    let home = paths::root();
    tokio::task::spawn_blocking(move || {
        let space = space(&folder);
        StorageInfo {
            folder: display(&folder),
            default_folder: display(&home),
            is_default: same_path(&folder, &home),
            drive: drive_root(&folder).map(|d| display(&d)).unwrap_or_default(),
            free_bytes: space.map(|s| s.0),
            total_bytes: space.map(|s| s.1),
            missing: !folder.is_dir(),
            moving: MOVING.load(Ordering::SeqCst),
        }
    })
    .await
    .expect("storage info")
}

// ---------- Paths ----------

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Comparable form: separators unified, no trailing separator, and (on Windows) case folded.
fn key(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = if cfg!(windows) { s.replace('/', "\\").to_lowercase() } else { s.into_owned() };
    s.trim_end_matches(['\\', '/']).to_string()
}

fn same_path(a: &Path, b: &Path) -> bool {
    key(a) == key(b)
}

/// `child` is `parent` or inside it.
fn inside(child: &Path, parent: &Path) -> bool {
    let (c, p) = (key(child), key(parent));
    let sep = if cfg!(windows) { '\\' } else { '/' };
    !p.is_empty() && (c == p || c.starts_with(&format!("{p}{sep}")))
}

/// Cleans up a typed or picked folder: quotes, spaces, `/`, a trailing separator; `D:` means `D:\`.
pub fn normalize(raw: &str) -> PathBuf {
    let s = raw.trim().trim_matches('"').trim();
    let s = if cfg!(windows) { s.replace('/', "\\") } else { s.to_string() };
    let trimmed = s.trim_end_matches(['\\', '/']);
    if cfg!(windows) && trimmed.len() == 2 && trimmed.ends_with(':') {
        return PathBuf::from(format!("{trimmed}\\"));
    }
    if trimmed.is_empty() {
        return PathBuf::from(&s);
    }
    PathBuf::from(trimmed)
}

/// The closest folder that exists: `path` itself or one of its parents.
fn existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|a| a.is_dir())
}

fn gb(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
    } else {
        format!("{} MB", (bytes as f64 / (1u64 << 20) as f64).ceil() as u64)
    }
}

// ---------- Drives (Windows) ----------

#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, GetVolumePathNameW};

    pub const REMOVABLE: u32 = 2;
    pub const FIXED: u32 = 3;
    pub const REMOTE: u32 = 4;
    pub const CDROM: u32 = 5;

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    /// The volume (drive letter or mount folder) holding `p`, which must exist.
    pub fn volume(p: &Path) -> Option<PathBuf> {
        let mut buf = [0u16; 1024];
        unsafe { GetVolumePathNameW(PCWSTR(wide(p).as_ptr()), &mut buf) }.ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
    }

    /// 0 unknown, 1 no such drive, 2 removable, 3 fixed, 4 network, 5 CD/DVD, 6 RAM disk.
    pub fn drive_type(root: &Path) -> u32 {
        unsafe { GetDriveTypeW(PCWSTR(wide(root).as_ptr())) }
    }

    /// (free for this user, total) bytes.
    pub fn space(p: &Path) -> Option<(u64, u64)> {
        let (mut free, mut total) = (0u64, 0u64);
        unsafe { GetDiskFreeSpaceExW(PCWSTR(wide(p).as_ptr()), Some(&mut free), Some(&mut total), None) }.ok()?;
        Some((free, total))
    }

    pub fn label(root: &Path) -> String {
        let mut name = [0u16; 261];
        if unsafe { GetVolumeInformationW(PCWSTR(wide(root).as_ptr()), Some(&mut name), None, None, None, None) }.is_err() {
            return String::new();
        }
        let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        String::from_utf16_lossy(&name[..len])
    }

    pub fn letters() -> Vec<PathBuf> {
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8).filter(|i| mask & (1 << i) != 0).map(|i| PathBuf::from(format!("{}:\\", (b'A' + i) as char))).collect()
    }
}

/// The drive or mount point holding `path` (or where it would be created).
fn drive_root(path: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        existing_ancestor(path).and_then(win::volume)
    }
    #[cfg(not(windows))]
    {
        path.ancestors().last().map(Path::to_path_buf)
    }
}

/// (free, total) bytes on the drive holding `path`.
fn space(path: &Path) -> Option<(u64, u64)> {
    #[cfg(windows)]
    {
        existing_ancestor(path).and_then(win::space)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// Whether two folders (existing or to be made) are on the same drive, so moving is a rename.
fn same_volume(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        matches!((drive_root(a), drive_root(b)), (Some(x), Some(y)) if same_path(&x, &y))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let dev = |p: &Path| existing_ancestor(p).and_then(|e| std::fs::metadata(e).ok()).map(|m| m.dev());
        matches!((dev(a), dev(b)), (Some(x), Some(y)) if x == y)
    }
}

/// Fixed and removable drives with their free space, for the suggestions.
pub fn drives() -> Vec<Drive> {
    let current = paths::games();
    let current_drive = drive_root(&current);
    #[cfg(windows)]
    {
        win::letters()
            .into_iter()
            .filter_map(|root| {
                let kind = win::drive_type(&root);
                if kind != win::FIXED && kind != win::REMOVABLE {
                    return None;
                }
                let (free, total) = win::space(&root)?;
                let here = current_drive.as_deref().is_some_and(|d| same_path(d, &root));
                Some(Drive {
                    suggestion: display(&if here && !same_path(&current, &paths::root()) { current.clone() } else { root.join("ResinCraft") }),
                    label: win::label(&root),
                    root: display(&root),
                    free_bytes: free,
                    total_bytes: total,
                    removable: kind == win::REMOVABLE,
                    current: here,
                })
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        let _ = current_drive;
        Vec::new()
    }
}

/// A problem with the drive itself: missing, on the network or read-only.
fn drive_problem(folder: &Path) -> Option<Text> {
    let path = display(folder);
    #[cfg(windows)]
    {
        if path.starts_with("\\\\") && !path.starts_with("\\\\?\\") {
            return Some(network(&path));
        }
        let root = folder.ancestors().last()?;
        match win::drive_type(root) {
            0 | 1 => Some(tr!("storage.noDrive", "There's no drive {drive} on this PC.", drive = display(root))),
            win::REMOTE => Some(network(&path)),
            win::CDROM => Some(tr!("storage.readOnly", "{drive} is a CD or DVD drive: ResinCraft can't keep files there.", drive = display(root))),
            _ => None,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

#[cfg(windows)]
fn network(path: &str) -> Text {
    tr!(
        "storage.network",
        "{path} is on a network drive. Minecraft loads slowly from one and breaks when the connection drops: pick a drive in this PC.",
        path = path
    )
}

// ---------- Checks ----------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Off {
    /// Program Files, Windows.
    System,
    /// OneDrive uploads every file and may keep them online only.
    OneDrive,
    /// Windows clears the temp folder.
    Temp,
}

/// What a new folder is checked against.
pub struct Rules {
    /// The games folder now.
    pub current: PathBuf,
    /// The launcher's own folder.
    pub home: PathBuf,
    pub off_limits: Vec<(PathBuf, Off)>,
}

impl Rules {
    pub fn system() -> Rules {
        let env = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
        let mut off = Vec::new();
        for name in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "SystemRoot", "windir"] {
            off.extend(env(name).map(|p| (p, Off::System)));
        }
        for name in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
            off.extend(env(name).map(|p| (p, Off::OneDrive)));
        }
        off.extend(env("TEMP").map(|p| (p, Off::Temp)));
        Rules { current: paths::games(), home: paths::root(), off_limits: off }
    }
}

/// Why `folder` can't be the games folder, or None. Makes the folder if it is missing (and removes it again
/// if it was only made for the check); `drive` checks the drive itself (unit tests skip that).
pub fn check_folder(folder: &Path, rules: &Rules, drive: impl Fn(&Path) -> Option<Text>) -> Option<Text> {
    let path = display(folder);
    if path.is_empty() || !folder.is_absolute() {
        let example = if cfg!(windows) { "D:\\ResinCraft" } else { "/home/you/ResinCraft" };
        return Some(tr!("storage.notAbsolute", "Type a whole folder path, like {example}.", example = example));
    }
    if let Some(problem) = drive(folder) {
        return Some(problem);
    }
    if folder.parent().is_none() {
        return Some(tr!(
            "storage.driveRoot",
            "Pick a folder on the drive rather than the whole drive, like {suggestion}.",
            suggestion = display(&folder.join("ResinCraft"))
        ));
    }
    if path.chars().count() > MAX_PATH_LEN {
        return Some(tr!("storage.tooLong", "That path is too long for Minecraft's files. Pick one under {max} characters.", max = MAX_PATH_LEN));
    }
    for (dir, why) in &rules.off_limits {
        if inside(folder, dir) {
            return Some(match why {
                Off::System => tr!("storage.system", "Windows protects {path}: pick a folder outside Program Files and Windows.", path = display(dir)),
                Off::OneDrive => tr!(
                    "storage.oneDrive",
                    "{path} is in OneDrive, which would upload every game file and may keep them online only. Pick a folder outside OneDrive.",
                    path = display(dir)
                ),
                Off::Temp => tr!("storage.temp", "Windows clears {path} from time to time. Pick another folder.", path = display(dir)),
            });
        }
    }
    if same_path(folder, &rules.current) {
        return Some(tr!("storage.same", "Your instances are already kept there."));
    }
    // Moving a folder into itself can't work; the launcher folder counts too (old files may still be there).
    for base in [&rules.current, &rules.home] {
        for top in paths::GAME_FOLDERS {
            if inside(folder, &base.join(top)) {
                return Some(tr!("storage.insideGames", "That folder is inside ResinCraft's own game files. Pick one outside {path}.", path = display(base)));
            }
        }
    }
    for a in folder.ancestors() {
        let name = a.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if a.join("instance.json").is_file() || a.join("server.json").is_file() || name.eq_ignore_ascii_case(".minecraft") {
            return Some(tr!("storage.insideInstance", "That folder is inside an instance or a server ({name}). Pick a folder outside it.", name = name));
        }
    }
    if folder.exists() && !folder.is_dir() {
        return Some(tr!("storage.notFolder", "{path} is a file, not a folder.", path = path));
    }
    writable(folder).err().map(|e| tr!("storage.notWritable", "ResinCraft can't save files in {path}: {reason}", path = path, reason = e.to_string()))
}

/// Tries to make the folder and write a file there; anything made only for the test is removed again.
fn writable(folder: &Path) -> io::Result<()> {
    let first_missing = folder.ancestors().take_while(|a| !a.exists()).last().map(Path::to_path_buf);
    let result = (|| {
        std::fs::create_dir_all(folder)?;
        let probe = folder.join(".resin-write-test");
        std::fs::write(&probe, b"ResinCraft")?;
        std::fs::remove_file(&probe)
    })();
    if let Some(dir) = first_missing {
        let _ = std::fs::remove_dir_all(dir);
    }
    result
}

/// An instance or server folder that already exists in `new` with the same name as one in `old`.
fn collision(old: &Path, new: &Path) -> Option<String> {
    for top in ["instances", "servers"] {
        let Ok(rd) = std::fs::read_dir(old.join(top)) else { continue };
        for e in rd.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) && new.join(top).join(e.file_name()).exists() {
                return Some(display(&new.join(top).join(e.file_name())));
            }
        }
    }
    None
}

fn has_files(old: &Path) -> bool {
    paths::GAME_FOLDERS.iter().any(|t| std::fs::read_dir(old.join(t)).is_ok_and(|mut rd| rd.next().is_some()))
}

/// Everything the Settings dialog needs to know about a folder before the player confirms.
pub async fn check(raw: &str) -> Result<FolderCheck> {
    let folder = normalize(raw);
    let blocked = blocker().await;
    tokio::task::spawn_blocking(move || {
        let rules = Rules::system();
        let problem = check_folder(&folder, &rules, drive_problem);
        let old = rules.current.clone();
        let same_drive = same_volume(&old, &folder);
        let has_files = has_files(&old);
        let need_bytes = if same_drive { 0 } else { paths::GAME_FOLDERS.iter().map(|t| fsutil::dir_size(&old.join(t))).sum() };
        let free_bytes = space(&folder).map(|s| s.0);
        let move_problem = if !old.is_dir() {
            Some(tr!("storage.currentMissing", "{path} can't be found (is its drive connected?), so there's nothing to move.", path = display(&old)))
        } else if let Some(path) = collision(&old, &folder) {
            Some(tr!("storage.collision", "{path} is already there. Move or rename it first, or start empty there.", path = path))
        } else {
            match free_bytes {
                // A little room to spare: Windows and the game need some too.
                Some(free) if need_bytes + (256 << 20) > free => Some(tr!(
                    "storage.noSpace",
                    "Moving needs {need}, but that drive has only {free} free.",
                    need = gb(need_bytes),
                    free = gb(free)
                )),
                _ => None,
            }
        };
        FolderCheck {
            is_default: same_path(&folder, &rules.home),
            folder: display(&folder),
            problem,
            move_problem,
            blocked,
            need_bytes,
            free_bytes,
            same_drive,
            has_files,
        }
    })
    .await
    .map_err(Into::into)
}

/// Why nothing can change right now: a game or server running, other work in the task list, a move.
async fn blocker() -> Option<Text> {
    if MOVING.load(Ordering::SeqCst) {
        return Some(tr!("storage.moving", "Your instances are being moved to another folder. Wait for that to finish."));
    }
    let running = crate::updater::blockers().await;
    if !running.games.is_empty() {
        return Some(tr!("storage.gameRunning", "Close Minecraft first: {names} is still running.", names = running.games.join(", ")));
    }
    if !running.servers.is_empty() {
        return Some(tr!("storage.serverRunning", "Stop your hosted servers first: {names}.", names = running.servers.join(", ")));
    }
    if events::running_tasks() > 0 {
        return Some(tr!("storage.busy", "Wait for the downloads and installs in the task list to finish first."));
    }
    None
}

// ---------- Changing and moving ----------

/// Clears MOVING when the move's thread ends, however it ends.
struct MovingGuard;
impl Drop for MovingGuard {
    fn drop(&mut self) {
        MOVING.store(false, Ordering::SeqCst);
    }
}

/// Sets the stop flag when the task's work is dropped (Cancel in the task list).
struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Switches to `raw` as the games folder. `move_files`: bring the current instances and game files along
/// (a cancellable task); otherwise start empty there and leave the current ones where they are.
pub async fn change(raw: &str, move_files: bool) -> Result<ChangeResult> {
    let c = check(raw).await?;
    if let Some(p) = c.problem.or(c.blocked) {
        return Err(p.into());
    }
    if move_files && c.has_files {
        if let Some(p) = c.move_problem {
            return Err(p.into());
        }
    }
    if MOVING.swap(true, Ordering::SeqCst) {
        bail_tr!("storage.moving", "Your instances are being moved to another folder. Wait for that to finish.");
    }
    let (old, new, home) = (paths::games(), PathBuf::from(&c.folder), paths::root());
    let rt = tokio::runtime::Handle::current();

    if !move_files || !c.has_files {
        let leftovers = tokio::task::spawn_blocking(move || {
            let _guard = MovingGuard;
            std::fs::create_dir_all(&new)?;
            commit(&home, &old, &new, false, &rt)?;
            anyhow::Ok(None)
        })
        .await??;
        return Ok(ChangeResult { info: info().await, leftovers });
    }

    let title = tr!("storage.task", "Moving instances to {path}", path = c.folder.clone());
    let leftovers = Task::run(title, None, move |task| async move {
        let stop = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Progress::default());
        let _stop = StopOnDrop(stop.clone());
        let (s, p) = (stop.clone(), progress.clone());
        let mut work = tokio::task::spawn_blocking(move || {
            let _guard = MovingGuard;
            let moved = move_all(&old, &new, &s, &p)?;
            let leftovers = commit(&home, &old, &new, true, &rt)?;
            // The old copies go only once the launcher uses the new folder.
            let failed = remove_old(&old, &moved);
            anyhow::Ok(leftovers.or(failed))
        });
        loop {
            tokio::select! {
                r = &mut work => break r?,
                _ = tokio::time::sleep(Duration::from_millis(250)) => {
                    let (done, total) = (progress.done.load(Ordering::Relaxed), progress.total.load(Ordering::Relaxed));
                    if progress.listing.load(Ordering::Relaxed) {
                        task.update(tr!("storage.listing", "Finding files…"), None);
                    } else if total > 0 {
                        let detail = tr!("storage.copying", "Copying {done} of {total}", done = gb(done), total = gb(total));
                        task.update(detail, Some(done as f64 / total as f64));
                    }
                }
            }
        }
    })
    .await?;
    Ok(ChangeResult { info: info().await, leftovers })
}

/// Saves the new games folder and switches to it. `rebase`: paths to the moved Java now point to the new folder.
fn commit(home: &Path, old: &Path, new: &Path, rebase: bool, rt: &tokio::runtime::Handle) -> Result<Option<String>> {
    let file = paths::games_folder_file(home);
    if same_path(new, home) {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    } else {
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::json!({ "folder": display(new) }).to_string())?;
        std::fs::rename(&tmp, &file)?;
    }
    paths::set_games((!same_path(new, home)).then(|| new.to_path_buf()));
    if rebase {
        rebase_java(old, new);
        let java = rt.block_on(settings::get()).java_path;
        if let Some(moved) = rebased(&java, old, new) {
            let _ = rt.block_on(settings::set(serde_json::json!({ "javaPath": moved })));
        }
    }
    Ok(None)
}

/// `value` with `old` swapped for `new` when it points inside `old`'s Java folder.
fn rebased(value: &str, old: &Path, new: &Path) -> Option<String> {
    let rel = Path::new(value).strip_prefix(old.join("java")).ok()?;
    Some(display(&new.join("java").join(rel)))
}

/// Instances and servers set to a Java ResinCraft downloaded now point to its new place.
fn rebase_java(old: &Path, new: &Path) {
    for (top, file) in [("instances", "instance.json"), ("servers", "server.json")] {
        let Ok(rd) = std::fs::read_dir(new.join(top)) else { continue };
        for e in rd.flatten() {
            let path = e.path().join(file);
            let Some(mut v) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
            let Some(moved) = v.get("javaPath").and_then(Value::as_str).and_then(|j| rebased(j, old, new)) else { continue };
            v["javaPath"] = Value::String(moved);
            if let Ok(text) = serde_json::to_string_pretty(&v) {
                let _ = std::fs::write(&path, text);
            }
        }
    }
}

#[derive(Default)]
pub struct Progress {
    pub listing: AtomicBool,
    pub done: AtomicU64,
    pub total: AtomicU64,
}

/// How a move went: which top folders were renamed and which copied (their old copies still to remove).
#[derive(Debug, Default)]
pub struct Moved {
    pub copied: Vec<&'static str>,
    pub renamed: Vec<&'static str>,
}

/// Moves the game folders from `old` into `new`, all or nothing: on an error or when `stop` is set, everything is
/// as it was. Folders already in `new` are merged (its files win, e.g. libraries both have); an instance or server
/// folder that exists in both is refused before anything happens. Old copies of copied folders are left for
/// `remove_old`, after the switch.
pub fn move_all(old: &Path, new: &Path, stop: &AtomicBool, progress: &Progress) -> Result<Moved> {
    if let Some(path) = collision(old, new) {
        bail_tr!("storage.collision", "{path} is already there. Move or rename it first, or start empty there.", path = path);
    }
    let tops: Vec<&'static str> = paths::GAME_FOLDERS.iter().copied().filter(|t| old.join(t).exists()).collect();
    std::fs::create_dir_all(new)?;
    if same_volume(old, new) {
        let mut journal = Vec::new();
        let mut result = Ok(());
        for t in &tops {
            result = merge_rename(&old.join(t), &new.join(t), &mut journal);
            if result.is_err() {
                break;
            }
        }
        match result {
            Ok(()) => return Ok(Moved { renamed: tops, ..Default::default() }),
            Err(e) => {
                undo(&journal);
                // Same drive by its name, but not for the file system (a mounted folder): copy instead.
                if !matches!(e.raw_os_error(), Some(17) | Some(18)) {
                    return Err(failed(e));
                }
            }
        }
    }
    copy_half(old, new, stop, progress)
}

/// Moving onto another drive: copy everything into the staging folder, then put it in place.
fn copy_half(old: &Path, new: &Path, stop: &AtomicBool, progress: &Progress) -> Result<Moved> {
    let tops: Vec<&'static str> = paths::GAME_FOLDERS.iter().copied().filter(|t| old.join(t).exists()).collect();
    let staging = new.join(STAGING);
    let _ = std::fs::remove_dir_all(&staging);
    progress.listing.store(true, Ordering::Relaxed);
    let mut jobs = Vec::new();
    for t in &tops {
        let listed = fsutil::list_copy(&old.join(t), &staging.join(t), &[]).map_err(|e| {
            let _ = std::fs::remove_dir_all(&staging);
            failed(e)
        })?;
        // Files the new folder already has (shared libraries…) stay as they are.
        jobs.extend(listed.into_iter().filter(|(_, to, _)| to.strip_prefix(&staging).map(|rel| !new.join(rel).exists()).unwrap_or(true)));
    }
    progress.total.store(jobs.iter().map(|j| j.2).sum::<u64>().max(1), Ordering::Relaxed);
    progress.listing.store(false, Ordering::Relaxed);
    let failures = fsutil::copy_many(&jobs, stop, &progress.done);
    if stop.load(Ordering::SeqCst) || !failures.is_empty() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(match failures.into_iter().next() {
            Some((_, e)) if fsutil::is_disk_full(&e) => err_tr!("storage.diskFull", "The new drive is full. Nothing was moved."),
            Some((i, e)) => failed(io::Error::new(e.kind(), format!("{}: {e}", display(&jobs[i].0)))),
            None => events::cancelled_error(),
        });
    }
    // Every file arrived: put them in place (renames on the new drive, undone if one fails).
    let mut journal = Vec::new();
    for t in &tops {
        if staging.join(t).exists() {
            if let Err(e) = merge_rename(&staging.join(t), &new.join(t), &mut journal) {
                undo(&journal);
                let _ = std::fs::remove_dir_all(&staging);
                return Err(failed(e));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(Moved { copied: tops, ..Default::default() })
}

fn failed(e: io::Error) -> anyhow::Error {
    err_tr!("storage.failed", "Couldn't move your instances ({reason}). Everything stays where it was.", reason = e.to_string())
}

/// Moves `src` to `dst` by renaming; when `dst` is already a folder, merges into it (a file it already has is
/// kept). Each rename is written to `journal` so `undo` can put it back.
fn merge_rename(src: &Path, dst: &Path, journal: &mut Vec<(PathBuf, PathBuf)>) -> io::Result<()> {
    match std::fs::symlink_metadata(dst) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(src, dst)?;
            journal.push((src.to_path_buf(), dst.to_path_buf()));
            Ok(())
        }
        Err(e) => Err(e),
        Ok(m) if m.is_dir() && src.is_dir() => {
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                merge_rename(&entry.path(), &dst.join(entry.file_name()), journal)?;
            }
            Ok(())
        }
        Ok(m) if !m.is_dir() && src.is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} is in the way", display(dst)))),
    }
}

fn undo(journal: &[(PathBuf, PathBuf)]) {
    for (src, dst) in journal.iter().rev() {
        let _ = std::fs::rename(dst, src);
    }
}

/// Removes what is left of the moved folders in `old`: copied folders, and files a rename-merge kept from the
/// new side. Returns `old` if something couldn't be removed (a file in use).
pub fn remove_old(old: &Path, moved: &Moved) -> Option<String> {
    let mut left = false;
    for t in moved.copied.iter().chain(&moved.renamed) {
        let dir = old.join(t);
        if dir.exists() && std::fs::remove_dir_all(&dir).is_err() {
            left = true;
        }
    }
    left.then(|| display(old))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh `<tmp>/resin-storage-<name>-<pid>` folder.
    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resin-storage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A games folder with two instances (one with a world), a server and shared libraries.
    fn games(dir: &Path) {
        write(&dir.join("instances/cozy/instance.json"), r#"{"id":"cozy"}"#);
        write(&dir.join("instances/cozy/minecraft/saves/World/level.dat"), "level");
        write(&dir.join("instances/cozy/minecraft/mods/sodium.jar"), "jar");
        write(&dir.join("instances/vanilla/instance.json"), r#"{"id":"vanilla"}"#);
        write(&dir.join("servers/smp/server.json"), r#"{"id":"smp"}"#);
        write(&dir.join("libraries/org/lwjgl/lwjgl.jar"), "lwjgl");
        write(&dir.join("settings.json"), "{}");
    }

    /// Every file under `dir` as (relative path, contents), sorted.
    fn tree(dir: &Path) -> Vec<(String, String)> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                if e.path().is_dir() {
                    walk(base, &e.path(), out);
                } else {
                    let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                    out.push((rel, std::fs::read_to_string(e.path()).unwrap_or_default()));
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    fn rules(current: &Path, home: &Path) -> Rules {
        Rules { current: current.to_path_buf(), home: home.to_path_buf(), off_limits: Vec::new() }
    }

    fn key_of(t: Option<Text>) -> String {
        t.map(|t| t.key).unwrap_or_default()
    }

    #[test]
    fn typed_folders_are_cleaned_up() {
        if cfg!(windows) {
            assert_eq!(normalize("  \"D:\\Games\\ResinCraft\\\" "), PathBuf::from("D:\\Games\\ResinCraft"));
            assert_eq!(normalize("D:/Games/ResinCraft/"), PathBuf::from("D:\\Games\\ResinCraft"));
            assert_eq!(normalize("F:"), PathBuf::from("F:\\"));
            assert!(same_path(Path::new("d:\\games"), Path::new("D:\\Games\\")));
            assert!(inside(Path::new("D:\\Games\\x"), Path::new("d:\\games")));
            assert!(!inside(Path::new("D:\\Gamesx"), Path::new("D:\\Games")));
        }
    }

    #[test]
    fn folders_that_cannot_work_are_refused() {
        let dir = sandbox("checks");
        let (current, home) = (dir.join("current"), dir.join("home"));
        games(&current);
        std::fs::create_dir_all(&home).unwrap();
        let r = rules(&current, &home);
        let check = |p: &Path| key_of(check_folder(p, &r, |_| None));

        assert_eq!(check(Path::new("relative\\folder")), "storage.notAbsolute");
        assert_eq!(check(dir.ancestors().last().unwrap()), "storage.driveRoot");
        assert_eq!(check(&dir.join("x".repeat(MAX_PATH_LEN))), "storage.tooLong");
        assert_eq!(check(&current), "storage.same");
        assert_eq!(check(&current.join("instances").join("elsewhere")), "storage.insideGames");
        assert_eq!(check(&current.join("libraries")), "storage.insideGames");
        assert_eq!(check(&home.join("servers").join("x")), "storage.insideGames");
        // Inside an instance or server of another launcher, or someone's .minecraft.
        write(&dir.join("other/inst/instance.json"), "{}");
        assert_eq!(check(&dir.join("other/inst/ResinCraft")), "storage.insideInstance");
        assert_eq!(check(&dir.join(".minecraft/ResinCraft")), "storage.insideInstance");
        write(&dir.join("a-file"), "x");
        assert_eq!(check(&dir.join("a-file")), "storage.notFolder");
        // A drive problem comes first.
        assert_eq!(key_of(check_folder(&dir.join("ok"), &r, |_| Some(tr!("test.drive", "A drive problem")))), "test.drive");
        // Off-limits folders.
        let off = Rules { off_limits: vec![(dir.join("Program Files"), Off::System), (dir.join("OneDrive"), Off::OneDrive)], ..rules(&current, &home) };
        assert_eq!(key_of(check_folder(&dir.join("program files/ResinCraft"), &off, |_| None)), if cfg!(windows) { "storage.system" } else { "" });
        assert_eq!(key_of(check_folder(&dir.join("OneDrive/Games"), &off, |_| None)), "storage.oneDrive");

        // A good folder passes, and checking it leaves nothing behind.
        let good = dir.join("new").join("ResinCraft");
        assert_eq!(check(&good), "");
        assert!(!dir.join("new").exists(), "the check removes the folders it made");
        // The launcher folder (going back to the default) is fine too.
        assert_eq!(check(&home), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn read_only_folders_are_refused() {
        let dir = sandbox("readonly");
        // A folder name Windows can't make.
        let t = check_folder(&dir.join("bad<name>"), &rules(&dir.join("cur"), &dir.join("home")), |_| None);
        assert_eq!(key_of(t), "storage.notWritable");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn moving_on_the_same_drive_renames_everything() {
        let dir = sandbox("rename");
        let (old, new) = (dir.join("old"), dir.join("new").join("ResinCraft"));
        games(&old);
        let before = tree(&old);
        let moved = move_all(&old, &new, &AtomicBool::new(false), &Progress::default()).unwrap();
        assert_eq!(moved.renamed, vec!["instances", "servers", "libraries"]);
        assert!(remove_old(&old, &moved).is_none());
        // Everything but the launcher's own files arrived; those stay.
        let expected: Vec<_> = before.into_iter().filter(|(p, _)| p != "settings.json").collect();
        assert_eq!(tree(&new), expected);
        assert_eq!(tree(&old), vec![("settings.json".to_string(), "{}".to_string())]);

        // And back again, into a folder that has its own settings.
        let moved = move_all(&new, &old, &AtomicBool::new(false), &Progress::default()).unwrap();
        remove_old(&new, &moved);
        assert_eq!(tree(&old).len(), 7);
        assert!(tree(&new).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merging_keeps_shared_files_and_refuses_twin_instances() {
        let dir = sandbox("merge");
        let (old, new) = (dir.join("old"), dir.join("new"));
        games(&old);
        write(&new.join("libraries/org/lwjgl/lwjgl.jar"), "already here");
        write(&new.join("instances/started-empty/instance.json"), "{}");
        let moved = move_all(&old, &new, &AtomicBool::new(false), &Progress::default()).unwrap();
        remove_old(&old, &moved);
        assert_eq!(std::fs::read_to_string(new.join("libraries/org/lwjgl/lwjgl.jar")).unwrap(), "already here");
        assert!(new.join("instances/started-empty/instance.json").is_file());
        assert!(new.join("instances/cozy/minecraft/saves/World/level.dat").is_file());

        // An instance folder with the same name on both sides: nothing happens.
        games(&old);
        let before = (tree(&old), tree(&new));
        let err = move_all(&old, &new, &AtomicBool::new(false), &Progress::default()).unwrap_err();
        assert_eq!(crate::core::text::text_of(&err).map(|t| t.key.as_str()), Some("storage.collision"));
        assert_eq!((tree(&old), tree(&new)), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_rename_puts_everything_back() {
        let dir = sandbox("rename-fail");
        let (old, new) = (dir.join("old"), dir.join("new"));
        games(&old);
        // A file where the libraries folder must go: instances and servers move first, then this fails.
        write(&new.join("libraries"), "in the way");
        let before = (tree(&old), tree(&new));
        let err = move_all(&old, &new, &AtomicBool::new(false), &Progress::default()).unwrap_err();
        assert_eq!(crate::core::text::text_of(&err).map(|t| t.key.as_str()), Some("storage.failed"));
        assert_eq!((tree(&old), tree(&new)), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The copy path (another drive), forced by calling its parts the way move_all does across drives.
    fn copy_move(old: &Path, new: &Path, stop: &AtomicBool) -> Result<Moved> {
        let _ = std::fs::create_dir_all(new);
        let p = Progress::default();
        copy_half(old, new, stop, &p)
    }

    #[test]
    fn copying_to_another_drive_moves_every_file() {
        let dir = sandbox("copy");
        let (old, new) = (dir.join("old"), dir.join("new"));
        games(&old);
        let expected: Vec<_> = tree(&old).into_iter().filter(|(p, _)| p != "settings.json").collect();
        let moved = copy_move(&old, &new, &AtomicBool::new(false)).unwrap();
        assert_eq!(moved.copied, vec!["instances", "servers", "libraries"]);
        // Both copies exist until the switch; then the old ones go.
        assert_eq!(tree(&new), expected);
        assert!(old.join("instances/cozy").is_dir());
        assert!(remove_old(&old, &moved).is_none());
        assert_eq!(tree(&old).len(), 1);
        assert!(!new.join(STAGING).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_copy_leaves_everything_as_it_was() {
        let dir = sandbox("copy-cancel");
        let (old, new) = (dir.join("old"), dir.join("new"));
        games(&old);
        write(&new.join("notes.txt"), "the player's own file");
        let before = (tree(&old), tree(&new));
        let err = copy_move(&old, &new, &AtomicBool::new(true)).unwrap_err();
        assert!(events::is_cancelled(&err));
        assert_eq!((tree(&old), tree(&new)), before);
        assert!(!new.join(STAGING).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn a_failed_copy_leaves_everything_as_it_was() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = sandbox("copy-fail");
        let (old, new) = (dir.join("old"), dir.join("new"));
        games(&old);
        let before = (tree(&old), tree(&new));
        // A file another program holds open without sharing can't be read.
        let lock = std::fs::OpenOptions::new().read(true).share_mode(0).open(old.join("instances/cozy/minecraft/mods/sodium.jar")).unwrap();
        let err = copy_move(&old, &new, &AtomicBool::new(false)).unwrap_err();
        drop(lock);
        assert_eq!(crate::core::text::text_of(&err).map(|t| t.key.as_str()), Some("storage.failed"));
        assert!(err.to_string().contains("sodium.jar"), "{err}");
        assert_eq!((tree(&old), tree(&new)), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn java_paths_follow_the_move() {
        let dir = sandbox("java");
        let (old, new) = (dir.join("old"), dir.join("new"));
        let java = old.join("java").join("java-21").join("bin").join("javaw.exe");
        write(&new.join("instances/a/instance.json"), &serde_json::json!({ "id": "a", "javaPath": display(&java) }).to_string());
        write(&new.join("instances/b/instance.json"), &serde_json::json!({ "id": "b", "javaPath": "C:\\Java\\bin\\javaw.exe" }).to_string());
        rebase_java(&old, &new);
        let read = |id: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(new.join("instances").join(id).join("instance.json")).unwrap()).unwrap() };
        assert_eq!(read("a")["javaPath"], display(&new.join("java").join("java-21").join("bin").join("javaw.exe")));
        assert_eq!(read("b")["javaPath"], "C:\\Java\\bin\\javaw.exe");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
