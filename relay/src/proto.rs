//! What the launcher's agent and the relay say to each other, and the rules for tunnel names.
//!
//! The agent connects to the relay's agent port (TLS), and both sides run yamux over that connection. The
//! agent opens the first stream: the control stream, carrying one JSON message per line ([`AgentMsg`] up,
//! [`RelayMsg`] down). For every player, the relay opens a new stream that starts with one JSON line
//! ([`DataHeader`]) followed by the player's raw Minecraft bytes, beginning with the handshake.
//!
//! A session goes: `hello` → `welcome`, then `login` (a Minecraft access token, checked once with Mojang) or
//! `resume` (the relay's own session token from an earlier login) → `authed`, then `check` / `reserve` /
//! `release` for the name picker, or `online` to bring a name online. `ping` keeps the connection alive.
//!
//! Protocol 2 (launcher 0.4) adds the session time limit (`session_warning`, `session_extended`, `continue`)
//! and the admin's on/off switch (`service`). The relay still serves protocol 1 launchers; it just never sends
//! them those messages (new error codes reach them as `unknown`, which they retry).
//!
//! Protocol 3 (launcher 0.5) adds Bedrock servers: `online` with `bedrock: true` gives the name a UDP port of
//! its own on the relay (`bedrock_port` in the answer). Each player's address gets a stream whose header says
//! `udp: true`; datagrams travel on it as [`write_datagram`] frames (a 2-byte big-endian length, then the bytes).
//!
//! Protocol 4 adds data top-ups: `top_up` asks for more of this month's data when little is left
//! (answered with `top_up`, which the relay also sends when its admin adds data), and [`Usage`] carries the
//! account's own cap (top-ups and unlimited accounts included). Older launchers never get `top_up` and ignore
//! the new [`Usage`] and [`Limits`] fields.

use serde::{Deserialize, Serialize};

/// The protocol this code speaks; the relay answers versions outside MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION
/// with `version`.
pub const PROTOCOL_VERSION: u32 = 4;
/// The first protocol with Bedrock (UDP) tunnels.
pub const BEDROCK_PROTOCOL_VERSION: u32 = 3;
/// The first protocol with data top-ups.
pub const TOPUP_PROTOCOL_VERSION: u32 = 4;
/// The oldest launcher protocol the relay still serves (launcher 0.3).
pub const MIN_PROTOCOL_VERSION: u32 = 1;
/// Longest control line either side accepts.
pub const MAX_LINE: usize = 16 * 1024;
/// ALPN id on the agent port's TLS.
pub const ALPN: &[u8] = b"resincraft-relay/1";
/// The agent pings this often; the relay drops agents that stay silent for three times as long.
pub const PING_SECS: u64 = 20;

/// Agent → relay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum AgentMsg {
    Hello { version: u32, client: String },
    /// A Minecraft services access token. The relay asks Mojang whose it is and forgets it right away.
    Login { minecraft_token: String },
    /// The relay's own session token from an earlier `authed`.
    Resume { session: String },
    /// Is this name free (or already ours)?
    Check { name: String },
    /// Reserve a name for this account; `release` frees another of ours at the same time (renaming).
    Reserve {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        release: Option<String>,
    },
    Release { name: String },
    /// Bring a name online on this connection (reserving it first when it's free). `bedrock`: a Bedrock server,
    /// reached on a UDP port of its own (protocol 3).
    Online {
        name: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bedrock: bool,
    },
    /// Keep the session going for another full session (after a `session_warning`). Protocol 2.
    Continue,
    /// More data for the rest of this month (when little or nothing is left). Protocol 4.
    TopUp,
    Ping { seq: u64 },
}

/// Relay → agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum RelayMsg {
    Welcome { version: u32, base_domain: String },
    Authed {
        uuid: String,
        username: String,
        /// Only after `login`: keep it (it replaces the Minecraft token for later connections).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
    NameStatus {
        name: String,
        available: bool,
        mine: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<ErrorCode>,
    },
    Reserved { name: String, address: String },
    Released { name: String },
    Online {
        name: String,
        address: String,
        limits: Limits,
        usage: Usage,
        /// Seconds until the session ends (None: no time limit).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secs_left: Option<u64>,
        /// A Bedrock tunnel's UDP port on the relay (players add `address` with this port). Protocol 3.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bedrock_port: Option<u16>,
    },
    /// The session ends soon unless the agent sends `continue`. Protocol 2.
    SessionWarning { secs_left: u64 },
    /// The session was extended (by `continue`, or by the relay's admin). Protocol 2.
    SessionExtended { secs_left: u64, continues: u32 },
    /// The admin switched the relay on, paused it (running tunnels go on until their session ends) or off.
    /// Protocol 2.
    Service {
        state: ServiceState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Sent now and then while online, and when the account's data changes.
    Usage { usage: Usage },
    /// The answer to `top_up`, or the relay's admin added data (`by_admin`). `refused`: why there's no top-up
    /// (nothing was added; the tunnel goes on). Protocol 4.
    TopUp {
        added_bytes: u64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        by_admin: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refused: Option<ErrorCode>,
        usage: Usage,
    },
    Pong { seq: u64 },
    Error { code: ErrorCode, message: String },
    /// The relay is restarting: reconnect after this long.
    Shutdown { reconnect_ms: u64 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    /// The agent's protocol version is too old (or too new).
    Version,
    /// Mojang didn't accept the Minecraft token.
    AuthFailed,
    /// The Microsoft account has no Minecraft: Java Edition profile.
    NotOwned,
    /// The relay session token is unknown or expired: log in with a Minecraft token again.
    SessionExpired,
    Banned,
    NameInvalid,
    NameReserved,
    NameTaken,
    TooManyNames,
    TooManyTunnels,
    OverQuota,
    RateLimited,
    /// Mojang's account service didn't answer.
    Unavailable,
    /// The same name came online on another connection.
    Replaced,
    /// The relay's admin disconnected this tunnel.
    Kicked,
    /// The relay's admin released or renamed this name.
    Released,
    /// The relay's admin paused it: no new tunnels for now (the message says why). Try again later.
    Paused,
    /// The relay's admin switched it off (the message says why). Try again later.
    ServiceOff,
    /// The tunnel's session time ran out without a `continue`.
    SessionEnded,
    /// The relay has no Bedrock ports (switched off in its config, or all in use). Protocol 3.
    BedrockUnavailable,
    /// A top-up was asked for while plenty of data is left. Protocol 4.
    TopUpNotYet,
    /// No top-ups left this month (or the relay has none): only its admin can add data. Protocol 4.
    TopUpLimit,
    ShuttingDown,
    Internal,
    #[serde(other)]
    Unknown,
}

impl ErrorCode {
    /// Whether reconnecting later could help (a dropped connection, a restart) or not (a ban, a taken name).
    pub fn retryable(self) -> bool {
        matches!(
            self,
            ErrorCode::Unavailable
                | ErrorCode::ShuttingDown
                | ErrorCode::Internal
                | ErrorCode::RateLimited
                | ErrorCode::Paused
                | ErrorCode::ServiceOff
                | ErrorCode::Unknown
        )
    }
}

/// The relay's words for a refused top-up (the launcher shows its own, translated wording).
pub fn top_up_refusal(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::TopUpNotYet => "there's still enough data left this month",
        _ => "no more top-ups this month; the relay's admin can add data",
    }
}

/// The admin's switch.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    #[default]
    On,
    /// No new tunnels; running ones go on until their session ends.
    Paused,
    /// No tunnels at all; running ones were closed.
    Off,
}

/// What one account may use.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Limits {
    /// Names online at the same time.
    pub tunnels: u32,
    /// Names reserved.
    pub names: u32,
    /// Players connected to one name at the same time.
    pub connections: u32,
    /// Traffic per calendar month (UTC), both directions; 0 = unlimited.
    pub monthly_bytes: u64,
    /// How long a session lasts before it needs a `continue`; 0 = no limit.
    #[serde(default)]
    pub session_secs: u64,
    /// The host may continue once this little time is left. Protocol 4 (older relays: 0, use your own).
    #[serde(default)]
    pub continue_secs: u64,
}

/// The account's use of the relay this month.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    /// "2026-10"
    pub month: String,
    pub bytes: u64,
    /// Players connected to this tunnel right now.
    pub connections: u32,
    /// This account's cap this month (the relay's cap plus top-ups); 0 = none. Protocol 4.
    #[serde(default)]
    pub cap_bytes: u64,
    /// The relay's admin gave this account unlimited use: no data cap and no session time limit. Protocol 4.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unlimited: bool,
    /// What one top-up adds (0: the relay has none). Protocol 4.
    #[serde(default)]
    pub topup_bytes: u64,
    /// Top-ups this account may still take this month. Protocol 4.
    #[serde(default)]
    pub topups_left: u32,
    /// A top-up can be asked for once less than this is left. Protocol 4.
    #[serde(default)]
    pub low_bytes: u64,
}

impl Usage {
    /// Data left this month (None: no cap).
    pub fn left_bytes(&self) -> Option<u64> {
        (self.cap_bytes > 0 && !self.unlimited).then(|| self.cap_bytes.saturating_sub(self.bytes))
    }

    /// Whether a top-up would be granted now.
    pub fn can_top_up(&self) -> bool {
        self.topup_bytes > 0 && self.topups_left > 0 && self.left_bytes().is_some_and(|l| l < self.low_bytes.max(1))
    }
}

/// First line of each player stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataHeader {
    /// The player's address as the relay sees it (for a PROXY-protocol-aware server later; not logged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// A Bedrock player: the stream carries datagrams ([`write_datagram`] frames), not a byte stream.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub udp: bool,
}

/// Largest datagram a frame carries (a UDP payload never is bigger).
pub const MAX_DATAGRAM: usize = 65_507;

/// Appends one datagram as a frame: its length (2 bytes, big-endian), then the bytes.
pub fn write_datagram(out: &mut Vec<u8>, datagram: &[u8]) {
    let len = datagram.len().min(MAX_DATAGRAM);
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(&datagram[..len]);
}

/// Reads one frame into `buf` (resized to the datagram); Ok(false) at a clean end of the stream.
pub async fn read_datagram<R: tokio::io::AsyncRead + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    use tokio::io::AsyncReadExt;
    let mut len = [0u8; 2];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(e) => return Err(e),
    }
    buf.resize(u16::from_be_bytes(len) as usize, 0);
    r.read_exact(buf).await?;
    Ok(true)
}

// ---------- Names ----------

/// Names nobody can reserve: infrastructure, staff-sounding and brand names.
pub const RESERVED_NAMES: &[&str] = &[
    "about", "abuse", "account", "accounts", "admin", "administrator", "api", "app", "assets", "auth", "beta", "billing", "blog",
    "cdn", "dev", "dns", "docs", "download", "downloads", "email", "ftp", "health", "help", "hostmaster", "imap", "info", "localhost",
    "login", "mail", "metrics", "minecraft", "mod", "moderator", "mods", "mojang", "microsoft", "mx", "no-reply", "noreply", "ns",
    "ns1", "ns2", "oauth", "official", "owner", "play", "pop", "postmaster", "puirkle", "qr", "relay", "resincore", "resincraft",
    "root", "rscraft", "secure", "security", "signin", "smtp", "staff", "staging", "static", "status", "store", "shop", "support", "system",
    "team", "test", "update", "updates", "webmaster", "www", "xbox",
];

pub const NAME_MIN: usize = 3;
pub const NAME_MAX: usize = 24;

/// `^[a-z0-9-]{3,24}$`, not starting or ending with a dash and without "--" (no punycode look-alikes).
pub fn name_syntax_ok(name: &str) -> bool {
    (NAME_MIN..=NAME_MAX).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// Checks a name against the syntax and the reserved words (`extra` comes from the relay's config).
pub fn check_name(name: &str, extra_reserved: &[String]) -> Result<(), ErrorCode> {
    if !name_syntax_ok(name) {
        return Err(ErrorCode::NameInvalid);
    }
    if RESERVED_NAMES.contains(&name) || extra_reserved.iter().any(|r| r.eq_ignore_ascii_case(name)) {
        return Err(ErrorCode::NameReserved);
    }
    Ok(())
}

/// A valid name made from free text (usually the server's name): "Puirkle's World!" → "puirkles-world".
pub fn suggest_name(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'a'..='z' | '0'..='9' => out.push(c),
            '\'' | '’' => {}
            _ if !out.ends_with('-') => out.push('-'),
            _ => {}
        }
    }
    let mut name = out.trim_matches('-').to_string();
    if name.len() > NAME_MAX {
        name.truncate(NAME_MAX);
        name = name.trim_end_matches('-').to_string();
    }
    if name.len() < NAME_MIN {
        name = if name.is_empty() { "my-server".into() } else { format!("{name}-mc") };
    }
    if check_name(&name, &[]).is_err() {
        name.truncate(NAME_MAX - 3);
        name = format!("{}-mc", name.trim_end_matches('-'));
    }
    name
}

/// The address players type for `name`.
pub fn address(name: &str, base_domain: &str) -> String {
    format!("{name}.{base_domain}")
}

/// "2026-10" for a Unix time in seconds (UTC).
pub fn month_of(unix_secs: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}")
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names() {
        for good in ["abc", "puirkle-smp", "a1-b2-c3", "x".repeat(24).as_str(), "123"] {
            assert_eq!(check_name(good, &[]), Ok(()), "{good}");
        }
        for bad in ["ab", "", "-abc", "abc-", "ab--cd", "xn--abc", "ABC", "a_b", "a.b", "a b", "ünï", "x".repeat(25).as_str()] {
            assert_eq!(check_name(bad, &[]), Err(ErrorCode::NameInvalid), "{bad}");
        }
        assert_eq!(check_name("www", &[]), Err(ErrorCode::NameReserved));
        assert_eq!(check_name("resincraft", &[]), Err(ErrorCode::NameReserved));
        assert_eq!(check_name("hypixel", &["Hypixel".into()]), Err(ErrorCode::NameReserved));
        assert_eq!(check_name("hypixel", &[]), Ok(()));
    }

    #[test]
    fn suggests_valid_names() {
        assert_eq!(suggest_name("Puirkle's World!"), "puirkles-world");
        assert_eq!(suggest_name("  My   SMP 2  "), "my-smp-2");
        assert_eq!(suggest_name("ab"), "ab-mc");
        assert_eq!(suggest_name("✨"), "my-server");
        assert_eq!(suggest_name("Relay"), "relay-mc");
        assert_eq!(suggest_name("Café Créatif"), "caf-cr-atif");
        let long = suggest_name("A very long server name that goes on and on");
        assert!(long.len() <= NAME_MAX && check_name(&long, &[]).is_ok(), "{long}");
        for text in ["", "--", "a--b", "x-y-", "WWW", "123456789012345678901234-5"] {
            let s = suggest_name(text);
            assert!(check_name(&s, &[]).is_ok(), "{text} → {s}");
        }
    }

    #[test]
    fn months() {
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_792_281_600), "2026-10"); // 2026-10-18
        assert_eq!(month_of(1_709_164_800), "2024-02"); // 2024-02-29 (leap day)
        assert_eq!(month_of(1_735_689_599), "2024-12"); // 2024-12-31 23:59:59
        assert_eq!(month_of(1_735_689_600), "2025-01");
    }

    #[test]
    fn messages_round_trip() {
        let up = AgentMsg::Reserve { name: "abc".into(), release: None };
        let text = serde_json::to_string(&up).unwrap();
        assert_eq!(text, r#"{"t":"reserve","name":"abc"}"#);
        assert_eq!(serde_json::from_str::<AgentMsg>(&text).unwrap(), up);
        let down: RelayMsg = serde_json::from_str(r#"{"t":"error","code":"something_new","message":"x"}"#).unwrap();
        assert_eq!(down, RelayMsg::Error { code: ErrorCode::Unknown, message: "x".into() });
        assert!(!ErrorCode::NameTaken.retryable() && ErrorCode::ShuttingDown.retryable());
        assert!(ErrorCode::Paused.retryable() && ErrorCode::ServiceOff.retryable() && !ErrorCode::SessionEnded.retryable());
        // Fields protocol 1 launchers don't know are skipped by them, and default when missing.
        let old: RelayMsg = serde_json::from_str(r#"{"t":"online","name":"a","address":"a.b","limits":{"tunnels":1,"names":1,"connections":1,"monthly_bytes":0},"usage":{"month":"2026-10","bytes":0,"connections":0}}"#).unwrap();
        assert!(matches!(old, RelayMsg::Online { secs_left: None, bedrock_port: None, limits: Limits { session_secs: 0, .. }, .. }));
        // A Java tunnel's `online` looks the same as in protocol 2; older relays ignore `bedrock`.
        assert_eq!(serde_json::to_string(&AgentMsg::Online { name: "a".into(), bedrock: false }).unwrap(), r#"{"t":"online","name":"a"}"#);
        assert_eq!(serde_json::from_str::<AgentMsg>(r#"{"t":"online","name":"a"}"#).unwrap(), AgentMsg::Online { name: "a".into(), bedrock: false });
        assert_eq!(serde_json::to_string(&DataHeader { peer: None, udp: true }).unwrap(), r#"{"udp":true}"#);
        assert_eq!(serde_json::from_str::<DataHeader>("{}").unwrap(), DataHeader::default());
        assert_eq!(serde_json::to_string(&AgentMsg::Continue).unwrap(), r#"{"t":"continue"}"#);
        // Protocol 4: top-ups, and the account's cap in usage reports (older usage lines still parse).
        assert_eq!(serde_json::to_string(&AgentMsg::TopUp).unwrap(), r#"{"t":"top_up"}"#);
        let old_usage: Usage = serde_json::from_str(r#"{"month":"2026-10","bytes":5,"connections":0}"#).unwrap();
        assert_eq!((old_usage.cap_bytes, old_usage.left_bytes(), old_usage.can_top_up()), (0, None, false));
        let low = Usage { month: "2026-10".into(), bytes: 99_500_000_000, cap_bytes: 100_000_000_000, topup_bytes: 30_000_000_000, topups_left: 1, low_bytes: 1_000_000_000, ..Usage::default() };
        assert_eq!(low.left_bytes(), Some(500_000_000));
        assert!(low.can_top_up());
        assert!(!Usage { topups_left: 0, ..low.clone() }.can_top_up());
        assert!(!Usage { bytes: 1, ..low.clone() }.can_top_up(), "plenty left");
        assert!(!Usage { unlimited: true, ..low.clone() }.can_top_up());
        let up = RelayMsg::TopUp { added_bytes: 30_000_000_000, by_admin: false, refused: None, usage: low };
        assert_eq!(serde_json::from_str::<RelayMsg>(&serde_json::to_string(&up).unwrap()).unwrap(), up);
        let refused: RelayMsg = serde_json::from_str(r#"{"t":"top_up","added_bytes":0,"refused":"top_up_limit","usage":{"month":"2026-10","bytes":0,"connections":0}}"#).unwrap();
        assert!(matches!(refused, RelayMsg::TopUp { refused: Some(ErrorCode::TopUpLimit), .. }));
        let svc = RelayMsg::Service { state: ServiceState::Paused, message: Some("Maintenance until 20:00".into()) };
        assert_eq!(serde_json::from_str::<RelayMsg>(&serde_json::to_string(&svc).unwrap()).unwrap(), svc);
    }
}
