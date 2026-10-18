//! Running as a Windows service ("ResinCraftRelay"): the Service Control Manager starts
//! `resincraft-relay.exe --config <relay.toml> service`; Stop-Service (or a shutdown) asks the relay to stop
//! gracefully: launchers are told to reconnect, players get drain_secs, the data is saved. Restarting after a
//! crash is the SCM's recovery actions (install-relay.ps1 sets them).

use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use resincraft_relay::server::{Config, Server};
use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

pub const NAME: &str = "ResinCraftRelay";

static CONFIG: OnceLock<Mutex<Option<Config>>> = OnceLock::new();

define_windows_service!(ffi_service_main, service_main);

/// Hands the process to the SCM; returns when the service has stopped.
pub fn run(cfg: Config) -> anyhow::Result<()> {
    CONFIG.get_or_init(|| Mutex::new(Some(cfg)));
    service_dispatcher::start(NAME, ffi_service_main)?;
    Ok(())
}

fn status(state: ServiceState, exit: u32, wait: Duration) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running { ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN } else { ServiceControlAccept::empty() },
        exit_code: if exit == 0 { ServiceExitCode::Win32(0) } else { ServiceExitCode::ServiceSpecific(exit) },
        checkpoint: 0,
        wait_hint: wait,
        process_id: None,
    }
}

fn service_main(_args: Vec<OsString>) {
    let Some(cfg) = CONFIG.get().and_then(|c| c.lock().unwrap().take()) else { return };
    let drain = Duration::from_secs(cfg.drain_secs + 10);
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let handler = move |event| match event {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = stop_tx.send(true);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let handle = match service_control_handler::register(NAME, handler) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("couldn't register the service handler: {e}");
            return;
        }
    };
    let _ = handle.set_service_status(status(ServiceState::StartPending, 0, Duration::from_secs(30)));
    let result = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(anyhow::Error::from).and_then(|rt| {
        rt.block_on(async {
            let server = Server::bind(cfg).await?;
            let _ = handle.set_service_status(status(ServiceState::Running, 0, Duration::ZERO));
            let mut rx = stop_rx;
            let handle2 = &handle;
            server
                .run(async move {
                    while !*rx.borrow() {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                    let _ = handle2.set_service_status(status(ServiceState::StopPending, 0, drain));
                    tracing::info!("Windows asked the service to stop");
                })
                .await
        })
    });
    let exit = match result {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!("the relay stopped: {e:#}");
            1
        }
    };
    let _ = handle.set_service_status(status(ServiceState::Stopped, exit, Duration::ZERO));
}
