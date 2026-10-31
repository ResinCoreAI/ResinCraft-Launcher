//! A server's old logs: lists `logs/*.log`, `logs/*.log.gz` and `crash-reports/*`, reads a page of one (gzipped or
//! not) and searches one or all of them, without unpacking anything to disk.
//!
//! Files are read as a stream, a line at a time, so memory stays flat whatever their size. Every call has a limit on
//! lines, on the bytes it returns and on the bytes it reads (a small .gz can unpack to any size), and says so when it
//! stops at one.
//!
//! Logs hold player IPs and sometimes tokens, so these channels never travel to the web console or to other PCs:
//! `remote.rs` doesn't list them.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Result};
use flate2::read::MultiGzDecoder;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::fsutil::safe_join_within;
use crate::core::router::json;
use crate::core::{shell, Args};

use super::store;

const LOGS: &str = "logs";
const CRASHES: &str = "crash-reports";
/// Lines in one page of a file.
const PAGE_LINES: usize = 2000;
/// Matches one search returns.
const MAX_HITS: usize = 1000;
/// The longest line kept; the rest of it is dropped (and isn't searched).
const MAX_LINE: usize = 16 * 1024;
/// What one call returns, however the lines add up.
const MAX_RESULT: usize = 8 * 1024 * 1024;
/// What one call reads, after unpacking.
const MAX_SCAN: u64 = 8 * 1024 * 1024 * 1024;
const MAX_QUERY: usize = 500;
/// A log line's level is in its first few columns; further along it's the message talking.
const HEAD: usize = 120;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    /// What to ask for it by: "logs/latest.log".
    pub path: String,
    pub name: String,
    /// True in crash-reports/.
    pub crash: bool,
    /// On disk, so packed for a .gz.
    pub size: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    /// Counted from 1.
    pub line: u64,
    pub text: String,
    pub level: Level,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPage {
    pub lines: Vec<Line>,
    /// There are lines after these.
    pub more: bool,
    /// The file ends early or isn't what its name says: these are the lines that could be read.
    pub damaged: bool,
    /// Stopped at MAX_SCAN, before the end of the file.
    pub capped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub path: String,
    #[serde(flatten)]
    pub line: Line,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearch {
    pub hits: Vec<Hit>,
    /// There are more matches than `hits`.
    pub more: bool,
    /// Stopped at MAX_SCAN: some files or lines weren't looked at.
    pub capped: bool,
    /// Files that could only be read in part, or not at all.
    pub damaged: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Query {
    /// One file; all of them when absent.
    path: Option<String>,
    query: String,
    /// Warnings and errors only.
    problems: bool,
}

// ---------- Paths ----------

/// Only what the list shows: a plain file name, right inside one of the two folders.
fn wanted(folder: &str, name: &str) -> bool {
    if name.is_empty() || name.contains(['/', '\\', ':', '\0']) {
        return false;
    }
    let lower = name.to_lowercase();
    match folder {
        LOGS => lower.ends_with(".log") || lower.ends_with(".log.gz"),
        CRASHES => !name.starts_with('.'),
        _ => false,
    }
}

/// A path the UI sent ("logs/latest.log") as a file in this server's folder. Refuses anything else, and links.
fn resolve(dir: &Path, rel: &str) -> Result<PathBuf> {
    let refuse = || anyhow!("That isn't one of this server's log files");
    let (folder, name) = rel.split_once('/').ok_or_else(refuse)?;
    if !wanted(folder, name) {
        return Err(refuse());
    }
    let path = safe_join_within(dir, rel).map_err(|_| refuse())?;
    match std::fs::symlink_metadata(&path) {
        Ok(m) if m.is_file() => Ok(path),
        Ok(_) => Err(refuse()),
        Err(e) if e.kind() == ErrorKind::NotFound => bail!("That log file is gone"),
        Err(e) => Err(e.into()),
    }
}

/// Newest first. A folder that's missing, or that leads out of the server's folder, is simply empty.
fn list_files(dir: &Path) -> Vec<LogFile> {
    let mut out = Vec::new();
    for folder in [LOGS, CRASHES] {
        let Ok(base) = safe_join_within(dir, folder) else { continue };
        let Ok(entries) = std::fs::read_dir(base) else { continue };
        for entry in entries.flatten() {
            let (Ok(name), Ok(kind)) = (entry.file_name().into_string(), entry.file_type()) else { continue };
            if !kind.is_file() || !wanted(folder, &name) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
            out.push(LogFile { path: format!("{folder}/{name}"), name, crash: folder == CRASHES, size: meta.len(), modified });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.path.cmp(&b.path)));
    out
}

fn is_crash(rel: &str) -> bool {
    rel.starts_with(CRASHES)
}

// ---------- Reading ----------

/// A file as raw text: unpacked if it starts like a gzip file, whatever it's called.
fn open(path: &Path) -> Result<Box<dyn Read + Send>> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 2];
    let got = file.read(&mut magic)?;
    file.seek(SeekFrom::Start(0))?;
    if got == 2 && magic == [0x1f, 0x8b] {
        // Multi: log rotation tools may append several gzip members to one file.
        Ok(Box::new(MultiGzDecoder::new(BufReader::new(file))))
    } else {
        Ok(Box::new(file))
    }
}

fn ansi() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[()][A-Za-z0-9]").unwrap())
}

/// Lines of a stream of bytes. It splits on the byte `\n`, which is never part of a multibyte UTF-8 character, so a
/// character that falls across two reads stays whole; bytes that aren't UTF-8 become U+FFFD.
struct Lines {
    reader: BufReader<Box<dyn Read + Send>>,
    buf: Vec<u8>,
    line: u64,
    read: u64,
    budget: u64,
    damaged: bool,
    capped: bool,
}

impl Lines {
    fn new(reader: Box<dyn Read + Send>, budget: u64) -> Self {
        Lines { reader: BufReader::with_capacity(64 * 1024, reader), buf: Vec::new(), line: 0, read: 0, budget, damaged: false, capped: false }
    }

    fn next(&mut self) -> Option<(u64, Cow<'_, str>)> {
        self.buf.clear();
        let mut cut = false;
        let mut any = false;
        loop {
            if self.damaged {
                break;
            }
            if self.read >= self.budget {
                self.capped = true;
                break;
            }
            let chunk = match self.reader.fill_buf() {
                Ok(chunk) => chunk,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.damaged = true;
                    break;
                }
            };
            if chunk.is_empty() {
                break;
            }
            any = true;
            let newline = chunk.iter().position(|b| *b == b'\n');
            let take = newline.unwrap_or(chunk.len());
            let room = MAX_LINE.saturating_sub(self.buf.len());
            self.buf.extend_from_slice(&chunk[..take.min(room)]);
            cut |= take > room;
            let used = take + newline.is_some() as usize;
            self.reader.consume(used);
            self.read += used as u64;
            if newline.is_some() {
                break;
            }
        }
        if !any {
            return None;
        }
        self.line += 1;
        Some((self.line, decode(&self.buf, self.line == 1, cut)))
    }
}

fn decode(raw: &[u8], first: bool, cut: bool) -> Cow<'_, str> {
    let mut bytes = raw.strip_suffix(b"\r").unwrap_or(raw);
    if first {
        bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    }
    if cut {
        // The cut may have split a character: that's not bad data, so it goes rather than showing as U+FFFD.
        if let Err(e) = std::str::from_utf8(bytes) {
            if e.error_len().is_none() {
                bytes = &bytes[..e.valid_up_to()];
            }
        }
    }
    let text = String::from_utf8_lossy(bytes);
    let text = if text.contains('\x1b') { Cow::Owned(ansi().replace_all(&text, "").into_owned()) } else { text };
    if cut {
        Cow::Owned(format!("{text}…"))
    } else {
        text
    }
}

/// Works out each line's level. Lines without one (a stack trace) belong to the line before.
struct Levels {
    crash: bool,
    last: Level,
}

impl Levels {
    fn new(crash: bool) -> Self {
        Levels { crash, last: Level::Info }
    }

    fn next(&mut self, text: &str) -> Level {
        if self.crash {
            // In a crash report the failure is the block from "Description:"; the rest is system details.
            if text.starts_with("Description:") {
                self.last = Level::Error;
            } else if text.starts_with("A detailed walkthrough") {
                self.last = Level::Info;
            }
            return self.last;
        }
        if let Some(level) = header_level(text) {
            self.last = level;
        }
        self.last
    }
}

/// The level in a line's header: `[12:00:00] [Server thread/WARN]: `, `[12:00:00 WARN]: `, `12:00:00 [WARNING] `,
/// `2026-10-31T12:00:00Z  WARN thing: `. The first one found wins, so a player typing `ERROR` in chat stays INFO.
fn header_level(text: &str) -> Option<Level> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?:^|[\[/ |])(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|SEVERE)(?:[\]: ]|$)").unwrap());
    let mut end = text.len().min(HEAD);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Some(match &re.captures(&text[..end])?[1] {
        "TRACE" | "DEBUG" => Level::Debug,
        "INFO" => Level::Info,
        "WARN" | "WARNING" => Level::Warn,
        _ => Level::Error,
    })
}

/// A page from line `from` (counted from 1), or the end of the file with 0.
fn read_page(path: &Path, crash: bool, from: u64, lines: usize, max_bytes: usize, budget: u64) -> Result<LogPage> {
    let mut reader = Lines::new(open(path)?, budget);
    let mut levels = Levels::new(crash);
    let mut page: VecDeque<Line> = VecDeque::new();
    let mut bytes = 0;
    let mut more = false;
    while let Some((line, text)) = reader.next() {
        let level = levels.next(&text);
        if line < from {
            continue;
        }
        if from > 0 && (page.len() == lines || bytes >= max_bytes) {
            more = true;
            break;
        }
        bytes += text.len();
        page.push_back(Line { line, text: text.into_owned(), level });
        if from == 0 {
            while page.len() > lines || bytes > max_bytes {
                let Some(gone) = page.pop_front() else { break };
                bytes -= gone.text.len();
            }
        }
    }
    Ok(LogPage { lines: page.into(), more, damaged: reader.damaged, capped: reader.capped })
}

/// Case-insensitive across Unicode, as plain text: nothing in `query` is a pattern.
fn matcher(query: &str) -> Result<Option<Regex>> {
    if query.trim().is_empty() {
        return Ok(None);
    }
    if query.len() > MAX_QUERY {
        bail!("That search is too long");
    }
    Ok(Some(RegexBuilder::new(&regex::escape(query)).case_insensitive(true).build()?))
}

/// Looks through `files` in order, stopping once it has `max_hits` (or `max_bytes`) and knows there are more.
fn search(files: &[(String, PathBuf)], needle: Option<&Regex>, problems: bool, max_hits: usize, max_bytes: usize, budget: u64, stop: &dyn Fn() -> bool) -> LogSearch {
    let mut out = LogSearch::default();
    let mut left = budget;
    let mut bytes = 0;
    for (rel, path) in files {
        if left == 0 {
            out.capped = true;
            break;
        }
        let Ok(reader) = open(path) else {
            out.damaged.push(rel.clone());
            continue;
        };
        let mut reader = Lines::new(reader, left);
        let mut levels = Levels::new(is_crash(rel));
        while let Some((line, text)) = reader.next() {
            if line % 4096 == 0 && stop() {
                return out;
            }
            let level = levels.next(&text);
            if (problems && level < Level::Warn) || needle.is_some_and(|re| !re.is_match(&text)) {
                continue;
            }
            if out.hits.len() >= max_hits || bytes >= max_bytes {
                out.more = true;
                return out;
            }
            bytes += text.len();
            out.hits.push(Hit { path: rel.clone(), line: Line { line, text: text.into_owned(), level } });
        }
        left -= reader.read.min(left);
        out.capped |= reader.capped;
        if reader.damaged {
            out.damaged.push(rel.clone());
        }
    }
    out
}

/// The newest search on this PC: starting one ends the one before, which nobody is waiting for any more.
static LATEST_SEARCH: AtomicU64 = AtomicU64::new(0);

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await?
}

/// Copies one where the player picks; the file stays as it is (a .gz stays packed).
async fn save(dir: &Path, rel: &str) -> Result<Option<String>> {
    let from = resolve(dir, rel)?;
    let name = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = Path::new(&name).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "*".into());
    let title = crate::tr!("app.saveTextTitle", "Save the file").text;
    let default = shell::downloads_dir().join(&name);
    let Some(to) = shell::save_file(&title, &default, &[("Log", &[ext.as_str()])]).await? else { return Ok(None) };
    tokio::fs::copy(&from, &to).await?;
    Ok(Some(to.to_string_lossy().into_owned()))
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    let id: String = args.get(0)?;
    store::get(&id).await?;
    let dir = store::dir(&id)?;
    match channel {
        "server:logFiles" => json(blocking(move || Ok(list_files(&dir))).await?),
        // [id, path, from line (0: the end of the file)]
        "server:logRead" => {
            let rel: String = args.get(1)?;
            let from: u64 = args.get::<Option<u64>>(2)?.unwrap_or(1);
            json(blocking(move || read_page(&resolve(&dir, &rel)?, is_crash(&rel), from, PAGE_LINES, MAX_RESULT, MAX_SCAN)).await?)
        }
        "server:logSearch" => {
            let q: Query = args.get(1)?;
            let needle = matcher(&q.query)?;
            if needle.is_none() && !q.problems {
                bail!("Type something to search for");
            }
            let ticket = LATEST_SEARCH.fetch_add(1, Ordering::SeqCst) + 1;
            let found = blocking(move || {
                let files: Vec<(String, PathBuf)> = match q.path {
                    Some(rel) => {
                        let path = resolve(&dir, &rel)?;
                        vec![(rel, path)]
                    }
                    None => list_files(&dir).into_iter().filter_map(|f| Some((f.path.clone(), resolve(&dir, &f.path).ok()?))).collect(),
                };
                Ok(search(&files, needle.as_ref(), q.problems, MAX_HITS, MAX_RESULT, MAX_SCAN, &|| LATEST_SEARCH.load(Ordering::SeqCst) != ticket))
            })
            .await?;
            json(found)
        }
        "server:logSave" => json(save(&dir, &args.get::<String>(1)?).await?),
        _ => bail!("Unknown channel {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("rc-logs-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(dir.join(LOGS)).unwrap();
            std::fs::create_dir_all(dir.join(CRASHES)).unwrap();
            Scratch(dir)
        }
        fn put(&self, rel: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(rel);
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn gz(bytes: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    fn page(path: &Path, from: u64) -> LogPage {
        read_page(path, false, from, PAGE_LINES, MAX_RESULT, MAX_SCAN).unwrap()
    }

    fn texts(p: &LogPage) -> Vec<&str> {
        p.lines.iter().map(|l| l.text.as_str()).collect()
    }

    fn find(files: &[(String, PathBuf)], q: &str, problems: bool) -> LogSearch {
        search(files, matcher(q).unwrap().as_ref(), problems, MAX_HITS, MAX_RESULT, MAX_SCAN, &|| false)
    }

    #[test]
    fn refuses_anything_but_a_listed_file() {
        let s = Scratch::new();
        s.put("logs/latest.log", b"x");
        s.put("server.properties", b"rcon.password=hunter2");
        s.put("logs/notes.txt", b"x");
        assert!(resolve(&s.0, "logs/latest.log").is_ok());
        for bad in [
            "../server.properties",
            "logs/../server.properties",
            "logs/../../server.properties",
            "logs\\..\\server.properties",
            "logs/..\\server.properties",
            "logs/sub/../latest.log",
            "logs//latest.log",
            "./logs/latest.log",
            "logs/./latest.log",
            "server.properties",
            "world/logs/latest.log",
            "LOGS/latest.log",
            "logs/notes.txt",
            "logs/",
            "logs/..",
            "crash-reports/..",
            "crash-reports/../server.properties",
            "crash-reports/.hidden",
            "/etc/passwd.log",
            "C:/Windows/win.log",
            "logs/C:/Windows/win.log",
            "logs/latest.log:secret.log",
            "logs/a\0.log",
            "",
        ] {
            assert!(resolve(&s.0, bad).is_err(), "{bad:?}");
        }
        assert!(resolve(&s.0, "logs/gone.log").unwrap_err().to_string().contains("gone"));
        std::fs::create_dir_all(s.0.join("logs/folder.log")).unwrap();
        assert!(resolve(&s.0, "logs/folder.log").is_err(), "a folder isn't a log");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_links_out_of_the_server() {
        let s = Scratch::new();
        let outside = Scratch::new();
        let secret = outside.put("logs/secret.log", b"token");
        std::os::unix::fs::symlink(&secret, s.0.join("logs/link.log")).unwrap();
        assert!(resolve(&s.0, "logs/link.log").is_err());
        assert!(list_files(&s.0).is_empty());
        std::fs::remove_dir_all(s.0.join(CRASHES)).unwrap();
        std::os::unix::fs::symlink(outside.0.join(LOGS), s.0.join(CRASHES)).unwrap();
        assert!(resolve(&s.0, "crash-reports/secret.log").is_err());
        assert!(list_files(&s.0).is_empty(), "a linked folder is not listed");
    }

    #[test]
    fn lists_only_logs_newest_first() {
        let s = Scratch::new();
        let old = s.put("logs/2026-10-01-1.log.gz", &gz(b"old"));
        s.put("logs/latest.log", b"now");
        s.put("logs/notes.txt", b"no");
        s.put("logs/LOUD.LOG", b"yes");
        s.put("crash-reports/crash-2026-10-30_server.txt", b"boom");
        s.put("server.log", b"not in the folder");
        File::options().write(true).open(&old).unwrap().set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(86_400)).unwrap();
        let list = list_files(&s.0);
        let mut paths: Vec<&str> = list.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths.pop(), Some("logs/2026-10-01-1.log.gz"), "the oldest last");
        paths.sort();
        assert_eq!(paths, ["crash-reports/crash-2026-10-30_server.txt", "logs/LOUD.LOG", "logs/latest.log"]);
        assert!(list.iter().find(|f| f.name.starts_with("crash")).unwrap().crash);
        assert_eq!(list_files(&s.0.join("nothing")).len(), 0);
    }

    #[test]
    fn reads_gzip_and_plain_the_same() {
        let s = Scratch::new();
        let text = "[12:00:00] [main/INFO]: one\r\n[12:00:01] [main/WARN]: two\n\nlast without newline";
        let plain = s.put("logs/a.log", text.as_bytes());
        let packed = s.put("logs/b.log.gz", &gz(text.as_bytes()));
        for path in [&plain, &packed] {
            let p = page(path, 1);
            assert_eq!(texts(&p), ["[12:00:00] [main/INFO]: one", "[12:00:01] [main/WARN]: two", "", "last without newline"]);
            assert_eq!(p.lines[3].line, 4);
            assert!(!p.more && !p.damaged);
        }
        // Named .gz but plain text, and the other way round.
        assert_eq!(page(&s.put("logs/c.log.gz", b"plain"), 1).lines[0].text, "plain");
        assert_eq!(page(&s.put("logs/d.log", &gz(b"packed")), 1).lines[0].text, "packed");
        assert!(page(&s.put("logs/e.log", b""), 1).lines.is_empty());
        // Two gzip members in one file.
        let mut two = gz(b"first\n");
        two.extend(gz(b"second\n"));
        assert_eq!(texts(&page(&s.put("logs/f.log.gz", &two), 1)), ["first", "second"]);
    }

    #[test]
    fn a_damaged_gzip_gives_what_it_can() {
        let s = Scratch::new();
        let body: String = (0..5000).map(|i| format!("line {i} {:x}\n", i * 2654435761u64)).collect();
        let whole = gz(body.as_bytes());
        let cut = s.put("logs/cut.log.gz", &whole[..whole.len() / 2]);
        assert_eq!(page(&cut, 1).lines[0].text, "line 0 0");
        // The damage shows once the reading gets there.
        let p = page(&cut, 0);
        assert!(p.damaged);
        assert!(!p.lines.is_empty());
        let junk = s.put("logs/junk.log.gz", &[0x1f, 0x8b, 0xff, 0xff, 1, 2, 3]);
        assert!(page(&junk, 1).damaged);
        let found = find(&[("logs/cut.log.gz".into(), cut), ("logs/junk.log.gz".into(), junk), ("logs/ok.log".into(), s.put("logs/ok.log", b"line 1 "))], "line 1 ", false);
        assert_eq!(found.damaged, ["logs/cut.log.gz", "logs/junk.log.gz"]);
        assert!(found.hits.iter().any(|h| h.path == "logs/ok.log"), "the search carries on past them");
    }

    #[test]
    fn multibyte_text_survives_every_split() {
        let s = Scratch::new();
        // Longer than the read buffer, so characters fall across reads (and, in the gzip, across blocks).
        let line = "Tiếng Việt 日本語 Привет 🎮 ".repeat(9000);
        let text = format!("{line}\nsecond ✓\n");
        assert!(text.len() > 256 * 1024);
        let path = s.put("logs/a.log.gz", &gz(text.as_bytes()));
        let p = page(&path, 1);
        assert_eq!(p.lines.len(), 2);
        let first = &p.lines[0].text;
        assert!(first.len() <= MAX_LINE + 3 && first.ends_with('…'), "a long line is cut");
        assert!(!first.contains('\u{FFFD}'), "and never inside a character");
        assert_eq!(p.lines[1].text, "second ✓");

        let exact = "é".repeat(MAX_LINE / 2 + 7);
        let got = page(&s.put("logs/b.log", exact.as_bytes()), 1);
        assert!(!got.lines[0].text.contains('\u{FFFD}'));
        // Bad bytes are replaced, and the good text around them stays.
        let bad = page(&s.put("logs/c.log", b"caf\xe9 ok \xf0\x9f\n\xe2\x9c\x93\n"), 1);
        assert_eq!(texts(&bad), ["caf\u{FFFD} ok \u{FFFD}", "✓"]);
    }

    #[test]
    fn cleans_what_shouldnt_show() {
        let s = Scratch::new();
        let p = page(&s.put("logs/a.log", b"\xef\xbb\xbf\x1b[33m[12:00:00 WARN]\x1b[0m: hi\r\n\xef\xbb\xbfsecond\n"), 1);
        assert_eq!(texts(&p), ["[12:00:00 WARN]: hi", "\u{feff}second"]);
        assert_eq!(p.lines[0].level, Level::Warn);
    }

    #[test]
    fn search_ignores_case_in_any_alphabet() {
        let s = Scratch::new();
        let path = s.put("logs/a.log.gz", &gz("[main/INFO]: LỖI Kết Nối\n[main/INFO]: ПРИВЕТ мир\n[main/INFO]: ΣΑΣ\n[main/INFO]: a.b(c)*\n[main/INFO]: axb\n".as_bytes()));
        let files = [("logs/a.log.gz".to_string(), path)];
        let line = |q: &str| find(&files, q, false).hits.iter().map(|h| h.line.line).collect::<Vec<_>>();
        assert_eq!(line("lỗi kẾT"), [1]);
        assert_eq!(line("привет МИР"), [2]);
        assert_eq!(line("σας"), [3]);
        assert_eq!(line("A.B(C)*"), [4], "nothing in a search is a pattern");
        assert_eq!(line("(?i)"), [] as [u64; 0]);
        assert_eq!(line("nothing"), [] as [u64; 0]);
        assert!(matcher("   ").unwrap().is_none());
        assert!(matcher(&"x".repeat(MAX_QUERY + 1)).is_err());
    }

    #[test]
    fn knows_a_lines_level() {
        for (line, want) in [
            ("[12:00:00] [Server thread/WARN]: Can't keep up!", Some(Level::Warn)),
            ("[12:00:00] [Server thread/ERROR]: oh no", Some(Level::Error)),
            ("[31Oct2026 12:00:00.123] [main/FATAL] [forge/]: bad", Some(Level::Error)),
            ("[12:00:00 WARN]: velocity", Some(Level::Warn)),
            ("[12:00:00 ERROR] [spark]: paper", Some(Level::Error)),
            ("12:00:00 [WARNING] bungee", Some(Level::Warn)),
            ("12:00:00 [SEVERE] bungee", Some(Level::Error)),
            ("2026-10-31T12:00:00.000Z  WARN pumpkin::net: slow", Some(Level::Warn)),
            ("[2026-10-31 12:00:00:123 ERROR] bedrock", Some(Level::Error)),
            ("[12:00:00] [Server thread/INFO]: <Bob> ERROR WARN everything is broken", Some(Level::Info)),
            ("[12:00:00] [Server thread/DEBUG]: x", Some(Level::Debug)),
            ("\tat net.minecraft.Foo.bar(Foo.java:1)", None),
            ("java.lang.NullPointerException: warning ignored", None),
            ("", None),
        ] {
            assert_eq!(header_level(line), want, "{line}");
        }
        // Past the header, or with a multibyte character on the edge of it.
        assert_eq!(header_level(&format!("{} WARN x", "é".repeat(HEAD))), None);
    }

    #[test]
    fn problems_include_stack_traces_and_crash_failures() {
        let s = Scratch::new();
        let log = s.put("logs/a.log", b"[1] [main/INFO]: fine\n[2] [main/ERROR]: boom\n\tat a.b(c)\nCaused by: x\n[3] [main/INFO]: fine\n\tnot a trace\n[4] [main/WARN]: hm\n");
        let crash = s.put("crash-reports/c.txt", b"---- Minecraft Crash Report ----\nTime: now\nDescription: Exception in server tick loop\n\njava.lang.Oops\n\tat a.b(c)\n\nA detailed walkthrough of the error\n-- System Details --\nJava: 21\n");
        let files = [("logs/a.log".to_string(), log), ("crash-reports/c.txt".to_string(), crash)];
        let found = find(&files, "", true);
        let got: Vec<(&str, u64)> = found.hits.iter().map(|h| (h.path.as_str(), h.line.line)).collect();
        assert_eq!(got, [("logs/a.log", 2), ("logs/a.log", 3), ("logs/a.log", 4), ("logs/a.log", 7), ("crash-reports/c.txt", 3), ("crash-reports/c.txt", 4), ("crash-reports/c.txt", 5), ("crash-reports/c.txt", 6), ("crash-reports/c.txt", 7)]);
        assert_eq!(found.hits[0].line.level, Level::Error);
        assert_eq!(found.hits[3].line.level, Level::Warn);
        let both = find(&files, "CAUSED", true);
        assert_eq!(both.hits.len(), 1);
        assert_eq!(find(&files, "fine", true).hits.len(), 0, "the filter applies to a text search too");
    }

    #[test]
    fn stops_at_the_limits_and_says_so() {
        let s = Scratch::new();
        let text: String = (1..=50).map(|i| format!("[main/INFO]: match {i}\n")).collect();
        let files = [("logs/a.log".to_string(), s.put("logs/a.log", text.as_bytes())), ("logs/b.log".to_string(), s.put("logs/b.log", text.as_bytes()))];
        let needle = matcher("match").unwrap();
        let some = search(&files, needle.as_ref(), false, 10, MAX_RESULT, MAX_SCAN, &|| false);
        assert_eq!((some.hits.len(), some.more), (10, true));
        assert_eq!(some.hits[9].line.text, "[main/INFO]: match 10");
        let exact = search(&files, needle.as_ref(), false, 100, MAX_RESULT, MAX_SCAN, &|| false);
        assert_eq!((exact.hits.len(), exact.more), (100, false), "exactly the limit isn't \"more\"");
        let few = search(&files, needle.as_ref(), false, 1000, 100, MAX_SCAN, &|| false);
        assert!(few.more && few.hits.len() < 10, "bytes returned are limited too");
        let spread = search(&files, needle.as_ref(), false, 1000, MAX_RESULT, text.len() as u64 + 10, &|| false);
        assert!(spread.capped);
        assert_eq!(spread.hits.len(), 51, "the read limit covers all the files together: all of one, a piece of a line of the next");
    }

    #[test]
    fn a_gzip_bomb_stops_at_the_read_limit() {
        let s = Scratch::new();
        let mut e = GzEncoder::new(Vec::new(), Compression::best());
        let zeros = vec![0u8; 1024 * 1024];
        (0..64).for_each(|_| e.write_all(&zeros).unwrap());
        let packed = e.finish().unwrap();
        assert!(packed.len() < 256 * 1024);
        let path = s.put("logs/bomb.log.gz", &packed);
        let p = read_page(&path, false, 1, PAGE_LINES, MAX_RESULT, 2 * 1024 * 1024).unwrap();
        assert!(p.capped);
        assert_eq!(p.lines.len(), 1, "one endless line");
        assert!(p.lines[0].text.len() <= MAX_LINE + 3);
    }

    #[test]
    fn pages_through_a_file_and_finds_its_end() {
        let s = Scratch::new();
        let text: String = (1..=25).map(|i| format!("[main/INFO]: line {i}\n")).collect();
        let path = s.put("logs/a.log.gz", &gz(text.as_bytes()));
        let first = read_page(&path, false, 1, 10, MAX_RESULT, MAX_SCAN).unwrap();
        assert_eq!((first.lines[0].line, first.lines.len(), first.more), (1, 10, true));
        let last = read_page(&path, false, 21, 10, MAX_RESULT, MAX_SCAN).unwrap();
        assert_eq!((last.lines[0].line, last.lines.len(), last.more), (21, 5, false));
        let exact = read_page(&path, false, 16, 10, MAX_RESULT, MAX_SCAN).unwrap();
        assert_eq!((exact.lines.len(), exact.more), (10, false));
        let end = read_page(&path, false, 0, 10, MAX_RESULT, MAX_SCAN).unwrap();
        assert_eq!((end.lines[0].line, end.lines[9].line, end.more), (16, 25, false));
        assert!(read_page(&path, false, 99, 10, MAX_RESULT, MAX_SCAN).unwrap().lines.is_empty());
        let levels = read_page(&s.put("logs/b.log", b"[main/ERROR]: x\n\tat a\n\tat b\n"), false, 3, 10, MAX_RESULT, MAX_SCAN).unwrap();
        assert_eq!(levels.lines[0].level, Level::Error, "a page in the middle of a trace still knows its level");
    }

    #[test]
    fn a_newer_search_ends_the_old_one() {
        let s = Scratch::new();
        let text = "[main/INFO]: x\n".repeat(10_000);
        let files = [("logs/a.log".to_string(), s.put("logs/a.log", text.as_bytes()))];
        let found = search(&files, matcher("x").unwrap().as_ref(), false, usize::MAX, usize::MAX, MAX_SCAN, &|| true);
        assert_eq!(found.hits.len(), 4095);
    }
}
