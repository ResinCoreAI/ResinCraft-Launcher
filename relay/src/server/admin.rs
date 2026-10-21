//! The local admin port (keep it on 127.0.0.1): health, metrics and a small bearer-token API for scripts.
//! The admin dashboard (dashboard.rs) is the friendlier way to do the same and more.
//!
//! - `GET /healthz`: 200 with `{"ok":true,…}` while the relay runs.
//! - `GET /metrics`: Prometheus counters.
//! - With `Authorization: Bearer <admin token>`: `GET /admin/tunnels`, `GET /admin/names`, `GET /admin/bans`,
//!   `POST /admin/ban` and `POST /admin/unban` with `{"uuid": "…"}` or `{"ip": "…"}` (a ban takes an optional
//!   `"reason"` and `"until"` (Unix seconds) or `"hours"`; without either it's permanent),
//!   `POST /admin/release` with `{"name": "…"}`.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

use super::http::{read_request, write_response, Request, Response};
use super::{same_secret, Relay};

pub(crate) async fn serve(relay: Arc<Relay>, listener: TcpListener) {
    loop {
        match listener.accept().await {
            Ok((sock, _)) => {
                tokio::spawn(handle(relay.clone(), sock));
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

async fn handle(relay: Arc<Relay>, mut sock: TcpStream) {
    let Ok(Ok(req)) = tokio::time::timeout(Duration::from_secs(5), read_request(&mut sock)).await else { return };
    let resp = route(&relay, &req).header("Cache-Control", "no-store");
    let _ = write_response(&mut sock, &resp).await;
}

/// The Prometheus text for /metrics (also used by the dashboard's overview).
pub(crate) fn metrics_text(relay: &Relay) -> String {
    let (names, sessions) = relay.store.read(|d| (d.names.len(), d.sessions.len()));
    let gauges = [
        ("tunnels_online", "Names online right now.", relay.tunnels_online() as i64),
        ("names_reserved", "Names reserved.", names as i64),
        ("sessions", "Valid launcher sessions.", sessions as i64),
    ];
    relay.metrics.render(&gauges)
}

fn route(relay: &Relay, req: &Request) -> Response {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/healthz") => Response::ok(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION"), "tunnels": relay.tunnels_online() })),
        ("GET", "/metrics") => Response::new("200 OK", "text/plain; version=0.0.4", metrics_text(relay)),
        (_, p) if p.starts_with("/admin/") => {
            let tokens = &relay.cfg.admin.tokens;
            if tokens.is_empty() {
                return Response::error("403 Forbidden", "the admin API is off: add [admin] tokens to the config");
            }
            if !req.bearer().is_some_and(|a| tokens.iter().any(|t| same_secret(a, t))) {
                return Response::error("401 Unauthorized", "missing or wrong admin token");
            }
            admin(relay, req.method.as_str(), p, &req.json())
        }
        _ => Response::error("404 Not Found", "not found"),
    }
}

fn admin(relay: &Relay, method: &str, path: &str, input: &Value) -> Response {
    let field = |k: &str| input.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    match (method, path) {
        ("GET", "/admin/tunnels") => {
            let list: Vec<Value> = relay
                .tunnel_list()
                .into_iter()
                .map(|(name, t)| {
                    json!({
                        "name": name,
                        "owner": t.owner,
                        "ownerName": t.owner_name,
                        "since": t.since,
                        "connections": t.conns.load(Ordering::Relaxed),
                        "bytesIn": t.bytes_in.load(Ordering::Relaxed),
                        "bytesOut": t.bytes_out.load(Ordering::Relaxed),
                    })
                })
                .collect();
            Response::ok(json!(list))
        }
        ("GET", "/admin/names") => Response::ok(relay.store.read(|d| json!(d.names))),
        ("GET", "/admin/bans") => {
            let (uuids, ips) = relay.store.read(|d| (json!(d.bans.uuids), json!(d.bans.ips)));
            Response::ok(json!({ "uuids": uuids, "ips": ips, "fromConfig": { "uuids": relay.cfg.bans.uuids, "ips": relay.cfg.bans.ips } }))
        }
        ("POST", "/admin/ban") | ("POST", "/admin/unban") => {
            let ban = match path.ends_with("/ban").then(|| super::ban_end(input)).transpose() {
                Ok(end) => end.map(|end| (super::ban_reason(field("reason")), end)),
                Err(why) => return Response::error("400 Bad Request", why),
            };
            let banned = ban.is_some();
            if let Some(uuid) = field("uuid") {
                let Some(id) = relay.ban_uuid(&uuid, ban) else { return Response::error("400 Bad Request", "not a UUID") };
                info!(uuid = %id, banned, "account ban changed (admin API)");
                relay.event(if banned { "ban" } else { "unban" }, format!("Account {id} (admin API)"), None);
                Response::ok(json!({ "ok": true, "uuid": id, "banned": banned }))
            } else if let Some(ip) = field("ip") {
                let Some(ip) = relay.ban_ip(&ip, ban) else { return Response::error("400 Bad Request", "not an IP address or CIDR range") };
                info!(banned, "IP ban changed (admin API)");
                relay.event(if banned { "ban" } else { "unban" }, format!("IP {ip} (admin API)"), None);
                Response::ok(json!({ "ok": true, "ip": ip, "banned": banned }))
            } else {
                Response::error("400 Bad Request", "send {\"uuid\": …} or {\"ip\": …}")
            }
        }
        ("POST", "/admin/release") => {
            let Some(name) = field("name") else { return Response::error("400 Bad Request", "send {\"name\": …}") };
            let existed = relay.admin_release(&name);
            info!(name = %name, "name released (admin API)");
            relay.event("name", format!("{name} released (admin API)"), None);
            Response::ok(json!({ "ok": true, "name": name, "existed": existed }))
        }
        _ => Response::error("404 Not Found", "not found"),
    }
}
