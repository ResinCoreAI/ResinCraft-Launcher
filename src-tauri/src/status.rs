//! The Status page (pages/Status.tsx): whether each service the launcher depends on answers right now, checked from
//! this PC. Every check runs at once with a short timeout, only when the page asks (on open and on Refresh); nothing
//! polls in the background.
//!
//! The hosts are the ones the engine really calls (see the file named beside each). A check counts a service as up
//! when it answers at all below HTTP 500: a 401, 403 or 404 still means the server is there. 5xx, a timeout, or no
//! connection count as down, so the relay's site can be up while its API (`/api`, behind the same name) returns 502.

use std::time::{Duration, Instant};

use futures::future::join_all;
use serde::Serialize;

/// How long one request may take before the service counts as down.
const TIMEOUT: Duration = Duration::from_secs(8);
/// Slower than this (the slowest of a service's hosts) and it shows as slow.
const SLOW_MS: u64 = 2000;

/// One thing to reach: a web address, or a TCP port (the relay's tunnel, which speaks TLS of its own, not HTTP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Http(String),
    Tcp(String, u16),
}

impl Target {
    fn host(&self) -> String {
        match self {
            Target::Http(u) => u.split("://").nth(1).unwrap_or(u).split(['/', '?']).next().unwrap_or_default().to_string(),
            Target::Tcp(h, p) => format!("{h}:{p}"),
        }
    }
}

/// A service on the page: its id (the page names it and says what it is for), group, and what to reach. `targets`
/// is empty when this build doesn't use it (no relay compiled in).
#[derive(Debug, Clone)]
pub struct Service {
    pub id: &'static str,
    pub group: &'static str,
    pub targets: Vec<Target>,
}

/// What one service looked like just now.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCheck {
    pub id: String,
    pub group: String,
    /// "up" | "slow" | "down" | "off" (this build doesn't use it)
    pub state: String,
    /// How long the slowest host took to answer (none when nothing answered).
    pub ms: Option<u64>,
    /// Why it's down: "timeout" | "dns" | "connect" | "http"; with "http", `code` is the status.
    pub problem: Option<String>,
    pub code: Option<u16>,
    /// The hosts checked, and the one that failed first when it's down.
    pub hosts: Vec<String>,
    pub failed_host: Option<String>,
    /// When the check finished (ms since 1970).
    pub checked_at: i64,
}

/// One host's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// It answered with this HTTP status (0 for a TCP port that accepted the connection), after `ms`.
    Answered { code: u16, ms: u64 },
    Failed { problem: &'static str, ms: u64 },
}

const GITHUB_FEED: &str = "https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/updates/latest.json";

/// Every service, in the page's order. The relay's addresses come from this build (server/relay.rs `settings`).
pub fn services() -> Vec<Service> {
    let relay = crate::server::relay::settings();
    let http = |u: &str| Target::Http(u.to_string());
    let relay_site = relay.as_ref().map(|s| vec![http(&format!("https://{}/", s.domain))]).unwrap_or_default();
    let relay_api = relay.as_ref().map(|s| vec![http(&format!("https://{}/api/servers/modpacks", s.domain))]).unwrap_or_default();
    let relay_tunnel = relay.as_ref().and_then(|s| tunnel_target(&s.url)).map(|t| vec![t]).unwrap_or_default();
    vec![
        // player/accounts.rs: MS_BASE, then Xbox Live's user and XSTS tokens.
        Service {
            id: "microsoft",
            group: "signin",
            targets: vec![
                http("https://login.microsoftonline.com/consumers/v2.0/.well-known/openid-configuration"),
                http("https://user.auth.xboxlive.com/"),
                http("https://xsts.auth.xboxlive.com/"),
            ],
        },
        // player/accounts.rs: login_with_xbox, entitlements and the profile.
        Service { id: "minecraftServices", group: "signin", targets: vec![http("https://api.minecraftservices.com/")] },
        // player/skins.rs: names to UUIDs, profiles with their skin, and the skin pictures.
        Service {
            id: "mojangSessions",
            group: "signin",
            targets: vec![
                http("https://sessionserver.mojang.com/session/minecraft/profile/069a79f444e94726a5befca90e38aaf5"),
                http("https://api.mojang.com/"),
                http("https://textures.minecraft.net/"),
            ],
        },
        // game/mojang.rs: MANIFEST_URL and the version files it points at (piston-data).
        Service {
            id: "versions",
            group: "downloads",
            targets: vec![http("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"), http("https://piston-data.mojang.com/")],
        },
        Service { id: "libraries", group: "downloads", targets: vec![http("https://libraries.minecraft.net/")] },
        // game/mojang.rs: RESOURCES_URL.
        Service { id: "resources", group: "downloads", targets: vec![http("https://resources.download.minecraft.net/")] },
        // content/modrinth.rs: the API and the files it serves.
        Service { id: "modrinth", group: "content", targets: vec![http("https://api.modrinth.com/v2/"), http("https://cdn.modrinth.com/")] },
        // content/curseforge.rs: API (the files themselves come from links it gives).
        Service { id: "curseforge", group: "content", targets: vec![http("https://api.curseforge.com/v1/games/432")] },
        // game/loaders.rs.
        Service { id: "fabric", group: "loaders", targets: vec![http("https://meta.fabricmc.net/v2/versions/loader/1.21.1")] },
        Service { id: "quilt", group: "loaders", targets: vec![http("https://meta.quiltmc.org/v3/versions/loader"), http("https://maven.quiltmc.org/")] },
        Service {
            id: "forge",
            group: "loaders",
            targets: vec![
                http("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"),
                http("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml"),
            ],
        },
        Service { id: "neoforge", group: "loaders", targets: vec![http("https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml")] },
        Service { id: "relaySite", group: "resincraft", targets: relay_site },
        Service { id: "relayApi", group: "resincraft", targets: relay_api },
        Service { id: "relayTunnel", group: "resincraft", targets: relay_tunnel },
        // updater.rs: STABLE_FEED (Beta's beta.json sits beside it on the same branch).
        Service { id: "updates", group: "resincraft", targets: vec![http(GITHUB_FEED)] },
        // server/playit.rs: API_BASE.
        Service { id: "playit", group: "resincraft", targets: vec![http("https://api.playit.gg/")] },
    ]
}

/// The relay's tunnel port from its address ("tls://relay.example:7000").
fn tunnel_target(url: &str) -> Option<Target> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let rest = rest.split('/').next()?;
    let (host, port) = rest.rsplit_once(':')?;
    Some(Target::Tcp(host.to_string(), port.parse().ok()?))
}

/// Up, slow or down from each host's answer (the worst of them).
pub fn judge(service: &Service, probes: &[Probe], checked_at: i64) -> ServiceCheck {
    let hosts: Vec<String> = service.targets.iter().map(Target::host).collect();
    let mut out = ServiceCheck {
        id: service.id.into(),
        group: service.group.into(),
        state: "off".into(),
        ms: None,
        problem: None,
        code: None,
        hosts,
        failed_host: None,
        checked_at,
    };
    if probes.is_empty() {
        return out;
    }
    let mut slowest = 0;
    for (i, p) in probes.iter().enumerate() {
        let failed = match p {
            Probe::Answered { code, ms } => {
                slowest = slowest.max(*ms);
                (*code >= 500).then(|| ("http", Some(*code)))
            }
            Probe::Failed { problem, .. } => Some((*problem, None)),
        };
        if let (Some((problem, code)), None) = (failed, &out.problem) {
            out.problem = Some(problem.into());
            out.code = code;
            out.failed_host = out.hosts.get(i).cloned();
        }
    }
    let answered = probes.iter().any(|p| matches!(p, Probe::Answered { .. }));
    out.ms = answered.then_some(slowest);
    out.state = if out.problem.is_some() {
        "down"
    } else if slowest > SLOW_MS {
        "slow"
    } else {
        "up"
    }
    .into();
    out
}

fn problem_of(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        return "timeout";
    }
    let mut text = e.to_string();
    let mut src = std::error::Error::source(e);
    while let Some(s) = src {
        text.push_str(&s.to_string());
        src = s.source();
    }
    let text = text.to_ascii_lowercase();
    if text.contains("dns") || text.contains("resolve") || text.contains("no such host") {
        "dns"
    } else {
        "connect"
    }
}

/// Reach one target. Only the status line and headers are read; the body is dropped.
pub async fn probe(client: &reqwest::Client, target: &Target, key: Option<&str>) -> Probe {
    let start = Instant::now();
    let ms = |s: Instant| s.elapsed().as_millis() as u64;
    match target {
        Target::Http(url) => {
            let mut req = client.get(url).timeout(TIMEOUT).header("Cache-Control", "no-cache");
            if let Some(k) = key.filter(|k| !k.is_empty()) {
                req = req.header("x-api-key", k);
            }
            match req.send().await {
                Ok(res) => Probe::Answered { code: res.status().as_u16(), ms: ms(start) },
                Err(e) => Probe::Failed { problem: problem_of(&e), ms: ms(start) },
            }
        }
        Target::Tcp(host, port) => match tokio::time::timeout(TIMEOUT, tokio::net::TcpStream::connect((host.as_str(), *port))).await {
            Ok(Ok(_)) => Probe::Answered { code: 0, ms: ms(start) },
            Ok(Err(e)) => {
                let text = e.to_string().to_ascii_lowercase();
                let dns = text.contains("resolve") || text.contains("no such host") || text.contains("dns") || text.contains("lookup");
                Probe::Failed { problem: if dns { "dns" } else { "connect" }, ms: ms(start) }
            }
            Err(_) => Probe::Failed { problem: "timeout", ms: ms(start) },
        },
    }
}

/// A notice the ResinCraft team posts on the relay (its Admin → Status): planned maintenance, a known outage.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusNotice {
    #[serde(default)]
    pub id: String,
    /// "outage" | "degraded" | "maintenance" | "info" (anything else reads as "info").
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// Unix seconds; `ends_at` 0 = until further notice.
    #[serde(default)]
    pub starts_at: i64,
    #[serde(default)]
    pub ends_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(serde::Deserialize)]
struct NoticeList {
    #[serde(default)]
    notices: Vec<StatusNotice>,
}

const LEVELS: [&str; 4] = ["outage", "degraded", "maintenance", "info"];

/// The notices worth showing at `now` (unix seconds), most severe first: known levels only (others become "info"),
/// none that has ended or not started, none without a title. The relay already sends only active ones, sorted; this
/// keeps a cached or older answer honest.
pub fn active_notices(list: Vec<StatusNotice>, now: i64) -> Vec<StatusNotice> {
    let mut out: Vec<StatusNotice> = list
        .into_iter()
        .filter(|n| !n.title.trim().is_empty() && (n.ends_at == 0 || n.ends_at > now) && n.starts_at <= now)
        .map(|mut n| {
            if !LEVELS.contains(&n.level.as_str()) {
                n.level = "info".into();
            }
            n
        })
        .collect();
    out.sort_by_key(|n| LEVELS.iter().position(|l| *l == n.level));
    out
}

/// The relay's notices from `url`. Anything but a readable 200 (an older relay answers 404, or it's down) is no
/// notices: the relay's own rows on the page already say when it can't be reached.
pub async fn fetch_notices(client: &reqwest::Client, url: &str, now: i64) -> Vec<StatusNotice> {
    let Ok(res) = client.get(url).timeout(TIMEOUT).send().await else { return Vec::new() };
    if !res.status().is_success() {
        return Vec::new();
    }
    match res.json::<NoticeList>().await {
        Ok(list) => active_notices(list.notices, now),
        Err(_) => Vec::new(),
    }
}

/// The ResinCraft team's notices for the Status page (`app:statusNotices`); none when this build has no relay.
pub async fn notices() -> Vec<StatusNotice> {
    let Some(relay) = crate::server::relay::settings() else { return Vec::new() };
    let url = format!("https://{}/api/status/notices", relay.domain);
    fetch_notices(crate::core::ctx::http(), &url, crate::core::fsutil::now_ms() / 1000).await
}

/// Check every service at once (`app:checkServices`).
pub async fn check_all() -> Vec<ServiceCheck> {
    let client = crate::core::ctx::http();
    let cf_key = crate::core::settings::curseforge_key().await;
    let list = services();
    let checks = list.iter().map(|s| {
        let cf_key = cf_key.clone();
        async move {
            let key = (s.id == "curseforge").then_some(cf_key.as_str());
            let probes = join_all(s.targets.iter().map(|t| probe(client, t, key))).await;
            judge(s, &probes, crate::core::fsutil::now_ms())
        }
    });
    join_all(checks).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn svc(targets: Vec<Target>) -> Service {
        Service { id: "x", group: "g", targets }
    }
    fn http(u: &str) -> Target {
        Target::Http(u.into())
    }

    #[test]
    fn every_service_has_a_unique_id_and_hosts() {
        let list = services();
        let mut ids: Vec<_> = list.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), list.len());
        for s in &list {
            if !s.id.starts_with("relay") {
                assert!(!s.targets.is_empty(), "{} has nothing to check", s.id);
            }
            for t in &s.targets {
                if let Target::Http(u) = t {
                    assert!(u.starts_with("https://"), "{u}");
                }
            }
        }
    }

    #[test]
    fn hosts_and_tunnel_address() {
        assert_eq!(http("https://api.modrinth.com/v2/").host(), "api.modrinth.com");
        assert_eq!(http("https://x.dev?a=1").host(), "x.dev");
        assert_eq!(tunnel_target("tls://relay.rscraft.example:7000"), Some(Target::Tcp("relay.rscraft.example".into(), 7000)));
        assert_eq!(tunnel_target("relay.example:443/x"), Some(Target::Tcp("relay.example".into(), 443)));
        assert_eq!(tunnel_target("tls://relay.example"), None);
    }

    #[test]
    fn answers_below_500_are_up_and_slow_past_two_seconds() {
        let s = svc(vec![http("https://a/"), http("https://b/")]);
        let c = judge(&s, &[Probe::Answered { code: 200, ms: 120 }, Probe::Answered { code: 404, ms: 300 }], 5);
        assert_eq!((c.state.as_str(), c.ms, c.checked_at), ("up", Some(300), 5));
        assert_eq!(c.hosts, vec!["a", "b"]);
        let c = judge(&s, &[Probe::Answered { code: 403, ms: 120 }, Probe::Answered { code: 200, ms: 2500 }], 0);
        assert_eq!(c.state, "slow");
    }

    #[test]
    fn a_502_or_a_timeout_is_down_with_the_host_that_failed() {
        let s = svc(vec![http("https://site/"), http("https://site/api/x")]);
        let c = judge(&s, &[Probe::Answered { code: 200, ms: 90 }, Probe::Answered { code: 502, ms: 40 }], 0);
        assert_eq!((c.state.as_str(), c.problem.as_deref(), c.code), ("down", Some("http"), Some(502)));
        assert_eq!(c.failed_host.as_deref(), Some("site"));
        let c = judge(&s, &[Probe::Failed { problem: "timeout", ms: 8000 }, Probe::Answered { code: 200, ms: 10 }], 0);
        assert_eq!((c.state.as_str(), c.problem.as_deref(), c.ms), ("down", Some("timeout"), Some(10)));
        let c = judge(&s, &[Probe::Failed { problem: "dns", ms: 3 }, Probe::Failed { problem: "connect", ms: 3 }], 0);
        assert_eq!((c.problem.as_deref(), c.ms), (Some("dns"), None));
    }

    #[test]
    fn nothing_to_check_is_off() {
        assert_eq!(judge(&svc(vec![]), &[], 0).state, "off");
    }

    /// A one-request server that answers with `status`.
    async fn answer_once(status: &'static str) -> u16 {
        answer_body(status, "").await
    }

    /// A one-request server that answers with `status` and `body`.
    async fn answer_body(status: &'static str, body: &'static str) -> u16 {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf).await;
            let head = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(format!("{head}{body}").as_bytes()).await;
        });
        port
    }

    fn notice(id: &str, level: &str, starts_at: i64, ends_at: i64) -> StatusNotice {
        StatusNotice { id: id.into(), level: level.into(), title: format!("t{id}"), body: String::new(), starts_at, ends_at, updated_at: 0 }
    }

    #[test]
    fn notices_keep_only_active_ones_most_severe_first() {
        let list = vec![
            notice("a", "info", 0, 0),
            notice("b", "maintenance", 50, 200),
            notice("c", "outage", 0, 0),
            notice("ended", "outage", 0, 90),
            notice("later", "degraded", 150, 0),
            notice("d", "weird", 0, 0),
            StatusNotice { title: "  ".into(), ..notice("blank", "outage", 0, 0) },
        ];
        let got: Vec<(String, String)> = active_notices(list, 100).into_iter().map(|n| (n.id, n.level)).collect();
        let want = [("c", "outage"), ("b", "maintenance"), ("a", "info"), ("d", "info")];
        assert_eq!(got, want.map(|(a, b)| (a.to_string(), b.to_string())));
    }

    #[tokio::test]
    async fn notices_from_the_relay_or_none() {
        let client = reqwest::Client::new();
        let body = r#"{"notices":[{"id":"n1","level":"degraded","title":"Slow joins","body":"Working on it","startsAt":10,"endsAt":0,"updatedAt":12}]}"#;
        let port = answer_body("200 OK", body).await;
        let got = fetch_notices(&client, &format!("http://127.0.0.1:{port}/api/status/notices"), 100).await;
        assert_eq!(got, vec![StatusNotice { id: "n1".into(), level: "degraded".into(), title: "Slow joins".into(), body: "Working on it".into(), starts_at: 10, ends_at: 0, updated_at: 12 }]);
        // An older relay without the page, a broken answer, or no relay at all: no notices, no error.
        let port = answer_body("404 Not Found", "not found").await;
        assert!(fetch_notices(&client, &format!("http://127.0.0.1:{port}/api/status/notices"), 100).await.is_empty());
        let port = answer_body("200 OK", "<html>").await;
        assert!(fetch_notices(&client, &format!("http://127.0.0.1:{port}/api/status/notices"), 100).await.is_empty());
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed = l.local_addr().unwrap().port();
        drop(l);
        assert!(fetch_notices(&client, &format!("http://127.0.0.1:{closed}/"), 100).await.is_empty());
    }

    #[tokio::test]
    async fn probes_a_real_server() {
        let client = reqwest::Client::new();
        let port = answer_once("502 Bad Gateway").await;
        let p = probe(&client, &Target::Http(format!("http://127.0.0.1:{port}/api/x")), None).await;
        assert!(matches!(p, Probe::Answered { code: 502, .. }), "{p:?}");
        let port = answer_once("200 OK").await;
        let p = probe(&client, &Target::Http(format!("http://127.0.0.1:{port}/")), None).await;
        assert!(matches!(p, Probe::Answered { code: 200, .. }), "{p:?}");
        // A port that's listening accepts the tunnel check; a closed one fails it.
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let open = l.local_addr().unwrap().port();
        assert!(matches!(probe(&client, &Target::Tcp("127.0.0.1".into(), open), None).await, Probe::Answered { code: 0, .. }));
        drop(l);
        assert!(matches!(probe(&client, &Target::Tcp("127.0.0.1".into(), open), None).await, Probe::Failed { .. }));
    }
}
