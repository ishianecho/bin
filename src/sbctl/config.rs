use serde_json::{json, Map, Value};

use crate::xrayctl::state::{Node, Route};

use super::state::State;

pub(super) const RULE_SET_DIR: &str = "/usr/share/sing-box/rule-set";
pub(super) const RULE_SET_CACHE_DIR: &str = "/var/lib/sing-box/rule-set";
const RULE_SET_BASE: &str = "https://raw.githubusercontent.com/SagerNet";

pub fn build_config(state: &State) -> Result<String, String> {
    let node = state
        .nodes
        .iter()
        .find(|node| node.tag == state.active)
        .ok_or_else(|| "No active node; run: sbctl node use <tag>".to_string())?;
    let tag = node.tag.clone();
    let mesh = state.mesh.clone().filter(|mesh| mesh.enabled);

    let mut rules = vec![json!({"action": "sniff"}), json!({"protocol": "dns", "action": "hijack-dns"})];
    for route in &state.routes {
        rules.push(translate_route(route, &tag)?);
    }
    if let Some(mesh) = &mesh {
        rules.push(json!({"inbound": ["wg"], "action": "route-options", "override_address": "127.0.0.1"}));
        rules.push(json!({"ip_cidr": [mesh.subnet], "action": "route", "outbound": "wg"}));
    }
    rules.push(json!({"network": "udp", "port": 443, "action": "reject"}));
    rules.push(json!({"ip_is_private": true, "action": "route", "outbound": "direct"}));
    rules.push(json!({"rule_set": ["geosite-cn"], "action": "route", "outbound": "direct"}));
    rules.push(json!({"rule_set": ["geoip-cn"], "action": "route", "outbound": "direct"}));

    let rule_sets: Vec<Value> = rule_set_tags(state)
        .iter()
        .map(|rule_tag| rule_set_definition(rule_tag))
        .collect();

    let config = json!({
        "log": {"level": "warn"},
        "dns": {
            "servers": [
                {"type": "udp", "tag": "cn", "server": "223.5.5.5"},
                {"type": "udp", "tag": "remote", "server": "1.1.1.1", "detour": tag.clone()}
            ],
            "rules": [{"rule_set": ["geosite-cn"], "server": "cn"}],
            "final": "remote"
        },
        "inbounds": [
            {"type": "tun", "tag": "tun-in", "address": ["172.19.0.1/30"], "mtu": 9000, "auto_route": true, "strict_route": true, "stack": "mixed"},
            {"type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": state.mixed_port}
        ],
        "outbounds": [
            {"type": "direct", "tag": "direct"},
            vless_outbound(node)
        ],
        "endpoints": mesh.map(|mesh| {
            json!([{
                "type": "wireguard",
                "tag": "wg",
                "address": [mesh.address],
                "private_key": mesh.private_key,
                "mtu": 1420,
                "detour": tag.clone(),
                "peers": [{
                    "address": "127.0.0.1",
                    "port": mesh.port,
                    "public_key": mesh.peer_public_key,
                    "allowed_ips": ["0.0.0.0/0"]
                }]
            }])
        }).unwrap_or_else(|| json!([])),
        "route": {
            "rules": rules,
            "rule_set": rule_sets,
            "auto_detect_interface": true,
            "default_domain_resolver": {"server": "cn"},
            "final": tag
        },
        "experimental": {"cache_file": {"enabled": true}}
    });
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

fn vless_outbound(node: &Node) -> Value {
    let tls = if node.security == "tls" {
        json!({"enabled": true, "server_name": node.sni, "utls": {"enabled": true, "fingerprint": "chrome"}})
    } else {
        json!({
            "enabled": true,
            "server_name": node.sni,
            "utls": {"enabled": true, "fingerprint": "chrome"},
            "reality": {
                "enabled": true,
                "public_key": node.public_key.trim_end_matches('='),
                "short_id": node.short_id
            }
        })
    };
    json!({
        "type": "vless",
        "tag": node.tag,
        "server": node.address,
        "server_port": node.port,
        "uuid": node.uuid,
        "flow": "xtls-rprx-vision",
        "packet_encoding": "xudp",
        "tls": tls
    })
}

pub fn translate_route(route: &Route, active: &str) -> Result<Value, String> {
    let mut rule = Map::new();
    if route.matcher == "geosite:private" {
        rule.insert("ip_is_private".into(), json!(true));
    } else if let Some(value) = route.matcher.strip_prefix("domain:") {
        rule.insert("domain_suffix".into(), json!([value]));
    } else if let Some(value) = route.matcher.strip_prefix("full:") {
        rule.insert("domain".into(), json!([value]));
    } else if let Some(value) = route.matcher.strip_prefix("regexp:") {
        rule.insert("domain_regex".into(), json!([value]));
    } else if let Some(value) = route.matcher.strip_prefix("geosite:") {
        rule.insert("rule_set".into(), json!([format!("geosite-{value}")]));
    } else if let Some(value) = route.matcher.strip_prefix("geoip:") {
        rule.insert("rule_set".into(), json!([format!("geoip-{value}")]));
    } else {
        rule.insert("ip_cidr".into(), json!([route.matcher]));
    }
    match route.action.as_str() {
        "proxy" => {
            rule.insert("action".into(), json!("route"));
            rule.insert("outbound".into(), json!(active));
        }
        "direct" => {
            rule.insert("action".into(), json!("route"));
            rule.insert("outbound".into(), json!("direct"));
        }
        "block" => {
            rule.insert("action".into(), json!("reject"));
        }
        other => return Err(format!("Unknown route action: {other}")),
    }
    Ok(Value::Object(rule))
}

fn route_set_tag(matcher: &str) -> Option<String> {
    if matcher == "geosite:private" {
        return None;
    }
    if let Some(value) = matcher.strip_prefix("geosite:") {
        Some(format!("geosite-{value}"))
    } else {
        matcher.strip_prefix("geoip:").map(|value| format!("geoip-{value}"))
    }
}

fn rule_set_tags(state: &State) -> Vec<String> {
    let mut tags = vec!["geosite-cn".to_string(), "geoip-cn".to_string()];
    for route in &state.routes {
        if let Some(tag) = route_set_tag(&route.matcher) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

pub(super) fn missing_rule_set_tags(state: &State) -> Vec<String> {
    rule_set_tags(state)
        .into_iter()
        .filter(|tag| {
            !std::path::Path::new(&format!("{RULE_SET_DIR}/{tag}.srs")).exists()
                && !std::path::Path::new(&format!("{RULE_SET_CACHE_DIR}/{tag}.srs")).exists()
        })
        .collect()
}

pub(super) fn rule_set_url(tag: &str) -> Option<String> {
    let repository = if tag.starts_with("geosite-") {
        "sing-geosite"
    } else if tag.starts_with("geoip-") {
        "sing-geoip"
    } else {
        return None;
    };
    Some(format!("{RULE_SET_BASE}/{repository}/rule-set/{tag}.srs"))
}

fn rule_set_path(tag: &str) -> String {
    let packaged = format!("{RULE_SET_DIR}/{tag}.srs");
    if std::path::Path::new(&packaged).exists() {
        return packaged;
    }
    format!("{RULE_SET_CACHE_DIR}/{tag}.srs")
}

fn rule_set_definition(tag: &str) -> Value {
    json!({"type": "local", "tag": tag, "format": "binary", "path": rule_set_path(tag)})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sbctl::state::Mesh;

    fn node() -> Node {
        Node {
            tag: "nyc".into(),
            address: "vpn.example.me".into(),
            port: 443,
            uuid: "11111111-1111-1111-1111-111111111111".into(),
            sni: "www.bing.com".into(),
            public_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=".into(),
            short_id: "0123456789abcdef".into(),
            security: String::new(),
            pin: String::new(),
        }
    }

    fn state() -> State {
        State {
            version: 1,
            active: "nyc".into(),
            mixed_port: 20808,
            nodes: vec![node()],
            routes: vec![],
            mesh: None,
        }
    }

    fn mesh() -> Mesh {
        Mesh {
            address: "10.20.30.2/32".into(),
            private_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=".into(),
            peer_public_key: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb=".into(),
            port: 51820,
            subnet: "10.20.30.0/24".into(),
            enabled: true,
        }
    }

    #[test]
    fn reality_public_key_strips_padding() {
        let data = build_config(&state()).unwrap();
        let config: Value = serde_json::from_str(&data).unwrap();
        let key = &config["outbounds"][1]["tls"]["reality"]["public_key"];
        assert_eq!(key.as_str().unwrap().len(), 43);
        assert!(!key.as_str().unwrap().contains('='));
    }

    #[test]
    fn mesh_toggle_adds_endpoint_and_rules() {
        let without = build_config(&state()).unwrap();
        let without: Value = serde_json::from_str(&without).unwrap();
        assert_eq!(without["endpoints"].as_array().unwrap().len(), 0);

        let mut with = state();
        with.mesh = Some(mesh());
        let with = build_config(&with).unwrap();
        let with: Value = serde_json::from_str(&with).unwrap();
        assert_eq!(with["endpoints"][0]["detour"].as_str().unwrap(), "nyc");
        assert_eq!(with["endpoints"][0]["peers"][0]["address"].as_str().unwrap(), "127.0.0.1");
        let rules = with["route"]["rules"].as_array().unwrap();
        assert!(rules.iter().any(|rule| rule.get("override_address").is_some()));
        assert!(rules.iter().any(|rule| rule["outbound"].as_str() == Some("wg")));
    }

    #[test]
    fn custom_rules_precede_builtin_rules() {
        let mut state = state();
        state.routes = vec![Route { action: "direct".into(), matcher: "domain:example.com".into() }];
        let data = build_config(&state).unwrap();
        let config: Value = serde_json::from_str(&data).unwrap();
        let rules = config["route"]["rules"].as_array().unwrap();
        let custom = rules
            .iter()
            .position(|rule| rule.get("domain_suffix").is_some())
            .unwrap();
        let geosite = rules
            .iter()
            .position(|rule| rule.get("rule_set").is_some())
            .unwrap();
        assert!(custom < geosite);
        assert_eq!(config["route"]["final"].as_str().unwrap(), "nyc");
    }

    #[test]
    fn translates_every_matcher_prefix() {
        let route = |matcher: &str| Route { action: "proxy".into(), matcher: matcher.into() };
        let translated = |matcher: &str| translate_route(&route(matcher), "nyc").unwrap();
        assert_eq!(translated("domain:a.com")["domain_suffix"][0].as_str().unwrap(), "a.com");
        assert_eq!(translated("full:a.com")["domain"][0].as_str().unwrap(), "a.com");
        assert_eq!(translated("regexp:a.*")["domain_regex"][0].as_str().unwrap(), "a.*");
        assert_eq!(translated("geosite:netflix")["rule_set"][0].as_str().unwrap(), "geosite-netflix");
        assert_eq!(translated("geoip:us")["rule_set"][0].as_str().unwrap(), "geoip-us");
        assert_eq!(translated("1.2.3.4")["ip_cidr"][0].as_str().unwrap(), "1.2.3.4");
        assert!(translated("geosite:private").get("ip_is_private").is_some());

        let blocked = translate_route(&Route { action: "block".into(), matcher: "full:ads.com".into() }, "nyc").unwrap();
        assert_eq!(blocked["action"].as_str().unwrap(), "reject");
        assert!(blocked.get("outbound").is_none());

        assert!(translate_route(&Route { action: "other".into(), matcher: "full:a.com".into() }, "nyc").is_err());
    }

    #[test]
    fn node_without_active_node_is_an_error() {
        let mut state = state();
        state.active = String::new();
        assert!(build_config(&state).is_err());
    }

    #[test]
    fn no_detour_to_the_empty_direct_outbound() {
        let data = build_config(&state()).unwrap();
        let config: Value = serde_json::from_str(&data).unwrap();
        let cn = config["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|server| server["tag"] == "cn")
            .unwrap();
        assert!(cn.get("detour").is_none());
        assert!(config["route"]["auto_detect_interface"].as_bool().unwrap());
    }
}
