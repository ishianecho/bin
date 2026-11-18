use std::{fs, io::Read, process::{Command, Stdio}};

use crate::xrayctl::regen::look_path;

use super::{
    config::{build_config, missing_rule_set_tags, rule_set_url, RULE_SET_CACHE_DIR},
    install_config, maybe_restart,
    state::{self, State},
};

pub fn regenerate(state: &State, no_restart: bool) -> Result<(), String> {
    for tag in missing_rule_set_tags(state) {
        download_rule_set(&tag)?;
    }
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

fn download_rule_set(tag: &str) -> Result<(), String> {
    let url = rule_set_url(tag).ok_or_else(|| format!("No rule-set source for {tag}"))?;
    let destination = format!("{RULE_SET_CACHE_DIR}/{tag}.srs");
    let temporary = state::config_dir().join(format!("{tag}.srs.part"));
    let status = Command::new("curl")
        .args(["-fsSL", "--max-time", "60", "-o"])
        .arg(&temporary)
        .arg(&url)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        fs::remove_file(&temporary).ok();
        return Err(format!(
            "Failed to download rule-set {tag} from {url}; alternatively: pacman -S sing-geosite-rule-set sing-geoip-rule-set"
        ));
    }
    let installed = Command::new("sudo")
        .args(["install", "-D", "-m", "0644", "-o", "sing-box", "-g", "sing-box"])
        .arg(&temporary)
        .arg(&destination)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    fs::remove_file(&temporary).ok();
    if !installed {
        return Err(format!("failed to install {destination}"));
    }
    println!("Downloaded rule-set {tag}");
    Ok(())
}
