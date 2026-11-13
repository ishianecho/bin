use std::{
    io::Write,
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
};

use super::regen::look_path;

const TPROXY_ENV_FILE: &str = "/usr/local/etc/xray/tproxy.env";
const TPROXY_SCRIPT: &str = "/usr/local/etc/xray/tproxy.sh";
const TPROXY_FWMARK_RX: &str = "fwmark 0x1 lookup 100";
const TPROXY_GROUP: &str = "xray_tproxy";

pub fn maybe_restart(no_restart: bool) -> Result<(), String> {
    if no_restart {
        return Ok(());
    }
    if look_path("systemctl").is_none() {
        return Ok(());
    }
    if !Command::new("systemctl")
        .args(["is-active", "--quiet", "xray"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if !stdin_is_terminal() {
        println!("Config regenerated; apply it with: xrayctl restart");
        return Ok(());
    }
    service_action("restart")
}

pub fn stdin_is_terminal() -> bool {
    unsafe { libc_isatty(0) }
}

#[cfg(unix)]
unsafe fn libc_isatty(fd: i32) -> bool {
    extern "C" {
        fn isatty(fd: i32) -> i32;
    }
    unsafe { isatty(fd) == 1 }
}

pub fn service_action(action: &str) -> Result<(), String> {
    if action != "start" && action != "stop" && action != "restart" {
        return Err("Action must be start, stop, or restart".into());
    }
    if look_path("systemctl").is_none() {
        return Err("systemctl not found".into());
    }
    if look_path("sudo").is_none() {
        return Err(format!("sudo is required to {action} xray"));
    }
    let status = Command::new("sudo")
        .args(["systemctl", action, "xray"])
        .status()
        .map_err(|e| format!("failed to {action} xray: {e}"))?;
    if !status.success() {
        return Err(format!("failed to {action} xray: exited with {status}"));
    }
    if action == "stop" {
        teardown_tproxy()?;
    }
    println!("xray {action}ed");
    Ok(())
}

fn teardown_tproxy() -> Result<(), String> {
    if !tproxy_routing_active() {
        return Ok(());
    }
    if !std::path::Path::new(TPROXY_SCRIPT).exists() {
        return Err(format!(
            "tproxy rules are still active but {TPROXY_SCRIPT} is missing; network traffic will be black-holed until they are removed"
        ));
    }
    let status = Command::new("sudo")
        .arg(TPROXY_SCRIPT)
        .arg("stop")
        .status()
        .map_err(|e| format!("failed to tear down the tproxy rules: {e}"))?;
    if !status.success() {
        return Err(format!(
            "failed to tear down the tproxy rules: exited with {status}"
        ));
    }
    println!("tproxy rules removed");
    Ok(())
}

fn tproxy_routing_active() -> bool {
    Command::new("ip")
        .args(["rule", "show"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(TPROXY_FWMARK_RX))
        .unwrap_or(false)
}

pub fn service_state() -> String {
    if look_path("systemctl").is_none() {
        return "unavailable".into();
    }
    match Command::new("systemctl").args(["is-active", "xray"]).output() {
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

pub fn sync_tproxy_env(port: i64, changed: bool) -> Result<(), String> {
    if !changed {
        return Ok(());
    }
    write_tproxy_env(port)
}

fn write_tproxy_env(port: i64) -> Result<(), String> {
    if look_path("sudo").is_none() {
        return Err(format!("sudo is required to update {TPROXY_ENV_FILE}"));
    }
    let mut child = Command::new("sudo")
        .arg("tee")
        .arg(TPROXY_ENV_FILE)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|_| format!("failed to update {TPROXY_ENV_FILE}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = writeln!(stdin, "TPROXY_PORT={port}");
    }
    if !child.wait().map(|s| s.success()).unwrap_or(false) {
        return Err(format!("failed to update {TPROXY_ENV_FILE}"));
    }
    Ok(())
}

pub fn tproxy_status() {
    println!("service: {}", service_state());
    if tproxy_routing_active() {
        println!("policy routing: active");
    } else {
        println!("policy routing: inactive");
    }
    if probe_chain("mangle", "XRAY").is_ok() {
        println!("mangle XRAY chain: present");
    } else {
        println!("mangle XRAY chain: absent (or sudo credentials required)");
    }
    if probe_chain("nat", "XRAY_DNS").is_ok() {
        println!("nat XRAY_DNS chain: present");
    } else {
        println!("nat XRAY_DNS chain: absent (or sudo credentials required)");
    }
}

fn probe_chain(table: &str, chain: &str) -> Result<(), String> {
    Command::new("sudo")
        .args(["-n", "iptables", "-t", table, "-S", chain])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("chain {chain} not found in {table}"))
            }
        })
        .map_err(|e| e.to_string())?
}

pub fn direct(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("direct requires a command".into());
    }
    let error;
    if in_group(TPROXY_GROUP) {
        let sg = must_look_path("sg");
        error = Command::new(&sg)
            .args([TPROXY_GROUP, "-c", &args.join(" ")])
            .exec();
    } else {
        if look_path("sudo").is_none() {
            return Err(format!("Join {TPROXY_GROUP} and re-login, or install sudo"));
        }
        let sudo = must_look_path("sudo");
        error = Command::new(&sudo)
            .arg("-g")
            .arg(TPROXY_GROUP)
            .arg("--")
            .args(args)
            .exec();
    }
    Err(error.to_string())
}

fn in_group(name: &str) -> bool {
    Command::new("id")
        .arg("-Gn")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).split_whitespace().any(|group| group == name))
        .unwrap_or(false)
}

fn must_look_path(name: &str) -> PathBuf {
    match look_path(name) {
        Some(path) => path,
        None => {
            eprintln!("xrayctl: {name} not found");
            std::process::exit(1);
        }
    }
}
