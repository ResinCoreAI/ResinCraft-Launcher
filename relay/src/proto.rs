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
//!
//! Protocol 5 adds whitelist requests from the web page at a server's relay address: the agent says whether
//! it takes them (`join_requests`), the relay passes each one on (`join_request`), and the agent answers
//! (`join_answer`). The relay never sends `join_request` to older launchers.
//!
//! Protocol 6 adds remote control from the web console (console.<base domain>): a connection that sends `remote`
//! (never alongside `online`) offers this launcher to its own account's browsers. The relay passes each browser
//! request on as `call` and the launcher answers with `call_result`; `watch` turns live updates for a topic on or off,
//! and they come as `push`. `viewers` says how many console tabs are open. The launcher keeps its own allowlist of
//! methods (src-tauri/src/remote.rs); the relay never sends protocol-6 messages to older launchers.
//!
//! Protocol 7 adds players' own domains, verified the way Vercel does it: `domain_add` attaches a domain to one of the
//! account's names and the relay answers `domain_status` with a TXT record (`_rscraft.<domain>`) that proves ownership.
//! The relay routes a domain's players to the name only while that TXT is there; `domain_check` looks again now, and
//! `domain_remove` detaches it. A domain verified by another account moves when this one's TXT shows up.
//!
//! Protocol 8 adds the public server list: `listing_set` lists one of the account's names (or takes it off),
//! `listing_submit` adds a server hosted elsewhere, and `listing_mine` / `listing_get` / `listing_remove` manage them;
//! the relay answers with `listing_status` / `my_listings`. Players `listing_like`, `listing_report` and `listing_join`
//! (a whitelist request under their own verified name, which reaches the host as a `join_request`). Browsing is plain
//! HTTPS (`GET /api/servers`), with no connection at all. The relay never sends protocol-8 messages to older launchers.
//!
//! A `banned` error carries the ban itself ([`Ban`]: until when, and why) in `ban`. It needs no protocol version
//! of its own: every launcher knows the `banned` code, and older ones skip the field.

use serde::{Deserialize, Serialize};

/// The protocol this code speaks; the relay answers versions outside MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION
/// with `version`.
pub const PROTOCOL_VERSION: u32 = 8;
/// The first protocol with Bedrock (UDP) tunnels.
pub const BEDROCK_PROTOCOL_VERSION: u32 = 3;
/// The first protocol with data top-ups.
pub const TOPUP_PROTOCOL_VERSION: u32 = 4;
/// The first protocol with whitelist requests from the web page.
pub const JOIN_REQUESTS_PROTOCOL_VERSION: u32 = 5;
/// The first protocol with remote control from the web console.
pub const REMOTE_PROTOCOL_VERSION: u32 = 6;
/// The first protocol with players' own, verified domains.
pub const DOMAINS_PROTOCOL_VERSION: u32 = 7;
/// The first protocol with the public server list (listings, likes, reports and whitelist requests from the launcher).
pub const LISTINGS_PROTOCOL_VERSION: u32 = 8;
/// Longest `call_result` / `push` line (results and log batches can be bigger than other messages).
pub const MAX_REMOTE_LINE: usize = 1024 * 1024;
/// The oldest launcher protocol the relay still serves (launcher 0.3).
pub const MIN_PROTOCOL_VERSION: u32 = 1;
/// Longest control line either side accepts.
pub const MAX_LINE: usize = 16 * 1024;
/// ALPN id on the agent port's TLS.
pub const ALPN: &[u8] = b"resincraft-relay/1";
/// The agent pings this often; the relay drops agents that stay silent for three times as long.
pub const PING_SECS: u64 = 20;

/// Agent → relay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        /// A Java server that also takes Bedrock players (Geyser): the relay adds a UDP port for them, given as
        /// `bedrock_port` in the answer. Relays that don't know this ignore it (no `bedrock_port`), so launchers try
        /// it without a protocol version of its own.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        geyser: bool,
    },
    /// Keep the session going for another full session (after a `session_warning`). Protocol 2.
    Continue,
    /// More data for the rest of this month (when little or nothing is left). Protocol 4.
    TopUp,
    /// Whether this tunnel takes whitelist requests from the web page; sent once online and whenever the host
    /// changes it (the relay forgets it when the tunnel ends). Protocol 5.
    JoinRequests { enabled: bool },
    /// The answer to a `join_request`. Protocol 5.
    JoinAnswer { id: u64, outcome: JoinOutcome },
    /// The host rejected request `id` and blocks whoever sent it (the relay knows their address for 24 hours;
    /// the launcher never sees it). Protocol 5.
    JoinBlock { id: u64 },
    /// The host accepted or rejected a server list request (a `join_request` with `listing`) from `username`, so the
    /// player's launcher can tell them. Only for those: relays without them close the connection on it. Protocol 8.
    JoinDecided { username: String, accepted: bool },
    /// Offer (or withdraw) this launcher for remote control from the web console: `device` is the PC's name.
    /// Only after `authed`, and only on a connection that never sent `online`. Protocol 6.
    Remote { enabled: bool, device: String, app_version: String },
    /// The answer to `call` `id`: `value` when `ok`, else `error` (worded for players). Protocol 6.
    CallResult { id: u64, ok: bool, value: Option<serde_json::Value>, error: Option<String> },
    /// A live update for a topic the relay asked to `watch`. Protocol 6.
    Push { topic: String, data: serde_json::Value },
    /// Attach `domain` (e.g. `play.example.com`) to the account's name `name`, replacing a domain it had; answered with
    /// `domain_status`. Protocol 7.
    DomainAdd { name: String, domain: String },
    /// Look up `domain`'s DNS again now; answered with `domain_status`. Protocol 7.
    DomainCheck { domain: String },
    /// Detach `name`'s domain; answered with `domain_removed`. Protocol 7.
    DomainRemove { name: String },
    /// List (or update) the account's name `name` in the server list; `listing: None` takes it off. Answered with
    /// `listing_status`. Protocol 8.
    ListingSet { name: String, listing: Option<ListingInfo> },
    /// Where `name`'s listing stands; answered with `listing_status`. Protocol 8.
    ListingGet { name: String },
    /// Submit a server that isn't hosted through ResinCraft (`address`: host or host:port). Answered with
    /// `listing_status`. Protocol 8.
    ListingSubmit { address: String, listing: ListingInfo },
    /// The account's submissions and ResinCraft listings; answered with `my_listings`. Protocol 8.
    ListingMine,
    /// Remove one of the account's own submissions; answered with `listing_status` (state off). Protocol 8.
    ListingRemove { id: String },
    /// Like (or unlike) a listing, one per account; answered with `liked`. Protocol 8.
    ListingLike { id: String, on: bool },
    /// Report a listing (reason ≤ 500 chars); answered with `reported`. Protocol 8.
    ListingReport { id: String, reason: String },
    /// Ask to be whitelisted on a ResinCraft listing: the relay passes it to the host's launcher as a `join_request`
    /// with this account's verified username. Answered with `join_asked`. Protocol 8.
    ListingJoin { id: String },
    /// Where this account's server list whitelist requests stand. Answered with `listing_joins`. Protocol 8.
    ListingJoins,
    /// Unlock the relay's admin tools on this connection with the dashboard's password; only for accounts the relay
    /// lists as admins (`authed` said `admin`). Answered with `admin_ready`. Protocol 8.
    AdminUnlock {
        /// Empty when signing in with `device_token`.
        #[serde(default)]
        password: String,
        /// Remember this PC: the answer carries a `device_token` for password-free unlocks.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        remember: bool,
        /// A token from an earlier `admin_ready` (instead of the password).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_token: Option<String>,
        /// The user's 2FA code (authenticator app), when they have 2FA on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        totp: Option<String>,
        /// This PC's name, shown in the dashboard's list of remembered launchers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device: Option<String>,
        /// The dashboard user to sign in as (relays with dashboard users); without it, the relay's legacy `admin` user.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user: Option<String>,
    },
    /// One admin request after `admin_ready`: the same as the dashboard's JSON API (GET /api/listings, POST
    /// /api/listings/...). Answered with `admin_result`. Protocol 8.
    AdminCall {
        id: u64,
        method: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<serde_json::Value>,
    },
    /// Serve these files of `listing`'s pack plan (lowercase hex SHA-1s of its `owner` files) from this launcher:
    /// players' `GET https://<base_domain>/files/<listing>/<sha1>` reach it as file streams (a [`DataHeader`] with
    /// `file`). Only the listing's owner; replaces an earlier offer. Answered with `files_ready`. Protocol 8.
    FilesOffer { listing: String, files: Vec<String> },
    /// Stop serving `listing`'s files from this connection (no answer). Protocol 8.
    FilesWithdraw { listing: String },
    Ping { seq: u64 },
}

/// What a server-list listing shows (written by its owner, checked by the relay). Protocol 8.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListingInfo {
    /// 1–48 characters.
    pub title: String,
    /// Plain text, at most 1000 characters.
    #[serde(default)]
    pub description: String,
    /// Category ids from [`CATEGORIES`] plus up to 5 custom tags (1–24 characters of `a-z0-9 -`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// "" or one of na, sa, eu, asia, oceania, africa, me.
    #[serde(default)]
    pub region: String,
    /// "" or a BCP-47 language ("en", "th", "pt-BR"…).
    #[serde(default)]
    pub language: String,
    /// The Minecraft version shown (the relay may fill it from a ping).
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub edition: Edition,
    #[serde(default)]
    pub modded: bool,
    /// Bedrock players can join a Java server too (Geyser). For a hosted server the launcher says so itself.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub crossplay: bool,
    /// Players without a Minecraft account (offline profiles) can join: `online-mode=false`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub offline: bool,
    /// Where Bedrock players join when that isn't `address` (Geyser's own port). The relay fills it in for hosted
    /// servers that it carries Geyser for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bedrock_address: Option<String>,
    /// The modpack players need (a listing with one counts as modded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modpack: Option<ListingModpack>,
    /// https:// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    /// discord.gg/… or discord.com/invite/….
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord: Option<String>,
}

/// The modpack a listed server runs. Protocol 8.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListingModpack {
    /// 1–64 characters.
    pub name: String,
    /// "modrinth", "curseforge", "ftb", "atlauncher", "technic", or "" for a custom pack.
    #[serde(default)]
    pub source: String,
    /// The pack's project id on that site ("" for a custom pack).
    #[serde(default)]
    pub project_id: String,
    /// The pack version the server runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    #[default]
    Java,
    Bedrock,
}

/// The server list's fixed categories; listings may add custom tags beyond these.
pub const CATEGORIES: &[&str] = &[
    "smp", "survival", "pvp", "minigames", "creative", "modded", "skyblock", "anarchy", "roleplay", "economy", "factions",
    "prison", "parkour", "hardcore", "vanilla", "towny", "lifesteal", "bedwars",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListingState {
    /// In the list.
    Listed,
    /// Waiting for the relay's admin.
    Pending,
    /// The admin said no (the message says why).
    Rejected,
    /// The admin hid it, or enough reports did until the admin looks.
    Hidden,
    /// Not listed.
    Off,
    #[serde(other)]
    Unknown,
}

/// Where a listing stands, for its owner. Protocol 8.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListingStatus {
    /// "rc-<name>" for ResinCraft servers, "s-<random>" for submissions.
    pub id: String,
    pub state: ListingState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// One of the account's own listings (the answer to `listing_mine`). Protocol 8.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MyListing {
    pub status: ListingStatus,
    pub address: String,
    pub listing: ListingInfo,
    #[serde(default)]
    pub likes: u32,
    /// Hosted through the relay (else submitted).
    #[serde(default)]
    pub resincraft: bool,
    /// The client pack players can install (the owner uploaded it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<ListingPack>,
}

/// One of a player's server list whitelist requests (`listing_joins`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListingJoinState {
    /// The listing ("rc-<name>").
    pub id: String,
    /// "waiting", "accepted" or "rejected".
    pub state: String,
    /// When it was asked, or decided (Unix seconds).
    pub at: u64,
}

/// A listing's client pack, which players install as an instance: a Modrinth .mrpack the owner uploaded (`format` 1),
/// or a pack plan (`format` 2, JSON: see server/clientplan.rs). Protocol 8.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListingPack {
    /// `https://<base domain>/packs/<listing id>.mrpack`, or `….json` for a plan.
    pub url: String,
    /// 1: a .mrpack; 2: a pack plan. Packs stored before plans read as 0 (a .mrpack).
    #[serde(default)]
    pub format: u32,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub mc_version: String,
    /// "forge", "neoforge", "fabric", "quilt", or "" (vanilla).
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub updated_at: u64,
}

/// What became of a whitelist request (the web page shows a sentence for each).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JoinOutcome {
    /// The host will see it.
    Received,
    AlreadyListed,
    AlreadyRequested,
    /// The server isn't taking requests (switched off, or too many waiting).
    Closed,
    /// Not a Minecraft username.
    Invalid,
    /// On the server's ban list.
    Banned,
    #[serde(other)]
    Unknown,
}

/// Relay → agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum RelayMsg {
    Welcome {
        version: u32,
        base_domain: String,
        /// The relay's certificate also covers `*.<base_domain>`, so each name's whitelist page is `https://<name>.<base_domain>/`;
        /// without it, `https://<base_domain>/join/<name>`. Protocol 7 (older relays leave it out).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        name_pages: bool,
    },
    Authed {
        uuid: String,
        username: String,
        /// The relay lists this account as one of its admins (protocol 8): the launcher shows its Admin page.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        admin: bool,
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
    /// The answer to `files_offer`: players' downloads of those files come to this connection now. Protocol 8.
    FilesReady {},
    Pong { seq: u64 },
    /// The answer to `remote {enabled: true}`: this launcher is listed for its account's console. Protocol 6.
    RemoteReady {},
    /// A request from the web console; `params` is a JSON array. Protocol 6.
    Call { id: u64, method: String, params: serde_json::Value },
    /// Start or stop sending `push`es for a topic. Protocol 6.
    Watch { topic: String, on: bool },
    /// How many console tabs this account has open on this launcher. Protocol 6.
    Viewers { count: u32 },
    /// Where a domain stands (the answer to `domain_add` and `domain_check`). Protocol 7.
    DomainStatus(DomainStatus),
    /// The answer to `domain_remove`. Protocol 7.
    DomainRemoved { name: String },
    /// Where a listing stands (the answer to `listing_set`, `listing_get`, `listing_submit` and `listing_remove`).
    /// Protocol 8.
    ListingStatus(ListingStatus),
    /// The answer to `listing_mine`. Protocol 8.
    MyListings { listings: Vec<MyListing> },
    /// The answer to `listing_like`. Protocol 8.
    Liked { id: String, likes: u32, liked: bool },
    /// The answer to `listing_report`. Protocol 8.
    Reported { id: String },
    /// The answer to `listing_join`. Protocol 8.
    JoinAsked { id: String, outcome: JoinOutcome },
    /// The answer to `listing_joins`. Protocol 8.
    ListingJoins { joins: Vec<ListingJoinState> },
    /// The answer to `admin_unlock`. Protocol 8.
    AdminReady {
        /// The dashboard user the account is linked to (relays with dashboard users).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user: Option<String>,
        /// After `remember`: unlocks this PC without the password from now on (revocable on the dashboard).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_token: Option<String>,
    },
    /// The answer to `admin_call` `id`: `value` when ok, else `error`. Protocol 8.
    AdminResult {
        id: u64,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// A player asks on the web page to be whitelisted. Protocol 5, and only after `join_requests` turned on.
    JoinRequest {
        id: u64,
        username: String,
        /// From the server list (a signed-in player): send what the host decides with `join_decided`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        listing: bool,
    },
    Error {
        code: ErrorCode,
        message: String,
        /// With `banned`: how long and why (relays from before this field leave it out).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ban: Option<Ban>,
    },
    /// The relay is restarting: reconnect after this long.
    Shutdown { reconnect_ms: u64 },
}

/// Where a player's own domain stands on the relay (protocol 7).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainStatus {
    pub domain: String,
    /// The relay name it leads to.
    pub name: String,
    pub state: DomainState,
    /// The TXT record that proves the account owns the domain: `_rscraft.<domain>` = `rscraft-verify=<token>`.
    pub txt_name: String,
    pub txt_value: String,
    /// The domain's CNAME (or SRV) points at the name's address, so players arrive.
    pub route_ok: bool,
    /// The relay serves the whitelist page at `https://<domain>/` with a certificate of its own for the domain (it gets
    /// one once the domain is verified).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub https: bool,
    /// Another account has this domain verified; it moves here once this account's TXT shows up.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub elsewhere: bool,
    /// When the relay last looked up the domain's DNS (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<u64>,
    /// What's wrong, for players (e.g. "No TXT record found yet").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DomainState {
    /// Waiting for the TXT record: players aren't routed yet.
    Pending,
    /// Owned and pointing here: players are routed.
    Verified,
    /// Owned, but the CNAME/SRV points elsewhere (Vercel's "Invalid Configuration").
    Misconfigured,
    #[serde(other)]
    Unknown,
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
    /// Not a domain the relay can take: malformed, the relay's own, or too many for the account. Protocol 7.
    DomainInvalid,
    /// A listing field the relay won't take (the message says which), or a name that isn't this account's. Protocol 8.
    ListingInvalid,
    /// Too many submitted servers for this account. Protocol 8.
    ListingLimit,
    /// The relay's admin banned this account from the server list (not from the relay): `ban` says until when and
    /// why. Protocol 8.
    ListingBanned,
    /// Not an admin account, a wrong password, or too many wrong ones (locked for a while). Protocol 8.
    AdminDenied,
    /// The dashboard user has 2FA on: a code is needed, or the one sent was wrong. Protocol 8.
    AdminTwoFactor,
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

/// An account's ban, as the banned launcher is told.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ban {
    /// When it ends (Unix seconds); None: never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,
    /// The admin's words (may be empty).
    #[serde(default)]
    pub reason: String,
}

impl Ban {
    /// "until 2026-11-01 18:00 UTC" or "permanently", then the reason: the English text in the error, which is
    /// what launchers that don't know `ban` can show.
    pub fn describe(&self) -> String {
        let how_long = match self.until {
            Some(t) => format!("until {}", utc_text(t)),
            None => "permanently".to_string(),
        };
        match self.reason.trim() {
            "" => format!("this account is banned from the relay {how_long}"),
            why => format!("this account is banned from the relay {how_long}. Reason: {why}"),
        }
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
    /// An owner file download (protocol 8): the file's SHA-1. Send exactly `size` bytes of it, then close the stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The listing the file belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listing: Option<String>,
    /// The file's size in the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
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
    let (year, month, _) = civil(unix_secs);
    format!("{year:04}-{month:02}")
}

/// "2026-11-01 18:00 UTC" for a Unix time in seconds.
pub fn utc_text(unix_secs: u64) -> String {
    let (year, month, day) = civil(unix_secs);
    let secs = unix_secs % 86_400;
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", secs / 3600, secs % 3600 / 60)
}

/// (year, month, day) of a Unix time in seconds (UTC).
fn civil(unix_secs: u64) -> (i64, i64, i64) {
    // Howard Hinnant's days-to-civil algorithm.
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_messages_have_the_agreed_json() {
        let remote = AgentMsg::Remote { enabled: true, device: "PC".into(), app_version: "1.2.12".into() };
        assert_eq!(serde_json::to_value(&remote).unwrap(), serde_json::json!({"t": "remote", "enabled": true, "device": "PC", "app_version": "1.2.12"}));
        let result = AgentMsg::CallResult { id: 7, ok: true, value: Some(serde_json::json!([1])), error: None };
        assert_eq!(serde_json::to_value(&result).unwrap(), serde_json::json!({"t": "call_result", "id": 7, "ok": true, "value": [1], "error": null}));
        let push = AgentMsg::Push { topic: "servers".into(), data: serde_json::json!({"event": "list"}) };
        assert_eq!(serde_json::to_value(&push).unwrap()["t"], "push");
        let call: RelayMsg = serde_json::from_str(r#"{"t":"call","id":3,"method":"servers.list","params":[]}"#).unwrap();
        assert_eq!(call, RelayMsg::Call { id: 3, method: "servers.list".into(), params: serde_json::json!([]) });
        let watch: RelayMsg = serde_json::from_str(r#"{"t":"watch","topic":"log:smp","on":true}"#).unwrap();
        assert_eq!(watch, RelayMsg::Watch { topic: "log:smp".into(), on: true });
        assert_eq!(serde_json::from_str::<RelayMsg>(r#"{"t":"remote_ready"}"#).unwrap(), RelayMsg::RemoteReady {});
        assert_eq!(serde_json::from_str::<RelayMsg>(r#"{"t":"viewers","count":2}"#).unwrap(), RelayMsg::Viewers { count: 2 });
    }

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
        assert_eq!(utc_text(1_709_164_800 + 13 * 3600 + 5 * 60 + 59), "2024-02-29 13:05 UTC");
        assert_eq!(utc_text(1_735_689_599), "2024-12-31 23:59 UTC");
    }

    #[test]
    fn bans_on_the_wire() {
        let ban = RelayMsg::Error { code: ErrorCode::Banned, message: "x".into(), ban: Some(Ban { until: Some(1_735_689_600), reason: "griefing".into() }) };
        let text = serde_json::to_string(&ban).unwrap();
        assert_eq!(text, r#"{"t":"error","code":"banned","message":"x","ban":{"until":1735689600,"reason":"griefing"}}"#);
        assert_eq!(serde_json::from_str::<RelayMsg>(&text).unwrap(), ban);
        // Permanent; and from a relay that doesn't send `ban` yet.
        let forever: RelayMsg = serde_json::from_str(r#"{"t":"error","code":"banned","message":"x","ban":{}}"#).unwrap();
        assert!(matches!(forever, RelayMsg::Error { ban: Some(Ban { until: None, .. }), .. }));
        let old: RelayMsg = serde_json::from_str(r#"{"t":"error","code":"banned","message":"x"}"#).unwrap();
        assert!(matches!(old, RelayMsg::Error { ban: None, .. }));
        assert_eq!(Ban { until: Some(1_735_689_600), reason: " griefing ".into() }.describe(), "this account is banned from the relay until 2025-01-01 00:00 UTC. Reason: griefing");
        assert_eq!(Ban::default().describe(), "this account is banned from the relay permanently");
    }

    #[test]
    fn messages_round_trip() {
        let up = AgentMsg::Reserve { name: "abc".into(), release: None };
        let text = serde_json::to_string(&up).unwrap();
        assert_eq!(text, r#"{"t":"reserve","name":"abc"}"#);
        assert_eq!(serde_json::from_str::<AgentMsg>(&text).unwrap(), up);
        let down: RelayMsg = serde_json::from_str(r#"{"t":"error","code":"something_new","message":"x"}"#).unwrap();
        assert_eq!(down, RelayMsg::Error { code: ErrorCode::Unknown, message: "x".into(), ban: None });
        assert!(!ErrorCode::NameTaken.retryable() && ErrorCode::ShuttingDown.retryable());
        assert!(ErrorCode::Paused.retryable() && ErrorCode::ServiceOff.retryable() && !ErrorCode::SessionEnded.retryable());
        // Fields protocol 1 launchers don't know are skipped by them, and default when missing.
        let old: RelayMsg = serde_json::from_str(r#"{"t":"online","name":"a","address":"a.b","limits":{"tunnels":1,"names":1,"connections":1,"monthly_bytes":0},"usage":{"month":"2026-10","bytes":0,"connections":0}}"#).unwrap();
        assert!(matches!(old, RelayMsg::Online { secs_left: None, bedrock_port: None, limits: Limits { session_secs: 0, .. }, .. }));
        // A Java tunnel's `online` looks the same as in protocol 2; older relays ignore `bedrock`.
        assert_eq!(serde_json::to_string(&AgentMsg::Online { name: "a".into(), bedrock: false, geyser: false }).unwrap(), r#"{"t":"online","name":"a"}"#);
        assert_eq!(serde_json::from_str::<AgentMsg>(r#"{"t":"online","name":"a"}"#).unwrap(), AgentMsg::Online { name: "a".into(), bedrock: false, geyser: false });
        assert_eq!(serde_json::to_string(&AgentMsg::Online { name: "a".into(), bedrock: false, geyser: true }).unwrap(), r#"{"t":"online","name":"a","geyser":true}"#);
        assert_eq!(serde_json::to_string(&DataHeader { peer: None, udp: true, ..Default::default() }).unwrap(), r#"{"udp":true}"#);
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

    #[test]
    fn domain_status_round_trips() {
        let st = DomainStatus {
            domain: "play.example.com".into(),
            name: "smp".into(),
            state: DomainState::Pending,
            txt_name: "_rscraft.play.example.com".into(),
            txt_value: "rscraft-verify=abc".into(),
            route_ok: false,
            https: false,
            elsewhere: false,
            checked_at: Some(1),
            message: None,
        };
        let line = serde_json::to_string(&RelayMsg::DomainStatus(st.clone())).unwrap();
        assert!(line.starts_with(r#"{"t":"domain_status","#), "{line}");
        assert_eq!(serde_json::from_str::<RelayMsg>(&line).unwrap(), RelayMsg::DomainStatus(st));
        // A state from a newer relay reads as unknown.
        let newer = line.replace(r#""pending""#, r#""quarantined""#);
        assert!(matches!(serde_json::from_str::<RelayMsg>(&newer).unwrap(), RelayMsg::DomainStatus(DomainStatus { state: DomainState::Unknown, .. })));
        let add = serde_json::to_string(&AgentMsg::DomainAdd { name: "smp".into(), domain: "play.example.com".into() }).unwrap();
        assert_eq!(add, r#"{"t":"domain_add","name":"smp","domain":"play.example.com"}"#);
    }

    #[test]
    fn listing_messages_round_trip() {
        let info = ListingInfo {
            title: "Puirkle SMP".into(),
            tags: vec!["smp".into(), "friends".into()],
            region: "asia".into(),
            language: "th".into(),
            discord: Some("https://discord.gg/abc".into()),
            crossplay: true,
            offline: true,
            ..Default::default()
        };
        for m in [
            AgentMsg::ListingSet { name: "smp".into(), listing: Some(info.clone()) },
            AgentMsg::ListingSet { name: "smp".into(), listing: None },
            AgentMsg::ListingGet { name: "smp".into() },
            AgentMsg::ListingSubmit { address: "play.example.com".into(), listing: info.clone() },
            AgentMsg::ListingMine,
            AgentMsg::ListingRemove { id: "s-1".into() },
            AgentMsg::ListingLike { id: "rc-smp".into(), on: true },
            AgentMsg::ListingReport { id: "rc-smp".into(), reason: "spam".into() },
            AgentMsg::ListingJoin { id: "rc-smp".into() },
        ] {
            let line = serde_json::to_string(&m).unwrap();
            assert_eq!(serde_json::from_str::<AgentMsg>(&line).unwrap(), m, "{line}");
        }
        assert_eq!(serde_json::to_string(&AgentMsg::ListingMine).unwrap(), r#"{"t":"listing_mine"}"#);
        let status = ListingStatus { id: "rc-smp".into(), state: ListingState::Pending, message: None };
        for m in [
            RelayMsg::ListingStatus(status.clone()),
            RelayMsg::MyListings {
                listings: vec![MyListing { status, address: "smp.example".into(), listing: info, likes: 2, resincraft: true, pack: None }],
            },
            RelayMsg::Liked { id: "rc-smp".into(), likes: 3, liked: true },
            RelayMsg::Reported { id: "rc-smp".into() },
            RelayMsg::JoinAsked { id: "rc-smp".into(), outcome: JoinOutcome::Received },
        ] {
            let line = serde_json::to_string(&m).unwrap();
            assert_eq!(serde_json::from_str::<RelayMsg>(&line).unwrap(), m, "{line}");
        }
        // A minimal listing from the relay fills in the defaults; a state from a newer relay reads as unknown.
        let s: RelayMsg = serde_json::from_str(r#"{"t":"listing_status","id":"s-1","state":"frozen"}"#).unwrap();
        assert_eq!(s, RelayMsg::ListingStatus(ListingStatus { id: "s-1".into(), state: ListingState::Unknown, message: None }));
        let i: ListingInfo = serde_json::from_str(r#"{"title":"x"}"#).unwrap();
        assert_eq!(i.edition, Edition::Java);
    }}
