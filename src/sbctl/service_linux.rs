use std::process::Command;

use crate::xrayctl::{regen::look_path, service_linux::stdin_is_terminal};

const SERVICE: &str = "sing-box";
const SERVICE_CONFIG: &str = "/etc/sing-box/config.json";

pub fn maybe_restart(no_restart: bool) -> Result<(), String> {
    if no_restart {
        return Ok(());
    }
    if look_path("systemctl").is_none() {
        return Ok(());
    }
    if !Command::new("systemctl")
        .args(["is-active", "--quiet", SERVICE])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if !stdin_is_terminal() {
        println!("Config installed; apply it with: sbctl restart");
        return Ok(());
    }
    service_action("restart")
}

pub fn service_action(action: &str) -> Result<(), String> {
    if action != "start" && action != "stop" && action != "restart" {
        return Err("Action must be start, stop, or restart".into());
    }
    if look_path("systemctl").is_none() {
        return Err("systemctl not found".into());
    }
    if look_path("sudo").is_none() {
        return Err(format!("sudo is required to {action} {SERVICE}"));
    }
    let status = Command::new("sudo")
        .args(["systemctl", action, SERVICE])
        .status()
        .map_err(|e| format!("failed to {action} {SERVICE}: {e}"))?;
    if !status.success() {
        return Err(format!("failed to {action} {SERVICE}: exited with {status}"));
    }
    let past = match action {
        "start" => "started",
        "stop" => "stopped",
        _ => "restarted",
    };
    println!("{SERVICE} {past}");
    Ok(())
}

pub fn service_state() -> String {
    if look_path("systemctl").is_none() {
        return "unavailable".into();
    }
    match Command::new("systemctl").args(["is-active", SERVICE]).output() {
        Ok(output) => {
            let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if state.is_empty() {
                "unknown".into()
            } else {
                state
            }
        }
        Err(_) => "unknown".into(),
    }
}

pub fn install_config() -> Result<(), String> {
    if look_path("sudo").is_none() {
        return Err(format!("sudo is required to install {SERVICE_CONFIG}"));
    }
    let status = Command::new("sudo")
        .args(["install", "-D", "-m", "0640", "-o", "root", "-g", SERVICE])
        .arg(super::state::config_path())
        .arg(SERVICE_CONFIG)
        .status()
        .map_err(|e| format!("failed to install {SERVICE_CONFIG}: {e}"))?;
    if !status.success() {
        return Err(format!("failed to install {SERVICE_CONFIG}: exited with {status}"));
    }
    println!("Installed {SERVICE_CONFIG}");
    Ok(())
}
