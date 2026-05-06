// pharma-service: Windows service host that runs the api crate.
// On non-Windows (CI lint, dev), falls back to running the api binary path placeholder.

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    win::run()
}

#[cfg(not(windows))]
fn main() -> anyhow::Result<()> {
    eprintln!("pharma-service is only supported on Windows. Run `cargo run -p api` instead.");
    Ok(())
}

#[cfg(windows)]
mod win {
    use std::ffi::OsString;
    use std::time::Duration;

    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    const SERVICE_NAME: &str = "PharmaServer";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    pub fn run() -> anyhow::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
        Ok(())
    }

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        if let Err(e) = run_service() {
            tracing::error!(error = %e, "service exited with error");
        }
    }

    fn run_service() -> windows_service::Result<()> {
        let _ = telemetry::init("pharma-service");

        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop => {
                    let _ = shutdown_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            // TODO: launch api server here once api exposes a library entry. For scaffold,
            // we simply wait for the stop signal.
            tracing::info!("pharma-service running (scaffold stub)");
            tokio::task::spawn_blocking(move || {
                let _ = shutdown_rx.recv();
            })
            .await
            .ok();
        });

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        Ok(())
    }
}
