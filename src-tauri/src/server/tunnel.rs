//! Zero-setup internet access through a free public TCP relay, for when the router can't open a port
//! (no UPnP, carrier-grade NAT…). Speaks the bore protocol (github.com/ekzhang/bore) to bore.pub, or to a
//! bore server the player picked (e.g. a nearer, self-hosted one: `bore server --secret …`). The launcher
//! keeps an outgoing control connection; for each player that connects to <relay>:<port>, the relay asks for
//! a new outgoing connection, which is then joined to the local server. Nothing needs to be opened or installed.
//!
//! Frames are JSON terminated by a NUL byte. Client → relay: {"Hello":0} (any port), {"Accept":"<uuid>"},
//! {"Authenticate":"<hex>"}. Relay → client: {"Hello":<port>}, "Heartbeat", {"Connection":"<uuid>"},
//! {"Error":"…"}, {"Challenge":"<uuid>"} (only relays with a secret, before anything else).

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use crate::core::types::ServerMeta;

pub const DEFAULT_RELAY: &str = "bore.pub";
const CONTROL_PORT: u16 = 7835;
const MAX_FRAME: usize = 256;

/// Which bore relay to use.
#[derive(Debug, Clone)]
pub struct Relay {
    pub host: String,
    pub port: u16,
    pub secret: Option<String>,
}

impl Relay {
    /// The server's own relay (`relay_host`, "host" or "host:port"), else bore.pub.
    pub fn for_server(meta: &ServerMeta) -> Relay {
        let custom = meta.relay_host.as_deref().map(str::trim).filter(|h| !h.is_empty());
        let Some(text) = custom else {
            return Relay { host: DEFAULT_RELAY.into(), port: CONTROL_PORT, secret: None };
        };
        let (host, port) = match text.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') || h.starts_with('[') => (h.trim_matches(['[', ']']).to_string(), p.parse().unwrap_or(CONTROL_PORT)),
            _ => (text.to_string(), CONTROL_PORT),
        };
        let secret = meta.relay_secret.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        Relay { host, port, secret }
    }

    async fn connect(&self) -> Result<(BufReader<OwnedReadHalf>, OwnedWriteHalf)> {
        let host = &self.host;
        let s = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect((host.as_str(), self.port)))
            .await
            .map_err(|_| anyhow!("The relay ({host}) didn't answer"))?
            .map_err(|e| anyhow!("Couldn't reach the relay ({host}): {e}"))?;
        let _ = s.set_nodelay(true);
        let (read, mut write) = s.into_split();
        let mut read = BufReader::new(read);
        if let Some(secret) = &self.secret {
            let msg = tokio::time::timeout(Duration::from_secs(10), recv(&mut read))
                .await
                .map_err(|_| anyhow!("The relay ({host}) didn't answer"))??
                .ok_or_else(|| anyhow!("The relay ({host}) closed the connection"))?;
            let challenge = msg.get("Challenge").and_then(Value::as_str).ok_or_else(|| anyhow!("The relay ({host}) doesn't use a secret; clear it in Settings"))?;
            let challenge = uuid::Uuid::parse_str(challenge).map_err(|_| anyhow!("The relay sent an unexpected message"))?;
            send(&mut write, json!({ "Authenticate": answer(secret, &challenge) })).await?;
        }
        Ok((read, write))
    }
}

/// bore's challenge answer: HMAC-SHA256 keyed with SHA-256(secret) over the challenge UUID's bytes, as hex.
fn answer(secret: &str, challenge: &uuid::Uuid) -> String {
    let key = Sha256::digest(secret.as_bytes());
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).expect("HMAC takes keys of any size");
    mac.update(challenge.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn send<W: AsyncWriteExt + Unpin>(w: &mut W, msg: Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(&msg)?;
    bytes.push(0);
    w.write_all(&bytes).await?;
    Ok(w.flush().await?)
}

/// Reads one NUL-terminated frame; None when the relay closed the connection.
async fn recv<R: AsyncBufReadExt + Unpin>(r: &mut R) -> Result<Option<Value>> {
    let mut buf = Vec::new();
    let n = r.read_until(0, &mut buf).await?;
    if n == 0 {
        return Ok(None);
    }
    if buf.last() == Some(&0) {
        buf.pop();
    }
    if buf.len() > MAX_FRAME {
        bail!("The relay sent an unexpected message");
    }
    Ok(Some(serde_json::from_slice(&buf)?))
}

/// An open relay: `address` is what friends type into Minecraft. Dropping `task` (abort) closes it.
pub struct Tunnel {
    pub address: String,
    pub task: JoinHandle<()>,
}

/// Opens a relay to the server listening on `local_port`. `on_closed` runs if the relay goes away later.
pub async fn open(relay: Relay, local_port: u16, on_closed: impl FnOnce() + Send + 'static) -> Result<Tunnel> {
    let host = relay.host.clone();
    let (mut read, mut write) = relay.connect().await?;
    send(&mut write, json!({ "Hello": 0 })).await?;
    let port = loop {
        let msg = tokio::time::timeout(Duration::from_secs(10), recv(&mut read))
            .await
            .map_err(|_| anyhow!("The relay ({host}) didn't answer"))??
            .ok_or_else(|| anyhow!("The relay ({host}) closed the connection"))?;
        if let Some(p) = msg.get("Hello").and_then(Value::as_u64) {
            break p;
        }
        if let Some(e) = msg.get("Error").and_then(Value::as_str) {
            bail!("The relay refused: {e}");
        }
        if msg.get("Challenge").is_some() {
            bail!("The relay ({host}) needs a secret. Enter it in the server's Settings");
        }
    };
    let address = format!("{host}:{port}");
    let task = tokio::spawn(async move {
        // Keep the write half alive for as long as the tunnel runs.
        let _write = write;
        loop {
            match recv(&mut read).await {
                Ok(Some(msg)) => {
                    if let Some(id) = msg.get("Connection").and_then(Value::as_str) {
                        tokio::spawn(forward(relay.clone(), id.to_string(), local_port));
                    }
                    // "Heartbeat" and anything unknown: nothing to do.
                }
                _ => break,
            }
        }
        on_closed();
    });
    Ok(Tunnel { address, task })
}

/// Joins one player's connection (accepted on a fresh relay connection) to the local server.
async fn forward(relay: Relay, id: String, local_port: u16) {
    let result: Result<()> = async {
        let (read, mut write) = relay.connect().await?;
        send(&mut write, json!({ "Accept": id })).await?;
        let mut remote = read.into_inner().reunite(write)?;
        let mut local = TcpStream::connect(("127.0.0.1", local_port)).await?;
        let _ = local.set_nodelay(true);
        tokio::io::copy_bidirectional(&mut remote, &mut local).await?;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        eprintln!("[server] relay connection ended: {e:#}");
    }
}
