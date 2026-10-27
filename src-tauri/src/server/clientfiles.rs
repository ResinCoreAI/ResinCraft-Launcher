//! The owner's own files of a published pack plan (clientplan.rs): mods and packs that aren't on Modrinth or
//! CurseForge stay on the owner's PC, and this launcher hands them to players through ResinCraft Relay.
//!
//! `server-list-files.json` keeps, per listing, which file (by SHA-1) is which path here. A file that changed or went
//! away since publishing isn't served (players' launchers check the SHA-1 anyway).

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::fsutil::{read_json, write_json};
use crate::core::paths;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Owned {
    /// Listing id → SHA-1 → path on this PC.
    #[serde(default)]
    listings: HashMap<String, HashMap<String, PathBuf>>,
}

fn file() -> PathBuf {
    paths::root().join("server-list-files.json")
}

/// Replaces what `listing` serves (an empty map stops serving it).
pub async fn set(listing: &str, files: HashMap<String, PathBuf>) -> Result<()> {
    let mut all: Owned = read_json(&file()).await.unwrap_or_default();
    if files.is_empty() {
        all.listings.remove(listing);
    } else {
        all.listings.insert(listing.to_string(), files);
    }
    write_json(&file(), &all).await
}

/// Every listing this PC serves files for, with their SHA-1s.
pub async fn offers() -> Vec<(String, Vec<String>)> {
    let all: Owned = read_json(&file()).await.unwrap_or_default();
    all.listings.into_iter().map(|(l, f)| (l, f.into_keys().collect())).collect()
}

/// Where `sha1` of `listing` is on this PC, if it's one of its files.
pub async fn path(listing: &str, sha1: &str) -> Option<PathBuf> {
    let all: Owned = read_json(&file()).await.unwrap_or_default();
    all.listings.get(listing)?.get(&sha1.to_ascii_lowercase()).cloned()
}
