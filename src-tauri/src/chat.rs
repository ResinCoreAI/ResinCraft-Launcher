//! Friends and chat: `chat:call [method, path, body]` sends one of the relay's `/api/chat…` requests over HTTPS with the
//! active Microsoft account's relay session (relay-server chat.rs), like the Inbox's mail.rs. A `sync?wait=1` waits on
//! the relay up to 25 seconds, which is how messages arrive while the launcher is open.
//!
//! The launcher keeps its own copy of each account's chats in `chats.db` (SQLite) in its data folder, so they open at
//! once and stay readable after the relay deletes old messages (it keeps them for a time the admins set). Messages are
//! only ever added or updated here, never deleted with the relay's. `chat:export` saves the whole copy as one compressed
//! PRAR file (a zip where this build has no PuirkleRAR) and `chat:import` reads one back, adding what's missing. The
//! uninstaller runs `resincraft.exe --export-chats <folder>` when the player wants their chats kept.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::core::router::json;
use crate::core::{paths, shell, Args};
use crate::err_tr;
use crate::player::accounts;
use crate::server::relay;

/// Messages per conversation `chat:load` hands over (older ones come with `chat:older`).
const LOAD_PER_CONV: i64 = 80;
const OLDER_MAX: i64 = 200;
/// What an exported file holds, and the manifest inside it.
const DB_NAME: &str = "chats.db";
const MANIFEST: &str = "resincraft-chats.json";
const FORMAT: &str = "resincraft-chats";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS messages(account TEXT NOT NULL, id INTEGER NOT NULL, seq INTEGER NOT NULL, conv TEXT NOT NULL, sender TEXT NOT NULL,
  at INTEGER NOT NULL, kind TEXT NOT NULL, body TEXT NOT NULL, data TEXT, reply INTEGER, edited INTEGER, deleted INTEGER, PRIMARY KEY(account, id));
CREATE INDEX IF NOT EXISTS messages_conv ON messages(account, conv, id);
-- Per account: the relay's sync cursor, the friends list as last seen, and the names of everyone it chatted with.
CREATE TABLE IF NOT EXISTS state(account TEXT PRIMARY KEY, cursor INTEGER NOT NULL DEFAULT 0, friends TEXT, names TEXT, updated INTEGER NOT NULL DEFAULT 0);
";

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "chat:call" => json(call(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<Value>>(2)?).await?),
        "chat:save" => {
            let (account, payload) = (args.get::<String>(0)?, args.get::<Value>(1)?);
            json(tokio::task::spawn_blocking(move || save(&account, &payload)).await??)
        }
        "chat:load" => {
            let account = args.get::<String>(0)?;
            json(tokio::task::spawn_blocking(move || load(&account)).await??)
        }
        "chat:older" => {
            let (account, conv, before, limit) = (args.get::<String>(0)?, args.get::<String>(1)?, args.get::<i64>(2)?, args.get::<Option<i64>>(3)?.unwrap_or(50));
            json(tokio::task::spawn_blocking(move || older(&account, &conv, before, limit)).await??)
        }
        "chat:info" => json(tokio::task::spawn_blocking(info).await??),
        "chat:export" => json(export_picked().await?),
        "chat:pickFiles" => json(pick_files().await?),
        "chat:upload" => json(upload_path(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?, args.get::<Option<Value>>(3)?.unwrap_or(Value::Null)).await?),
        "chat:uploadBytes" => json(upload_bytes(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?, &args.get::<String>(3)?, &args.get::<String>(4)?, args.get::<Option<Value>>(5)?.unwrap_or(Value::Null)).await?),
        "chat:fileData" => json(file_data(&args.get::<String>(0)?).await?),
        "chat:fileLocal" => json(file_local(&args.get::<String>(0)?).await?.display().to_string()),
        "chat:fileSave" => json(file_save(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "chat:attention" => {
            attention();
            json(true)
        }
        "chat:notify" => json(notify(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?)),
        "chat:import" => json(import_picked().await?),
        _ => bail!("Unknown channel {channel}"),
    }
}

// ---------- The relay ----------

/// Only the player's own chat routes go through here (the admins' go over the admin connection).
fn chat_path(path: &str) -> bool {
    let route = path.split('?').next().unwrap_or("");
    (route == "/api/chat" || route.starts_with("/api/chat/"))
        && !path.contains("..")
        && path.chars().all(|c| c.is_ascii_alphanumeric() || "/?=&-_:.%".contains(c))
}

pub async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    if !chat_path(path) || !matches!(method, "GET" | "POST") {
        bail!("not a chat route: {method} {path}");
    }
    if relay::settings().is_none() {
        return Err(err_tr!("chat.noRelay", "This build of ResinCraft has no relay, so it has no chat."));
    }
    let Some(account) = accounts::active_microsoft_account().await else { return Err(relay::needs_microsoft()) };
    for retry in [false, true] {
        let (session, domain) = relay::session_token(&account).await?;
        let url = format!("https://{domain}{path}");
        let http = crate::core::ctx::http();
        let req = if method == "GET" { http.get(url) } else { http.post(url).json(&body.clone().unwrap_or_else(|| json!({}))) };
        let res = req
            .bearer_auth(&session)
            .timeout(std::time::Duration::from_secs(45))
            .send()
            .await
            .map_err(|e| err_tr!("chat.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = e.without_url().to_string()))?;
        let status = res.status();
        if status.as_u16() == 401 && !retry {
            relay::forget_session(&account).await;
            continue;
        }
        // A relay from before chat.
        if status.as_u16() == 404 && path.starts_with("/api/chat/sync") {
            return Err(err_tr!("chat.notYet", "ResinCraft Relay doesn't have chat yet."));
        }
        let answer: Value = res.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let why = answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(err_tr!("chat.relaySaid", "ResinCraft Relay: {error}", error = why));
        }
        return Ok(answer);
    }
    Err(err_tr!("chat.relaySaid", "ResinCraft Relay: {error}", error = "HTTP 401"))
}

// ---------- Files (through the relay, to its Telegram drive) ----------

/// The biggest file the picker hands over (the relay has its own limit, usually lower).
const PICK_MAX: u64 = 2_000_000_000;
/// Pictures and voice messages shown in chat are read into the page (bigger files are saved instead).
const PREVIEW_MAX: u64 = 25_000_000;

/// A content type from a file name (the relay keeps it as sent; unknown: a plain file).
pub fn mime_of(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "webm" => "audio/webm",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "jar" => "application/java-archive",
        "zip" | "mrpack" => "application/zip",
        "txt" | "log" => "text/plain",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// A request to the relay with the active account's session (one sign-in again on 401).
async fn relay_send(build: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder) -> Result<reqwest::Response> {
    if relay::settings().is_none() {
        return Err(err_tr!("chat.noRelay", "This build of ResinCraft has no relay, so it has no chat."));
    }
    let Some(account) = accounts::active_microsoft_account().await else { return Err(relay::needs_microsoft()) };
    for retry in [false, true] {
        let (session, domain) = relay::session_token(&account).await?;
        let res = build(crate::core::ctx::http(), &format!("https://{domain}"))
            .bearer_auth(&session)
            .send()
            .await
            .map_err(|e| err_tr!("chat.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = e.without_url().to_string()))?;
        if res.status().as_u16() == 401 && !retry {
            relay::forget_session(&account).await;
            continue;
        }
        return Ok(res);
    }
    Err(err_tr!("chat.relaySaid", "ResinCraft Relay: {error}", error = "HTTP 401"))
}

async fn relay_json(res: reqwest::Response) -> Result<Value> {
    let status = res.status();
    let answer: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let why = answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        return Err(err_tr!("chat.relaySaid", "ResinCraft Relay: {error}", error = why));
    }
    Ok(answer)
}

/// The upload's address: the chat, the file's name, kind and type, and what the page measured (w, h, duration).
fn upload_url(base: &str, conv: &str, name: &str, kind: &str, mime: &str, extra: &Value) -> String {
    let mut q = format!(
        "{base}/api/chat/upload?conv={}&name={}&kind={}&mime={}",
        urlencoding::encode(conv),
        urlencoding::encode(name),
        urlencoding::encode(kind),
        urlencoding::encode(mime)
    );
    for k in ["w", "h", "duration"] {
        if let Some(n) = extra.get(k).and_then(Value::as_f64) {
            q.push_str(&format!("&{k}={n}"));
        }
    }
    q
}

/// Files the player picks to send: path, name, size.
async fn pick_files() -> Result<Vec<Value>> {
    let title = crate::tr!("chat.pickTitle", "Send files").to_string();
    let picked = shell::pick_files(&title, &[]).await?;
    let mut out = Vec::new();
    for p in picked {
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        if size == 0 || size > PICK_MAX {
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        out.push(json!({ "path": p.display().to_string(), "name": name, "size": size, "mime": mime_of(&name) }));
    }
    Ok(out)
}

/// Uploads a file from this PC into a chat (`kind`: image, voice, file, avatar, emoji). The relay's file record.
async fn upload_path(conv: &str, path: &str, kind: &str, extra: Value) -> Result<Value> {
    let p = PathBuf::from(path);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let bytes = tokio::fs::read(&p).await.with_context(|| format!("couldn't read {}", p.display()))?;
    let mime = extra.get("mime").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| mime_of(&name).to_string());
    send_upload(conv, &name, kind, &mime, bytes, &extra).await
}

/// Uploads bytes the page has (a pasted picture, a voice message, a profile picture), base64.
async fn upload_bytes(conv: &str, name: &str, mime: &str, kind: &str, data: &str, extra: Value) -> Result<Value> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).context("the file's data was damaged")?;
    send_upload(conv, name, kind, mime, bytes, &extra).await
}

async fn send_upload(conv: &str, name: &str, kind: &str, mime: &str, bytes: Vec<u8>, extra: &Value) -> Result<Value> {
    let res = relay_send(|http, base| {
        http.post(upload_url(base, conv, name, kind, mime, extra))
            .header("Content-Type", "application/octet-stream")
            .body(bytes.clone())
            .timeout(std::time::Duration::from_secs(1800))
    })
    .await?;
    Ok(relay_json(res).await?["file"].clone())
}

fn cache_dir() -> PathBuf {
    paths::root().join("chat-files")
}

/// The file in this PC's cache, downloaded through the relay the first time.
async fn file_local(id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        bail!("not a file: {id}");
    }
    let dest = cache_dir().join(id);
    if dest.exists() {
        return Ok(dest);
    }
    let res = relay_send(|http, base| http.get(format!("{base}/api/chat/file/{id}")).timeout(std::time::Duration::from_secs(1800))).await?;
    if !res.status().is_success() {
        relay_json(res).await?;
        bail!("couldn't download the file");
    }
    let bytes = res.bytes().await.context("the download stopped")?;
    tokio::fs::create_dir_all(cache_dir()).await?;
    let tmp = dest.with_extension("part");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &dest).await?;
    Ok(dest)
}

/// A picture or voice message for the page: its type and base64 bytes.
async fn file_data(id: &str) -> Result<Value> {
    use base64::Engine;
    let path = file_local(id).await?;
    let size = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);
    if size > PREVIEW_MAX {
        bail!("too big to show here; save it instead");
    }
    let bytes = tokio::fs::read(&path).await?;
    let mime = sniff(&bytes);
    Ok(json!({ "mime": mime, "data": base64::engine::general_purpose::STANDARD.encode(&bytes) }))
}

/// The type from the first bytes (what the page may show).
fn sniff(b: &[u8]) -> &'static str {
    if b.starts_with(b"\x89PNG") {
        "image/png"
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "image/webp"
    } else if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        "audio/webm"
    } else if b.starts_with(b"OggS") {
        "audio/ogg"
    } else if b.starts_with(b"ID3") || b.starts_with(&[0xFF, 0xFB]) {
        "audio/mpeg"
    } else {
        "application/octet-stream"
    }
}

/// Saves a chat file where the player picks. Where it went, or None when cancelled.
async fn file_save(id: &str, name: &str) -> Result<Option<String>> {
    let title = crate::tr!("chat.saveTitle", "Save the file").to_string();
    let default = shell::downloads_dir().join(name);
    let Some(dest) = shell::save_file(&title, &default, &[]).await? else { return Ok(None) };
    let src = file_local(id).await?;
    tokio::fs::copy(&src, &dest).await?;
    let _ = shell::reveal(&dest);
    Ok(Some(dest.display().to_string()))
}

/// A Windows pop-up for a chat message (the launcher isn't in front): who and what. Clicking it brings the launcher
/// back and opens that chat (`resin:chatOpen`). False where there are no pop-ups.
fn notify(title: &str, body: &str, conv: &str) -> bool {
    #[cfg(windows)]
    {
        use tauri_winrt_notification::{Duration, Sound, Toast};
        let conv = conv.to_string();
        let shown = Toast::new(crate::APP_ID)
            .title(&title.chars().take(120).collect::<String>())
            .text1(&body.chars().take(200).collect::<String>())
            .sound(Some(Sound::IM))
            .duration(Duration::Short)
            .on_activated(move |_| {
                if let Some(app) = crate::core::ctx::app() {
                    crate::tray::show(app);
                }
                crate::core::events::emit("chatOpen", json!({ "conv": conv }));
                Ok(())
            })
            .show();
        return shown.is_ok();
    }
    #[cfg(not(windows))]
    {
        let _ = (title, body, conv);
        false
    }
}

/// The taskbar button flashes (a message came while the launcher wasn't in front).
fn attention() {
    use tauri::Manager;
    if let Some(app) = crate::core::ctx::app() {
        if let Some(w) = app.webview_windows().values().next() {
            if !w.is_focused().unwrap_or(false) {
                let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
            }
        }
    }
}

// ---------- The launcher's own copy ----------

fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DB_NAME)
}

fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let c = Connection::open(path).with_context(|| format!("couldn't open {}", path.display()))?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    let _: String = c.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    c.pragma_update(None, "synchronous", "NORMAL")?;
    c.execute_batch(SCHEMA)?;
    Ok(c)
}

fn with_db<T>(f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
    static DB: OnceLock<Mutex<Option<(PathBuf, Connection)>>> = OnceLock::new();
    let path = db_path(&paths::root());
    let mut guard = DB.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|p| p.into_inner());
    if guard.as_ref().is_none_or(|(p, _)| p != &path) {
        *guard = Some((path.clone(), open(&path)?));
    }
    f(&mut guard.as_mut().unwrap().1)
}

/// An account key as the relay sends it (`me`: 32 hex characters).
fn check_account(account: &str) -> Result<()> {
    if account.len() != 32 || !account.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("not an account: {account}");
    }
    Ok(())
}

fn upsert(tx: &rusqlite::Transaction, account: &str, m: &Value) -> rusqlite::Result<()> {
    let data = m.get("data").filter(|d| !d.is_null()).map(Value::to_string);
    tx.execute(
        "INSERT INTO messages(account, id, seq, conv, sender, at, kind, body, data, reply, edited, deleted) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(account, id) DO UPDATE SET seq = excluded.seq, body = excluded.body, data = excluded.data, edited = excluded.edited, deleted = excluded.deleted
         WHERE excluded.seq >= messages.seq",
        params![
            account,
            m["id"].as_i64().unwrap_or(0),
            m["seq"].as_i64().unwrap_or(0),
            m["conv"].as_str().unwrap_or(""),
            m["sender"].as_str().unwrap_or(""),
            m["at"].as_i64().unwrap_or(0),
            m["kind"].as_str().unwrap_or("text"),
            m["body"].as_str().unwrap_or(""),
            data,
            m["reply"].as_i64(),
            m["edited"].as_i64(),
            m["deleted"].as_i64(),
        ],
    )?;
    Ok(())
}

/// Keeps what a sync (or a page of history) brought: `{messages, cursor?, friends?}`. The cursor only moves forward.
pub fn save(account: &str, payload: &Value) -> Result<Value> {
    check_account(account)?;
    with_db(|db| {
        let tx = db.transaction()?;
        let mut n = 0;
        for m in payload["messages"].as_array().into_iter().flatten() {
            if m["id"].as_i64().unwrap_or(0) > 0 {
                upsert(&tx, account, m)?;
                n += 1;
            }
        }
        let now = chrono::Utc::now().timestamp();
        tx.execute("INSERT OR IGNORE INTO state(account, updated) VALUES(?1, ?2)", params![account, now])?;
        if let Some(c) = payload["cursor"].as_i64() {
            tx.execute("UPDATE state SET cursor = MAX(cursor, ?2), updated = ?3 WHERE account = ?1", params![account, c, now])?;
        }
        if let Some(f) = payload.get("friends").filter(|f| f.is_object()) {
            // Names of everyone in the list, added to the ones known before (old chats keep their names).
            let old: Option<String> = tx.query_row("SELECT names FROM state WHERE account = ?1", params![account], |r| r.get(0)).optional()?.flatten();
            let mut names: serde_json::Map<String, Value> = old.and_then(|o| serde_json::from_str(&o).ok()).unwrap_or_default();
            for list in ["friends", "incoming", "outgoing", "blocked"] {
                for p in f[list].as_array().into_iter().flatten() {
                    if let (Some(u), Some(n)) = (p["uuid"].as_str(), p["name"].as_str().filter(|n| !n.is_empty())) {
                        names.insert(u.to_string(), json!(n));
                    }
                }
            }
            tx.execute("UPDATE state SET friends = ?2, names = ?3 WHERE account = ?1", params![account, f.to_string(), Value::Object(names).to_string()])?;
        }
        tx.commit()?;
        Ok(json!({ "saved": n }))
    })
}

fn row_json(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let data: Option<String> = r.get("data")?;
    Ok(json!({
        "id": r.get::<_, i64>("id")?,
        "seq": r.get::<_, i64>("seq")?,
        "conv": r.get::<_, String>("conv")?,
        "sender": r.get::<_, String>("sender")?,
        "at": r.get::<_, i64>("at")?,
        "kind": r.get::<_, String>("kind")?,
        "body": r.get::<_, String>("body")?,
        "data": data.and_then(|d| serde_json::from_str::<Value>(&d).ok()).unwrap_or(Value::Null),
        "reply": r.get::<_, Option<i64>>("reply")?,
        "edited": r.get::<_, Option<i64>>("edited")?,
        "deleted": r.get::<_, Option<i64>>("deleted")?,
    }))
}

/// The account's copy: its cursor, the friends list as last seen, names, and the newest messages of each conversation
/// (oldest first), with how many each has here.
pub fn load(account: &str) -> Result<Value> {
    check_account(account)?;
    with_db(|db| {
        let st: Option<(i64, Option<String>, Option<String>)> = db.query_row("SELECT cursor, friends, names FROM state WHERE account = ?1", params![account], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        let (cursor, friends, names) = st.unwrap_or((0, None, None));
        let mut convs = Vec::new();
        let mut st = db.prepare("SELECT conv, COUNT(*), MAX(at) FROM messages WHERE account = ?1 GROUP BY conv ORDER BY MAX(at) DESC")?;
        let list: Vec<(String, i64, i64)> = st.query_map(params![account], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut msgs = db.prepare(&format!("SELECT * FROM (SELECT * FROM messages WHERE account = ?1 AND conv = ?2 ORDER BY id DESC LIMIT {LOAD_PER_CONV}) ORDER BY id"))?;
        for (conv, count, last) in list {
            let messages: Vec<Value> = msgs.query_map(params![account, conv], row_json)?.collect::<rusqlite::Result<_>>()?;
            convs.push(json!({ "conv": conv, "count": count, "last": last, "messages": messages }));
        }
        Ok(json!({
            "cursor": cursor,
            "friends": friends.and_then(|f| serde_json::from_str::<Value>(&f).ok()),
            "names": names.and_then(|n| serde_json::from_str::<Value>(&n).ok()).unwrap_or_else(|| json!({})),
            "convs": convs,
        }))
    })
}

/// Messages of a conversation from before `before` (newest first), from the copy here.
pub fn older(account: &str, conv: &str, before: i64, limit: i64) -> Result<Value> {
    check_account(account)?;
    with_db(|db| {
        let mut st = db.prepare("SELECT * FROM messages WHERE account = ?1 AND conv = ?2 AND id < ?3 ORDER BY id DESC LIMIT ?4")?;
        let messages: Vec<Value> = st.query_map(params![account, conv, before, limit.clamp(1, OLDER_MAX)], row_json)?.collect::<rusqlite::Result<_>>()?;
        Ok(json!({ "messages": messages }))
    })
}

/// How big the copy is.
pub fn info() -> Result<Value> {
    let path = db_path(&paths::root());
    let bytes = [path.clone(), path.with_extension("db-wal")].iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum::<u64>();
    let (messages, accounts) = if path.exists() { with_db(|db| Ok((db.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))?, db.query_row("SELECT COUNT(DISTINCT account) FROM messages", [], |r| r.get::<_, i64>(0))?)))? } else { (0, 0) };
    Ok(json!({ "bytes": bytes, "messages": messages, "accounts": accounts, "path": path.display().to_string(), "prar": crate::core::backup_prar::available() }))
}

// ---------- Saving and opening a chat history file ----------

fn file_name(prar: bool) -> String {
    format!("ResinCraft-chats-{}.{}", chrono::Local::now().format("%Y-%m-%d"), if prar { "prar" } else { "zip" })
}

/// Writes the copy in `data_dir` to `dest` (a .prar, or a .zip). The number of messages, or None with nothing to save.
pub fn export_to(data_dir: &Path, dest: &Path) -> Result<Option<i64>> {
    let src = db_path(data_dir);
    if !src.exists() {
        return Ok(None);
    }
    let db = open(&src)?;
    let messages: i64 = db.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
    if messages == 0 {
        return Ok(None);
    }
    let work = std::env::temp_dir().join(format!("resincraft-chats-{}-{}", std::process::id(), chrono::Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&work)?;
    let result = (|| -> Result<()> {
        let copy = work.join(DB_NAME);
        // A clean, compact copy (WAL folded in), taken while the launcher may be running.
        db.execute("VACUUM INTO ?1", params![copy.to_string_lossy().to_string()])?;
        let accounts: Vec<Value> = {
            let mut st = db.prepare("SELECT account, COUNT(*), MIN(at), MAX(at) FROM messages GROUP BY account")?;
            let rows = st.query_map([], |r| Ok(json!({ "account": r.get::<_, String>(0)?, "messages": r.get::<_, i64>(1)?, "from": r.get::<_, i64>(2)?, "until": r.get::<_, i64>(3)? })))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let manifest = json!({ "format": FORMAT, "version": 1, "created": chrono::Utc::now().timestamp(), "app": crate::updater::current_version(), "messages": messages, "accounts": accounts });
        let manifest_text = manifest.to_string();
        let manifest_file = work.join(MANIFEST);
        std::fs::write(&manifest_file, &manifest_text)?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let files = vec![(DB_NAME.to_string(), copy), (MANIFEST.to_string(), manifest_file)];
        if dest.extension().and_then(|e| e.to_str()) == Some("prar") && crate::core::backup_prar::available() {
            crate::core::backup_prar::pack_small(dest, &files, Some(&manifest_text))
        } else {
            write_zip(dest, &files)
        }
    })();
    let _ = std::fs::remove_dir_all(&work);
    result.map(|()| Some(messages))
}

fn write_zip(dest: &Path, files: &[(String, PathBuf)]) -> Result<()> {
    let tmp = dest.with_extension("zip.part");
    {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, path) in files {
            zip.start_file(name.as_str(), opts)?;
            std::io::copy(&mut std::fs::File::open(path)?, &mut zip)?;
        }
        zip.finish()?;
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// Adds the messages in a saved file to the copy in `data_dir`. How many were new.
pub fn import_from(data_dir: &Path, file: &Path) -> Result<i64> {
    let work = std::env::temp_dir().join(format!("resincraft-chats-in-{}-{}", std::process::id(), chrono::Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&work)?;
    let result = (|| -> Result<i64> {
        let is_zip = std::fs::File::open(file).and_then(|mut f| {
            let mut magic = [0u8; 2];
            std::io::Read::read_exact(&mut f, &mut magic).map(|()| magic == *b"PK")
        })?;
        if is_zip {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(file)?)?;
            for name in [DB_NAME, MANIFEST] {
                let mut entry = zip.by_name(name).map_err(|_| err_tr!("chat.notHistory", "That file isn't a ResinCraft chat history."))?;
                std::io::copy(&mut entry, &mut std::fs::File::create(work.join(name))?)?;
            }
        } else {
            crate::core::backup_prar::unpack_small(file, &work).map_err(|_| err_tr!("chat.notHistory", "That file isn't a ResinCraft chat history."))?;
        }
        let manifest: Value = std::fs::read_to_string(work.join(MANIFEST)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        if manifest["format"] != FORMAT || !work.join(DB_NAME).exists() {
            return Err(err_tr!("chat.notHistory", "That file isn't a ResinCraft chat history."));
        }
        let mut db = open(&db_path(data_dir))?;
        let tx = db.transaction()?;
        tx.execute("ATTACH DATABASE ?1 AS saved", params![work.join(DB_NAME).to_string_lossy().to_string()])?;
        let before: i64 = tx.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
        tx.execute("INSERT OR IGNORE INTO messages SELECT * FROM saved.messages", [])?;
        // Names and friends lists for accounts this copy didn't know; the cursor stays (the relay's is what counts).
        tx.execute("INSERT OR IGNORE INTO state(account, cursor, friends, names, updated) SELECT account, 0, friends, names, updated FROM saved.state", [])?;
        let after: i64 = tx.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
        tx.commit()?;
        db.execute("DETACH DATABASE saved", [])?;
        Ok(after - before)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

async fn export_picked() -> Result<Option<Value>> {
    let prar = crate::core::backup_prar::available();
    let default = documents_dir().join(file_name(prar));
    let title = crate::tr!("chat.exportTitle", "Save your chat history").to_string();
    let filters: &[shell::Filter] = if prar { &[("PuirkleRAR", &["prar"])] } else { &[("Zip", &["zip"])] };
    let Some(dest) = shell::save_file(&title, &default, filters).await? else { return Ok(None) };
    let data_dir = paths::root();
    let d2 = dest.clone();
    let n = tokio::task::spawn_blocking(move || export_to(&data_dir, &d2)).await??;
    match n {
        Some(messages) => {
            let _ = shell::reveal(&dest);
            Ok(Some(json!({ "path": dest.display().to_string(), "messages": messages })))
        }
        None => Err(err_tr!("chat.nothingToSave", "There are no chats to save yet.")),
    }
}

async fn import_picked() -> Result<Option<Value>> {
    let title = crate::tr!("chat.importTitle", "Open a saved chat history").to_string();
    let Some(file) = shell::pick_file(&title, &[("ResinCraft chat history", &["prar", "zip"])]).await? else { return Ok(None) };
    let data_dir = paths::root();
    let added = tokio::task::spawn_blocking(move || import_from(&data_dir, &file)).await??;
    Ok(Some(json!({ "added": added })))
}

fn documents_dir() -> PathBuf {
    use tauri::Manager;
    crate::core::ctx::app().and_then(|a| a.path().document_dir().ok()).unwrap_or_else(shell::downloads_dir)
}

/// `resincraft.exe --export-chats <folder>` (the uninstaller, before it removes the launcher): saves the chats into
/// that folder without opening a window. The exit code: 0 saved, 3 nothing to save, 1 failed.
pub fn export_cli(folder: &Path) -> i32 {
    // The data folder an installed launcher uses (lib.rs: the config folder, "ResinCraft Launcher").
    let config = std::env::var_os("APPDATA").map(PathBuf::from);
    let data_dir = paths::default_data_dir(config);
    let dest = folder.join(file_name(crate::core::backup_prar::available()));
    match export_to(&data_dir, &dest) {
        Ok(Some(_)) => 0,
        Ok(None) => 3,
        Err(e) => {
            eprintln!("couldn't save the chats: {e:#}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "8667ba71b85a4004af54457a9734eed7";

    fn msg(id: i64, seq: i64, body: &str) -> Value {
        json!({ "id": id, "seq": seq, "conv": "dm:a:b", "sender": ME, "at": 1000 + id, "kind": "text", "body": body, "data": null, "reply": null, "edited": null, "deleted": null })
    }

    #[test]
    fn routes() {
        assert!(chat_path("/api/chat/sync?after=10&v=3&wait=1"));
        assert!(chat_path("/api/chat/friends/add"));
        for bad in ["/api/inbox", "/api/chat-admin", "/api/chat/../mail", "/api/chat/x#y", "/api/chats", "/api/chat/a b"] {
            assert!(!chat_path(bad), "{bad}");
        }
    }

    #[test]
    fn keeps_edits_and_saves_a_file_that_opens_again() {
        let dir = std::env::temp_dir().join(format!("rc-chats-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = open(&db_path(&dir)).unwrap();
        drop(db);
        // Writing through a connection of its own (with_db needs the app's data folder).
        let mut c = open(&db_path(&dir)).unwrap();
        {
            let tx = c.transaction().unwrap();
            upsert(&tx, ME, &msg(1, 1, "hello")).unwrap();
            upsert(&tx, ME, &msg(2, 2, "there")).unwrap();
            // An edit (a newer seq) replaces it; an older copy of the message doesn't.
            upsert(&tx, ME, &msg(1, 5, "hello!")).unwrap();
            upsert(&tx, ME, &msg(1, 3, "stale")).unwrap();
            tx.commit().unwrap();
        }
        let body: String = c.query_row("SELECT body FROM messages WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(body, "hello!");
        drop(c);
        let file = dir.join("out").join("chats.zip");
        assert_eq!(export_to(&dir, &file).unwrap(), Some(2));
        // Into a fresh folder: both come back once.
        let other = dir.join("other");
        assert_eq!(import_from(&other, &file).unwrap(), 2);
        assert_eq!(import_from(&other, &file).unwrap(), 0);
        assert_eq!(export_to(&dir.join("empty"), &dir.join("none.zip")).unwrap(), None);
        std::fs::write(dir.join("junk.zip"), b"PK\x03\x04junk").unwrap();
        assert!(import_from(&other, &dir.join("junk.zip")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
