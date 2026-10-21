//! What a mod jar says about itself: the mod id (plus name and version) in fabric.mod.json, quilt.mod.json,
//! META-INF/neoforge.mods.toml or META-INF/mods.toml. Two jars with the same id are the same mod whichever site, if
//! any, they came from: the game won't start with both.
//!
//! A jar on Modrinth or CurseForge is read with HTTP range requests (the end of the file, then the one small entry),
//! so checking a mod before installing it doesn't download it.

use std::collections::BTreeMap;
use std::future::Future;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use serde_json::Value;

use super::index::{open_zip, read_zip_text};
use crate::core::ctx::http;
use crate::core::net::HttpError;

/// The metadata files that name a mod, one per loader family.
const META_FILES: [&str; 5] = ["fabric.mod.json", "quilt.mod.json", "META-INF/neoforge.mods.toml", "META-INF/mods.toml", "mcmod.info"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JarMeta {
    /// The main mod id of each metadata file, lowercase (a jar built for several loaders usually repeats one id).
    pub ids: Vec<String>,
    pub name: Option<String>,
    pub version: Option<String>,
}

impl JarMeta {
    fn add(&mut self, id: Option<String>, name: Option<String>, version: Option<String>) {
        let Some(id) = id.map(|i| i.trim().to_lowercase()).filter(|i| usable(i) && !is_template_id(i)) else { return };
        if !self.ids.contains(&id) {
            self.ids.push(id);
        }
        if self.name.is_none() {
            self.name = name.map(|n| n.trim().to_string()).filter(|n| usable(n));
        }
        if self.version.is_none() {
            self.version = version.map(|v| v.trim().to_string()).filter(|v| usable(v));
        }
    }
}

/// Ids left over from a mod template, which many unrelated jars share ("examplemod").
fn is_template_id(id: &str) -> bool {
    matches!(id, "examplemod" | "example_mod" | "modid" | "mod_id" | "yourmodid" | "examplemodid" | "mymod" | "tutorialmod" | "templatemod")
}

/// Not empty and not a build placeholder left unfilled ("${version}").
fn usable(s: &str) -> bool {
    !s.is_empty() && !s.contains("${")
}

/// Whether two jars are the same mod by id.
pub fn same_mod_id(a: &JarMeta, b: &JarMeta) -> bool {
    a.ids.iter().any(|x| b.ids.contains(x))
}

// ---------- Metadata formats ----------

fn str_at(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str().map(str::to_string)
}

fn json_of(text: &str) -> Option<Value> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// The first `"key": "value"` in a file serde can't read (Fabric's own reader tolerates a little more than JSON).
fn loose_value(text: &str, key: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r#""{}"\s*:\s*"([^"]*)""#, regex::escape(key))).ok()?;
    re.captures(text).map(|c| c[1].to_string())
}

/// fabric.mod.json: `id`, `name`, `version` at the top level.
pub fn parse_fabric(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    match json_of(text) {
        Some(v) => (str_at(&v, &["id"]), str_at(&v, &["name"]), str_at(&v, &["version"])),
        None => (loose_value(text, "id"), loose_value(text, "name"), loose_value(text, "version")),
    }
}

/// quilt.mod.json: `quilt_loader.id`, `quilt_loader.version` and `quilt_loader.metadata.name`.
pub fn parse_quilt(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let Some(v) = json_of(text) else { return (None, None, None) };
    let id = str_at(&v, &["quilt_loader", "id"]).or_else(|| str_at(&v, &["id"]));
    (id, str_at(&v, &["quilt_loader", "metadata", "name"]), str_at(&v, &["quilt_loader", "version"]))
}

/// A TOML string value at the start of `v` ("x", 'x' or """x"""), without what follows it (comments).
fn toml_string(v: &str) -> Option<String> {
    let v = v.trim_start();
    for q in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(rest) = v.strip_prefix(q) {
            let end = if q == "\"" {
                // Basic strings can escape quotes.
                let mut esc = false;
                rest.char_indices().find(|&(_, c)| {
                    let hit = c == '"' && !esc;
                    esc = c == '\\' && !esc;
                    hit
                })?.0
            } else {
                rest.find(q)?
            };
            return Some(rest[..end].replace("\\\"", "\""));
        }
    }
    None
}

/// mods.toml / neoforge.mods.toml: `modId`, `displayName` and `version` of the first `[[mods]]` table (the jar's
/// own mod; `[[dependencies.x]]` tables name other mods and are skipped).
pub fn parse_mods_toml(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let (mut id, mut name, mut version) = (None, None, None);
    let mut in_mods = false;
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            if in_mods {
                break; // the first mod is done
            }
            let header: String = line.split('#').next().unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
            in_mods = header == "[[mods]]";
            continue;
        }
        if !in_mods {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let slot = match key.trim().trim_matches('"') {
            "modId" => &mut id,
            "displayName" => &mut name,
            "version" => &mut version,
            _ => continue,
        };
        *slot = toml_string(value);
    }
    (id, name, version)
}

/// mcmod.info (Forge for Minecraft ≤1.12): `modid`, `name` and `version` of the first mod listed, either a bare list
/// or `{"modList": [...]}`. Hand-written and often not quite JSON, so read loosely when serde can't.
pub fn parse_mcmod(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    if let Some(v) = json_of(text) {
        let first = match &v {
            Value::Array(list) => list.first(),
            Value::Object(_) => v.get("modList").and_then(Value::as_array).and_then(|l| l.first()),
            _ => None,
        };
        if let Some(m) = first {
            return (str_at(m, &["modid"]), str_at(m, &["name"]), str_at(m, &["version"]));
        }
    }
    (loose_value(text, "modid"), loose_value(text, "name"), loose_value(text, "version"))
}

/// Reads a jar's metadata from its metadata files' text (looked up by name).
fn meta_from(mut text: impl FnMut(&str) -> Option<String>) -> JarMeta {
    let mut m = JarMeta::default();
    for file in META_FILES {
        let Some(t) = text(file) else { continue };
        let (id, name, version) = match file {
            "fabric.mod.json" => parse_fabric(&t),
            "quilt.mod.json" => parse_quilt(&t),
            "mcmod.info" => parse_mcmod(&t),
            _ => parse_mods_toml(&t),
        };
        m.add(id, name, version);
    }
    m
}

/// A jar's metadata from an open zip.
pub fn read_meta<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> JarMeta {
    meta_from(|name| read_zip_text(zip, name).ok().flatten())
}

/// A local jar's metadata (blocking: call from spawn_blocking). Empty if it isn't a readable zip.
pub fn read_jar(path: &Path) -> JarMeta {
    open_zip(path).map(|mut z| read_meta(&mut z)).unwrap_or_default()
}

// ---------- Jars on a download site ----------

/// The file installing a project would download.
#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub url: String,
    pub size: Option<u64>,
}

/// What a ranged GET brought back: the bytes asked for, or the whole file (the server ignored the range).
pub enum Chunk {
    Part(Vec<u8>),
    Whole(Vec<u8>),
}

/// Bytes read from the end of a remote jar first: enough for the central directory of most mods.
const TAIL: u64 = 1 << 16;
/// The most a whole jar is read into memory when a server won't send parts, and the largest metadata entry read.
const MAX_WHOLE: u64 = 64 << 20;
const MAX_ENTRY: u64 = 1 << 20;

static REMOTE: Mutex<BTreeMap<String, JarMeta>> = Mutex::new(BTreeMap::new());

/// The metadata of a jar on a download site, read without downloading it all. None when it can't be read.
pub async fn remote_meta(file: &RemoteFile) -> Option<JarMeta> {
    if let Some(m) = REMOTE.lock().unwrap().get(&file.url) {
        return Some(m.clone());
    }
    let meta = match file.size.filter(|s| *s > 0) {
        Some(size) => read_remote(size, |start, end| get_range(&file.url, start, end)).await,
        None => match get_whole(&file.url).await {
            Ok(Some(all)) => meta_of_bytes(all).await,
            _ => None,
        },
    }?;
    REMOTE.lock().unwrap().insert(file.url.clone(), meta.clone());
    Some(meta)
}

async fn get_range(url: &str, start: u64, end: u64) -> Result<Option<Chunk>> {
    let res = http().get(url).header(reqwest::header::RANGE, format!("bytes={start}-{end}")).send().await?;
    let status = res.status();
    if !status.is_success() {
        return Err(HttpError { status: status.as_u16(), url: url.to_string(), body: String::new() }.into());
    }
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        return Ok(Some(Chunk::Part(res.bytes().await?.to_vec())));
    }
    Ok(read_capped(res).await?.map(Chunk::Whole))
}

async fn get_whole(url: &str) -> Result<Option<Vec<u8>>> {
    let res = http().get(url).send().await?;
    if !res.status().is_success() {
        return Err(HttpError { status: res.status().as_u16(), url: url.to_string(), body: String::new() }.into());
    }
    read_capped(res).await
}

/// A response body, unless it's bigger than `MAX_WHOLE`.
async fn read_capped(mut res: reqwest::Response) -> Result<Option<Vec<u8>>> {
    if res.content_length().is_some_and(|n| n > MAX_WHOLE) {
        return Ok(None);
    }
    let mut all = Vec::new();
    while let Some(chunk) = res.chunk().await? {
        all.extend_from_slice(&chunk);
        if all.len() as u64 > MAX_WHOLE {
            return Ok(None);
        }
    }
    Ok(Some(all))
}

async fn meta_of_bytes(all: Vec<u8>) -> Option<JarMeta> {
    tokio::task::spawn_blocking(move || zip::ZipArchive::new(Cursor::new(all)).ok().map(|mut z| read_meta(&mut z))).await.ok().flatten()
}

fn u16_at(b: &[u8], i: usize) -> Option<u64> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as u64)
}
fn u32_at(b: &[u8], i: usize) -> Option<u64> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as u64)
}
fn u64_at(b: &[u8], i: usize) -> Option<u64> {
    b.get(i..i + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
}

const MAX32: u64 = 0xFFFF_FFFF;

/// A metadata entry found in the central directory.
struct Entry {
    name: &'static str,
    method: u64,
    compressed: u64,
    offset: u64,
}

/// Where the central directory is (offset, size), from the end of the file. `tail` starts at `tail_start`.
fn find_directory(tail: &[u8], tail_start: u64) -> Option<(u64, u64)> {
    let eocd = (0..=tail.len().checked_sub(22)?).rev().find(|&i| tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])?;
    let (entries, size, offset) = (u16_at(tail, eocd + 10)?, u32_at(tail, eocd + 12)?, u32_at(tail, eocd + 16)?);
    if entries != 0xFFFF && size != MAX32 && offset != MAX32 {
        return Some((offset, size));
    }
    // ZIP64: the locator just before the end record points at the ZIP64 end record.
    let loc = eocd.checked_sub(20)?;
    if tail[loc..loc + 4] != [0x50, 0x4b, 0x06, 0x07] {
        return None;
    }
    let rec = usize::try_from(u64_at(tail, loc + 8)?.checked_sub(tail_start)?).ok()?;
    if tail.get(rec..rec + 4)? != [0x50, 0x4b, 0x06, 0x06] {
        return None;
    }
    Some((u64_at(tail, rec + 48)?, u64_at(tail, rec + 40)?))
}

/// The metadata files listed in a central directory.
fn find_entries(dir: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut i = 0;
    while dir.get(i..i + 4) == Some(&[0x50, 0x4b, 0x01, 0x02][..]) {
        let (Some(flags), Some(method), Some(mut compressed), Some(uncompressed), Some(nlen), Some(xlen), Some(clen), Some(mut offset)) = (
            u16_at(dir, i + 8),
            u16_at(dir, i + 10),
            u32_at(dir, i + 20),
            u32_at(dir, i + 24),
            u16_at(dir, i + 28),
            u16_at(dir, i + 30),
            u16_at(dir, i + 32),
            u32_at(dir, i + 42),
        ) else {
            break;
        };
        let (nlen, xlen) = (nlen as usize, xlen as usize);
        let name = dir.get(i + 46..i + 46 + nlen).and_then(|n| std::str::from_utf8(n).ok()).unwrap_or_default();
        if let Some(&known) = META_FILES.iter().find(|f| **f == name) {
            // ZIP64 sizes and offsets live in an extra field, in this order, when the 32-bit ones are maxed out.
            let extra = dir.get(i + 46 + nlen..i + 46 + nlen + xlen).unwrap_or_default();
            let maxed = [uncompressed == MAX32, compressed == MAX32, offset == MAX32];
            let mut j = 0;
            while let (Some(id), Some(len)) = (u16_at(extra, j), u16_at(extra, j + 2)) {
                if id == 1 {
                    let mut k = j + 4;
                    for (field, maxed) in [None, Some(&mut compressed), Some(&mut offset)].into_iter().zip(maxed) {
                        if maxed {
                            if let (Some(f), Some(v)) = (field, u64_at(extra, k)) {
                                *f = v;
                            }
                            k += 8;
                        }
                    }
                }
                j += 4 + len as usize;
            }
            // Encrypted entries can't be read.
            if flags & 1 == 0 && compressed <= MAX_ENTRY && offset != MAX32 {
                out.push(Entry { name: known, method, compressed, offset });
            }
        }
        i += 46 + nlen + xlen + clen as usize;
    }
    out
}

/// An entry's text from the bytes starting at its local header.
fn entry_text(local: &[u8], e: &Entry) -> Option<String> {
    if local.get(0..4)? != [0x50, 0x4b, 0x03, 0x04] {
        return None;
    }
    let start = 30 + u16_at(local, 26)? as usize + u16_at(local, 28)? as usize;
    let data = local.get(start..start + e.compressed as usize)?;
    let mut out = Vec::new();
    match e.method {
        0 => out.extend_from_slice(data),
        8 => {
            flate2::read::DeflateDecoder::new(data).take(MAX_ENTRY).read_to_end(&mut out).ok()?;
        }
        _ => return None,
    }
    String::from_utf8(out).ok()
}

/// Reads a remote jar's metadata through `fetch(start, end)` (inclusive byte ranges): the end of the file, the
/// central directory if it didn't fit, then each metadata entry.
pub async fn read_remote<F, Fut>(size: u64, fetch: F) -> Option<JarMeta>
where
    F: Fn(u64, u64) -> Fut,
    Fut: Future<Output = Result<Option<Chunk>>>,
{
    if size == 0 {
        return None;
    }
    let tail_start = size.saturating_sub(TAIL);
    let tail = match fetch(tail_start, size - 1).await.ok()?? {
        Chunk::Part(b) => b,
        Chunk::Whole(all) => return meta_of_bytes(all).await,
    };
    let (dir_start, dir_size) = find_directory(&tail, tail_start)?;
    if dir_size > MAX_WHOLE || dir_start.checked_add(dir_size)? > size {
        return None;
    }
    let dir = if dir_start >= tail_start {
        let from = (dir_start - tail_start) as usize;
        tail.get(from..from + dir_size as usize)?.to_vec()
    } else {
        match fetch(dir_start, dir_start + dir_size - 1).await.ok()?? {
            Chunk::Part(b) => b,
            Chunk::Whole(all) => return meta_of_bytes(all).await,
        }
    };
    let mut texts: BTreeMap<&str, String> = BTreeMap::new();
    for e in find_entries(&dir) {
        // The local header repeats the name and may carry a different extra field; ask for some slack.
        let want = 30 + e.name.len() as u64 + e.compressed + 512;
        let end = (e.offset + want).min(size) - 1;
        let Some(Chunk::Part(mut local)) = fetch(e.offset, end).await.ok().flatten() else { continue };
        if let (Some(n), Some(x)) = (u16_at(&local, 26), u16_at(&local, 28)) {
            let need = 30 + n + x + e.compressed;
            if need > local.len() as u64 && e.offset + need <= size {
                if let Some(Chunk::Part(rest)) = fetch(e.offset + local.len() as u64, e.offset + need - 1).await.ok().flatten() {
                    local.extend(rest);
                }
            }
        }
        if let Some(t) = entry_text(&local, &e) {
            texts.insert(e.name, t);
        }
    }
    Some(meta_from(|name| texts.get(name).cloned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn jar(files: &[(&str, &str)], padding: usize, method: zip::CompressionMethod) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(method);
        // Class files before the metadata, like a real jar, so the entries sit mid-file.
        for i in 0..padding {
            w.start_file(format!("com/example/C{i}.class"), opts.compression_method(zip::CompressionMethod::Stored)).unwrap();
            w.write_all(&vec![i as u8; 4096]).unwrap();
        }
        for (name, text) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(text.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_fabric_and_quilt() {
        let f = r#"{"schemaVersion": 1, "id": "sodium", "version": "0.6.0+mc1.21", "name": "Sodium",
            "custom": {"modmenu": {"parent": {"id": "not-this"}}}, "depends": {"minecraft": ">=1.21"}}"#;
        assert_eq!(parse_fabric(f), (Some("sodium".into()), Some("Sodium".into()), Some("0.6.0+mc1.21".into())));
        // Trailing commas: serde refuses, the loose reader still finds the id.
        let loose = "{\n \"id\": \"iris\",\n \"version\": \"1.8\",\n}";
        assert_eq!(parse_fabric(loose).0.as_deref(), Some("iris"));
        let q = r#"{"schema_version": 1, "quilt_loader": {"group": "org.x", "id": "qsl", "version": "7.0", "metadata": {"name": "QSL"}}}"#;
        assert_eq!(parse_quilt(q), (Some("qsl".into()), Some("QSL".into()), Some("7.0".into())));
    }

    #[test]
    fn reads_mods_toml() {
        let t = r#"
modLoader="javafml" # the loader
loaderVersion="[47,)"
[properties]
modId="wrong"
[[mods]] # the first mod is the jar's own
  modId = "jei" # comment
  version='19.21.0.247'
  displayName="Just Enough Items"
[[mods]]
  modId="jei_extra"
[[dependencies.jei]]
  modId="minecraft"
"#;
        assert_eq!(parse_mods_toml(t), (Some("jei".into()), Some("Just Enough Items".into()), Some("19.21.0.247".into())));
        // Dependencies alone, or ids still holding a build placeholder, name nothing.
        assert_eq!(parse_mods_toml("[[dependencies.x]]\nmodId=\"minecraft\"\n").0, None);
        let mut m = JarMeta::default();
        m.add(Some("${mod_id}".into()), None, None);
        assert!(m.ids.is_empty());
        assert_eq!(toml_string(r#""a \"b\" c" # x"#).as_deref(), Some(r#"a "b" c"#));
    }

    #[test]
    fn reads_jars_and_compares_ids() {
        let both = jar(
            &[
                ("fabric.mod.json", r#"{"id": "Create", "name": "Create Fabric", "version": "6.0"}"#),
                ("META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"create\"\ndisplayName=\"Create\"\n"),
            ],
            2,
            zip::CompressionMethod::Deflated,
        );
        let m = read_meta(&mut zip::ZipArchive::new(Cursor::new(both)).unwrap());
        assert_eq!(m.ids, vec!["create".to_string()]);
        assert_eq!((m.name.as_deref(), m.version.as_deref()), (Some("Create Fabric"), Some("6.0")));

        let other = jar(&[("META-INF/mods.toml", "[[mods]]\nmodId=\"create\"\n")], 0, zip::CompressionMethod::Stored);
        let o = read_meta(&mut zip::ZipArchive::new(Cursor::new(other)).unwrap());
        assert!(same_mod_id(&m, &o));
        let none = jar(&[("assets/x.txt", "hi")], 0, zip::CompressionMethod::Stored);
        let n = read_meta(&mut zip::ZipArchive::new(Cursor::new(none)).unwrap());
        assert!(n.ids.is_empty() && !same_mod_id(&m, &n));
    }

    /// Serves `bytes` like a server that honours ranges (or doesn't), counting what was sent.
    fn server(bytes: Vec<u8>, ranges: bool, sent: &AtomicU64) -> impl Fn(u64, u64) -> std::future::Ready<Result<Option<Chunk>>> + '_ {
        move |start, end| {
            let chunk = if ranges { bytes[start as usize..=end as usize].to_vec() } else { bytes.clone() };
            sent.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            std::future::ready(Ok(Some(if ranges { Chunk::Part(chunk) } else { Chunk::Whole(chunk) })))
        }
    }

    #[test]
    fn reads_remote_jars_in_parts() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        // 200 class files of 4 KiB: the central directory doesn't fit in the first 64 KiB read from the end.
        let fabric = r#"{"id": "lithium", "name": "Lithium", "version": "0.14"}"#;
        for method in [zip::CompressionMethod::Deflated, zip::CompressionMethod::Stored] {
            let bytes = jar(&[("fabric.mod.json", fabric)], 200, method);
            let size = bytes.len() as u64;
            let sent = AtomicU64::new(0);
            let m = rt.block_on(read_remote(size, server(bytes.clone(), true, &sent))).expect("readable");
            assert_eq!((m.ids, m.name.as_deref()), (vec!["lithium".to_string()], Some("Lithium")));
            assert!(sent.load(Ordering::Relaxed) < size / 4, "read {} of {size} bytes", sent.load(Ordering::Relaxed));

            // A server that ignores ranges sends everything; that still works.
            let sent = AtomicU64::new(0);
            let m = rt.block_on(read_remote(size, server(bytes, false, &sent))).expect("readable");
            assert_eq!(m.ids, vec!["lithium".to_string()]);
        }
        // A small jar fits in one read.
        let small = jar(&[("META-INF/mods.toml", "[[mods]]\nmodId=\"ftblibrary\"\n")], 1, zip::CompressionMethod::Deflated);
        let sent = AtomicU64::new(0);
        let m = rt.block_on(read_remote(small.len() as u64, server(small, true, &sent))).unwrap();
        assert_eq!(m.ids, vec!["ftblibrary".to_string()]);
        // Not a zip: nothing.
        let sent = AtomicU64::new(0);
        assert!(rt.block_on(read_remote(5000, server(vec![7; 5000], true, &sent))).is_none());
    }
}

#[cfg(test)]
mod mcmod_tests {
    use super::*;

    #[test]
    fn reads_old_forge_mod_info() {
        assert_eq!(
            parse_mcmod(r#"[{"modid": "BiblioCraft", "name": "BiblioCraft", "version": "1.11.7"}]"#),
            (Some("BiblioCraft".into()), Some("BiblioCraft".into()), Some("1.11.7".into()))
        );
        assert_eq!(parse_mcmod(r#"{"modListVersion": 2, "modList": [{"modid": "ForgeMultipart", "name": "Forge Multipart"}, {"modid": "McMultipart"}]}"#).0.as_deref(), Some("ForgeMultipart"));
        // Not quite JSON (trailing comma, raw newline in a string), as many old mods ship it.
        let loose = "[\n{\n \"modid\": \"bibliowoodsnatura\",\n \"name\": \"BiblioWoods\",\n \"description\": \"Line one\nline two\",\n},\n]";
        assert_eq!(parse_mcmod(loose).0.as_deref(), Some("bibliowoodsnatura"));
        // Two unrelated jars that both kept the template's id are not one mod.
        let mut a = JarMeta::default();
        a.add(Some("examplemod".into()), None, None);
        assert!(a.ids.is_empty());
    }
}
