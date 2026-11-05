//! The launcher's window is served over HTTPS (`https://tauri.localhost`, which Stripe's card form needs for real
//! payments). Before, it was `http://tauri.localhost`, and the browser keeps each origin's storage apart: what the UI
//! kept in `localStorage` (Library groups and order, Discover's site, hidden pack updates…) would be gone. So the first
//! start after the change reads the old origin's entries from WebView2's own files (a copy of its `Local Storage`
//! LevelDB), and the launcher's window keeps every one it doesn't have yet (`import`). It is done once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

const DONE: &str = "webstorage-https.done";
/// How WebView2 (Chromium) keys an origin's entries: `_<origin>\0`, then the key, stored as a value is.
const OLD_ORIGIN: &[u8] = b"_http://tauri.localhost\x00";

fn marker() -> PathBuf {
    crate::core::paths::root().join(DONE)
}

/// A value as Chromium stores it: a format byte, then the text (0: UTF-16LE, 1: Latin-1).
pub fn decode(raw: &[u8]) -> Option<String> {
    let (&kind, rest) = raw.split_first()?;
    match kind {
        0 if rest.len() % 2 == 0 => String::from_utf16(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()).ok(),
        1 => Some(rest.iter().map(|&b| b as char).collect()),
        _ => None,
    }
}

/// The old origin's entries in a `Local Storage/leveldb` folder, read from a copy (WebView2 holds the folder open).
pub fn read_old(leveldb: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let copy = std::env::temp_dir().join(format!("resincraft-webstorage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&copy);
    std::fs::create_dir_all(&copy)?;
    for e in std::fs::read_dir(leveldb)?.flatten() {
        let name = e.file_name();
        if name != "LOCK" && e.file_type().is_ok_and(|t| t.is_file()) {
            std::fs::copy(e.path(), copy.join(&name))?;
        }
    }
    let read = (|| -> anyhow::Result<BTreeMap<String, String>> {
        let mut db = rusty_leveldb::DB::open(&copy, rusty_leveldb::Options { create_if_missing: false, ..Default::default() }).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut it = db.new_iter().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut out = BTreeMap::new();
        use rusty_leveldb::LdbIterator;
        while let Some((k, v)) = it.next() {
            let Some(key) = k.strip_prefix(OLD_ORIGIN) else { continue };
            if let (Some(key), Some(value)) = (decode(key), decode(&v)) {
                out.insert(key, value);
            }
        }
        Ok(out)
    })();
    let _ = std::fs::remove_dir_all(&copy);
    read
}

fn leveldb_dir() -> Option<PathBuf> {
    use tauri::Manager;
    let app = crate::core::ctx::app()?;
    Some(app.path().app_local_data_dir().ok()?.join("EBWebView").join("Default").join("Local Storage").join("leveldb"))
}

/// For the launcher's window: the old origin's entries, once (null when there is nothing to carry over).
pub async fn import() -> Value {
    if marker().exists() {
        return Value::Null;
    }
    let Some(dir) = leveldb_dir().filter(|d| d.is_dir()) else {
        let _ = std::fs::write(marker(), "");
        return Value::Null;
    };
    let read = tokio::task::spawn_blocking(move || read_old(&dir)).await;
    let _ = std::fs::write(marker(), "");
    match read {
        Ok(Ok(entries)) => serde_json::to_value(entries).unwrap_or(Value::Null),
        Ok(Err(e)) => {
            eprintln!("[storage] couldn't read the old storage: {e:#}");
            Value::Null
        }
        Err(_) => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_decode_as_chromium_stores_them() {
        assert_eq!(decode(b"\x01system").as_deref(), Some("system"));
        assert_eq!(decode(&[0, b'h', 0, b'i', 0, 0x01, 0x0e]).as_deref(), Some("hi\u{e01}"));
        assert_eq!(decode(b"\x02x"), None);
        assert_eq!(decode(b""), None);
    }
}
