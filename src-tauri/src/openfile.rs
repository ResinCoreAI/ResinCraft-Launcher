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
    if let Some(path) = from_args(&std::env::args().collect::<Vec<_>>()) {
        *PENDING.lock().unwrap() = Some(path);
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
