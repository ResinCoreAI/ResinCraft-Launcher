//! Files Windows asks ResinCraft to open (double-click, "Open with"): themes (`.rctheme`), backups (`.resincraft`)
//! and modpacks (`.mrpack`, CurseForge `.zip`). The first start keeps the file until the UI is up and asks
//! (`app:pendingOpen`); opening one while ResinCraft runs comes through the single-instance plugin, and the UI hears
//! `openFile`. tauri.conf.json registers `.rctheme` and `.resincraft` as ResinCraft's own.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Result};
use serde_json::json;

use crate::core::events;

/// The kinds of file ResinCraft opens.
const KINDS: [&str; 4] = ["rctheme", "resincraft", "mrpack", "zip"];

/// A theme file is JSON with its picture inside: 8 MB as base64 plus the rest.
const MAX_THEME_BYTES: u64 = 16 * 1024 * 1024;

static PENDING: Mutex<Option<String>> = Mutex::new(None);
static PENDING_LINK: Mutex<Option<String>> = Mutex::new(None);

/// ResinCraft's own links (tauri.conf.json `plugins.deep-link`): `resincraft://join/<address>` and
/// `resincraft://friend/<code>`. Windows starts ResinCraft with the link as an argument; a running copy gets it
/// through the single-instance plugin. The UI reads what the link means (links.ts).
const SCHEME: &str = "resincraft://";
/// Longer than any address or invite code; anything bigger isn't one of ours.
const MAX_LINK: usize = 512;

fn link_from_args(args: &[String]) -> Option<String> {
    args.iter().skip(1).find(|a| a.len() <= MAX_LINK && a.to_ascii_lowercase().starts_with(SCHEME)).cloned()
}

/// `app:pendingLink`: the link ResinCraft was started with, once.
pub fn take_link() -> Option<String> {
    PENDING_LINK.lock().unwrap().take()
}

/// The file among a start's arguments (the first one is the program itself).
fn from_args(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .find(|a| {
            let p = Path::new(a);
            p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| KINDS.iter().any(|k| e.eq_ignore_ascii_case(k)))
        })
        .cloned()
}

/// At start: remembers a file ResinCraft was started with, for the UI.
pub fn at_start() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(path) = from_args(&args) {
        *PENDING.lock().unwrap() = Some(path);
    }
    if let Some(link) = link_from_args(&args) {
        *PENDING_LINK.lock().unwrap() = Some(link);
    }
}

/// `app:pendingOpen`: the file ResinCraft was started with, once.
pub fn take() -> Option<String> {
    PENDING.lock().unwrap().take()
}

/// A second start with a file: the running ResinCraft opens it.
pub fn opened(args: Vec<String>) {
    if let Some(path) = from_args(&args) {
        events::emit("openFile", json!({ "path": path }));
    }
    if let Some(url) = link_from_args(&args) {
        events::emit("openLink", json!({ "url": url }));
    }
}

/// `app:readThemeFile`: a .rctheme file's text (the UI checks what's in it).
pub async fn read_theme(path: &str) -> Result<String> {
    let p = Path::new(path);
    if !p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("rctheme")) {
        bail!("That isn't a ResinCraft theme file.");
    }
    if tokio::fs::metadata(p).await?.len() > MAX_THEME_BYTES {
        bail!("That theme file is too big.");
    }
    Ok(tokio::fs::read_to_string(p).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_file_among_the_arguments() {
        let dir = std::env::temp_dir().join(format!("rc-openfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let theme = dir.join("Sakura.RCTHEME");
        std::fs::write(&theme, "{}").unwrap();
        let theme = theme.to_string_lossy().to_string();
        let exe = "C:\\ResinCraft\\resincraft.exe".to_string();
        assert_eq!(from_args(&[exe.clone(), "--flag".into(), theme.clone()]), Some(theme.clone()));
        // The program itself, unknown kinds and missing files are skipped.
        assert_eq!(from_args(&[theme.clone()]), None);
        assert_eq!(from_args(&[exe.clone(), dir.join("notes.txt").to_string_lossy().to_string()]), None);
        assert_eq!(from_args(&[exe, dir.join("gone.rctheme").to_string_lossy().to_string()]), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
