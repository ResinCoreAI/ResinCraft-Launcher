//! The relay's small database: one JSON file with name reservations, session tokens (hashed), bans and
//! monthly traffic. Kept in memory, written atomically (temp file + rename) when something changed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub names: BTreeMap<String, NameEntry>,
    /// Keyed by the SHA-256 (hex) of the session token; the token itself is never stored.
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionEntry>,
    #[serde(default)]
    pub bans: Bans,
    /// Traffic per account (UUID) for the current month.
    #[serde(default)]
    pub usage: BTreeMap<String, UsageEntry>,
    /// Settings changed in the admin dashboard; they win over relay.toml until reset there.
    #[serde(default)]
    pub overrides: Overrides,
    /// The admin's on/off switch.
    #[serde(default)]
    pub service: ServiceSetting,
    /// Accounts (UUID) the admin gave unlimited use: no data cap, no session time limit.
    #[serde(default)]
    pub unlimited: BTreeMap<String, UnlimitedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnlimitedEntry {
    /// The account's Minecraft name when it was set (for the dashboard).
    #[serde(default)]
    pub name: String,
    pub since: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceSetting {
    pub state: crate::proto::ServiceState,
    /// Shown to launchers ("Maintenance until 20:00").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default)]
    pub since: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Overrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<super::config::LimitsConfig>,
    /// Extra reserved names (replaces [auth] reserved_names when set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserved_names: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameEntry {
    /// Minecraft UUID (dashed, lowercase).
    pub owner: String,
    /// The owner's Minecraft name when last seen.
    pub owner_name: String,
    pub created: u64,
    pub last_used: u64,
    /// The relay's UDP port this name had as a Bedrock server (it gets the same one next time when it's free).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bedrock_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub uuid: String,
    pub username: String,
    pub created: u64,
    pub expires: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bans {
    #[serde(default)]
    pub uuids: BTreeMap<String, BanEntry>,
    #[serde(default)]
    pub ips: BTreeMap<String, BanEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanEntry {
    #[serde(default)]
    pub reason: String,
    pub at: u64,
    /// When the ban ends (Unix seconds); None: never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<u64>,
}

impl BanEntry {
    pub fn active(&self, now: u64) -> bool {
        self.expires.is_none_or(|e| e > now)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageEntry {
    pub month: String,
    pub bytes: u64,
    /// Added to the cap this month: the host's top-ups and the admin's "Add data".
    #[serde(default, skip_serializing_if = "is_zero")]
    pub extra_bytes: u64,
    /// Top-ups the host took this month.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub topups: u32,
}

fn is_zero<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

impl UsageEntry {
    /// Starts over when the month changed.
    pub fn for_month(&mut self, month: &str) -> &mut Self {
        if self.month != month {
            *self = UsageEntry { month: month.to_string(), ..UsageEntry::default() };
        }
        self
    }
}

pub struct Store {
    path: PathBuf,
    data: Mutex<Data>,
    dirty: AtomicBool,
}

impl Store {
    /// Loads the file (a missing file is an empty database; a broken one is an error, never overwritten).
    pub fn load(path: &Path) -> Result<Store> {
        let data = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("{} is damaged; fix or restore it from a backup", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(e) => return Err(e).with_context(|| format!("couldn't read {}", path.display())),
        };
        Ok(Store { path: path.to_path_buf(), data: Mutex::new(data), dirty: AtomicBool::new(false) })
    }

    pub fn read<R>(&self, f: impl FnOnce(&Data) -> R) -> R {
        f(&self.data.lock().unwrap())
    }

    /// Changes the data; it's written on the next save.
    pub fn write<R>(&self, f: impl FnOnce(&mut Data) -> R) -> R {
        let r = f(&mut self.data.lock().unwrap());
        self.dirty.store(true, Ordering::Relaxed);
        r
    }

    /// Writes the file if anything changed since the last save.
    pub async fn save(&self) -> Result<()> {
        if !self.dirty.swap(false, Ordering::Relaxed) {
            return Ok(());
        }
        let bytes = self.read(serde_json::to_vec_pretty)?;
        let path = self.path.clone();
        let result = tokio::task::spawn_blocking(move || write_atomic(&path, &bytes)).await?;
        if result.is_err() {
            self.dirty.store(true, Ordering::Relaxed);
        }
        result
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("couldn't create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).with_context(|| format!("couldn't write {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).with_context(|| format!("couldn't replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("resincraft-relay-store-{}", std::process::id()));
        let path = dir.join("relay.json");
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::load(&path).unwrap();
        store.write(|d| {
            d.names.insert("smp".into(), NameEntry { owner: "u".into(), owner_name: "P".into(), created: 1, last_used: 2, bedrock_port: None });
        });
        store.save().await.unwrap();
        let again = Store::load(&path).unwrap();
        assert_eq!(again.read(|d| d.names["smp"].owner.clone()), "u");
        std::fs::write(&path, b"{ broken").unwrap();
        assert!(Store::load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
