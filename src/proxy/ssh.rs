use std::{
    io::{Read, Write},
    net::{IpAddr, TcpStream},
};

use super::config;

pub fn relay(host: &str, port: &str) -> Result<(), String> {
    let target = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let proxy_url = config::read().map(|cfg| cfg.socks).unwrap_or_default();
    let mut stream = dial(&target, &proxy_url)?;
    let mut socket = stream
        .try_clone()
        .map_err(|e| format!("failed to use the connection: {e}"))?;
    let stdout = std::thread::spawn(move || {
        std::io::copy(&mut socket, &mut std::io::stdout()).map(|_| ())
    });
    let _ = std::io::copy(&mut std::io::stdin(), &mut stream);
    stdout
        .join()
        .map_err(|_| "connection relay failed".to_string())?
        .map_err(|e| e.to_string())
}

fn dial(target: &str, proxy_url: &str) -> Result<TcpStream, String> {
    if proxy_url.is_empty() {
        return TcpStream::connect(target).map_err(|e| e.to_string());
    }
    let parsed = url::Url::parse(proxy_url)
        .map_err(|_| format!("unsupported proxy URL: {proxy_url}"))?;
    let Some(host) = parsed.host_str() else {
        return Err(format!("unsupported proxy URL: {proxy_url}"));
    };
    let Some(port) = parsed.port() else {
        return Err(format!("unsupported proxy URL: {proxy_url}"));
    };
    let address = format!("{host}:{port}");
    match parsed.scheme() {
        "socks5" | "socks5h" => socks5_connect(&address, target),
        "http" | "https" => http_connect(&address, target),
        _ => Err(format!("unsupported proxy URL: {proxy_url}")),
    }
}

fn split_target(target: &str) -> Result<(String, u16), String> {
    let index = target
        .rfind(':')
        .ok_or_else(|| format!("invalid target: {target}"))?;
    let (host, port) = (&target[..index], &target[index + 1..]);
    let port = port
        .parse::<u16>()
        .map_err(|_| format!("invalid target: {target}"))?;
    let host = host.trim_matches(['[', ']']).to_string();
    Ok((host, port))
}

fn socks5_connect(address: &str, target: &str) -> Result<TcpStream, String> {
    let mut stream =
        TcpStream::connect(address).map_err(|e| format!("failed to reach {address}: {e}"))?;
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .map_err(|e| e.to_string())?;
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).map_err(|e| e.to_string())?;
    if reply != [0x05, 0x00] {
        return Err("socks5 authentication failed".into());
    }
    let (host, port) = split_target(target)?;
    let mut request = vec![0x05, 0x01, 0x00];
    if let Ok(ip) = host.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(v4) => {
                request.push(0x01);
                request.extend_from_slice(&v4.octets());
            }
            IpAddr::V6(v6) => {
                request.push(0x04);
                request.extend_from_slice(&v6.octets());
            }
        }
    } else {
        let bytes = host.as_bytes();
        if bytes.len() > 255 {
            return Err(format!("target host is too long: {host}"));
        }
        request.push(0x03);
        request.push(bytes.len() as u8);
        request.extend_from_slice(bytes);
    }
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).map_err(|e| e.to_string())?;
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).map_err(|e| e.to_string())?;
    if header[1] != 0x00 {
        return Err(format!("socks5 connect failed with code {}", header[1]));
    }
    let tail = match header[3] {
        0x01 => 6,
        0x03 => {
            let mut length = [0u8; 1];
            stream.read_exact(&mut length).map_err(|e| e.to_string())?;
            length[0] as usize + 2
        }
        0x04 => 18,
        _ => return Err("socks5 reply has an unknown address type".into()),
    };
    let mut rest = vec![0u8; tail];
    stream.read_exact(&mut rest).map_err(|e| e.to_string())?;
    Ok(stream)
}

fn http_connect(address: &str, target: &str) -> Result<TcpStream, String> {
    let mut stream =
        TcpStream::connect(address).map_err(|e| format!("failed to reach {address}: {e}"))?;
    let request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                response.push(byte[0]);
                if response.ends_with(b"\r\n\r\n") {
                    break;
                }
                if response.len() > 64 * 1024 {
                    return Err("proxy CONNECT response is too large".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    let head = String::from_utf8_lossy(&response);
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        let status = head.lines().next().unwrap_or("");
        return Err(format!("proxy CONNECT failed: {status}"));
    }
    Ok(stream)
}
