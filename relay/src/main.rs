//! resincraft-relay [--config <file>] [check | health | set-admin-password [--stdin] | service]
//!
//! Runs ResinCraft Relay with the given config (default: $RESINCRAFT_RELAY_CONFIG, else relay.toml next to the
//! program, else ./relay.toml).
//! - `check` validates the config and the TLS files (and that the ports are free), then exits.
//! - `health` asks a running relay's local admin port for /healthz (exit code 0 when healthy).
//! - `set-admin-password` asks for the admin dashboard's password (or reads one line from stdin with
//!   `--stdin`) and writes its Argon2 hash into the config. A running relay picks it up at the next sign-in.
//! - `service` runs as the Windows service (install-relay.ps1 registers it); Stop-Service shuts it down
//!   gracefully.
//!
//! SIGTERM / SIGINT (Ctrl+C) shut it down gracefully; see the README for restarts and upgrades.

#[cfg(windows)]
mod winservice;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use resincraft_relay::server::{dashboard, Config, Server};

enum Command {
    Run,
    Check,
    Health,
    SetPassword { stdin: bool },
    #[cfg(windows)]
    Service,
}

fn default_config() -> PathBuf {
    if let Some(p) = std::env::var_os("RESINCRAFT_RELAY_CONFIG") {
        return PathBuf::from(p);
    }
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("relay.toml")));
    beside.filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("relay.toml"))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut config = default_config();
    let mut command = Command::Run;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => match args.next() {
                Some(p) => config = PathBuf::from(p),
                None => return fail("--config needs a file"),
            },
            "check" => command = Command::Check,
            "health" => command = Command::Health,
            "set-admin-password" => command = Command::SetPassword { stdin: false },
            "--stdin" => match &mut command {
                Command::SetPassword { stdin } => *stdin = true,
                _ => return fail("--stdin goes with set-admin-password"),
            },
            #[cfg(windows)]
            "service" => command = Command::Service,
            "--version" | "-V" => {
                println!("resincraft-relay {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!("usage: resincraft-relay [--config <file>] [check | health | set-admin-password [--stdin] | service]");
                return ExitCode::SUCCESS;
            }
            other => return fail(&format!("unknown argument {other} (try --help)")),
        }
    }
    if let Command::SetPassword { stdin } = command {
        return match set_password(&config, stdin) {
            Ok(()) => {
                println!("Admin password saved in {}. Sign in to the dashboard with it (no restart needed).", config.display());
                ExitCode::SUCCESS
            }
            Err(e) => fail(&format!("{e:#}")),
        };
    }
    let cfg = match Config::load(&config) {
        Ok(c) => c,
        Err(e) => return fail(&format!("{e:#}")),
    };
    match command {
        Command::Health => match check_health(cfg.admin_listen.as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&format!("unhealthy: {e}")),
        },
        #[cfg(windows)]
        Command::Service => {
            let _logs = init_logs(&cfg);
            match winservice::run(cfg) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => fail(&format!("{e:#} (the service command is meant to be started by Windows; use install-relay.ps1)")),
            }
        }
        Command::Check => run(cfg, true),
        _ => {
            let _logs = init_logs(&cfg);
            run(cfg, false)
        }
    }
}

fn run(cfg: Config, check_only: bool) -> ExitCode {
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return fail(&e.to_string()),
    };
    let path = cfg.path.clone().unwrap_or_default();
    let result = rt.block_on(async move {
        let server = Server::bind(cfg).await?;
        if check_only {
            println!("config OK: {}", path.display());
            return Ok(());
        }
        server.run(shutdown_signal()).await
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            fail(&format!("{e:#}"))
        }
    }
}

fn set_password(config: &std::path::Path, stdin: bool) -> anyhow::Result<()> {
    if !config.is_file() {
        anyhow::bail!("{} doesn't exist yet: create it from the example config first", config.display());
    }
    let password = if stdin {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        line.trim_end_matches(['\r', '\n']).to_string()
    } else {
        let first = rpassword::prompt_password("New admin password: ")?;
        let again = rpassword::prompt_password("Again: ")?;
        if first != again {
            anyhow::bail!("the two passwords differ");
        }
        first
    };
    let hash = dashboard::hash_password(&password)?;
    dashboard::write_password_hash(config, &hash)
}

/// GET /healthz on the admin port (an unspecified listen address means this machine).
fn check_health(admin: Option<&str>) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
    use std::time::Duration;
    let admin = admin.ok_or("admin_listen isn't set")?;
    let mut addr: SocketAddr = admin.parse().map_err(|_| format!("admin_listen {admin} isn't an address"))?;
    if addr.ip().is_unspecified() {
        addr.set_ip(if addr.is_ipv4() { IpAddr::V4(Ipv4Addr::LOCALHOST) } else { IpAddr::V6(Ipv6Addr::LOCALHOST) });
    }
    let mut sock = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| format!("{addr}: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_secs(3))).map_err(|e| e.to_string())?;
    sock.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").map_err(|e| e.to_string())?;
    let mut reply = String::new();
    let _ = sock.read_to_string(&mut reply);
    if reply.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        Err(reply.lines().next().unwrap_or("no answer").to_string())
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("resincraft-relay: {msg}");
    ExitCode::FAILURE
}

/// Logs to stdout and, with log_file, to daily files too (kept for log_keep_days). Keep the guard alive: it
/// writes out what's buffered when dropped.
fn init_logs(cfg: &Config) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (file, guard) = match &cfg.log_file {
        Some(path) => {
            let dir = path.parent().map(PathBuf::from).unwrap_or_default();
            let prefix = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "relay".into());
            let suffix = path.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "log".into());
            match tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix(prefix)
                .filename_suffix(suffix)
                .max_log_files(cfg.log_keep_days.max(1))
                .build(&dir)
            {
                Ok(appender) => {
                    let (writer, guard) = tracing_appender::non_blocking(appender);
                    (Some(writer), Some(guard))
                }
                Err(e) => {
                    eprintln!("resincraft-relay: can't write logs to {}: {e}", dir.display());
                    (None, None)
                }
            }
        }
        None => (None, None),
    };
    let ansi = std::io::stdout().is_terminal() && file.is_none();
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_target(false).with_ansi(ansi);
    match (cfg.log_format.as_str(), file) {
        ("json", Some(f)) => builder.json().flatten_event(true).with_writer(std::io::stdout.and(f)).init(),
        ("json", None) => builder.json().flatten_event(true).init(),
        (_, Some(f)) => builder.with_writer(std::io::stdout.and(f)).init(),
        (_, None) => builder.init(),
    }
    guard
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = term.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
