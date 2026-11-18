use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub use crate::xrayctl::state::{Node, Route};

#[derive(Serialize, Deserialize, Clone)]
pub struct Mesh {
    pub address: String,
    pub private_key: String,
    pub peer_public_key: String,
    pub port: i64,
    pub subnet: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct State {
    pub version: i64,
    #[serde(default)]
    pub active: String,
    #[serde(default = "default_mixed_port")]
    pub mixed_port: i64,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub routes: Vec<Route>,
    #[serde(default)]
    pub mesh: Option<Mesh>,
}

fn default_mixed_port() -> i64 {
    20808
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Path::new(&dir).join("sbctl");
        }
    }
    match home_dir() {
        Some(home) => home.join(".config").join("sbctl"),
        None => PathBuf::from(".config").join("sbctl"),
    }
}

fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

pub fn state_path() -> PathBuf {
    config_dir().join("state.json")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load() -> Result<State, String> {
    let path = state_path();
    match fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let state = State {
                version: 1,
                active: String::new(),
                mixed_port: default_mixed_port(),
                nodes: Vec::new(),
                routes: Vec::new(),
                mesh: None,
            };
            save(&state)?;
            Ok(state)
        }
        Err(e) => Err(e.to_string()),
        Ok(data) => serde_json::from_slice(&data)
            .map_err(|_| format!("Invalid state file at {}", path.display())),
    }
}

pub fn save(state: &State) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut data = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    data.push('\n');
    let temporary = dir.join(format!(
        "state.{}.{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    fs::write(&temporary, data).map_err(|e| e.to_string())?;
    set_private(&temporary)?;
    let result = fs::rename(&temporary, state_path());
    fs::remove_file(&temporary).ok();
    result.map_err(|e| e.to_string())
}

#[cfg(unix)]
fn set_private(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn set_private(_path: &Path) -> Result<(), String> {
    Ok(())
}

impl State {
    pub fn find_node(&mut self, tag: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.tag == tag)
    }
    pub fn find_route(&self, matcher: &str) -> Option<&Route> {
        self.routes.iter().find(|route| route.matcher == matcher)
    }
}
