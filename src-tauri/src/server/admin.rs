//! The Admin page (relay protocol 8): for Microsoft accounts the relay lists as its admins, the server list's admin
//! tools (approve, pin, recommend, ban…) over the relay connection, unlocked with the dashboard's password.
//!
//! The relay checks every request itself; this side only keeps one unlocked connection, with the password in memory
//! (never on disk) so a dropped connection can be unlocked again without asking.

use anyhow::Result;
use resincraft_relay::agent::Connection;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::player::accounts;
use crate::{err_tr, tr};

use super::relay;

struct Unlocked {
    conn: Connection,
    uuid: String,
    password: String,
}

static ADMIN: Mutex<Option<Unlocked>> = Mutex::const_new(None);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    /// The active Microsoft account is one of the relay's admins.
    pub admin: bool,
    /// Unlocked with the password in this session.
    pub unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// Whether the active account may see the Admin page (asks the relay; false whenever it can't be asked).
pub async fn status() -> AdminStatus {
    let Some(account) = accounts::microsoft_account(None).await else { return AdminStatus { admin: false, unlocked: false, username: None } };
    if let Some(u) = ADMIN.lock().await.as_ref() {
        if same(&u.uuid, &account.uuid) {
            return AdminStatus { admin: true, unlocked: true, username: Some(account.username) };
        }
    }
    let admin = match relay::connect_for_list(&account).await {
        Ok(conn) => conn.has_listings() && conn.admin,
        Err(_) => false,
    };
    AdminStatus { admin, unlocked: false, username: Some(account.username) }
}

fn same(a: &str, b: &str) -> bool {
    a.replace('-', "").eq_ignore_ascii_case(&b.replace('-', ""))
}

fn denied() -> anyhow::Error {
    err_tr!("admin.denied", "The relay didn't accept that. Check the password, or wait a few minutes after several wrong tries.")
}

async fn open(password: &str) -> Result<Unlocked> {
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    let mut conn = relay::connect_for_list(&account).await?;
    if !conn.has_listings() || !conn.admin {
        return Err(err_tr!("admin.notAdmin", "This account isn't one of the relay's admins."));
    }
    match conn.admin_unlock(password).await {
        Ok(()) => Ok(Unlocked { conn, uuid: account.uuid, password: password.to_string() }),
        Err(e) if e.code() == Some(resincraft_relay::proto::ErrorCode::AdminDenied) => Err(denied()),
        Err(e) => Err(relay::describe(&e, "")),
    }
}

pub async fn unlock(password: &str) -> Result<AdminStatus> {
    let u = open(password).await?;
    *ADMIN.lock().await = Some(u);
    Ok(status().await)
}

pub async fn lock() {
    *ADMIN.lock().await = None;
}

/// One admin request; a dropped connection is opened and unlocked again once.
pub async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let mut guard = ADMIN.lock().await;
    let Some(u) = guard.as_mut() else { return Err(err_tr!("admin.locked", "Unlock the Admin page with the relay's password first.")) };
    let answer = match u.conn.admin_call(method, path, body.clone()).await {
        Ok(a) => a,
        Err(e) if e.retryable() => {
            let password = u.password.clone();
            *u = open(&password).await?;
            u.conn.admin_call(method, path, body).await.map_err(|e| relay::describe(&e, ""))?
        }
        Err(e) => {
            *guard = None;
            return Err(relay::describe(&e, ""));
        }
    };
    answer.map_err(|why| tr!("admin.refused", "The relay refused: {error}", error = why).into())
}
