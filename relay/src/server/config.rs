//! relay.toml: domains, ports, limits, bans and admin tokens. See relay.example.toml for every option.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Players join `<name>.<base_domain>`.
    pub base_domain: String,
    /// Where players connect (the Minecraft port).
    pub public_listen: Vec<String>,
    /// Where launchers connect.
    pub agent_listen: Vec<String>,
    /// /healthz, /metrics and the admin API. Keep it on localhost.
    pub admin_listen: Option<String>,
    /// Name reservations, sessions, bans and traffic counters.
    pub data_file: PathBuf,
    /// "text" or "json".
    pub log_format: String,
    /// Also write the log to daily files: "logs/relay.log" becomes logs/relay.2026-10-18.log and so on.
    pub log_file: Option<PathBuf>,
    /// How many daily log files to keep.
    pub log_keep_days: usize,
    /// TLS on the agent port. Leave out to run plain TCP behind a TLS terminator (or for local tests).
    pub tls: Option<TlsConfig>,
    pub limits: LimitsConfig,
    pub auth: AuthConfig,
    pub bans: BanConfig,
    pub admin: AdminConfig,
    pub custom_domains: CustomDomainConfig,
    pub dashboard: DashboardConfig,
    /// On shutdown, how long players already connected may keep playing before the relay exits.
    pub drain_secs: u64,
    /// The file this config was read from (relative paths in it are next to it; the dashboard re-reads the
    /// admin password hash from it, so `set-admin-password` needs no restart).
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsConfig {
    /// PEM certificate chain (Let's Encrypt: fullchain.pem). Re-read when it changes.
    pub cert: PathBuf,
    /// PEM private key (Let's Encrypt: privkey.pem).
    pub key: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsConfig {
    pub tunnels_per_account: u32,
    pub names_per_account: u32,
    pub connections_per_tunnel: u32,
    /// Players connected from one IP address at once (across all tunnels).
    pub connections_per_ip: u32,
    /// Launchers connected from one IP address at once.
    pub agents_per_ip: u32,
    /// Traffic per account per calendar month (UTC), in GB (10^9 bytes); 0 = unlimited.
    pub monthly_gb_per_account: f64,
    pub handshake_timeout_secs: u64,
    /// A player connection with no traffic either way for this long is closed.
    pub idle_timeout_secs: u64,
    /// Sign-ins with a Minecraft token per IP address per hour (each one asks Mojang).
    pub logins_per_ip_per_hour: u32,
    /// Names nobody has brought online for this long are freed again.
    pub name_expiry_days: u64,
    /// How long a relay session stays valid without being used.
    pub session_days: u64,
    /// Players still sending their handshake, across the whole relay.
    pub max_pending_handshakes: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuthConfig {
    /// Where the relay checks who a Minecraft token belongs to.
    pub profile_url: String,
    /// Names nobody may reserve, on top of the built-in list.
    pub reserved_names: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BanConfig {
    /// Minecraft UUIDs (with or without dashes).
    pub uuids: Vec<String>,
    /// IP addresses or CIDR ranges ("203.0.113.7", "198.51.100.0/24", "2001:db8::/32").
    pub ips: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdminConfig {
    /// Bearer tokens for the /admin API (at least 24 characters each). Empty: the admin API is off.
    pub tokens: Vec<String>,
}

/// The admin dashboard: its own port, HTTPS with the [tls] certificate.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashboardConfig {
    /// Where it listens, e.g. "0.0.0.0:7443" (or "127.0.0.1:7443" to reach it only through an SSH tunnel or
    /// Remote Desktop). Leave out to turn it off. Without [tls] it only runs on a loopback address.
    pub listen: Option<String>,
    /// Argon2 hash of the admin password. Set it with `resincraft-relay set-admin-password`.
    pub password_hash: String,
    /// Only these IP addresses / CIDR ranges may open it (empty: any address; the password still applies).
    pub allow_ips: Vec<String>,
    /// How long a sign-in lasts.
    pub session_hours: u64,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        DashboardConfig { listen: None, password_hash: String::new(), allow_ips: Vec::new(), session_hours: 12 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomDomainConfig {
    /// Route a player's own domain (play.example.com) whose CNAME points at `<name>.<base_domain>`.
    pub enabled: bool,
    /// DNS resolvers asked for those CNAMEs.
    pub resolvers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            base_domain: "rscraft.resincore.tech".into(),
            public_listen: vec!["0.0.0.0:25565".into()],
            agent_listen: vec!["0.0.0.0:7000".into()],
            admin_listen: Some("127.0.0.1:7001".into()),
            data_file: PathBuf::from("data/relay.json"),
            log_format: "text".into(),
            log_file: None,
            log_keep_days: 14,
            tls: None,
            limits: LimitsConfig::default(),
            auth: AuthConfig::default(),
            bans: BanConfig::default(),
            admin: AdminConfig::default(),
            custom_domains: CustomDomainConfig::default(),
            dashboard: DashboardConfig::default(),
            drain_secs: 10,
            path: None,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        LimitsConfig {
            tunnels_per_account: 3,
            names_per_account: 5,
            connections_per_tunnel: 64,
            connections_per_ip: 16,
            agents_per_ip: 16,
            monthly_gb_per_account: 100.0,
            handshake_timeout_secs: 5,
            idle_timeout_secs: 300,
            logins_per_ip_per_hour: 20,
            name_expiry_days: 180,
            session_days: 30,
            max_pending_handshakes: 2048,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig { profile_url: "https://api.minecraftservices.com/minecraft/profile".into(), reserved_names: Vec::new() }
    }
}

impl Default for CustomDomainConfig {
    fn default() -> Self {
        CustomDomainConfig { enabled: true, resolvers: vec!["1.1.1.1:53".into(), "8.8.8.8:53".into()] }
    }
}

impl LimitsConfig {
    pub fn monthly_bytes(&self) -> u64 {
        if self.monthly_gb_per_account <= 0.0 {
            0
        } else {
            (self.monthly_gb_per_account * 1e9) as u64
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).with_context(|| format!("couldn't read {}", path.display()))?;
        let mut cfg: Config = toml::from_str(&text).with_context(|| format!("{} isn't valid", path.display()))?;
        // Relative paths (data, certificates, logs) are next to the config file, wherever the relay is started.
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let near = |p: &mut PathBuf| {
            if p.is_relative() {
                *p = dir.join(&*p);
            }
        };
        near(&mut cfg.data_file);
        if let Some(t) = cfg.tls.as_mut() {
            near(&mut t.cert);
            near(&mut t.key);
        }
        if let Some(l) = cfg.log_file.as_mut() {
            near(l);
        }
        cfg.path = Some(path.to_path_buf());
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&mut self) -> Result<()> {
        self.base_domain = self.base_domain.trim().trim_matches('.').to_ascii_lowercase();
        if !crate::handshake::looks_like_domain(&self.base_domain) {
            bail!("base_domain must be a domain name like play.example.net");
        }
        if self.public_listen.is_empty() || self.agent_listen.is_empty() {
            bail!("public_listen and agent_listen need at least one address");
        }
        for t in &self.admin.tokens {
            if t.len() < 24 {
                bail!("admin tokens must be at least 24 characters (try: openssl rand -hex 32)");
            }
        }
        if !matches!(self.log_format.as_str(), "text" | "json") {
            bail!("log_format must be \"text\" or \"json\"");
        }
        if self.limits.tunnels_per_account == 0 || self.limits.names_per_account == 0 {
            bail!("tunnels_per_account and names_per_account must be at least 1");
        }
        self.auth.reserved_names = self.auth.reserved_names.iter().map(|n| n.trim().to_ascii_lowercase()).collect();
        if let Some(l) = self.dashboard.listen.as_deref() {
            let addr: std::net::SocketAddr = l.trim().parse().map_err(|_| anyhow::anyhow!("[dashboard] listen must be an address like 0.0.0.0:7443"))?;
            if self.tls.is_none() && !addr.ip().is_loopback() {
                bail!("the dashboard needs [tls] (HTTPS) unless it listens on a loopback address like 127.0.0.1:7443");
            }
        }
        for ip in self.dashboard.allow_ips.iter().chain(&self.bans.ips) {
            if ip.split('/').next().is_none_or(|a| a.trim().parse::<std::net::IpAddr>().is_err()) {
                bail!("{ip} isn't an IP address or CIDR range");
            }
        }
        if self.dashboard.session_hours == 0 {
            bail!("[dashboard] session_hours must be at least 1");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_example_config() {
        let text = include_str!("../../relay.example.toml");
        let mut cfg: Config = toml::from_str(text).unwrap();
        cfg.validate().unwrap();
        assert_eq!(cfg.base_domain, "rscraft.resincore.tech");
        assert_eq!(cfg.limits.monthly_bytes(), 100_000_000_000);
        assert!(cfg.tls.is_some());
        assert_eq!(cfg.dashboard.listen.as_deref(), Some("0.0.0.0:7443"));
    }

    #[test]
    fn windows_example_resolves_paths_next_to_the_config() {
        let dir = std::env::temp_dir().join(format!("resincraft-relay-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.toml");
        std::fs::write(&path, include_str!("../../deploy/windows/relay.toml.example")).unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.base_domain, "rscraft.resincore.tech");
        assert_eq!(cfg.data_file, dir.join("data/relay.json"));
        assert_eq!(cfg.log_file.as_deref(), Some(dir.join("logs/relay.log").as_path()));
        let tls = cfg.tls.unwrap();
        assert_eq!((tls.cert, tls.key), (dir.join("certs/relay-chain.pem"), dir.join("certs/relay-key.pem")));
        assert_eq!(cfg.dashboard.listen.as_deref(), Some("0.0.0.0:7443"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_bad_values() {
        let mut cfg: Config = toml::from_str("base_domain = \"localhost\"").unwrap();
        assert!(cfg.validate().is_err());
        assert!(toml::from_str::<Config>("unknown_option = 1").is_err());
        let mut short: Config = toml::from_str("[admin]\ntokens = [\"short\"]").unwrap();
        assert!(short.validate().is_err());
        // A dashboard on a public address without TLS would send the password in the clear.
        let mut open: Config = toml::from_str("[dashboard]\nlisten = \"0.0.0.0:7443\"").unwrap();
        assert!(open.validate().is_err());
        let mut local: Config = toml::from_str("[dashboard]\nlisten = \"127.0.0.1:7443\"").unwrap();
        assert!(local.validate().is_ok());
        let mut bad_ip: Config = toml::from_str("[dashboard]\nallow_ips = [\"nope\"]").unwrap();
        assert!(bad_ip.validate().is_err());
    }
}
