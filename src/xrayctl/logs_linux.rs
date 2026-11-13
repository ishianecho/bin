use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    os::fd::OwnedFd,
    os::unix::net::UnixStream,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Command, Stdio},
};

use super::{regen::look_path, state};

#[derive(Default, Clone)]
struct LogSession {
    inbound: String,
    from: String,
}

pub fn filter_log_lines<R: Read, W: Write>(input: R, output: &mut W, default_tag: &str) {
    let reader = BufReader::new(input);
    let mut sessions: HashMap<String, LogSession> = HashMap::new();
    for line in reader.lines().map_while(Result::ok) {
        let prefix = match log_timestamp(&line) {
            Some(timestamp) => format!("{timestamp} "),
            None => String::new(),
        };
        if let Some((id, source)) = match_marker(&line, " proxy/dokodemo: received request for ") {
            reset_sessions(&mut sessions);
            sessions.insert(
                id,
                LogSession {
                    inbound: "tproxy".into(),
                    from: log_host(&source),
                },
            );
            continue;
        }
        if let Some((id, source)) = match_marker(&line, " proxy/socks: client UDP connection from ") {
            reset_sessions(&mut sessions);
            sessions.insert(
                id,
                LogSession {
                    inbound: "socks".into(),
                    from: log_host(&source),
                },
            );
            continue;
        }
        if let Some((id, inbound)) = match_inbound(&line) {
            reset_sessions(&mut sessions);
            sessions.insert(
                id,
                LogSession {
                    inbound,
                    from: String::new(),
                },
            );
            continue;
        }
        if let Some((id, outbound, target)) = match_detour(&line) {
            write_flow(output, &prefix, sessions.get(&id), &target, "->", &outbound);
            continue;
        }
        if let Some((id, target)) = match_default_route(&line) {
            write_flow(output, &prefix, sessions.get(&id), &target, ">>", default_tag);
            continue;
        }
        if line.contains("[Info]") {
            continue;
        }
        let _ = writeln!(output, "{line}");
    }
}

fn write_flow<W: Write>(
    output: &mut W,
    prefix: &str,
    session: Option<&LogSession>,
    target: &str,
    arrow: &str,
    dest: &str,
) {
    let session = session.cloned().unwrap_or_default();
    let inbound = if session.inbound.is_empty() {
        "tproxy"
    } else {
        &session.inbound
    };
    let from = if session.from.is_empty() {
        "-"
    } else {
        &session.from
    };
    let _ = writeln!(output, "{prefix}from {from} to {target} [{inbound} {arrow} {dest}]");
}

fn reset_sessions(sessions: &mut HashMap<String, LogSession>) {
    if sessions.len() > 8192 {
        sessions.clear();
    }
}

fn marker_session<'a>(line: &'a str, marker: &str) -> Option<(String, &'a str)> {
    let index = line.find(marker)?;
    let before = line[..index].strip_suffix(']')?;
    let open = before.rfind('[')?;
    let id = &before[open + 1..];
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((id.to_string(), &line[index + marker.len()..]))
}

fn log_timestamp(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let head = &line[..19];
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !(digits(&head[0..4])
        && head.as_bytes()[4] == b'/'
        && digits(&head[5..7])
        && head.as_bytes()[7] == b'/'
        && digits(&head[8..10])
        && head.as_bytes()[10] == b' '
        && digits(&head[11..13])
        && head.as_bytes()[13] == b':'
        && digits(&head[14..16])
        && head.as_bytes()[16] == b':'
        && digits(&head[17..19]))
    {
        return None;
    }
    let mut end = 19;
    if bytes[19] == b'.' {
        let mut i = 20;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == 20 {
            return None;
        }
        end = i;
    }
    if bytes.get(end) == Some(&b' ') {
        Some(line[..end].to_string())
    } else {
        None
    }
}

fn match_marker(line: &str, marker: &str) -> Option<(String, String)> {
    let (id, rest) = marker_session(line, marker)?;
    let value = rest.split_whitespace().next().unwrap_or_default();
    Some((id, value.to_string()))
}

fn match_inbound(line: &str) -> Option<(String, String)> {
    if let Some((id, _)) = marker_session(line, " proxy/socks:") {
        return Some((id, "socks".into()));
    }
    if let Some((id, _)) = marker_session(line, " proxy/dokodemo:") {
        return Some((id, "tproxy".into()));
    }
    None
}

fn match_detour(line: &str) -> Option<(String, String, String)> {
    let (id, rest) = marker_session(line, " app/dispatcher: taking detour [")?;
    let bytes = rest.as_bytes();
    let mut split = None;
    for i in 0..bytes.len() {
        if bytes[i] == b']' && rest[i + 1..].starts_with(" for [") {
            split = Some(i);
            break;
        }
    }
    let split = split?;
    let outbound = rest[..split].to_string();
    let tail = &rest[split + 7..];
    let target = tail.find(']').map(|i| tail[..i].to_string())?;
    Some((id, outbound, target))
}

fn match_default_route(line: &str) -> Option<(String, String)> {
    let (id, rest) = marker_session(line, " app/dispatcher: default route for ")?;
    let target = rest.split_whitespace().next().unwrap_or_default();
    Some((id, target.to_string()))
}

fn log_host(addr: &str) -> String {
    if let Some((host, port)) = addr.rsplit_once(':') {
        if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) {
            return host.trim_matches(['[', ']']).to_string();
        }
    }
    addr.to_string()
}

fn pager_commands(mode: &str) -> Vec<Vec<String>> {
    let custom: Vec<String> = std::env::var("PAGER")
        .map(|pager| pager.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    if custom.is_empty() {
        return vec![vec!["less".into(), mode.into()]];
    }
    if custom[0] == "less" {
        if custom.iter().skip(1).any(|arg| arg.starts_with('+')) {
            return vec![custom];
        }
        let mut args = custom.clone();
        args.push(mode.into());
        return vec![args];
    }
    vec![custom, vec!["less".into(), mode.into()]]
}

enum PagerSource {
    Fd(OwnedFd),
    Bytes(Vec<u8>),
}

fn pager_input(source: PagerSource) -> Result<OwnedFd, String> {
    match source {
        PagerSource::Fd(fd) => Ok(fd),
        PagerSource::Bytes(bytes) => {
            let (reader, mut writer) = UnixStream::pair().map_err(|e| e.to_string())?;
            std::thread::spawn(move || {
                let _ = writer.write_all(&bytes);
            });
            Ok(reader.into())
        }
    }
}

fn run_pager(source: PagerSource, commands: Vec<Vec<String>>) -> Result<(), String> {
    let mut source = pager_input(source)?;
    let count = commands.len();
    for argv in commands.iter().take(count.saturating_sub(1)) {
        let (reader, writer) = UnixStream::pair().map_err(|e| e.to_string())?;
        let mut command = pager_command(argv);
        command.stdin(Stdio::from(source));
        command.stdout(Stdio::from(OwnedFd::from(writer)));
        command.stderr(Stdio::inherit());
        command.process_group(0);
        command.spawn().map_err(|e| e.to_string())?;
        source = reader.into();
    }
    let last = commands.last().ok_or("no pager")?;
    let status = pager_command(last)
        .stdin(Stdio::from(source))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() || status.signal() == Some(13) {
        Ok(())
    } else {
        Err(format!("pager exited with {status}"))
    }
}

fn pager_command(argv: &[String]) -> Command {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    command
}

pub fn run_log(lines: i64, follow: bool) -> Result<(), String> {
    if look_path("journalctl").is_none() {
        return Err("journalctl not found".into());
    }
    let interactive = unsafe { isatty(0) == 1 };
    let base = [
        "-u", "xray", "--no-pager", "-n", &lines.to_string(), "-o", "cat",
    ];
    let mut default_tag = "default".to_string();
    if let Ok(state) = state::load() {
        if !state.active.is_empty() {
            default_tag = state.active;
        }
    }
    if follow && interactive {
        let argv = journal_argv(&base, true);
        let (journal_write, journal_read) = UnixStream::pair().map_err(|e| e.to_string())?;
        let (mut pager_write, pager_read) = UnixStream::pair().map_err(|e| e.to_string())?;
        let mut journal = Command::new(&argv[0]);
        journal
            .args(&argv[1..])
            .stdout(Stdio::from(OwnedFd::from(journal_write)))
            .stderr(Stdio::inherit())
            .process_group(0);
        let mut journal = journal.spawn().map_err(|e| e.to_string())?;
        let tag = default_tag.clone();
        let filter = std::thread::spawn(move || {
            let _ = filter_log_lines(journal_read, &mut pager_write, &tag);
        });
        let pager_error = run_pager(PagerSource::Fd(pager_read.into()), pager_commands("+F"));
        let _ = journal.kill();
        let _ = journal.wait();
        filter.join().map_err(|_| "log filtering failed".to_string())?;
        return pager_error;
    }
    if follow {
        let argv = journal_argv(&base, true);
        let mut stream = Command::new(&argv[0]);
        stream
            .args(&argv[1..])
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut stream = stream.spawn().map_err(|e| e.to_string())?;
        if let Some(stdout) = stream.stdout.take() {
            let stdout_handle = std::io::stdout();
            let mut handle = stdout_handle.lock();
            filter_log_lines(stdout, &mut handle, &default_tag);
        }
        return stream.wait().map(|_| ()).map_err(|e| e.to_string());
    }
    let argv = journal_argv(&base, false);
    let first = Command::new(&argv[0])
        .args(&argv[1..])
        .stderr(Stdio::piped())
        .output();
    let data = match first {
        Ok(output) if output.status.success() => output.stdout,
        first => {
            let stderr = match &first {
                Ok(output) => String::from_utf8_lossy(&output.stderr).trim().to_string(),
                Err(_) => String::new(),
            };
            if argv[0] == "sudo" {
                return Err(format!("failed to read the xray journal: {stderr}"));
            }
            let elevated = Command::new("sudo")
                .args(&argv)
                .stdin(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| format!("failed to read the xray journal: {e}"))?;
            if !elevated.status.success() {
                return Err(format!("failed to read the xray journal: {stderr}"));
            }
            elevated.stdout
        }
    };
    let mut filtered = Vec::new();
    filter_log_lines(&data[..], &mut filtered, &default_tag);
    if !interactive {
        std::io::stdout()
            .write_all(&filtered)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    run_pager(PagerSource::Bytes(filtered), pager_commands("+G"))
}

fn journal_argv(base: &[&str], follow: bool) -> Vec<String> {
    let mut argv: Vec<String> = std::iter::once("journalctl".to_string())
        .chain(base.iter().map(|s| s.to_string()))
        .collect();
    if follow {
        argv.push("-f".into());
    }
    let readable = Command::new("journalctl")
        .args(["-u", "xray", "-n", "0", "--no-pager"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !readable {
        let mut elevated = vec!["sudo".to_string()];
        elevated.extend(argv);
        return elevated;
    }
    argv
}

extern "C" {
    fn isatty(fd: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        let mut output = Vec::new();
        filter_log_lines(input.as_bytes(), &mut output, "default");
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn filters_default_route_lines() {
        assert_eq!(
            run("2026/11/13 19:12:04.784338 [Info] [123] app/dispatcher: default route for tcp:example.com:443\n"),
            "2026/11/13 19:12:04.784338 from - to tcp:example.com:443 [tproxy >> default]\n"
        );
    }

    #[test]
    fn filters_detour_with_session() {
        assert_eq!(
            run("[7] proxy/dokodemo: received request for 192.168.1.5:1234\n[7] app/dispatcher: taking detour [my-node] for [tcp:example.com:443]\n"),
            "from 192.168.1.5 to tcp:example.com:443 [tproxy -> my-node]\n"
        );
    }

    #[test]
    fn filters_socks_session_and_skips_info() {
        assert_eq!(
            run("[9] proxy/socks: client UDP connection from 10.0.0.2:5555\n[9] app/dispatcher: default route for udp:1.1.1.1:53\n[9] [Info] transport/internet/tcp: dialing TCP to tcp:example.com:443\n"),
            "from 10.0.0.2 to udp:1.1.1.1:53 [socks >> default]\n"
        );
    }

    #[test]
    fn passes_non_info_lines_through() {
        assert_eq!(
            run("2026/11/13 19:12:04.784338 [Warning] something else\n"),
            "2026/11/13 19:12:04.784338 [Warning] something else\n"
        );
    }
}
