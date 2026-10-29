//! Other PCs' servers in this launcher's Servers tab: the account's own PCs with remote control on, and PCs whose
//! owners let this account in (protocol 9). One connection to ResinCraft Relay, signed in as the active Microsoft
//! account, carries the calls (the web console's methods, checked again by the other PC) and the live updates of the
//! PC that's open. The Servers page talks to it through `remoteView:*`; updates arrive as `remoteViewPush`
//! `{device, topic, data}` and `remoteViewGone` `{device}` events, which the page turns back into its own.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Result};
use resincraft_relay::agent::{ViewCommand, ViewEvent};
use resincraft_relay::proto::ViewDevice;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

use crate::core::events::emit;
use crate::core::router::Args;
use crate::player::accounts;
use crate::server::relay;

type Answer = Result<Value, String>;

/// The open connection: where requests go.
static CONN: Mutex<Option<mpsc::UnboundedSender<ViewCommand>>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);
static PENDING: Mutex<Option<HashMap<u64, oneshot::Sender<Answer>>>> = Mutex::new(None);
static DEVICES: Mutex<Vec<oneshot::Sender<Vec<ViewDevice>>>> = Mutex::new(Vec::new());
/// What's watched on each PC (sent again after reconnecting), and whose log the page follows.
static WATCHING: Mutex<Option<HashMap<u64, HashSet<String>>>> = Mutex::new(None);
static LOG_FOR: Mutex<Option<(u64, String)>> = Mutex::new(None);

/// The longest a call may wait for the other PC (making a server downloads one).
const CALL_SECS: u64 = 15 * 60;

fn pending() -> std::sync::MutexGuard<'static, Option<HashMap<u64, oneshot::Sender<Answer>>>> {
    PENDING.lock().unwrap()
}

/// The connection, opened (and signed in) when there's none.
async fn conn() -> Result<mpsc::UnboundedSender<ViewCommand>> {
    if let Some(tx) = CONN.lock().unwrap().as_ref().filter(|tx| !tx.is_closed()) {
        return Ok(tx.clone());
    }
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    let conn = relay::connect_for_list(&account).await?;
    if !conn.has_shares() {
        bail!("ResinCraft Relay can't open other PCs yet. Ask its admin to update it.");
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<ViewCommand>();
    // Watches from before a reconnect go out again first.
    for (device, topics) in WATCHING.lock().unwrap().iter().flat_map(|m| m.iter()) {
        for topic in topics {
            let _ = tx.send(ViewCommand::Watch { device: *device, topic: topic.clone(), on: true });
        }
    }
    *CONN.lock().unwrap() = Some(tx.clone());
    tauri::async_runtime::spawn(async move {
        let ended = conn.view(on_event, &mut rx).await;
        // Whatever was waiting hears so; the next request connects again.
        for (_, waiter) in pending().take().unwrap_or_default() {
            let _ = waiter.send(Err(relay::describe(&ended, "").to_string()));
        }
        DEVICES.lock().unwrap().clear();
        // `rx` goes now, so the sender in CONN reads as closed and the next request connects again.
        drop(rx);
    });
    Ok(tx)
}

fn on_event(e: ViewEvent) {
    match e {
        ViewEvent::Devices(list) => {
            for waiter in DEVICES.lock().unwrap().drain(..) {
                let _ = waiter.send(list.clone());
            }
        }
        ViewEvent::Result { id, result } => {
            if let Some(waiter) = pending().as_mut().and_then(|m| m.remove(&id)) {
                let _ = waiter.send(result);
            }
        }
        ViewEvent::Push { device, topic, data } => emit("remoteViewPush", json!({ "device": device, "topic": topic, "data": data })),
        ViewEvent::Gone { device } => {
            if let Some(m) = WATCHING.lock().unwrap().as_mut() {
                m.remove(&device);
            }
            emit("remoteViewGone", json!({ "device": device }));
        }
    }
}

fn watch(device: u64, topic: &str, on: bool, tx: &mpsc::UnboundedSender<ViewCommand>) {
    let changed = {
        let mut w = WATCHING.lock().unwrap();
        let topics = w.get_or_insert_with(HashMap::new).entry(device).or_default();
        if on { topics.insert(topic.to_string()) } else { topics.remove(topic) }
    };
    if changed {
        let _ = tx.send(ViewCommand::Watch { device, topic: topic.to_string(), on });
    }
}

/// Stops watching everything on every PC but `keep`.
fn unwatch_all_but(keep: Option<u64>, tx: &mpsc::UnboundedSender<ViewCommand>) {
    let gone: Vec<(u64, String)> = {
        let mut w = WATCHING.lock().unwrap();
        let m = w.get_or_insert_with(HashMap::new);
        let devices: Vec<u64> = m.keys().copied().filter(|d| Some(*d) != keep).collect();
        devices.into_iter().flat_map(|d| m.remove(&d).unwrap_or_default().into_iter().map(move |t| (d, t))).collect()
    };
    for (device, topic) in gone {
        let _ = tx.send(ViewCommand::Watch { device, topic, on: false });
    }
}

/// The PCs this account may open (never this one).
async fn devices() -> Result<Value> {
    let tx = conn().await?;
    let (w, r) = oneshot::channel();
    DEVICES.lock().unwrap().push(w);
    let _ = tx.send(ViewCommand::Devices);
    let list = tokio::time::timeout(Duration::from_secs(20), r)
        .await
        .map_err(|_| anyhow::anyhow!("ResinCraft Relay didn't answer in time"))?
        .map_err(|_| anyhow::anyhow!("The connection to ResinCraft Relay dropped. Try again."))?;
    let own = crate::remote::own_device();
    Ok(Value::Array(
        list.into_iter()
            .filter(|d| Some(d.id) != own)
            .map(|d| json!({ "id": d.id, "name": d.name, "appVersion": d.app_version, "since": d.since, "owner": d.owner, "ownerName": d.owner_name, "shared": d.shared }))
            .collect(),
    ))
}

/// One request to PC `device`, with the web console's method names (`server.start`).
async fn call(device: u64, method: &str, params: Value) -> Result<Value> {
    let tx = conn().await?;
    // The page follows the log it shows: watch that server's log topic, and only that one.
    if method == "server.log" {
        if let Some(id) = params.get(0).and_then(Value::as_str) {
            let previous = LOG_FOR.lock().unwrap().replace((device, id.to_string()));
            if previous.as_ref() != Some(&(device, id.to_string())) {
                if let Some((d, old)) = previous {
                    watch(d, &format!("log:{old}"), false, &tx);
                }
                watch(device, &format!("log:{id}"), true, &tx);
            }
        }
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (w, r) = oneshot::channel();
    pending().get_or_insert_with(HashMap::new).insert(id, w);
    if tx.send(ViewCommand::Call { id, device, method: method.to_string(), params }).is_err() {
        pending().as_mut().and_then(|m| m.remove(&id));
        bail!("The connection to ResinCraft Relay dropped. Try again.");
    }
    match tokio::time::timeout(Duration::from_secs(CALL_SECS), r).await {
        Ok(Ok(Ok(v))) => Ok(v),
        Ok(Ok(Err(e))) => bail!("{e}"),
        Ok(Err(_)) => bail!("The connection to ResinCraft Relay dropped. Try again."),
        Err(_) => {
            pending().as_mut().and_then(|m| m.remove(&id));
            bail!("The other PC didn't answer in time.")
        }
    }
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "remoteView:devices" => devices().await,
        // Opening a PC: its server list and tasks follow live; whatever else was open stops.
        "remoteView:open" => {
            let device = args.get::<u64>(0)?;
            let tx = conn().await?;
            unwatch_all_but(Some(device), &tx);
            *LOG_FOR.lock().unwrap() = None;
            watch(device, "servers", true, &tx);
            watch(device, "tasks", true, &tx);
            Ok(Value::Null)
        }
        "remoteView:call" => {
            let device = args.get::<u64>(0)?;
            let method = args.get::<String>(1)?;
            let params = args.get::<Option<Value>>(2)?.filter(Value::is_array).unwrap_or_else(|| json!([]));
            call(device, &method, params).await
        }
        // Back to this PC: nothing watched; the connection ends.
        "remoteView:close" => {
            *LOG_FOR.lock().unwrap() = None;
            if let Some(tx) = CONN.lock().unwrap().take() {
                unwatch_all_but(None, &tx);
            }
            *WATCHING.lock().unwrap() = None;
            Ok(Value::Null)
        }
        _ => bail!("Unknown request: {channel}"),
    }
}
