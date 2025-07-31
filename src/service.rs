use tokio_util::sync::CancellationToken;

use anyhow::{Result, anyhow};
use std::{ffi::OsString, thread, time::Duration};
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

use crate::config::Config;
use crate::server::server_executor;

const SERVICE_NAME: &str = "socks5ws_srv";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;
const SERVICE_DISPLAY: &str = "socks5ws proxy";
const SERVICE_DESCRIPTION: &str = "SOCKS5 proxy windows service";

trait ServiceStatusEx {
    fn running() -> ServiceStatus;
    fn stopped() -> ServiceStatus;
    fn stopped_with_error(code: u32) -> ServiceStatus;
}

impl ServiceStatusEx for ServiceStatus {
    fn running() -> ServiceStatus {
        ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        }
    }

    fn stopped() -> ServiceStatus {
        ServiceStatus {
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            ..Self::running()
        }
    }

    fn stopped_with_error(code: u32) -> ServiceStatus {
        ServiceStatus {
            exit_code: ServiceExitCode::ServiceSpecific(code),
            ..Self::stopped()
        }
    }
}

pub fn install() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_binary_path = std::env::current_exe()?;

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: OsString::from(SERVICE_DISPLAY),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec!["run".into()],
        dependencies: vec![],
        account_name: Some(OsString::from(r#"NT AUTHORITY\NetworkService"#)),
        account_password: None,
    };
    let service = service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description(SERVICE_DESCRIPTION)?;
    log::info!("service installed");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager.open_service(SERVICE_NAME, service_access)?;

    let service_status = service.query_status()?;
    if service_status.current_state != ServiceState::Stopped {
        log::warn!("stopping service");
        service.stop()?;
        // Wait for service to stop
        thread::sleep(Duration::from_secs(5));
    }

    service.delete()?;
    log::warn!("service deleted");
    Ok(())
}

pub fn stop() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP;
    let service = service_manager.open_service(SERVICE_NAME, service_access)?;

    let service_status = service.query_status()?;
    if service_status.current_state != ServiceState::Stopped {
        log::info!("stopping service");
        service.stop()?;
    }
    Ok(())
}

pub fn start() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::START;
    let service = service_manager.open_service(SERVICE_NAME, service_access)?;

    let service_status = service.query_status()?;
    if service_status.current_state != ServiceState::Running {
        log::info!("start service");
        service.start(Vec::<&str>::new().as_slice())?;
    }
    Ok(())
}

pub fn run() -> Result<()> {
    // Register generated `ffi_service_main` with the system and start the service, blocking
    // this thread until the service is stopped.
    log::info!("service run");
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;

    Ok(())
}

// Generate the windows service boilerplate.
// The boilerplate contains the low-level service entry function (ffi_service_main) that parses
// incoming service arguments into Vec<OsString> and passes them to user defined service
// entry (my_service_main).
define_windows_service!(ffi_service_main, my_service_main);

// Service entry function which is called on background thread by the system with service
// parameters. There is no stdout or stderr at this point so make sure to configure the log
// output to file if needed.
pub fn my_service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        log::error!("error: {e}");
    }
}

pub fn run_service() -> Result<()> {
    // Create a cancellation token to be able to cancell server
    let control_token = CancellationToken::new();
    let server_token = control_token.child_token();

    // Define system service event handler that will be receiving service events.
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            // Notifies a service to report its current status information to the service
            // control manager. Always return NoError even if not implemented.
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,

            // Handle stop
            ServiceControl::Stop => {
                log::info!("service stop event received");
                control_token.cancel();
                ServiceControlHandlerResult::NoError
            }

            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler.
    // The returned status handle should be used to report service status changes to the system.
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Tell the system that service is running
    status_handle.set_service_status(ServiceStatus::running())?;

    let cfg = Config::get();
    log::info!("start with config: {cfg:#?}");

    let result = std::thread::spawn(move || server_executor(cfg, server_token)).join();

    log::info!("server thread stoped");

    // join() => Err(), when thread panic
    if let Err(e) = result {
        log::error!("server panic: {e:#?}");
        status_handle.set_service_status(ServiceStatus::stopped_with_error(1))?;
        return Err(anyhow!("server panic"));
    }

    // join() => Ok(Err()), when server executor error
    if let Err(e) = result.unwrap() {
        log::error!("server error: {e:#?}");
        status_handle.set_service_status(ServiceStatus::stopped_with_error(2))?;
        return Err(anyhow!("server error"));
    }

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus::stopped())?;

    log::info!("service stoped");
    Ok(())
}
