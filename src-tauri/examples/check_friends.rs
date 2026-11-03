//! Real two-client check of "Play with friends", through the launcher's own requests. Run the two sides as separate
//! processes, like two computers:
//!   cargo run --example check_friends -- host <instance> <name> <code file> [world]   an offline profile hosts (in <world>
//!                                                                                 if given); open to LAN by hand
//!   cargo run --example check_friends -- join <instance> <code file> [--offline <name>]
//!   cargo run --example check_friends -- login        a Microsoft account for `join` (RESINCRAFT_MS_CLIENT_ID in the environment)
//!   cargo run --example check_friends -- logout       removes it again
//! `host` ends when <code file>.stop appears: it stops the invite, then says whether the port still answers.

use std::path::{Path, PathBuf};
use std::time::Duration;

use resincraft_lib::core::router::{dispatch, Args};
use resincraft_lib::core::{ctx, settings};
use resincraft_lib::game::launch::{self, LaunchHooks, Target};
use resincraft_lib::player::accounts::{self, LaunchAuth};
use serde_json::{json, Value};

async fn call(channel: &str, args: Vec<Value>) -> anyhow::Result<Value> {
    dispatch(channel, Args::new(args)).await
}

fn offline(name: &str) -> LaunchAuth {
    LaunchAuth {
        account_id: format!("offline-{name}"),
        username: name.to_string(),
        uuid: accounts::offline_uuid(name),
        access_token: "0".into(),
        user_type: "legacy",
        xuid: None,
    }
}

async fn start(instance: &str, auth: LaunchAuth, target: Option<Target>) {
    let settings = settings::get().await;
    let hooks = LaunchHooks { on_started: None, on_exit: Some(Box::new(|code| println!("game exited: {code:?}"))) };
    launch::launch_instance_to(instance, &settings, auth, hooks, target.as_ref()).await.expect("launch");
}

async fn host(instance: &str, name: &str, code_file: &Path, world: Option<&str>) {
    start(instance, offline(name), world.map(|w| Target::World(w.to_string()))).await;
    println!("game starting: open a world to LAN");
    let run_id = loop {
        let s = call("friends:state", vec![]).await.unwrap();
        if let Some(w) = s["worlds"].as_array().and_then(|w| w.first()) {
            println!("LAN world seen: {w}");
            break w["runId"].as_str().unwrap().to_string();
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    let port = call("friends:state", vec![]).await.unwrap()["worlds"][0]["port"].as_u64().unwrap();
    let s = call("friends:invite", vec![json!(run_id)]).await.expect("invite");
    let invite = &s["invite"];
    println!("invite: {invite}");
    std::fs::write(code_file, invite["code"].as_str().unwrap()).unwrap();
    let stop = code_file.with_extension("stop");
    let mut last = 0;
    while !stop.exists() {
        let s = call("friends:state", vec![]).await.unwrap();
        let now = s["invite"]["connected"].as_u64().unwrap_or(0);
        if now != last {
            println!("connected: {now}");
            last = now;
        }
        if s["invite"].is_null() {
            println!("invite ended: {}", s["ended"]);
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let tunnel = invite["port"].as_u64().unwrap() as u16;
    call("friends:stopInvite", vec![]).await.unwrap();
    println!("stopped. tunnel port {tunnel} open: {}", tokio::net::TcpStream::connect(("127.0.0.1", tunnel)).await.is_ok());
    println!("LAN world port {port} still open (the game keeps it): {}", tokio::net::TcpStream::connect(("127.0.0.1", port as u16)).await.is_ok());
    let _ = call("instances:kill", vec![json!(instance)]).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
}

async fn join(instance: &str, code_file: &Path, offline_name: Option<&str>) {
    while !code_file.exists() {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let code = std::fs::read_to_string(code_file).unwrap();
    println!("code length {}", code.len());
    let joined = call("friends:open", vec![json!(code)]).await.expect("open");
    println!("joined: {joined}");
    match offline_name {
        Some(name) => {
            // Past the launcher's refusal, to see what the world itself does with an offline profile.
            start(instance, offline(name), Some(Target::Server(joined["address"].as_str().unwrap().to_string()))).await;
        }
        None => {
            call("friends:join", vec![json!({ "instanceId": instance })]).await.expect("join");
        }
    }
    println!("game starting");
    let stop = code_file.with_extension("stop-join");
    while !stop.exists() {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let _ = call("instances:kill", vec![json!(instance)]).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
}

async fn microsoft() -> Vec<Value> {
    let list = call("accounts:list", vec![]).await.unwrap();
    list.as_array().unwrap().iter().filter(|a| a["type"] != "offline").cloned().collect()
}

async fn login() {
    let code = call("accounts:beginMicrosoft", vec![]).await.expect("sign-in");
    println!("sign in at {} with the code {}", code["verificationUri"].as_str().unwrap_or_default(), code["userCode"].as_str().unwrap_or_default());
    let account = loop {
        if let Some(a) = microsoft().await.into_iter().next() {
            break a;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    call("accounts:setActive", vec![account["id"].clone()]).await.unwrap();
    println!("signed in: {} ({})", account["username"], account["type"]);
}

async fn logout() {
    for a in microsoft().await {
        call("accounts:remove", vec![a["id"].clone()]).await.unwrap();
    }
    let list = call("accounts:list", vec![]).await.unwrap();
    if let Some(a) = list.as_array().unwrap().first() {
        call("accounts:setActive", vec![a["id"].clone()]).await.unwrap();
    }
    println!("accounts left: {}", list.as_array().unwrap().len());
}

fn main() {
    // Launching is a deep future for a debug build's main thread.
    let run = || tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(run());
    std::thread::Builder::new().stack_size(64 << 20).spawn(run).unwrap().join().unwrap();
}

async fn run() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap().join("ResinCraft/.dev-data");
    ctx::init(std::env::var_os("RC_DATA").map(PathBuf::from).unwrap_or(data), None);
    let a: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    match a[..] {
        ["host", instance, name, file] => host(instance, name, Path::new(file), None).await,
        ["host", instance, name, file, world] => host(instance, name, Path::new(file), Some(world)).await,
        ["join", instance, file] => join(instance, Path::new(file), None).await,
        ["join", instance, file, "--offline", name] => join(instance, Path::new(file), Some(name)).await,
        ["login"] => login().await,
        ["logout"] => logout().await,
        _ => eprintln!("see the top of this file"),
    }
}
