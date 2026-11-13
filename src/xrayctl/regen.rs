use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
};

use super::{
    config::build_config,
    maybe_restart,
    state::{self, State},
};

pub fn regenerate(state: &State, no_restart: bool) -> Result<(), String> {
    let mut data = build_config(state)?;
    data.push('\n');
    let new_path = state::config_dir().join("config.new.json");
    fs::write(&new_path, data).map_err(|e| e.to_string())?;
    if look_path("xray").is_some() {
        let mut child = Command::new("xray")
            .args(["run", "-test", "-c"])
            .arg(&new_path)
            .stderr(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut message = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut message);
        }
        if !child.wait().map_err(|e| e.to_string())?.success() {
            fs::remove_file(&new_path).ok();
            return Err(format!(
                "Generated configuration failed 'xray run -test'; the previous config is kept: {}",
                message.trim()
            ));
        }
    }
    fs::rename(&new_path, state::config_path()).map_err(|e| e.to_string())?;
    println!("Generated {}", state::config_path().display());
    maybe_restart(no_restart)
}

pub fn sync_proxy_env(socks_port: i64) {
    let name = if cfg!(windows) { "proxy.exe" } else { "proxy" };
    let path = match look_path(name) {
        Some(path) => path,
        None => match home_dir() {
            Some(home) => {
                let candidate = home.join(".local").join("bin").join(name);
                if candidate.exists() {
                    candidate
                } else {
                    return;
                }
            }
            None => return,
        },
    };
    let _ = Command::new(&path)
        .arg("set")
        .arg("--http")
        .arg(format!("http://127.0.0.1:{socks_port}"))
        .arg("--socks")
        .arg(format!("socks5://127.0.0.1:{socks_port}"))
        .status();
}

pub fn look_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}
