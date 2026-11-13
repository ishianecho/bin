use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Default)]
pub struct Config {
    pub http: String,
    pub socks: String,
    pub no_proxy: String,
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Path::new(&dir).join("proxy");
        }
    }
    match home_dir() {
        Some(home) => home.join(".config").join("proxy"),
        None => PathBuf::from(".config").join("proxy"),
    }
}

pub fn config_path() -> PathBuf {
    config_dir().join("config")
}

fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

pub fn read() -> Result<Config, String> {
    let path = config_path();
    let data = fs::read_to_string(&path)
        .map_err(|_| format!("No proxy configuration found at {}", path.display()))?;
    let mut cfg = Config::default();
    for line in data.split('\n') {
        for (key, target) in [
            ("PROXY_HTTP", &mut cfg.http),
            ("PROXY_SOCKS", &mut cfg.socks),
            ("PROXY_NO_PROXY", &mut cfg.no_proxy),
        ] {
            let prefix = format!("{key}='");
            if let Some(rest) = line.strip_prefix(&prefix) {
                if let Some(value) = rest.strip_suffix('\'') {
                    target.push_str(value);
                }
            }
        }
    }
    Ok(cfg)
}

pub fn write(cfg: &Config) -> Result<(), String> {
    if cfg.no_proxy.contains(['\r', '\n', '\0']) {
        return Err("--no-proxy must be a single comma-separated line".into());
    }
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = format!(
        "PROXY_HTTP='{}'\nPROXY_SOCKS='{}'\nPROXY_NO_PROXY='{}'\n",
        cfg.http, cfg.socks, cfg.no_proxy
    );
    let temporary = dir.join(format!(
        "config.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    fs::write(&temporary, content).map_err(|e| e.to_string())?;
    set_private(&temporary)?;
    let result = fs::rename(&temporary, config_path());
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

pub fn validate(value: &str, kind: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    if value.contains([' ', '\t', '\n', '\r', '"', '\'', '\\']) {
        return Err(format!("{kind} must not contain whitespace, quotes, or backslashes"));
    }
    let allowed: &[&str] = match kind {
        "HTTP_PROXY" => &["http", "https"],
        "SOCKS_PROXY" => &["socks5", "socks5h"],
        _ => &[],
    };
    let scheme = url::Url::parse(value)
        .map(|parsed| parsed.scheme().to_string())
        .unwrap_or_default();
    if !allowed.contains(&scheme.as_str()) {
        return Err(format!("{kind} must use one of: {}", allowed.join("://, ") + "://"));
    }
    Ok(())
}

impl Config {
    pub fn default_no_proxy(&self) -> String {
        if !self.no_proxy.is_empty() {
            return self.no_proxy.clone();
        }
        "localhost,127.0.0.1,::1".into()
    }
}
