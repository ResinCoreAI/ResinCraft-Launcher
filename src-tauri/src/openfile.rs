//! Files Windows asks ResinCraft to open (double-click, "Open with"): themes (`.rctheme`), backups (`.resincraft`),
//! instances sent in chat (`.resinpack`) and modpacks (`.mrpack`, CurseForge `.zip`). The first start keeps the file until the UI is up and asks
//! (`app:pendingOpen`); opening one while ResinCraft runs comes through the single-instance plugin, and the UI hears
//! `openFile`. tauri.conf.json registers `.rctheme`, `.resincraft` and `.resinpack` as ResinCraft's own.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Result};
use serde_json::json;

use crate::core::events;

/// The kinds of file ResinCraft opens.
const KINDS: [&str; 5] = ["rctheme", "resincraft", "resinpack", "mrpack", "zip"];

/// A theme file is JSON with its picture inside: 8 MB as base64 plus the rest.
const MAX_THEME_BYTES: u64 = 16 * 1024 * 1024;
/// The JSON's name inside a PRAR theme file.
const THEME_ENTRY: &str = "theme.json";

static PENDING: Mutex<Option<String>> = Mutex::new(None);
static PENDING_LINK: Mutex<Option<String>> = Mutex::new(None);

/// ResinCraft's own links (tauri.conf.json `plugins.deep-link`): `resincraft://join/<address>` and
/// `resincraft://friend/<code>`. Windows starts ResinCraft with the link as an argument; a running copy gets it
/// through the single-instance plugin. The UI reads what the link means (links.ts).
const SCHEME: &str = "resincraft://";
/// Longer than any address or invite code; anything bigger isn't one of ours.
const MAX_LINK: usize = 512;

/// The one kind of link that is never taken from a real URL: it starts a game, so only a desktop shortcut's
/// `--play=` argument makes it (see `play_from_args`).
const PLAY_LINK: &str = "resincraft://play";

fn link_from_args(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .find(|a| {
            let lower = a.to_ascii_lowercase();
            a.len() <= MAX_LINK && lower.starts_with(SCHEME) && !is_play_link(&lower)
        })
        .cloned()
        .or_else(|| play_from_args(args))
}

/// Whether a link asks to start a game, however it is spelled. The UI skips empty path parts, so
/// `resincraft:///play/x` and `resincraft://play//x` mean the same as `resincraft://play/x`.
fn is_play_link(lower: &str) -> bool {
    let rest = lower.strip_prefix(SCHEME).unwrap_or(lower);
    let path = rest.split(['?', '#']).next().unwrap_or("");
    path.split(['/', '\\']).map(str::trim).find(|s| !s.is_empty()) == Some("play")
}

/// `--play=<instance id>` (a desktop shortcut, core/shortcut.rs), as the internal link the UI starts the instance
/// for. A web page can open `resincraft://` links but can't add an argument, so it can't reach this.
fn play_from_args(args: &[String]) -> Option<String> {
    let id = args.iter().skip(1).find_map(|a| a.strip_prefix(crate::core::shortcut::PLAY_ARG))?;
    crate::core::shortcut::plausible_id(id).then(|| format!("{PLAY_LINK}/{id}"))
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
/// `fresh`: the window is being made again (it had closed to save memory), so there's no page to tell yet; the new
/// page asks for it once it loads, as at a start.
pub fn opened(args: Vec<String>, fresh: bool) {
    if let Some(path) = from_args(&args) {
        if fresh {
            *PENDING.lock().unwrap() = Some(path);
        } else {
            events::emit("openFile", json!({ "path": path }));
        }
    }
    if let Some(url) = link_from_args(&args) {
        if fresh {
            *PENDING_LINK.lock().unwrap() = Some(url);
        } else {
            events::emit("openLink", json!({ "url": url }));
        }
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
    theme_text(tokio::fs::read(p).await?).await
}

/// `app:themeFromBytes`: a theme file's bytes (base64) as its JSON text.
pub async fn theme_from_base64(b64: &str) -> Result<String> {
    use base64::Engine;
    if b64.len() as u64 > MAX_THEME_BYTES * 4 / 3 + 4 {
        bail!("That theme file is too big.");
    }
    theme_text(base64::engine::general_purpose::STANDARD.decode(b64.trim())?).await
}

/// A theme file's JSON: unpacked from PRAR, or (themes saved before, and by builds without PuirkleRAR) the file itself.
async fn theme_text(bytes: Vec<u8>) -> Result<String> {
    if !bytes.starts_with(b"PRAR") {
        return Ok(String::from_utf8(bytes)?);
    }
    if !crate::core::backup_prar::available() {
        bail!("This build of ResinCraft can't open PRAR theme files.");
    }
    tokio::task::spawn_blocking(move || {
        let work = std::env::temp_dir().join(format!("resincraft-theme-{}-{}", std::process::id(), crate::core::fsutil::now_ms()));
        let read = (|| {
            std::fs::create_dir_all(&work)?;
            let file = work.join("theme.rctheme");
            std::fs::write(&file, &bytes)?;
            crate::core::backup_prar::unpack_small(&file, &work.join("out"))?;
            let json = work.join("out").join(THEME_ENTRY);
            if std::fs::metadata(&json)?.len() > MAX_THEME_BYTES {
                bail!("That theme file is too big.");
            }
            Ok(std::fs::read_to_string(json)?)
        })();
        let _ = std::fs::remove_dir_all(&work);
        read
    })
    .await?
}

/// Writes a theme's JSON to `dest` as a PRAR file (on a blocking thread).
pub fn pack_theme(dest: &Path, text: &str) -> Result<()> {
    let tmp = std::env::temp_dir().join(format!("resincraft-theme-{}-{}.json", std::process::id(), crate::core::fsutil::now_ms()));
    std::fs::write(&tmp, text)?;
    let r = crate::core::backup_prar::pack_small(dest, &[(THEME_ENTRY.to_string(), tmp.clone())], None);
    let _ = std::fs::remove_file(&tmp);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn theme_files_are_prar_or_plain_json() {
        assert_eq!(theme_text(br#"{"name":"Old"}"#.to_vec()).await.unwrap(), r#"{"name":"Old"}"#);
        if !crate::core::backup_prar::available() {
            return;
        }
        let dest = std::env::temp_dir().join(format!("rc-theme-prar-{}.rctheme", std::process::id()));
        let json = format!(r#"{{"name":"New","image":"{}"}}"#, "A".repeat(100_000));
        pack_theme(&dest, &json).unwrap();
        let bytes = std::fs::read(&dest).unwrap();
        assert!(bytes.starts_with(b"PRAR"));
        assert!(bytes.len() < json.len() / 4, "{}", bytes.len());
        assert_eq!(read_theme(&dest.to_string_lossy()).await.unwrap(), json);
        use base64::Engine;
        assert_eq!(theme_from_base64(&base64::engine::general_purpose::STANDARD.encode(&bytes)).await.unwrap(), json);
        let _ = std::fs::remove_file(&dest);
    }

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

    #[test]
    fn a_web_link_never_starts_a_game() {
        let exe = "C:\\ResinCraft\\resincraft.exe".to_string();
        for link in ["resincraft://play/abc", "RESINCRAFT://PLAY/abc", "resincraft:///play/abc", "resincraft:////play//abc", "resincraft://play?x=1"] {
            let got = link_from_args(&[exe.clone(), link.into()]);
            assert!(got.as_deref().map_or(true, |l| !l.to_ascii_lowercase().contains("play")), "{link} -> {got:?}");
        }
        assert_eq!(link_from_args(&[exe.clone(), "resincraft://join/play.example.com".into()]).as_deref(), Some("resincraft://join/play.example.com"));
        assert_eq!(link_from_args(&[exe.clone(), "resincraft://join/a.b".into(), "--play=abc".into()]).as_deref(), Some("resincraft://join/a.b"));
    }
}
