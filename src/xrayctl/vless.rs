use uuid::Uuid;

use super::state::Node;

pub fn parse_vless_link(link: &str) -> Result<Node, String> {
    let mut node = Node {
        tag: String::new(),
        address: String::new(),
        port: 0,
        uuid: String::new(),
        sni: String::new(),
        public_key: String::new(),
        short_id: String::new(),
        security: String::new(),
        pin: String::new(),
    };
    let Some(rest) = link.strip_prefix("vless://") else {
        return Err("Expected a vless:// link".into());
    };
    let mut rest = rest;
    if let Some(index) = rest.find('#') {
        node.tag = percent_decode(&rest[index + 1..]);
        rest = &rest[..index];
    }
    let mut query = "";
    if let Some(index) = rest.find('?') {
        query = &rest[index + 1..];
        rest = &rest[..index];
    }
    let mut uuid_part = rest;
    if let Some(index) = rest.rfind('@') {
        uuid_part = &rest[..index];
        rest = &rest[index + 1..];
    }
    node.uuid = uuid_part.to_string();
    let mut address = rest;
    let mut port = String::new();
    if let Some(index) = rest.rfind(':') {
        address = &rest[..index];
        port = rest[index + 1..].to_string();
    }
    node.address = address.trim_matches(['[', ']']).to_string();

    let mut security = String::new();
    let mut network = String::new();
    let mut flow = String::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.find('=') {
            Some(index) => (&pair[..index], percent_decode(&pair[index + 1..])),
            None => (pair, String::new()),
        };
        match key {
            "security" => security = value,
            "sni" => node.sni = value,
            "pbk" => node.public_key = value,
            "sid" => node.short_id = value,
            "type" => network = value,
            "flow" => flow = value,
            "pin" => node.pin = value,
            _ => {}
        }
    }
    if !security.is_empty() && security != "reality" && security != "tls" {
        return Err("Only reality and tls security are supported".into());
    }
    if !network.is_empty() && network != "tcp" {
        return Err("Only the tcp transport is supported".into());
    }
    if !flow.is_empty() && flow != "xtls-rprx-vision" {
        return Err("Only the xtls-rprx-vision flow is supported".into());
    }

    node.security = security;
    if !port.is_empty() {
        node.port = port
            .parse::<i64>()
            .map_err(|_| "node has an invalid port".to_string())?;
    }
    Ok(node)
}

pub fn encode_vless_link(node: &Node) -> String {
    let security = if node.security.is_empty() {
        "reality".to_string()
    } else {
        node.security.clone()
    };
    let mut pairs: Vec<(String, String)> = vec![
        ("flow".into(), "xtls-rprx-vision".into()),
        ("security".into(), security.clone()),
        ("sni".into(), node.sni.clone()),
        ("type".into(), "tcp".into()),
    ];
    if security == "reality" {
        pairs.push(("pbk".into(), node.public_key.clone()));
        if !node.short_id.is_empty() {
            pairs.push(("sid".into(), node.short_id.clone()));
        }
    }
    if security == "tls" && !node.pin.is_empty() {
        pairs.push(("pin".into(), node.pin.clone()));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let query = pairs
        .iter()
        .map(|(key, value)| format!("{}={}", key, query_escape(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!(
        "vless://{}@{}:{}?{}#{}",
        node.uuid,
        node.address,
        node.port,
        query,
        path_escape(&node.tag)
    )
}

fn query_escape(value: &str) -> String {
    let mut escaped = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                escaped.push(byte as char)
            }
            b' ' => escaped.push('+'),
            _ => escaped.push_str(&format!("%{byte:02X}")),
        }
    }
    escaped
}

fn path_escape(value: &str) -> String {
    let mut escaped = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'$' | b'&'
            | b'+' | b',' | b'/' | b':' | b';' | b'=' | b'@' => escaped.push(byte as char),
            _ => escaped.push_str(&format!("%{byte:02X}")),
        }
    }
    escaped
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Ok(n) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                decoded.push(n);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub fn validate_node(node: &mut Node, name: &str) -> Result<(), String> {
    if node.uuid.is_empty() {
        return Err(format!("{name} is missing a UUID"));
    }
    if Uuid::parse_str(&node.uuid).is_err() {
        return Err(format!("{name} has an invalid UUID"));
    }
    if node.address.is_empty() {
        return Err(format!("{name} is missing an address"));
    }
    if node.sni.is_empty() {
        return Err(format!("{name} is missing sni"));
    }
    if !node.security.is_empty() && node.security != "reality" && node.security != "tls" {
        return Err(format!("{name} has an unsupported security"));
    }
    if node.security != "tls" && node.public_key.is_empty() {
        return Err(format!("{name} is missing a public key"));
    }
    if node.port == 0 {
        node.port = 443;
    }
    if !(1..=65535).contains(&node.port) {
        return Err(format!("{name} has an invalid port"));
    }
    for value in [
        &node.tag,
        &node.address,
        &node.uuid,
        &node.sni,
        &node.public_key,
        &node.short_id,
    ] {
        if value.contains([' ', '\t', '\n', '\r', '"', '\'', '\\']) {
            return Err(format!("{name} must not contain whitespace, quotes, or backslashes"));
        }
    }
    Ok(())
}

pub fn validate_matcher(matcher: &str) -> Result<(), String> {
    for prefix in ["domain:", "geosite:", "full:", "regexp:", "geoip:"] {
        if matcher.starts_with(prefix) {
            return Ok(());
        }
    }
    for c in matcher.chars() {
        if !(c.is_ascii_digit()
            || matches!(c, 'a'..='f' | 'A'..='F' | ':' | '.'))
        {
            return Err(format!(
                "Invalid matcher: {matcher} (use domain: geosite: full: regexp: geoip: or a bare IP/CIDR)"
            ));
        }
    }
    Ok(())
}

pub fn validate_port(port: i64) -> Result<(), String> {
    if !(1..=65535).contains(&port) {
        return Err(format!("Invalid port: {port}"));
    }
    Ok(())
}
