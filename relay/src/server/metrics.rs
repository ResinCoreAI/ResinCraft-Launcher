//! Counters for /metrics (Prometheus text format).

use std::fmt::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

macro_rules! metrics {
    ($($field:ident: $kind:ident = $help:literal),* $(,)?) => {
        #[derive(Default)]
        pub struct Metrics {
            $(pub $field: metrics!(@ty $kind),)*
        }

        impl Metrics {
            /// Gauges supplied by the caller (tunnels online…) come first.
            pub fn render(&self, gauges: &[(&str, &str, i64)]) -> String {
                let mut out = String::new();
                for (name, help, value) in gauges {
                    let _ = writeln!(out, "# HELP resincraft_relay_{name} {help}\n# TYPE resincraft_relay_{name} gauge\nresincraft_relay_{name} {value}");
                }
                $(
                    let _ = writeln!(
                        out,
                        "# HELP resincraft_relay_{0} {1}\n# TYPE resincraft_relay_{0} {2}\nresincraft_relay_{0} {3}",
                        stringify!($field),
                        $help,
                        metrics!(@name $kind),
                        self.$field.load(Ordering::Relaxed)
                    );
                )*
                out
            }
        }
    };
    (@ty counter) => { AtomicU64 };
    (@ty gauge) => { AtomicI64 };
    (@name counter) => { "counter" };
    (@name gauge) => { "gauge" };
}

metrics! {
    player_connections_total: counter = "Player connections accepted on the public port.",
    player_connections_active: gauge = "Players currently joined to a tunnel.",
    player_forwarded_total: counter = "Player connections forwarded to a tunnel.",
    offline_status_total: counter = "Server list pings answered by the relay because the tunnel was offline.",
    offline_login_total: counter = "Logins turned away because the tunnel was offline, full or over its limit.",
    legacy_pings_total: counter = "Pre-1.7 server list pings.",
    bad_handshakes_total: counter = "Connections that didn't send a valid Minecraft handshake in time.",
    rejected_banned_total: counter = "Connections refused because of a ban.",
    rejected_limit_total: counter = "Connections refused by a per-IP or global limit.",
    custom_domain_lookups_total: counter = "Handshakes routed through a CNAME lookup.",
    bytes_to_host_total: counter = "Bytes forwarded from players to hosts.",
    bytes_to_player_total: counter = "Bytes forwarded from hosts to players.",
    agent_connections_active: gauge = "Launchers connected to the agent port.",
    agent_connections_total: counter = "Launcher connections accepted.",
    logins_total: counter = "Successful sign-ins with a Minecraft token.",
    login_failures_total: counter = "Sign-ins Mojang refused or couldn't answer.",
    resumes_total: counter = "Sign-ins with a relay session token.",
}

impl Metrics {
    pub fn inc(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_prometheus_text() {
        let m = Metrics::default();
        Metrics::inc(&m.logins_total);
        m.player_connections_active.fetch_add(2, Ordering::Relaxed);
        let text = m.render(&[("tunnels_online", "Names online.", 3)]);
        assert!(text.contains("# TYPE resincraft_relay_logins_total counter\nresincraft_relay_logins_total 1"));
        assert!(text.contains("resincraft_relay_player_connections_active 2"));
        assert!(text.starts_with("# HELP resincraft_relay_tunnels_online Names online."));
    }
}
