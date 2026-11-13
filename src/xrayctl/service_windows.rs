use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use super::{regen::look_path, state};

const SERVICE_NAME: &str = "xray";

pub fn maybe_restart(no_restart: bool) -> Result<(), String> {
    if no_restart {
        return Ok(());
    }
    if service_state() != "Running" {
        return Ok(());
    }
    let nssm = nssm_path()?;
    run_nssm(&nssm, &["restart", SERVICE_NAME])?;
    println!("xray restarted");
    Ok(())
}

pub fn service_state() -> String {
    unsafe {
        let manager = windows_sys::Win32::System::Services::OpenSCManagerW(
            std::ptr::null(),
            windows_sys::Win32::System::Services::SC_MANAGER_CONNECT,
        );
        if manager.is_null() {
            return "unknown".into();
        }
        let name: Vec<u16> = SERVICE_NAME.encode_utf16().chain([0]).collect();
        let service = windows_sys::Win32::System::Services::OpenServiceW(
            manager,
            name.as_ptr(),
            windows_sys::Win32::System::Services::SERVICE_QUERY_STATUS,
        );
        if service.is_null() {
            windows_sys::Win32::System::Services::CloseServiceHandle(manager);
            return "not installed".into();
        }
        let mut status = windows_sys::Win32::System::Services::SERVICE_STATUS {
            dwServiceType: 0,
            dwCurrentState: 0,
            dwControlsAccepted: 0,
            dwWin32ExitCode: 0,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: 0,
        };
        let result = windows_sys::Win32::System::Services::QueryServiceStatus(service, &mut status);
        windows_sys::Win32::System::Services::CloseServiceHandle(service);
        windows_sys::Win32::System::Services::CloseServiceHandle(manager);
        if result == 0 {
            return "unknown".into();
        }
        use windows_sys::Win32::System::Services as svc;
        match status.dwCurrentState {
            x if x == svc::SERVICE_STOPPED => "Stopped",
            x if x == svc::SERVICE_START_PENDING => "StartPending",
            x if x == svc::SERVICE_STOP_PENDING => "StopPending",
            x if x == svc::SERVICE_RUNNING => "Running",
            x if x == svc::SERVICE_CONTINUE_PENDING => "ContinuePending",
            x if x == svc::SERVICE_PAUSE_PENDING => "PausePending",
            x if x == svc::SERVICE_PAUSED => "Paused",
            _ => "unknown",
        }
        .into()
    }
}

pub fn sync_tproxy_env(_port: i64, _changed: bool) -> Result<(), String> {
    Ok(())
}

pub fn install_service() -> Result<(), String> {
    let nssm = nssm_path()?;
    let exe = look_path("xray").ok_or("xray not found")?;
    let config = state::config_path().to_string_lossy().to_string();
    run_nssm(
        &nssm,
        &["install", SERVICE_NAME, &exe.to_string_lossy(), &format!("run -c {config}")],
    )?;
    run_nssm(&nssm, &["set", SERVICE_NAME, "AppDirectory", &directory(&exe)])?;
    run_nssm(&nssm, &["start"])
}

pub fn uninstall_service() -> Result<(), String> {
    let nssm = nssm_path()?;
    run_nssm(&nssm, &["stop"])?;
    run_nssm(&nssm, &["remove", SERVICE_NAME, "confirm"])
}

fn directory(exe: &std::path::Path) -> String {
    exe.parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn run_nssm(nssm: &PathBuf, args: &[&str]) -> Result<(), String> {
    let status = Command::new(nssm)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|_| format!("nssm {} failed", args[0]))?;
    if !status.success() {
        return Err(format!("nssm {} failed", args[0]));
    }
    Ok(())
}

pub fn nssm_path() -> Result<PathBuf, String> {
    if let Some(path) = look_path("nssm") {
        return Ok(path);
    }
    let root = std::env::var("SCOOP")
        .ok()
        .and_then(|scoop| if scoop.is_empty() { None } else { Some(PathBuf::from(scoop)) })
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .map(|root| root.join("scoop"))
        .ok_or("nssm not found; install it with the nssm installer or: scoop install nssm")?;
    let candidate = root.join("apps").join("nssm").join("current").join("nssm.exe");
    if candidate.exists() {
        return Ok(candidate);
    }
    Err("nssm not found; install it with the nssm installer or: scoop install nssm".into())
}

pub fn require_admin() -> Result<(), String> {
    unsafe {
        let mut token = std::ptr::null_mut();
        let process = windows_sys::Win32::System::Threading::GetCurrentProcess();
        if windows_sys::Win32::System::Threading::OpenProcessToken(
            process,
            windows_sys::Win32::Security::TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return Err("Service management requires an elevated (administrator) terminal".into());
        }
        let mut elevation = windows_sys::Win32::Security::TOKEN_ELEVATION {
            TokenIsElevated: 0,
        };
        let mut length = 0u32;
        let result = windows_sys::Win32::Security::GetTokenInformation(
            token,
            windows_sys::Win32::Security::TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<windows_sys::Win32::Security::TOKEN_ELEVATION>() as u32,
            &mut length,
        );
        windows_sys::Win32::Foundation::CloseHandle(token);
        if result == 0 || elevation.TokenIsElevated == 0 {
            return Err("Service management requires an elevated (administrator) terminal".into());
        }
    }
    Ok(())
}

pub fn service_action(action: &str) -> Result<(), String> {
    if action != "start" && action != "stop" && action != "restart" {
        return Err("Action must be start, stop, or restart".into());
    }
    require_admin()?;
    let nssm = nssm_path()?;
    run_nssm(&nssm, &[action])
}
