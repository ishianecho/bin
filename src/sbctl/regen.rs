use std::{fs, io::Read, process::{Command, Stdio}};

use crate::xrayctl::regen::look_path;

use super::{
    config::build_config,
    install_config, maybe_restart,
    state::{self, State},
};

pub fn regenerate(state: &State, no_restart: bool) -> Result<(), String> {
    let mut data = build_config(state)?;
    data.push('\n');
    let new_path = state::config_dir().join("config.new.json");
    fs::write(&new_path, data).map_err(|e| e.to_string())?;
    if look_path("sing-box").is_some() {
        let mut child = Command::new("sing-box")
            .args(["check", "-c"])
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
                "Generated configuration failed 'sing-box check'; the previous config is kept: {}",
                message.trim()
            ));
        }
    }
    fs::rename(&new_path, state::config_path()).map_err(|e| e.to_string())?;
    println!("Generated {}", state::config_path().display());
    install_config()?;
    maybe_restart(no_restart)
}
