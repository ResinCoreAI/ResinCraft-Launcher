//! ResinCraft Panel: the launcher's server engine with no window, for Linux machines without a desktop. It hosts the
//! servers with no account at all. Signing in with Microsoft adds ResinCraft's services: it then connects to ResinCraft
//! Relay like a PC with remote control on, so the launcher's Servers page and the web console run them from anywhere.
//!
//!   resincraft-panel run                      host: start the service (systemd runs this)
//!   resincraft-panel servers                  the servers here
//!   resincraft-panel start|stop <id>          starts or stops a server
//!   resincraft-panel call <channel> [json…]   any engine request, e.g. call server:list
//!   resincraft-panel full-control on|off      lets the owner use files and Java settings from afar (off by default)
//!   resincraft-panel containers on|off        hosts servers in Docker or Podman from their next start (off by default)
//!   resincraft-panel web <domain> <email>     a website on your domain, HTTPS from Let's Encrypt (port 443 must be open)
//!   resincraft-panel web <domain> --cert <file> --key <file> [--port <port>]   the same with your own certificate
//!   resincraft-panel web password             a new website password (the old one stops working)
//!   resincraft-panel web off                  closes the website
//!   resincraft-panel login                    sign in with Microsoft (a code to enter on another device)
//!   resincraft-panel accounts                 the accounts signed in here
//!
//! Commands reach the running service through panel.sock in the data folder (only its user can open it).
//!
//! The data folder is $RESINCRAFT_PANEL_DATA, or /var/lib/resincraft-panel.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use resincraft_lib::core::{ctx, events, router, settings, Args};
use resincraft_lib::{panelweb, player, remote, server};
use serde_json::{json, Value};

const USAGE: &str = "usage: resincraft-panel <run|servers|start|stop|call|full-control|containers|web|login|accounts>";
/// How long the servers get to save and stop when the service stops.
const STOP_GRACE: Duration = Duration::from_secs(60);
/// Tells the running service that web.json changed.
const WEB_APPLY: &str = "panel:webApply";

fn data_dir() -> PathBuf {
    std::env::var_os("RESINCRAFT_PANEL_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/var/lib/resincraft-panel"))
}

fn socket() -> PathBuf {
    data_dir().join("panel.sock")
}

/// Answers one request per line, `{"channel": …, "args": […]}`, with `{"ok": …}` or `{"error": …}`.
#[cfg(unix)]
async fn serve_socket() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let path = socket();
    let _ = std::fs::remove_file(&path);
    let listener = tokio::net::UnixListener::bind(&path).context("Couldn't open the control socket")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let req: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                let channel = req["channel"].as_str().unwrap_or_default().to_string();
                let args = req["args"].as_array().cloned().unwrap_or_default();
                let answered = if channel == WEB_APPLY {
                    panelweb::apply().await;
                    Ok(Value::Null)
                } else {
                    router::dispatch(&channel, Args::new(args)).await
                };
                let answer = match answered {
                    Ok(v) => json!({ "ok": v }),
                    Err(e) => json!({ "error": format!("{e:#}") }),
                };
                if write.write_all(format!("{answer}\n").as_bytes()).await.is_err() {
                    break;
                }
            }
        });
    }
}

/// One request: to the running service when there is one, else here (only for what needs no running server).
async fn call(channel: &str, args: Vec<Value>) -> Result<Value> {
    #[cfg(unix)]
    if let Ok(stream) = tokio::net::UnixStream::connect(socket()).await {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (read, mut write) = stream.into_split();
        write.write_all(format!("{}\n", json!({ "channel": channel, "args": args })).as_bytes()).await?;
        let line = BufReader::new(read).lines().next_line().await?.context("The service closed the connection")?;
        let answer: Value = serde_json::from_str(&line)?;
        return match answer["error"].as_str() {
            Some(e) => bail!("{e}"),
            None => Ok(answer["ok"].clone()),
        };
    }
    router::dispatch(channel, Args::new(args)).await
}

/// `call <channel> [json…]`: each argument is JSON, or plain text when it isn't.
async fn call_command(words: &[String]) -> Result<()> {
    let [channel, rest @ ..] = words else { bail!("usage: resincraft-panel call <channel> [json…]") };
    let args = rest.iter().map(|w| serde_json::from_str(w).unwrap_or_else(|_| Value::String(w.clone()))).collect();
    println!("{}", serde_json::to_string_pretty(&call(channel, args).await?)?);
    Ok(())
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

/// Takes the new website settings live; with no service running they're read at its next start.
async fn apply_web() {
    #[cfg(unix)]
    if tokio::net::UnixStream::connect(socket()).await.is_ok() {
        if let Err(e) = call(WEB_APPLY, Vec::new()).await {
            eprintln!("{e:#}");
        }
    }
}

/// `--name value` from the words after `web <domain>`.
fn flag<'a>(words: &'a [String], name: &str) -> Option<&'a str> {
    words.iter().position(|w| w == name).and_then(|i| words.get(i + 1)).map(String::as_str)
}

async fn web(words: &[String]) -> Result<()> {
    match words {
        [word] if word == "off" => {
            panelweb::disable().await?;
            println!("The website is closed.");
        }
        [word] if word == "password" => {
            let password = panelweb::new_password().await?;
            println!("New website password (shown only now): {password}");
        }
        [domain, rest @ ..] if !rest.is_empty() => {
            let cert = flag(rest, "--cert").unwrap_or_default();
            let key = flag(rest, "--key").unwrap_or_default();
            let email = if rest[0].starts_with("--") { "" } else { rest[0].as_str() };
            let port = flag(rest, "--port").map(str::parse).transpose().context("That port isn't a number")?;
            if let Some(password) = panelweb::enable(domain, email, port, cert, key).await? {
                println!("Website password (shown only now): {password}");
            }
            let config = panelweb::config().await;
            println!("Point {} at this machine in DNS and open port {} in its firewall.", config.domain, config.port);
        }
        _ => bail!("usage: resincraft-panel web <domain> <email> | <domain> --cert <file> --key <file> [--port <port>] | password | off"),
    }
    apply_web().await;
    Ok(())
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
    // Signed in, the panel is reachable through ResinCraft's services; without an account it just hosts.
    let signed_in = player::accounts::microsoft_account(None).await.is_some();
    if signed_in && !settings::get().await.remote_control {
        settings::set(json!({ "remoteControl": true })).await?;
    }
    remote::init().await;
    tokio::spawn(resincraft_lib::core::autobackup::init());
    tokio::spawn(server::clientfiles::init());
    tokio::spawn(server::schedule::init());
    tokio::spawn(server::discordbot::init());
    #[cfg(unix)]
    tokio::spawn(async {
        if let Err(e) = serve_socket().await {
            eprintln!("{e:#}");
        }
    });
    panelweb::apply().await;
    if signed_in {
        println!("ResinCraft Panel is running. Open it from the launcher's Servers page or the web console.");
    } else {
        println!("ResinCraft Panel is running without an account. Sign in (resincraft-panel login) to use ResinCraft Relay.");
    }
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
    let words: Vec<String> = std::env::args().skip(1).collect();
    let command = words.first().cloned().unwrap_or_default();
    let id = || words.get(1).cloned().map(Value::String).ok_or_else(|| anyhow::anyhow!("Which server? See: resincraft-panel servers"));
    let data = data_dir();
    if let Err(e) = std::fs::create_dir_all(&data) {
        eprintln!("Can't use {}: {e}", data.display());
        std::process::exit(1);
    }
    ctx::init(data, None);
    let done = match command.as_str() {
        "run" => run().await,
        "login" => login().await,
        "web" => web(&words[1..]).await,
        "accounts" => accounts().await,
        "servers" => servers().await,
        "start" | "stop" => match id() {
            Ok(id) => call(&format!("server:{command}"), vec![id]).await.map(|_| ()),
            Err(e) => Err(e),
        },
        "call" => call_command(&words[1..]).await,
        "full-control" => match words.get(1).map(String::as_str) {
            Some(word @ ("on" | "off")) => call("settings:set", vec![json!({ "remoteFullControl": word == "on" })]).await.map(|_| ()),
            _ => Err(anyhow::anyhow!("usage: resincraft-panel full-control on|off")),
        },
        "containers" => match words.get(1).map(String::as_str) {
            Some(word @ ("on" | "off")) => call("settings:set", vec![json!({ "containers": word == "on" })]).await.map(|_| ()),
            _ => Err(anyhow::anyhow!("usage: resincraft-panel containers on|off")),
        },
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
