use serde::Serialize;
use serde_json::{json, Value};

use super::state::{Node, State};

#[derive(Serialize)]
struct GeneratedConfig {
    log: LogConfig,
    dns: DnsConfig,
    inbounds: Vec<Value>,
    outbounds: Vec<Value>,
    routing: RoutingConfig,
}

#[derive(Serialize)]
struct LogConfig {
    #[serde(rename = "loglevel")]
    loglevel: String,
    #[serde(rename = "access", skip_serializing_if = "String::is_empty")]
    access: String,
}

#[derive(Serialize)]
struct DnsConfig {
    hosts: Value,
    servers: Vec<Value>,
}

#[derive(Serialize)]
struct RoutingConfig {
    #[serde(rename = "domainStrategy")]
    domain_strategy: String,
    rules: Vec<Value>,
}

pub fn build_config(state: &State) -> Result<String, String> {
    if state.active.is_empty() {
        return Err("No active node; run: xrayctl node use <tag>".into());
    }
    let node = state
        .nodes
        .iter()
        .find(|node| node.tag == state.active)
        .ok_or_else(|| {
            format!(
                "Active node '{}' not found in {}",
                state.active,
                super::state::state_path().display()
            )
        })?;

    let config = GeneratedConfig {
        log: LogConfig {
            loglevel: "info".into(),
            access: "none".into(),
        },
        dns: DnsConfig {
            hosts: json!({
                "cloudflare-dns.com": ["104.16.249.249", "104.16.248.249"],
                "dns.alidns.com": ["223.5.5.5", "223.6.6.6"],
                "dns.google": ["8.8.8.8", "8.8.4.4"],
                "doh.pub": ["1.12.12.12", "120.53.53.53"],
                "dot.pub": ["1.12.12.12", "120.53.53.53"],
                "one.one.one.one": ["1.1.1.1", "1.0.0.1"],
            }),
            servers: vec![
                json!({
                    "address": "https://dns.alidns.com/dns-query",
                    "domains": ["geosite:private", "geosite:cn"],
                    "skipFallback": true,
                }),
                json!({
                    "address": "223.5.5.5",
                    "domains": ["full:dns.alidns.com", "full:cloudflare-dns.com"],
                    "skipFallback": true,
                }),
                Value::String("https://cloudflare-dns.com/dns-query".into()),
            ],
        },
        inbounds: inbounds(state),
        outbounds: outbounds(node),
        routing: RoutingConfig {
            domain_strategy: "AsIs".into(),
            rules: routing_rules(state),
        },
    };
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

fn inbounds(state: &State) -> Vec<Value> {
    let socks = json!({
        "listen": "0.0.0.0",
        "port": state.ports.socks,
        "protocol": "mixed",
        "settings": {
            "allowTransparent": false,
            "auth": "noauth",
            "udp": true,
        },
        "sniffing": {
            "destOverride": ["http", "tls"],
            "enabled": true,
            "routeOnly": false,
        },
        "tag": "socks",
    });
    if cfg!(windows) {
        let tun = json!({
            "protocol": "tun",
            "settings": {
                "autoOutboundsInterface": "auto",
                "autoSystemRoutingTable": ["0.0.0.0/0"],
                "desc": "Xray",
                "dns": ["223.5.5.5", "1.1.1.1"],
                "gateway": ["172.19.0.1/30"],
                "name": "XrayTun",
            },
            "tag": "tun",
        });
        return vec![socks, tun];
    }
    if cfg!(target_os = "linux") {
        let tproxy = json!({
            "listen": "0.0.0.0",
            "port": state.ports.tproxy,
            "protocol": "tunnel",
            "settings": {
                "followRedirect": true,
                "network": "tcp,udp",
            },
            "sniffing": {
                "destOverride": ["http", "tls"],
                "enabled": true,
            },
            "streamSettings": {
                "sockopt": {
                    "tproxy": "tproxy",
                },
            },
            "tag": "tproxy",
        });
        return vec![socks, tproxy];
    }
    vec![socks]
}

fn outbounds(node: &Node) -> Vec<Value> {
    let security = if node.security.is_empty() {
        "reality"
    } else {
        node.security.as_str()
    };
    let mut stream = serde_json::Map::new();
    stream.insert("network".into(), "tcp".into());
    stream.insert("security".into(), security.into());
    if security == "tls" {
        let mut tls_settings = serde_json::Map::new();
        tls_settings.insert("serverName".into(), node.sni.clone().into());
        tls_settings.insert("fingerprint".into(), "chrome".into());
        if !node.pin.is_empty() {
            tls_settings.insert(
                "pinnedPeerCertChainSha256".into(),
                vec![node.pin.clone()].into(),
            );
        }
        stream.insert("tlsSettings".into(), tls_settings.into());
    } else {
        stream.insert(
            "realitySettings".into(),
            json!({
                "fingerprint": "chrome",
                "publicKey": node.public_key,
                "serverName": node.sni,
                "shortId": node.short_id,
                "show": false,
                "spiderX": "",
            }),
        );
    }
    if cfg!(target_os = "linux") {
        stream.insert("sockopt".into(), json!({ "mark": 255 }));
    }
    let vless = json!({
        "mux": {
            "concurrency": -1,
            "enabled": false,
        },
        "protocol": "vless",
        "settings": {
            "vnext": [{
                "address": node.address,
                "port": node.port,
                "users": [{
                    "email": "t@t.tt",
                    "encryption": "none",
                    "flow": "xtls-rprx-vision",
                    "id": node.uuid,
                    "security": "auto",
                }],
            }],
        },
        "streamSettings": Value::Object(stream),
        "tag": node.tag,
    });
    let mut direct = json!({
        "protocol": "freedom",
        "tag": "direct",
    });
    if cfg!(target_os = "linux") {
        direct
            .as_object_mut()
            .unwrap()
            .insert("streamSettings".into(), json!({ "sockopt": { "mark": 255 } }));
    }
    let block = json!({
        "protocol": "blackhole",
        "tag": "block",
    });
    vec![vless, direct, block]
}

fn routing_rules(state: &State) -> Vec<Value> {
    let mut rules = Vec::with_capacity(state.routes.len() + 7);
    for route in &state.routes {
        let outbound = if route.action == "proxy" {
            state.active.clone()
        } else {
            route.action.clone()
        };
        let field = if is_domain_matcher(&route.matcher) {
            "domain"
        } else {
            "ip"
        };
        let mut rule = serde_json::Map::new();
        rule.insert("type".into(), "field".into());
        rule.insert("network".into(), "tcp,udp".into());
        rule.insert("outboundTag".into(), outbound.into());
        rule.insert(field.into(), vec![route.matcher.clone()].into());
        rules.push(Value::Object(rule));
    }
    rules.push(json!({"network": "udp", "outboundTag": "block", "port": "443", "type": "field"}));
    rules.push(json!({"domain": ["geosite:google"], "outboundTag": state.active, "type": "field"}));
    rules.push(json!({"ip": ["geoip:private"], "outboundTag": "direct", "type": "field"}));
    rules.push(json!({"domain": ["geosite:private"], "outboundTag": "direct", "type": "field"}));
    rules.push(json!({
        "domain": ["domain:alidns.com", "domain:doh.pub", "domain:dot.pub", "domain:360.cn", "domain:onedns.net"],
        "outboundTag": "direct",
        "type": "field",
    }));
    rules.push(json!({"ip": ["geoip:cn"], "outboundTag": "direct", "type": "field"}));
    rules.push(json!({"domain": ["geosite:cn"], "outboundTag": "direct", "type": "field"}));
    rules
}

fn is_domain_matcher(matcher: &str) -> bool {
    ["domain:", "geosite:", "full:", "regexp:"]
        .iter()
        .any(|prefix| matcher.starts_with(prefix))
}
