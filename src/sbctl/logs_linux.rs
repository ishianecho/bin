use std::{os::unix::process::CommandExt, process::{Command, Stdio}};

use crate::xrayctl::regen::look_path;

const SERVICE: &str = "sing-box";

pub fn run_log(lines: i64, follow: bool) -> Result<(), String> {
    if look_path("journalctl").is_none() {
        return Err("journalctl not found".into());
    }
    let interactive = unsafe { isatty(0) == 1 };
    let mut argv = vec![
        "journalctl".to_string(),
        "-u".into(),
        SERVICE.into(),
        "-n".into(),
        lines.to_string(),
        "-o".into(),
        "cat".into(),
    ];
    if follow {
        argv.push("-f".into());
    }
    if !journal_readable() {
        let mut elevated = vec!["sudo".to_string()];
        elevated.extend(argv.clone());
        argv = elevated;
    }
    if !interactive || follow {
        argv.push("--no-pager".into());
        let error = Command::new(&argv[0]).args(&argv[1..]).exec();
        return Err(error.to_string());
    }
    argv.push("--no-pager".into());
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("failed to read the {SERVICE} journal: {e}"))?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn journal_readable() -> bool {
    Command::new("journalctl")
        .args(["-u", SERVICE, "-n", "0", "--no-pager"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

extern "C" {
    fn isatty(fd: i32) -> i32;
}
