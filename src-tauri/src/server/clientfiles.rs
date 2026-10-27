//! The owner's own files of a published pack plan (clientplan.rs): mods and packs that aren't on Modrinth or
//! CurseForge stay on the owner's PC, and this launcher hands them to players through ResinCraft Relay.
//!
//! `server-list-files.json` keeps, per listing, the account it was published with and which file (by SHA-1) is which
//! path here. While the launcher runs, one relay connection per account (never a tunnel's, so a session ending
//! doesn't take the files down) offers them (`files_offer`) and answers players' downloads; it reconnects when it
//! drops. A file that changed or went away since publishing isn't fixed up here: players' launchers check the SHA-1.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use resincraft_relay::agent::FileSource;
use serde::{Deserialize, Serialize};

use crate::core::fsutil::{read_json, write_json};
use crate::core::paths;
use crate::player::accounts;

use super::relay;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Listing {
    /// The Microsoft account's UUID it was published with (the listing's owner).
    #[serde(default)]
    account: String,
    /// SHA-1 → path on this PC.
    #[serde(default)]
    files: HashMap<String, PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Owned {
    #[serde(default)]
    listings: HashMap<String, Listing>,
}

fn file() -> PathBuf {
    paths::root().join("server-list-files.json")
}

/// The running file servers (one task per account).
static SERVING: Mutex<Vec<tauri::async_runtime::JoinHandle<()>>> = Mutex::new(Vec::new());

/// Replaces what `listing` serves (an empty map stops serving it), then restarts the file servers.
pub async fn set(listing: &str, account: &str, files: HashMap<String, PathBuf>) -> Result<()> {
    let mut all: Owned = read_json(&file()).await.unwrap_or_default();
    if files.is_empty() {
        all.listings.remove(listing);
    } else {
        all.listings.insert(listing.to_string(), Listing { account: account.to_string(), files });
    }
    write_json(&file(), &all).await?;
    restart().await;
    Ok(())
}

/// At start: serve whatever was published from this PC.
pub async fn init() {
    restart().await;
}

async fn restart() {
    for task in SERVING.lock().unwrap().drain(..) {
        task.abort();
    }
    if relay::settings().is_none() {
        return;
    }
    let all: Owned = read_json(&file()).await.unwrap_or_default();
    let mut by_account: HashMap<String, HashMap<String, Listing>> = HashMap::new();
    for (id, l) in all.listings {
        by_account.entry(l.account.clone()).or_default().insert(id, l);
    }
    let mut tasks = SERVING.lock().unwrap();
    for (account, listings) in by_account {
        tasks.push(tauri::async_runtime::spawn(serve(account, listings)));
    }
}

/// Keeps `account`'s files offered while the launcher runs, reconnecting (with a growing pause) when it drops.
async fn serve(account: String, listings: HashMap<String, Listing>) {
    let paths: HashMap<(String, String), PathBuf> =
        listings.iter().flat_map(|(id, l)| l.files.iter().map(move |(sha, p)| ((id.clone(), sha.clone()), p.clone()))).collect();
    let paths = Arc::new(paths);
    let source: FileSource = {
        let paths = paths.clone();
        Arc::new(move |listing: &str, sha1: &str| paths.get(&(listing.to_string(), sha1.to_ascii_lowercase())).filter(|p| p.is_file()).cloned())
    };
    let mut pause = Duration::from_secs(15);
    loop {
        let ran = async {
            let Some(acc) = accounts::microsoft_account(Some(&account)).await.filter(|a| a.uuid.replace('-', "").eq_ignore_ascii_case(&account.replace('-', ""))) else {
                anyhow::bail!("the account isn't signed in here");
            };
            let mut conn = relay::connect_for_list(&acc).await?;
            if !conn.has_listings() {
                anyhow::bail!("the relay doesn't serve owner files");
            }
            conn.set_file_source(source.clone());
            for (id, l) in &listings {
                if let Err(e) = conn.offer_files(id, l.files.keys().cloned().collect()).await {
                    eprintln!("[client files] {id}: the relay didn't take the offer: {e}");
                }
            }
            eprintln!("[client files] serving {} file(s) for {} listing(s)", paths.len(), listings.len());
            let ended = conn.serve_files(|_| {}).await;
            anyhow::Ok(ended.to_string())
        }
        .await;
        match ran {
            // It ran: the connection dropped (relay restart, network); come back soon.
            Ok(why) => {
                eprintln!("[client files] connection ended: {why}");
                pause = Duration::from_secs(15);
            }
            Err(e) => {
                eprintln!("[client files] can't serve right now: {e}");
                pause = (pause * 2).min(Duration::from_secs(600));
            }
        }
        tokio::time::sleep(pause).await;
    }
}
