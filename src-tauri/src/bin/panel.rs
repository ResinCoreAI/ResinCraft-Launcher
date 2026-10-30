//! ResinCraft Panel: the launcher's server engine with no window, for Linux machines without a desktop. It hosts the
//! servers and connects to ResinCraft Relay like a PC with remote control on, so the launcher's Servers page and the web
//! console run them from anywhere.
//!
//!   resincraft-panel login     sign in with a Microsoft account (a code to enter on another device)
//!   resincraft-panel accounts  the accounts signed in here
//!   resincraft-panel servers   the servers here
//!   resincraft-panel run       host: start the service (systemd runs this)
//!
//! The data folder is $RESINCRAFT_PANEL_DATA, or /var/lib/resincraft-panel.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use resincraft_lib::core::{ctx, events, router, settings, Args};
use resincraft_lib::{player, remote, server};
use serde_json::{json, Value};

const USAGE: &str = "usage: resincraft-panel <run|login|accounts|servers>";
/// How long the servers get to save and stop when the service stops.
const STOP_GRACE: Duration = Duration::from_secs(60);

fn data_dir() -> PathBuf {
    std::env::var_os("RESINCRAFT_PANEL_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/var/lib/resincraft-panel"))
}

async fn call(channel: &str, args: Vec<Value>) -> Result<Value> {
    router::dispatch(channel, Args::new(args)).await
}

/// The device-code sign-in: the code shows here, and is entered in a browser anywhere.
async fn login() -> Result<()> {
    let mut seen = events::tap();
    let code = player::accounts::begin_microsoft_login().await?;
    println!("Open {} on any device and enter: {}", code.verification_uri, code.user_code);
    println!("Waiting for you to sign in…");
    loop {
        let (channel, payload) = seen.recv().await.context("sign-in stopped")?;
        if channel != "login" {
            continue;
        }
        return match payload["state"].as_str() {
            Some("success") => {
                println!("Signed in as {}.", payload["account"]["username"].as_str().unwrap_or("your account"));
                Ok(())
            }
            _ => bail!("{}", payload["error"].as_str().unwrap_or("Sign-in failed")),
        };
    }
}

async fn accounts() -> Result<()> {
    let list = call("accounts:list", Vec::new()).await?;
    for a in list.as_array().into_iter().flatten() {
        println!("{}", a["username"].as_str().unwrap_or("?"));
    }
    Ok(())
}

async fn servers() -> Result<()> {
    let list = call("server:list", Vec::new()).await?;
    for s in list.as_array().into_iter().flatten() {
        println!("{}\t{}\t{} {}\tport {}", s["id"].as_str().unwrap_or("?"), s["state"].as_str().unwrap_or("?"), s["type"].as_str().unwrap_or("?"), s["mcVersion"].as_str().unwrap_or(""), s["port"]);
    }
    Ok(())
}

/// Hosts until the service is told to stop, then lets the servers save.
async fn run() -> Result<()> {
    if server::relay::settings().is_none() {
        bail!("This build has no ResinCraft Relay address, so nothing could reach it.");
    }
    if player::accounts::microsoft_account(None).await.is_none() {
        bail!("Sign in first: resincraft-panel login");
    }
    // Being reachable is the point of the panel.
    if !settings::get().await.remote_control {
        settings::set(json!({ "remoteControl": true })).await?;
    }
    remote::init().await;
    tokio::spawn(resincraft_lib::core::autobackup::init());
    tokio::spawn(server::clientfiles::init());
    tokio::spawn(server::discordbot::init());
    println!("ResinCraft Panel is running. Open it from the launcher's Servers page or the web console.");
    wait_for_stop().await;
    println!("Stopping the servers…");
    server::runtime::shutdown_all(STOP_GRACE).await;
    Ok(())
}

async fn wait_for_stop() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = term.recv() => return,
                _ = tokio::signal::ctrl_c() => return,
            }
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

/// The engine's request handler is one big future: more stack than a thread gets by default.
const STACK: usize = 64 * 1024 * 1024;

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(STACK).build().expect("Tokio runtime");
    let main = std::thread::Builder::new().stack_size(STACK).spawn(move || runtime.block_on(panel())).expect("main thread");
    let _ = main.join();
}

async fn panel() {
    let command = std::env::args().nth(1).unwrap_or_default();
    let data = data_dir();
    if let Err(e) = std::fs::create_dir_all(&data) {
        eprintln!("Can't use {}: {e}", data.display());
        std::process::exit(1);
    }
    ctx::init(data, None);
    let done = match command.as_str() {
        "run" => run().await,
        "login" => login().await,
        "accounts" => accounts().await,
        "servers" => servers().await,
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = done {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
