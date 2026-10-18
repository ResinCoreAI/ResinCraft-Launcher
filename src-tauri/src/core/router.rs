//! The single `ipc(channel, args)` entry point the UI calls, routed to each feature group.
//! Channel names match the UI's API (see src/shared/bridge.ts).

use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Positional arguments from the UI. Missing trailing arguments read as `null` (fine for `Option<T>`).
pub struct Args(Vec<Value>);

impl Args {
    pub fn new(v: Vec<Value>) -> Self {
        Args(v)
    }
    pub fn get<T: DeserializeOwned>(&self, i: usize) -> Result<T> {
        let v = self.0.get(i).cloned().unwrap_or(Value::Null);
        serde_json::from_value(v).with_context(|| format!("Argument {} is invalid", i + 1))
    }
    pub fn raw(&self, i: usize) -> Value {
        self.0.get(i).cloned().unwrap_or(Value::Null)
    }
}

/// Serialises a handler's return value.
pub fn json<T: Serialize>(v: T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

/// For handlers that return nothing.
pub fn done() -> Result<Value> {
    Ok(Value::Null)
}

pub async fn dispatch(channel: &str, args: Args) -> Result<Value> {
    let group = channel.split(':').next().unwrap_or_default();
    match (group, channel) {
        (_, "instances:launch" | "instances:kill" | "instances:running") => crate::game::handle(channel, args).await,
        ("app" | "settings" | "instances", _) => crate::app_cmds::handle(channel, args).await,
        ("versions" | "java", _) => crate::game::handle(channel, args).await,
        ("content", _) => crate::content::handle(channel, args).await,
        ("server", _) => crate::server::handle(channel, args).await,
        ("files", _) => crate::files::handle(channel, args).await,
        ("update", _) => crate::updater::handle(channel, args).await,
        ("accounts" | "notes" | "worlds" | "servers" | "screenshots", _) => crate::player::handle(channel, args).await,
        _ => bail!("Unknown request: {channel}"),
    }
}
