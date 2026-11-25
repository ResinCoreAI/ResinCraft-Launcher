//! Discover's "ResinCraft" source: modpacks, mods, resource packs and shaders that players post, kept on ResinCraft
//! Relay (its community.rs; the files on its Telegram storage) and shown only once an admin approved them. Project ids
//! are "rc:<id>".
//!
//! Reading is public (no sign-in). Posting (Discover → My projects) goes with the relay session of the active Microsoft
//! account: a draft, its files (a file from this PC, or one of the player's instances packed as a `.resinpack`), a
//! picture, and Send for review.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::details;
use super::index::{folder_of, index_key, load_index, loaders_for, remove_project_files, safe_name, save_index, IndexEntry};
use crate::core::ctx::http;
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_file, hash_file, DownloadItem, Hash, HashAlgo};
use crate::core::types::{ContentKind, ContentSource, Instance, LoaderType, ProjectDetails, ProjectHit, ProjectVersion, SearchQuery, SearchResult, VersionInfo};
use crate::player::accounts;
use crate::server::relay;
use crate::{bail_tr, err_tr, tr};

pub const PREFIX: &str = "rc:";

/// `https://<relay domain>`: Discover reads from there.
fn base() -> Result<String> {
    let s = relay::settings().ok_or_else(|| err_tr!("community.noRelay", "This build of ResinCraft has no relay, so it has no community projects."))?;
    Ok(format!("https://{}", s.domain))
}

fn id_of(project_id: &str) -> Result<&str> {
    let id = project_id.strip_prefix(PREFIX).unwrap_or(project_id);
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("not a ResinCraft project: {project_id}");
    }
    Ok(id)
}

fn relay_error(answer: &Value, status: reqwest::StatusCode) -> anyhow::Error {
    let why = answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
    err_tr!("chat.relaySaid", "ResinCraft Relay: {error}", error = why)
}

/// A public read.
async fn get(path: &str) -> Result<Value> {
    let res = http()
        .get(format!("{}{path}", base()?))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| err_tr!("chat.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = e.without_url().to_string()))?;
    let status = res.status();
    let answer: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(relay_error(&answer, status));
    }
    Ok(answer)
}

fn kind_of(k: &str) -> ContentKind {
    match k {
        "modpack" => ContentKind::Modpack,
        "resourcepack" => ContentKind::Resourcepack,
        "shader" => ContentKind::Shader,
        _ => ContentKind::Mod,
    }
}

/// Unix seconds as the dates Discover shows ("2026-11-25T10:00:00Z").
fn date(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default()
}

fn str_list(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

fn to_hit(p: &Value, base: &str) -> ProjectHit {
    let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut categories = str_list(&p["categories"]);
    categories.extend(str_list(&p["loaders"]));
    ProjectHit {
        id: format!("{PREFIX}{}", s("id")),
        slug: s("slug"),
        title: s("title"),
        description: s("summary"),
        icon_url: p.get("icon").and_then(Value::as_str).map(|i| format!("{base}{i}")),
        author: s("author"),
        downloads: p["downloads"].as_u64().unwrap_or(0),
        follows: 0,
        categories,
        kind: kind_of(&s("kind")),
        updated: date(p["updated"].as_i64().unwrap_or(0)),
        source: ContentSource::Resin,
        url: String::new(),
        horizon: None,
    }
}

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    let base = base()?;
    let kind = match q.kind {
        ContentKind::World => return Ok(SearchResult { hits: Vec::new(), total: 0, offset: q.offset.unwrap_or(0) as u64 }),
        k => k.as_str(),
    };
    let sort = match q.sort.as_deref() {
        Some("newest") => "new",
        Some("updated") => "updated",
        _ => "downloads",
    };
    let mut path = format!(
        "/api/community/search?kind={kind}&sort={sort}&offset={}&limit={}&q={}",
        q.offset.unwrap_or(0),
        q.limit.unwrap_or(20).clamp(1, 50),
        urlencoding::encode(q.query.trim())
    );
    if let Some(v) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
        path.push_str(&format!("&mc={}", urlencoding::encode(v)));
    }
    if let Some(l) = q.loader.filter(|l| *l != LoaderType::Vanilla) {
        path.push_str(&format!("&loader={}", l.as_str()));
    }
    // Shader loaders are categories in Discover (Modrinth's way), and loaders here.
    for c in &q.categories {
        if matches!(c.as_str(), "iris" | "optifine" | "canvas") {
            path.push_str(&format!("&loader={c}"));
        } else if super::modrinth::is_category(c) {
            path.push_str(&format!("&cat={}", urlencoding::encode(c)));
        }
    }
    let r = get(&path).await?;
    Ok(SearchResult {
        hits: r["hits"].as_array().map(|a| a.iter().map(|p| to_hit(p, &base)).collect()).unwrap_or_default(),
        total: r["total"].as_u64().unwrap_or(0),
        offset: r["offset"].as_u64().unwrap_or(0),
    })
}

async fn project(project_id: &str) -> Result<Value> {
    get(&format!("/api/community/project/{}", id_of(project_id)?)).await
}

pub async fn details(project_id: &str) -> Result<ProjectDetails> {
    let p = project(project_id).await?;
    let body = p["body"].as_str().unwrap_or_default();
    Ok(ProjectDetails {
        body: if body.trim().is_empty() { String::new() } else { details::markdown(body, false) },
        gallery: Vec::new(),
        links: Vec::new(),
        license: None,
        changelogs: true,
        dependencies: false,
        hit: to_hit(&p, &base()?),
    })
}

pub async fn version_info(project_id: &str, version_id: &str) -> Result<VersionInfo> {
    let p = project(project_id).await?;
    let changelog = p["versions"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["id"] == version_id))
        .and_then(|v| v["changelog"].as_str())
        .filter(|c| !c.trim().is_empty())
        .map(|c| details::markdown(c, true));
    Ok(VersionInfo { changelog, dependencies: Vec::new() })
}

fn to_version(v: &Value) -> ProjectVersion {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    ProjectVersion {
        id: s("id"),
        name: s("name"),
        version_number: s("name"),
        game_versions: str_list(&v["mc"]),
        loaders: str_list(&v["loaders"]),
        kind: "release".into(),
        published: date(v["created"].as_i64().unwrap_or(0)),
        downloads: v["downloads"].as_u64().unwrap_or(0),
    }
}

/// The approved versions, newest first.
pub async fn versions(project_id: &str, _instance_id: Option<&str>) -> Result<Vec<ProjectVersion>> {
    let p = project(project_id).await?;
    Ok(p["versions"].as_array().map(|a| a.iter().map(to_version).collect()).unwrap_or_default())
}

/// Whether a version fits an instance: its Minecraft version, and (for mods) its loader.
fn fits(v: &Value, inst: &Instance, kind: ContentKind) -> bool {
    let mc = str_list(&v["mc"]);
    if !mc.is_empty() && !mc.contains(&inst.mc_version) {
        return false;
    }
    if kind != ContentKind::Mod {
        return true;
    }
    let loaders = str_list(&v["loaders"]);
    loaders.is_empty() || loaders_for(inst).iter().any(|l| loaders.contains(l))
}

/// The version to put in: `version_id`, else the newest that fits.
fn choose(p: &Value, version_id: Option<&str>, inst: Option<&Instance>, kind: ContentKind) -> Result<Value> {
    let list = p["versions"].as_array().cloned().unwrap_or_default();
    let title = p["title"].as_str().unwrap_or_default().to_string();
    let found = match version_id {
        Some(id) => list.into_iter().find(|v| v["id"] == id),
        None => list.into_iter().find(|v| inst.is_none_or(|i| fits(v, i, kind))),
    };
    found.ok_or_else(|| match inst {
        Some(i) => err_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = title, target = format!("{} {}", i.loader.label(), i.mc_version)),
        None => err_tr!("community.noFile", "{name} has no file to download yet", name = title),
    })
}

/// Downloads a version's file to `dest`, checked against its SHA-1.
async fn fetch(v: &Value, dest: &Path) -> Result<()> {
    let url = format!("{}/api/community/file/{}", base()?, v["id"].as_str().unwrap_or_default());
    let sha1 = v["sha1"].as_str().filter(|s| s.len() == 40).map(|s| Hash::sha1(s));
    download_file(&DownloadItem::new(&url, dest).hash(sha1).size(v["size"].as_u64().filter(|s| *s > 0))).await?;
    Ok(())
}

pub async fn install(instance_id: &str, project_id: &str, kind: ContentKind, version_id: Option<String>) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    if kind == ContentKind::Modpack {
        bail_tr!("community.packIsInstance", "A modpack goes in as its own instance: use Install on its page");
    }
    if kind == ContentKind::Mod && matches!(inst.loader, LoaderType::Vanilla | LoaderType::Horizon) {
        bail_tr!(
            "modrinth.vanillaInstance",
            "{name} is a vanilla instance. Mods need a mod loader such as Fabric or NeoForge; create a modded instance first.",
            name = inst.name
        );
    }
    let p = project(project_id).await?;
    let v = choose(&p, version_id.as_deref(), Some(&inst), kind)?;
    let project_id = project_id.to_string();
    Task::run(tr!("community.installing", "Installing from ResinCraft"), Some(inst.id.clone()), move |task| async move {
        let title = p["title"].as_str().unwrap_or_default().to_string();
        task.update(tr!("content.installingItem", "Installing {name}…", name = title.clone()), None);
        let mut idx = load_index(&inst.id).await;
        let folder = folder_of(&inst, kind);
        tokio::fs::create_dir_all(&folder).await?;
        remove_project_files(&inst, kind, &project_id, &mut idx).await?;
        let file = v["fileName"].as_str().unwrap_or("file").to_string();
        let dest = safe_join(&folder, safe_name(&file)?)?;
        fetch(&v, &dest).await?;
        idx.insert(
            index_key(kind, &file),
            IndexEntry {
                project_id: project_id.clone(),
                version_id: v["id"].as_str().unwrap_or_default().to_string(),
                version_number: v["name"].as_str().unwrap_or_default().to_string(),
                title,
                icon_url: p.get("icon").and_then(Value::as_str).and_then(|i| base().ok().map(|b| format!("{b}{i}"))),
                sha1: hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default(),
            },
        );
        save_index(&inst.id, &idx).await
    })
    .await
}

/// A posted modpack as a new instance: a `.mrpack` like any, or a `.resinpack` (an instance sent from a launcher).
pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    let p = project(project_id).await?;
    let v = choose(&p, version_id.as_deref(), None, ContentKind::Modpack)?;
    let file = safe_name(v["fileName"].as_str().unwrap_or("pack.mrpack"))?.to_string();
    let dir = crate::core::paths::cache().join("community");
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(format!("{}-{file}", v["id"].as_str().unwrap_or("v")));
    let title = p["title"].as_str().unwrap_or_default().to_string();
    Task::run(tr!("content.installingItem", "Installing {name}…", name = title), None, {
        let (v, dest) = (v.clone(), dest.clone());
        move |_task| async move { fetch(&v, &dest).await }
    })
    .await?;
    let lower = file.to_lowercase();
    let made = if lower.ends_with(".resinpack") {
        let added = crate::core::backup::import_share(dest.display().to_string()).await;
        match added {
            Ok(a) => instances::get(&a.id).await,
            Err(e) => Err(e),
        }
    } else {
        super::sources::import_modpack_file(&dest).await
    };
    let _ = tokio::fs::remove_file(&dest).await;
    made
}

// ---------- Posting (Discover → My projects) ----------

/// Where uploads go: the relay's direct address (past the website's proxy) when it gave one, else the usual one.
static FILES_URL: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn upload_base(domain: &str) -> String {
    let usual = format!("https://{domain}");
    let Some(direct) = FILES_URL.lock().unwrap().clone() else { return usual };
    let host = direct.trim_start_matches("https://").split(':').next().unwrap_or("").to_ascii_lowercase();
    if direct.starts_with("https://") && (host == domain || host.ends_with(&format!(".{domain}"))) {
        direct
    } else {
        usual
    }
}

/// A request with the active account's relay session (signed in again once on 401).
async fn with_session(build: impl Fn(&reqwest::Client, &str, &str) -> Result<reqwest::RequestBuilder>) -> Result<Value> {
    if relay::settings().is_none() {
        return Err(err_tr!("community.noRelay", "This build of ResinCraft has no relay, so it has no community projects."));
    }
    let Some(account) = accounts::active_microsoft_account().await else { return Err(relay::needs_microsoft()) };
    for retry in [false, true] {
        let (session, domain) = relay::session_token(&account).await?;
        let res = build(http(), &format!("https://{domain}"), &upload_base(&domain))?
            .bearer_auth(&session)
            .send()
            .await
            .map_err(|e| err_tr!("chat.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = e.without_url().to_string()))?;
        let status = res.status();
        if status.as_u16() == 401 && !retry {
            relay::forget_session(&account).await;
            continue;
        }
        let answer: Value = res.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            if status.as_u16() == 413 && answer.get("error").is_none() {
                return Err(err_tr!("chat.uploadTooBig", "That file is too big for ResinCraft Relay right now."));
            }
            return Err(relay_error(&answer, status));
        }
        return Ok(answer);
    }
    Err(err_tr!("chat.relaySaid", "ResinCraft Relay: {error}", error = "HTTP 401"))
}

async fn post(path: &'static str, body: Value) -> Result<Value> {
    with_session(move |h, base, _| Ok(h.post(format!("{base}{path}")).json(&body).timeout(std::time::Duration::from_secs(45)))).await
}

/// The player's projects, every state.
async fn mine() -> Result<Value> {
    let answer = with_session(|h, base, _| Ok(h.get(format!("{base}/api/community/mine")).timeout(std::time::Duration::from_secs(45)))).await?;
    *FILES_URL.lock().unwrap() = answer.get("filesUrl").and_then(Value::as_str).map(|u| u.trim_end_matches('/').to_string());
    // Icons as full addresses, for the page to show.
    let mut answer = answer;
    let b = base()?;
    if let Some(list) = answer.get_mut("projects").and_then(Value::as_array_mut) {
        for p in list {
            if let Some(i) = p.get("icon").and_then(Value::as_str).filter(|i| i.starts_with('/')).map(|i| format!("{b}{i}")) {
                p["icon"] = json!(i);
            }
        }
    }
    Ok(answer)
}

/// Streams a file from this PC as a project's new version.
async fn upload(project: &str, path: &Path, version: &str, mc: &str, loaders: &str, changelog: &str, title: String) -> Result<Value> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let len = tokio::fs::metadata(path).await.with_context(|| format!("couldn't read {}", path.display()))?.len();
    let query = format!(
        "/api/community/upload?project={}&name={}&version={}&mc={}&loaders={}&changelog={}",
        urlencoding::encode(project),
        urlencoding::encode(&name),
        urlencoding::encode(version),
        urlencoding::encode(mc),
        urlencoding::encode(loaders),
        urlencoding::encode(changelog)
    );
    let path = path.to_path_buf();
    Task::run(tr!("community.uploading", "Posting {name}", name = title), None, move |_task| async move {
        with_session(|h, _, files| {
            // Opened again for the second try after a sign-in.
            let file = std::fs::File::open(&path).with_context(|| format!("couldn't read {}", path.display()))?;
            let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::with_capacity(tokio::fs::File::from_std(file), 256 * 1024));
            Ok(h.post(format!("{files}{query}"))
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", len)
                .body(body)
                .timeout(std::time::Duration::from_secs(1800 + len / 250_000)))
        })
        .await
    })
    .await
}

/// A picture from this PC as the project's icon.
async fn upload_icon(project: &str, path: &Path) -> Result<Value> {
    let bytes = tokio::fs::read(path).await.with_context(|| format!("couldn't read {}", path.display()))?;
    let lower = path.to_string_lossy().to_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        bail_tr!("community.iconType", "An icon is a PNG, JPEG, WebP or GIF picture");
    };
    if bytes.len() > 512 * 1024 {
        bail_tr!("community.iconTooBig", "An icon is at most 512 KB");
    }
    let url = format!("/api/community/icon?project={}&mime={}", urlencoding::encode(project), urlencoding::encode(mime));
    with_session(move |h, base, _| Ok(h.post(format!("{base}{url}")).header("Content-Type", "application/octet-stream").body(bytes.clone()).timeout(std::time::Duration::from_secs(120)))).await
}

/// One of the player's instances, packed as a `.resinpack` (backup.rs, as Send to chat does), as a new version.
/// The packed file is removed once it's sent (or failed).
async fn upload_instance(project: &str, instance_id: &str, version: &str, changelog: &str) -> Result<Value> {
    let inst = instances::get(instance_id).await?;
    let packed = crate::core::backup::share_instance(instance_id.to_string()).await?;
    let path = PathBuf::from(&packed.path);
    let loaders = match inst.loader {
        LoaderType::Vanilla => String::new(),
        l => l.as_str().to_string(),
    };
    let r = upload(project, &path, version, &inst.mc_version, &loaders, changelog, inst.name.clone()).await;
    let _ = crate::core::backup::drop_share(&packed.path).await;
    r
}

/// The file a project's kind takes, picked on this PC.
async fn pick_file(kind: &str) -> Result<Option<String>> {
    let title = tr!("community.pickFile", "Choose the file to post").to_string();
    let filter: crate::core::shell::Filter = match kind {
        "mod" => ("Mod", &["jar"]),
        "modpack" => ("Modpack", &["mrpack", "resinpack"]),
        _ => ("Zip", &["zip"]),
    };
    Ok(crate::core::shell::pick_file(&title, &[filter]).await?.map(|p| p.display().to_string()))
}

async fn pick_icon() -> Result<Option<String>> {
    let title = tr!("community.pickIcon", "Choose a picture").to_string();
    Ok(crate::core::shell::pick_file(&title, &[("Picture", &["png", "jpg", "jpeg", "webp", "gif"])]).await?.map(|p| p.display().to_string()))
}

/// `community:*` requests.
pub async fn handle(channel: &str, args: crate::core::Args) -> Result<Value> {
    use crate::core::router::json;
    let s = |i: usize| args.get::<String>(i);
    match channel {
        "community:mine" => json(mine().await?),
        "community:save" => json(post("/api/community/save", args.get::<Value>(0)?).await?),
        "community:submit" => json(post("/api/community/submit", json!({ "id": s(0)? })).await?),
        "community:delete" => json(post("/api/community/delete", json!({ "id": s(0)? })).await?),
        "community:deleteVersion" => json(post("/api/community/version/delete", json!({ "id": s(0)? })).await?),
        // [project id, file path, version name, Minecraft versions (comma list), loaders (comma list), changelog]
        "community:upload" => {
            let path = PathBuf::from(s(1)?);
            let title = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            json(upload(&s(0)?, &path, &s(2)?, &s(3)?, &s(4)?, &s(5)?, title).await?)
        }
        // [project id, instance id, version name, changelog]
        "community:uploadInstance" => json(upload_instance(&s(0)?, &s(1)?, &s(2)?, &s(3)?).await?),
        "community:uploadIcon" => json(upload_icon(&s(0)?, Path::new(&s(1)?)).await?),
        "community:pickFile" => json(pick_file(&s(0)?).await?),
        "community:pickIcon" => json(pick_icon().await?),
        "community:base" => json(base()?),
        _ => bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(mc: &str, loader: &str) -> Instance {
        serde_json::from_value(json!({ "id": "a", "name": "A", "mcVersion": mc, "loader": loader, "color": "#fff", "createdAt": 0 })).unwrap()
    }

    #[test]
    fn versions_fit_their_instance() {
        let v = json!({ "mc": ["1.21.1"], "loaders": ["fabric"] });
        assert!(fits(&v, &inst("1.21.1", "fabric"), ContentKind::Mod));
        assert!(fits(&v, &inst("1.21.1", "quilt"), ContentKind::Mod), "Quilt runs Fabric mods");
        assert!(!fits(&v, &inst("1.21.1", "neoforge"), ContentKind::Mod));
        assert!(!fits(&v, &inst("1.20.4", "fabric"), ContentKind::Mod));
        // A shader or resource pack doesn't care about the loader.
        assert!(fits(&v, &inst("1.21.1", "vanilla"), ContentKind::Shader));
    }

    #[test]
    fn picks_the_newest_that_fits() {
        let p = json!({ "title": "T", "versions": [
            { "id": "new", "mc": ["1.21.4"], "loaders": ["fabric"] },
            { "id": "old", "mc": ["1.21.1"], "loaders": ["fabric"] },
        ] });
        assert_eq!(choose(&p, None, Some(&inst("1.21.1", "fabric")), ContentKind::Mod).unwrap()["id"], "old");
        assert_eq!(choose(&p, None, None, ContentKind::Modpack).unwrap()["id"], "new");
        assert_eq!(choose(&p, Some("old"), None, ContentKind::Mod).unwrap()["id"], "old");
        assert!(choose(&p, None, Some(&inst("1.20.1", "fabric")), ContentKind::Mod).is_err());
    }

    #[test]
    fn ids_are_checked() {
        assert_eq!(id_of("rc:Ab3x9").unwrap(), "Ab3x9");
        assert!(id_of("rc:../x").is_err());
        assert!(id_of("rc:").is_err());
    }

    #[test]
    fn hits_carry_the_relay_icon_and_source() {
        let h = to_hit(&json!({ "id": "x1", "slug": "s", "title": "T", "summary": "S", "author": "Alice", "kind": "shader", "icon": "/api/community/icon/x1?v=3", "downloads": 5, "updated": 0, "categories": ["fantasy"], "loaders": ["iris"] }), "https://r.example");
        assert_eq!(h.id, "rc:x1");
        assert_eq!(h.icon_url.as_deref(), Some("https://r.example/api/community/icon/x1?v=3"));
        assert_eq!(h.kind, ContentKind::Shader);
        assert_eq!(h.source, ContentSource::Resin);
        assert_eq!(h.categories, vec!["fantasy", "iris"]);
    }
}
