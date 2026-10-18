//! The built-in file manager (`files:*`): browse, edit and organise the files of an instance's game folder or a
//! hosted server's folder. Every path the UI sends is relative to one of those roots and goes through
//! [`safe_join_within`], so nothing outside the root is reachable, not even through a symlink or junction.
//! Deleting always goes to the Recycle Bin.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::fsutil::safe_join_within;
use crate::core::router::{done, json};
use crate::core::{paths, shell, Args};
use crate::{bail_tr, err_tr};

/// Files larger than this open as "too large to edit".
pub const MAX_EDIT_BYTES: u64 = 2 * 1024 * 1024;
const BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RootKind {
    Instance,
    Server,
}

/// Whose files: `{ kind: "instance", id }` is the instance's game folder, `{ kind: "server", id }` the server folder.
#[derive(Debug, Clone, Deserialize)]
pub struct FileRoot {
    pub kind: RootKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    /// Relative to the root, `/`-separated.
    pub path: String,
    pub dir: bool,
    /// Bytes (0 for folders).
    pub size: u64,
    /// Last modified, in ms since the epoch.
    pub modified: i64,
    /// How many entries a folder holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<u32>,
    /// A symlink or junction.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub link: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    #[serde(flatten)]
    pub entry: FileEntry,
    /// "text" (with `text`), "binary" or "tooLarge".
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The file starts with a UTF-8 byte order mark (kept on save).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bom: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WriteOptions {
    /// Make a new file; fails if the name is taken.
    pub create: bool,
    /// The `modified` time the editor loaded; saving fails if the file changed on disk since.
    pub expect_modified: Option<i64>,
    pub bom: bool,
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    let root: FileRoot = args.get(0)?;
    let base = root_dir(&root).await?;
    match channel {
        "files:list" => json(list(&base, &args.get::<Option<String>>(1)?.unwrap_or_default()).await?),
        "files:readText" => json(read_text(&base, &args.get::<String>(1)?).await?),
        "files:writeText" => {
            let Value::String(text) = args.raw(2) else { bail_tr!("files.notText", "The file contents must be text") };
            let opts: Option<WriteOptions> = args.get(3)?;
            json(write_text(&base, &args.get::<String>(1)?, &text, opts.unwrap_or_default()).await?)
        }
        "files:mkdir" => json(mkdir(&base, &args.get::<String>(1)?).await?),
        "files:rename" => json(rename(&base, &args.get::<String>(1)?, &args.get::<String>(2)?).await?),
        "files:move" => json(move_to(&base, &args.get::<Vec<String>>(1)?, &args.get::<String>(2)?).await?),
        "files:copy" => json(copy_to(&base, &args.get::<Vec<String>>(1)?, &args.get::<String>(2)?).await?),
        "files:import" => json(import(&base, &args.get::<String>(1)?, &args.get::<Vec<String>>(2)?).await?),
        "files:trash" => {
            for rel in args.get::<Vec<String>>(1)? {
                let path = entry_path(&base, &rel)?;
                shell::trash(&path).await?;
            }
            done()
        }
        "files:reveal" => {
            let rel: String = args.get(1)?;
            let path = resolve(&base, &rel)?;
            // A folder opens; a file is shown selected in its folder.
            if path.is_dir() {
                shell::open_path(&path)?;
            } else {
                shell::reveal(&path)?;
            }
            done()
        }
        _ => anyhow::bail!("Unknown request: {channel}"),
    }
}

/// The folder a root names, after checking the instance or server exists. Created if missing.
pub async fn root_dir(root: &FileRoot) -> Result<PathBuf> {
    let dir = match root.kind {
        RootKind::Instance => {
            crate::player::files::instance_root(&root.id).await?;
            paths::game_dir(&root.id)
        }
        RootKind::Server => {
            crate::server::store::get(&root.id).await?;
            crate::server::store::dir(&root.id)?
        }
    };
    tokio::fs::create_dir_all(&dir).await?;
    Ok(dir)
}

/// `rel` inside `base`, refusing anything that escapes it.
fn resolve(base: &Path, rel: &str) -> Result<PathBuf> {
    safe_join_within(base, rel)
}

/// Like [`resolve`], for an operation on an entry itself (rename, move, delete): never the root, and it must exist.
fn entry_path(base: &Path, rel: &str) -> Result<PathBuf> {
    let path = resolve(base, rel)?;
    if path == base {
        bail_tr!("files.isRoot", "The top folder can't be changed");
    }
    if std::fs::symlink_metadata(&path).is_err() {
        bail_tr!("files.notFound", "{name} no longer exists", name = display_name(&path));
    }
    Ok(path)
}

/// A folder inside `base` that must already exist.
fn dir_path(base: &Path, rel: &str) -> Result<PathBuf> {
    let path = resolve(base, rel)?;
    if !path.is_dir() {
        bail_tr!("files.notFolder", "{name} isn't a folder", name = display_name(&path));
    }
    Ok(path)
}

fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Path relative to `base`, with `/` separators.
fn rel_of(base: &Path, path: &Path) -> String {
    path.strip_prefix(base).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")).unwrap_or_default()
}

fn millis(t: std::io::Result<SystemTime>) -> i64 {
    t.ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Whether `name` is usable as a file or folder name on every platform the launcher runs on.
pub fn valid_name(name: &str) -> bool {
    const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let device = RESERVED.contains(&stem.as_str())
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit() && stem.as_bytes()[3] != b'0');
    !name.is_empty()
        && name.len() <= 255
        && !name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !name.ends_with(['.', ' '])
        && !device
}

fn check_name(name: &str) -> Result<()> {
    if !valid_name(name) {
        bail_tr!("files.badName", "\"{name}\" can't be used as a name", name = name.to_string());
    }
    Ok(())
}

fn entry_for(base: &Path, path: &Path) -> Result<FileEntry> {
    let link = std::fs::symlink_metadata(path).map_err(|_| err_tr!("files.notFound", "{name} no longer exists", name = display_name(path)))?;
    let meta = std::fs::metadata(path).unwrap_or_else(|_| link.clone());
    let dir = meta.is_dir();
    let items = if dir { std::fs::read_dir(path).ok().map(|rd| rd.take(100_000).count() as u32) } else { None };
    Ok(FileEntry {
        name: display_name(path),
        path: rel_of(base, path),
        dir,
        size: if dir { 0 } else { meta.len() },
        modified: millis(meta.modified()),
        items,
        link: link.file_type().is_symlink() || is_junction(&link),
    })
}

#[cfg(windows)]
fn is_junction(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_junction(_: &std::fs::Metadata) -> bool {
    false
}

/// The entries of a folder (unsorted; the UI sorts).
pub async fn list(base: &Path, rel: &str) -> Result<Vec<FileEntry>> {
    let dir = dir_path(base, rel)?;
    let base = base.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir)?.flatten() {
            if let Ok(entry) = entry_for(&base, &e.path()) {
                out.push(entry);
            }
        }
        Ok(out)
    })
    .await?
}

/// A file for the editor: its text when it's UTF-8 text up to [`MAX_EDIT_BYTES`], otherwise only its details.
pub async fn read_text(base: &Path, rel: &str) -> Result<TextFile> {
    let path = entry_path(base, rel)?;
    let entry = entry_for(base, &path)?;
    if entry.dir {
        bail_tr!("files.isFolder", "{name} is a folder", name = entry.name);
    }
    if entry.size > MAX_EDIT_BYTES {
        return Ok(TextFile { entry, kind: "tooLarge", text: None, bom: false });
    }
    let bytes = tokio::fs::read(&path).await?;
    let bom = bytes.starts_with(BOM);
    let body = if bom { &bytes[BOM.len()..] } else { &bytes[..] };
    let text = (!body[..body.len().min(8000)].contains(&0)).then(|| std::str::from_utf8(body).ok()).flatten();
    Ok(match text {
        Some(t) => TextFile { entry, kind: "text", text: Some(t.to_string()), bom },
        None => TextFile { entry, kind: "binary", text: None, bom: false },
    })
}

/// Saves a text file atomically (temp file + rename).
pub async fn write_text(base: &Path, rel: &str, text: &str, opts: WriteOptions) -> Result<FileEntry> {
    let path = resolve(base, rel)?;
    if path == base {
        bail_tr!("files.isRoot", "The top folder can't be changed");
    }
    let name = display_name(&path);
    check_name(&name)?;
    if opts.bom as u64 + text.len() as u64 > MAX_EDIT_BYTES {
        bail_tr!("files.tooLarge", "{name} is too large to edit here", name = name);
    }
    match tokio::fs::metadata(&path).await {
        Ok(_) if opts.create => bail_tr!("files.exists", "There's already something called {name} here", name = name),
        Ok(m) if m.is_dir() => bail_tr!("files.isFolder", "{name} is a folder", name = name),
        Ok(m) => {
            if opts.expect_modified.is_some_and(|t| t != millis(m.modified())) {
                bail_tr!("files.changedOnDisk", "{name} was changed by another program after you opened it", name = name);
            }
        }
        Err(_) if !opts.create => bail_tr!("files.notFound", "{name} no longer exists", name = name),
        Err(_) => {}
    }
    if !path.parent().is_some_and(|p| p.is_dir()) {
        bail_tr!("files.notFolder", "{name} isn't a folder", name = path.parent().map(display_name).unwrap_or_default());
    }
    let mut data = Vec::with_capacity(text.len() + 3);
    if opts.bom {
        data.extend_from_slice(BOM);
    }
    data.extend_from_slice(text.as_bytes());
    crate::player::files::write_atomic(&path, &data).await?;
    entry_for(base, &path)
}

/// Makes a new folder `rel` (its parent must exist).
pub async fn mkdir(base: &Path, rel: &str) -> Result<FileEntry> {
    let path = resolve(base, rel)?;
    let name = display_name(&path);
    check_name(&name)?;
    if std::fs::symlink_metadata(&path).is_ok() {
        bail_tr!("files.exists", "There's already something called {name} here", name = name);
    }
    tokio::fs::create_dir(&path).await?;
    entry_for(base, &path)
}

/// Renames an entry in place.
pub async fn rename(base: &Path, rel: &str, new_name: &str) -> Result<FileEntry> {
    check_name(new_name)?;
    let path = entry_path(base, rel)?;
    let parent = path.parent().unwrap_or(base);
    let target = resolve(base, &format!("{}/{new_name}", rel_of(base, parent)))?;
    let case_only = display_name(&path).to_lowercase() == new_name.to_lowercase();
    if target == path {
        return entry_for(base, &path);
    }
    if !case_only && std::fs::symlink_metadata(&target).is_ok() {
        bail_tr!("files.exists", "There's already something called {name} here", name = new_name.to_string());
    }
    tokio::fs::rename(&path, &target).await?;
    entry_for(base, &target)
}

/// Refuses putting a folder inside itself.
fn check_not_inside(src: &Path, dest_dir: &Path) -> Result<()> {
    let (s, d) = (std::fs::canonicalize(src)?, std::fs::canonicalize(dest_dir)?);
    if d.starts_with(&s) {
        bail_tr!("files.intoItself", "{name} can't go inside itself", name = display_name(src));
    }
    Ok(())
}

/// Moves entries into the folder `dest`. Names already taken there are refused, never overwritten.
pub async fn move_to(base: &Path, rels: &[String], dest: &str) -> Result<Vec<String>> {
    let dest_dir = dir_path(base, dest)?;
    let mut moved = Vec::new();
    for rel in rels {
        let src = entry_path(base, rel)?;
        if src.parent() == Some(dest_dir.as_path()) {
            continue;
        }
        check_not_inside(&src, &dest_dir)?;
        let name = display_name(&src);
        let target = resolve(base, &format!("{}/{name}", rel_of(base, &dest_dir)))?;
        if std::fs::symlink_metadata(&target).is_ok() {
            bail_tr!("files.exists", "There's already something called {name} here", name = name);
        }
        tokio::fs::rename(&src, &target).await?;
        moved.push(rel_of(base, &target));
    }
    Ok(moved)
}

/// `name`, or `name (2)`, `name (3)`… whichever is free in `dir`.
async fn free_name(dir: &Path, name: &str, is_dir: bool) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && !is_dir => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    crate::player::files::unique_name(dir, stem, ext).await
}

/// Copies a file or folder tree. Links inside it are skipped, so a copy never pulls in files from outside.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() || is_junction(&meta) {
        return Ok(());
    }
    if meta.is_dir() {
        std::fs::create_dir(to)?;
        for e in std::fs::read_dir(from)?.flatten() {
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
    } else {
        std::fs::copy(from, to)?;
    }
    Ok(())
}

async fn copy_into(base: &Path, src: &Path, dest_dir: &Path) -> Result<String> {
    if src.is_dir() {
        check_not_inside(src, dest_dir)?;
    }
    let name = free_name(dest_dir, &display_name(src), src.is_dir()).await;
    let target = resolve(base, &format!("{}/{name}", rel_of(base, dest_dir)))?;
    let (from, to) = (src.to_path_buf(), target.clone());
    tokio::task::spawn_blocking(move || copy_tree(&from, &to)).await??;
    Ok(rel_of(base, &target))
}

/// Copies entries into the folder `dest`; a taken name gets " (2)" etc. Returns the new paths.
pub async fn copy_to(base: &Path, rels: &[String], dest: &str) -> Result<Vec<String>> {
    let dest_dir = dir_path(base, dest)?;
    let mut out = Vec::new();
    for rel in rels {
        let src = entry_path(base, rel)?;
        out.push(copy_into(base, &src, &dest_dir).await?);
    }
    Ok(out)
}

/// Copies files or folders from elsewhere on the computer (dropped onto the window) into the folder `dest`.
pub async fn import(base: &Path, dest: &str, sources: &[String]) -> Result<Vec<String>> {
    let dest_dir = dir_path(base, dest)?;
    let mut out = Vec::new();
    for source in sources {
        let src = PathBuf::from(source);
        if !src.is_absolute() || std::fs::symlink_metadata(&src).is_err() {
            bail_tr!("files.sourceMissing", "Couldn't find {path}", path = source.to_string());
        }
        out.push(copy_into(base, &src, &dest_dir).await?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resin-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("config").join("a.json"), "{}").unwrap();
        std::fs::write(dir.join("server.properties"), "motd=hi\n").unwrap();
        dir
    }

    #[test]
    fn names() {
        for ok in ["server.properties", ".minecraft", "a b", "Datei ü.txt", "CONFIG", "com0", "console.log"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", ".", "..", "a/b", "a\\b", "a:b", "x?", "trailing.", "trailing ", "CON", "nul.txt", "com1", "LPT9.log", "a\u{7}"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn edit_cycle() {
        let base = sandbox("cycle");
        let mut names: Vec<String> = list(&base, "").await.unwrap().into_iter().map(|e| e.name).collect();
        names.sort();
        assert_eq!(names, ["config", "server.properties"]);

        let file = read_text(&base, "server.properties").await.unwrap();
        assert_eq!((file.kind, file.text.as_deref()), ("text", Some("motd=hi\n")));
        let opts = WriteOptions { expect_modified: Some(file.entry.modified), ..Default::default() };
        write_text(&base, "server.properties", "motd=hello\n", opts).await.unwrap();
        assert_eq!(std::fs::read_to_string(base.join("server.properties")).unwrap(), "motd=hello\n");
        let stale = WriteOptions { expect_modified: Some(1), ..Default::default() };
        assert!(write_text(&base, "server.properties", "x", stale).await.is_err());

        let create = WriteOptions { create: true, ..Default::default() };
        write_text(&base, "config/new.toml", "", create.clone()).await.unwrap();
        assert!(write_text(&base, "config/new.toml", "", create).await.is_err(), "create never overwrites");

        mkdir(&base, "config/sub").await.unwrap();
        move_to(&base, &["config/new.toml".into()], "config/sub").await.unwrap();
        assert!(base.join("config/sub/new.toml").exists());
        assert!(move_to(&base, &["config".into()], "config/sub").await.is_err(), "not into itself");

        let copies = copy_to(&base, &["config/a.json".into()], "config").await.unwrap();
        assert_eq!(copies, ["config/a (2).json"]);
        rename(&base, "config/a (2).json", "b.json").await.unwrap();
        assert!(base.join("config/b.json").exists());
        assert!(rename(&base, "config/b.json", "a.json").await.is_err(), "name taken");
        assert!(rename(&base, "config/b.json", "../b.json").await.is_err());
        assert!(rename(&base, "", "x").await.is_err(), "not the root");

        std::fs::write(base.join("level.dat"), [0x1f, 0x8b, 0, 0, 1]).unwrap();
        assert_eq!(read_text(&base, "level.dat").await.unwrap().kind, "binary");
    }

    #[tokio::test]
    async fn refuses_escapes() {
        let base = sandbox("escape");
        let outside = base.parent().unwrap().join(format!("resin-files-outside-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        let rel_out = format!("../{}/secret.txt", display_name(&outside));
        assert!(read_text(&base, &rel_out).await.is_err());
        assert!(list(&base, "..").await.is_err());
        assert!(write_text(&base, "../evil.txt", "x", WriteOptions { create: true, ..Default::default() }).await.is_err());
        assert!(mkdir(&base, "config/../../evil").await.is_err());
        assert!(move_to(&base, &["server.properties".into()], "..").await.is_err());
        assert!(copy_to(&base, &[rel_out.clone()], "config").await.is_err());
        assert!(entry_path(&base, "..\\..\\Windows").is_err());
        assert!(outside.join("secret.txt").exists());
        assert!(!base.parent().unwrap().join("evil.txt").exists());
    }
}
