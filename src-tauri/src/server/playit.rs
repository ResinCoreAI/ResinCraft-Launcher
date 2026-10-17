//! playit.gg: a stable public address ("something.joinmc.link") for hosted servers, with no router setup and
//! no extra program. The engine runs playit's open-source agent (BSD-2-Clause, github.com/playit-cloud/playit-agent)
//! itself. The first time, the player links the launcher to a free playit account once in the browser
//! (playit.gg/claim/<code>, a guest account is enough); the agent's secret key is then kept (sealed like account
//! tokens) in `<dataDir>/servers/playit.json`, with one Minecraft Java tunnel per server, so each server keeps
//! its address across restarts. The agent runs while at least one server uses it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use playit_agent_core::network::origin_lookup::OriginLookup;
use playit_agent_core::network::tcp::tcp_settings::TcpSettings;
use playit_agent_core::network::udp::udp_settings::UdpSettings;
use playit_agent_core::playit_agent::{PlayitAgent, PlayitAgentSettings};
use playit_api_client::api::{
    AccountTunnelOriginCreate, AgentOrigin, AgentRunDataV1, AgentTunnelAttr, AgentTunnelConfig, ApiError, ApiErrorNoFail,
    ApiResponseError, ClaimAgentType, ClaimSetupError, ClaimSetupResponse, ReqClaimExchange, ReqClaimSetup, ReqTunnelsConfigV1,
    ReqTunnelsCreateV1, TunnelPortDetails, TunnelType,
};
use playit_api_client::PlayitApi;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::core::fsutil::{read_json, write_json};
use crate::player::accounts::{seal, unseal};

use super::store;

const API_BASE: &str = "https://api.playit.gg";
/// How long the player gets to approve the launcher in the browser.
const CLAIM_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// The agent's secret key, sealed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secret: Option<String>,
    /// Server id → its playit tunnel id.
    #[serde(default)]
    tunnels: HashMap<String, String>,
}

fn saved_file() -> std::path::PathBuf {
    store::root().join("playit.json")
}

async fn load() -> Saved {
    read_json(&saved_file()).await.unwrap_or_default()
}

async fn save(s: &Saved) -> Result<()> {
    write_json(&saved_file(), s).await
}

/// True once this launcher is linked to a playit account.
pub async fn linked() -> bool {
    load().await.secret.is_some()
}

/// Forgets the link (and the tunnels' ids); the next start asks to link again.
pub async fn unlink() -> Result<()> {
    stop_agent().await;
    save(&Saved::default()).await
}

struct Agent {
    api: PlayitApi,
    lookup: Arc<OriginLookup>,
    agent_id: uuid::Uuid,
    cancel: CancellationToken,
    /// Servers that use the agent right now.
    users: HashSet<String>,
}

static AGENT: Mutex<Option<Agent>> = Mutex::const_new(None);
/// One claim at a time: a second server that starts meanwhile waits for it and then uses the same link.
static CLAIM: Mutex<()> = Mutex::const_new(());

fn is_auth_error<C>(e: &ApiErrorNoFail<C>) -> bool {
    matches!(e, ApiErrorNoFail::ApiError(ApiResponseError::Auth(_)))
}

/// The saved secret, or a new one after the player approved the launcher in the browser. `on_claim` gets the
/// page to open; `wanted` says whether the server still needs it (it may stop while the player decides).
async fn secret(on_claim: &(dyn Fn(String) + Send + Sync), wanted: &(dyn Fn() -> bool + Send + Sync)) -> Result<String> {
    let _claiming = CLAIM.lock().await;
    if let Some(s) = load().await.secret {
        return unseal(Some(&s));
    }
    let code = hex::encode(&uuid::Uuid::new_v4().as_bytes()[..5]);
    let api = PlayitApi::create(API_BASE.into(), None);
    let mut shown = false;
    let deadline = Instant::now() + CLAIM_TIMEOUT;
    loop {
        if !wanted() {
            bail!("Stopped before playit.gg was linked");
        }
        if Instant::now() > deadline {
            bail!("playit.gg wasn't linked in time");
        }
        let req = ReqClaimSetup { code: code.clone(), agent_type: ClaimAgentType::SelfManaged, version: format!("ResinCraft {}", env!("CARGO_PKG_VERSION")) };
        match api.claim_setup(req).await {
            Ok(ClaimSetupResponse::UserAccepted) => break,
            Ok(ClaimSetupResponse::UserRejected) => bail!("Linking playit.gg was declined in the browser"),
            Ok(_) if !shown => {
                shown = true;
                on_claim(format!("https://playit.gg/claim/{code}"));
            }
            Ok(_) => {}
            Err(ApiError::Fail(ClaimSetupError::CodeExpired)) => bail!("The playit.gg link expired. Restart the server to try again"),
            Err(e) => eprintln!("[server] playit claim: {e}"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let key = loop {
        match api.claim_exchange(ReqClaimExchange { code: code.clone() }).await {
            Ok(k) => break k.secret_key,
            Err(ApiError::Fail(_)) if Instant::now() < deadline => tokio::time::sleep(Duration::from_secs(1)).await,
            Err(e) => bail!("playit.gg didn't finish linking: {e}"),
        }
    };
    let mut saved = load().await;
    saved.secret = Some(seal(&key));
    saved.tunnels.clear();
    save(&saved).await?;
    Ok(key)
}

async fn stop_agent() {
    if let Some(a) = AGENT.lock().await.take() {
        a.cancel.cancel();
    }
}

/// Starts the agent (or reuses the running one) and returns what's needed to manage tunnels.
async fn agent(on_claim: &(dyn Fn(String) + Send + Sync), wanted: &(dyn Fn() -> bool + Send + Sync)) -> Result<(PlayitApi, Arc<OriginLookup>, uuid::Uuid)> {
    if let Some(a) = AGENT.lock().await.as_ref() {
        return Ok((a.api.clone(), a.lookup.clone(), a.agent_id));
    }
    let mut retried = false;
    let (key, api, data) = loop {
        let key = secret(on_claim, wanted).await?;
        let api = PlayitApi::create(API_BASE.into(), Some(key.clone()));
        match api.v1_agents_rundata().await {
            Ok(d) => break (key, api, d),
            // The agent was deleted on playit.gg: link again, once.
            Err(e) if is_auth_error(&e) && !retried => {
                retried = true;
                save(&Saved::default()).await?;
            }
            Err(e) => bail!("playit.gg didn't answer: {e}"),
        }
    };
    let lookup = Arc::new(OriginLookup::default());
    lookup.update_from_run_data(&data).await;
    let settings = PlayitAgentSettings { api_url: API_BASE.into(), secret_key: key, tcp_settings: TcpSettings::default(), udp_settings: UdpSettings::default() };
    let runner = PlayitAgent::new(settings, lookup.clone()).await.map_err(|e| anyhow!("Couldn't connect to playit.gg ({e:?})"))?;
    let cancel = runner.cancellation_token();
    let mut slot = AGENT.lock().await;
    if let Some(a) = slot.as_ref() {
        // Another server started the agent meanwhile.
        cancel.cancel();
        return Ok((a.api.clone(), a.lookup.clone(), a.agent_id));
    }
    tokio::spawn(runner.run());
    *slot = Some(Agent { api: api.clone(), lookup: lookup.clone(), agent_id: data.agent_id, cancel, users: HashSet::new() });
    Ok((api, lookup, data.agent_id))
}

fn tunnel_config(port: u16) -> AgentTunnelConfig {
    AgentTunnelConfig {
        fields: vec![
            AgentTunnelAttr { name: "local_ip".into(), value: "127.0.0.1".into() },
            AgentTunnelAttr { name: "local_port".into(), value: port.to_string() },
        ],
    }
}

fn local_port(data: &AgentRunDataV1, tunnel: uuid::Uuid) -> Option<u16> {
    let t = data.tunnels.iter().find(|t| t.id == tunnel)?;
    t.agent_config.fields.iter().find(|f| f.name == "local_port").and_then(|f| f.value.parse().ok())
}

/// Shares the server listening on `port` through playit.gg and returns its public address.
pub async fn open(
    id: &str,
    port: u16,
    name: &str,
    on_claim: impl Fn(String) + Send + Sync,
    wanted: impl Fn() -> bool + Send + Sync,
) -> Result<String> {
    let (api, lookup, agent_id) = agent(&on_claim, &wanted).await?;
    if let Some(a) = AGENT.lock().await.as_mut() {
        a.users.insert(id.to_string());
    }
    let result = async {
        let mut saved = load().await;
        let data = api.v1_agents_rundata().await.map_err(|e| anyhow!("playit.gg didn't answer: {e}"))?;
        // The tunnel made for this server earlier, unless it was deleted on playit.gg.
        let exists = |t: &uuid::Uuid| data.tunnels.iter().any(|x| x.id == *t) || data.pending.iter().any(|p| p.id == *t);
        let known = saved.tunnels.get(id).and_then(|t| uuid::Uuid::parse_str(t).ok()).filter(exists);
        let tunnel = match known {
            Some(t) => {
                if local_port(&data, t).is_some_and(|p| p != port) {
                    api.v1_tunnels_config(ReqTunnelsConfigV1 { tunnel_id: t, new_agent_id: None, new_config: Some(tunnel_config(port)) })
                        .await
                        .map_err(|e| anyhow!("playit.gg didn't update the tunnel: {e}"))?;
                }
                t
            }
            None => {
                let label: String = format!("ResinCraft {name}").chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(40).collect();
                let req = ReqTunnelsCreateV1 {
                    ports: TunnelPortDetails::TunnelType(TunnelType::MinecraftJava),
                    origin: AccountTunnelOriginCreate::Agent(AgentOrigin { agent_id: Some(agent_id), config: tunnel_config(port) }),
                    enabled: true,
                    alloc: None,
                    name: Some(label.trim().to_string()),
                    firewall_id: None,
                };
                let created = api.v1_tunnels_create(req).await.map_err(|e| anyhow!("playit.gg didn't create a tunnel: {e}"))?;
                saved.tunnels.insert(id.to_string(), created.id.to_string());
                save(&saved).await?;
                created.id
            }
        };
        // A new tunnel takes a moment to get its address.
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let data = api.v1_agents_rundata().await.map_err(|e| anyhow!("playit.gg didn't answer: {e}"))?;
            lookup.update_from_run_data(&data).await;
            if let Some(t) = data.tunnels.iter().find(|t| t.id == tunnel) {
                if let Some(why) = &t.disabled_reason {
                    bail!("playit.gg disabled the tunnel: {why}");
                }
                if !t.display_address.is_empty() && local_port(&data, tunnel) == Some(port) {
                    return Ok(t.display_address.clone());
                }
            }
            if let Some(p) = data.pending.iter().find(|p| p.id == tunnel) {
                if Instant::now() > deadline {
                    bail!("playit.gg is still setting up the tunnel: {}", p.status_msg);
                }
            } else if Instant::now() > deadline {
                bail!("playit.gg didn't set up the tunnel in time");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    .await;
    if result.is_err() {
        close(id).await;
    }
    result
}

/// The server no longer needs playit; the agent stops when no server does. The tunnel (and its address) stays.
pub async fn close(id: &str) {
    let mut slot = AGENT.lock().await;
    let empty = slot.as_mut().is_some_and(|a| {
        a.users.remove(id);
        a.users.is_empty()
    });
    if empty {
        if let Some(a) = slot.take() {
            a.cancel.cancel();
        }
    }
}
