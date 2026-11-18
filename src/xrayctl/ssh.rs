use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

const BEGIN: &str = "# xrayctl begin";
const END: &str = "# xrayctl end";
const BLOCK: &str = "# xrayctl begin\nHost github.com\n  ProxyJump none\n# xrayctl end\n";

pub fn sync(active: bool) {
    let result = match config_local() {
        Some(path) => {
            if active {
                remove_block(&path)
            } else {
                append_block(&path)
            }
        }
        None => Err("HOME is not set".into()),
    };
    if let Err(e) = result {
        eprintln!("ssh config: {e}");
    }
}

fn config_local() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)?;
    Some(home.join(".ssh").join("config.local"))
}

fn append_block(path: &Path) -> Result<(), String> {
    if let Ok(data) = fs::read_to_string(path) {
        if data.contains(BEGIN) {
            return Ok(());
        }
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let data = if fs::metadata(path).map(|meta| meta.len() > 0).unwrap_or(false) {
        "\n"
    } else {
        ""
    };
    file.write_all(format!("{data}{BLOCK}").as_bytes())
        .map_err(|e| e.to_string())
}

fn remove_block(path: &Path) -> Result<(), String> {
    let data = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if !data.contains(BEGIN) {
        return Ok(());
    }
    let mut kept = String::new();
    let mut inside = false;
    for line in data.lines() {
        if line.trim() == BEGIN {
            inside = true;
        } else if line.trim() == END {
            inside = false;
        } else if !inside {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    fs::write(path, kept).map_err(|e| e.to_string())
}
