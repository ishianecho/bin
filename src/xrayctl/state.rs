use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Node {
    pub tag: String,
    pub address: String,
    pub port: i64,
    pub uuid: String,
    pub sni: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(rename = "shortId")]
    pub short_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub security: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pin: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Route {
    pub action: String,
    pub matcher: String,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Ports {
    pub socks: i64,
    pub tproxy: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct State {
    pub version: i64,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub ports: Ports,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub routes: Vec<Route>,
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Path::new(&dir).join("xray");
        }
    }
    match home_dir() {
        Some(home) => home.join(".config").join("xray"),
        None => PathBuf::from(".config").join("xray"),
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

pub fn env_path() -> PathBuf {
    config_dir().join("env")
}

impl Default for Ports {
    fn default() -> Self {
        Ports {
            socks: 10808,
            tproxy: 10800,
        }
    }
}

pub fn load() -> Result<State, String> {
    let path = state_path();
    match fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let state = State {
                version: 1,
                active: String::new(),
                ports: Ports::default(),
                nodes: Vec::new(),
                routes: Vec::new(),
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
