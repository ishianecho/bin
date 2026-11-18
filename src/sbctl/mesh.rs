use serde_json::Value;

use super::state::Mesh;

pub fn import(path: &str) -> Result<Mesh, String> {
    let data =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    let config: Value =
        serde_json::from_str(&data).map_err(|e| format!("invalid JSON in {path}: {e}"))?;
    let endpoint = config
        .pointer("/endpoints/0")
        .ok_or("the config has no endpoints[0]")?;
    if endpoint.get("type").and_then(Value::as_str) != Some("wireguard") {
        return Err("endpoints[0] is not a wireguard endpoint".into());
    }
    let address = endpoint
        .pointer("/address/0")
        .and_then(Value::as_str)
        .ok_or("the endpoint has no address")?;
    let private_key = endpoint
        .get("private_key")
        .and_then(Value::as_str)
        .ok_or("the endpoint has no private_key")?;
    let peer = endpoint
        .pointer("/peers/0")
        .ok_or("the endpoint has no peers[0]")?;
    let peer_public_key = peer
        .get("public_key")
        .and_then(Value::as_str)
        .ok_or("the peer has no public_key")?;
    let port = peer.get("port").and_then(Value::as_i64).ok_or("the peer has no port")?;
    let subnet = config["route"]["rules"]
        .as_array()
        .and_then(|rules| {
            rules
                .iter()
                .find(|rule| rule.get("outbound").and_then(Value::as_str) == Some("wg"))
        })
        .and_then(|rule| rule.pointer("/ip_cidr/0"))
        .and_then(Value::as_str)
        .ok_or("no route rule sends traffic to the wg outbound")?;
    if private_key.is_empty() || peer_public_key.is_empty() {
        return Err("the endpoint has an empty key".into());
    }
    if !(1..=65535).contains(&port) {
        return Err(format!("invalid peer port: {port}"));
    }
    Ok(Mesh {
        address: address.into(),
        private_key: private_key.into(),
        peer_public_key: peer_public_key.into(),
        port,
        subnet: subnet.into(),
        enabled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "endpoints": [
        {
          "type": "wireguard",
          "tag": "wg",
          "address": ["10.20.30.2/32"],
          "private_key": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=",
          "mtu": 1420,
          "detour": "vless",
          "peers": [
            {"address": "127.0.0.1", "port": 51820, "public_key": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb=", "allowed_ips": ["0.0.0.0/0"]}
          ]
        }
      ],
      "route": {
        "rules": [
          {"action": "sniff"},
          {"ip_cidr": ["10.20.30.0/24"], "action": "route", "outbound": "wg"}
        ]
      }
    }"#;

    #[test]
    fn parses_the_vpn_template_shape() {
        std::fs::write("/tmp/sbctl-mesh-fixture.json", FIXTURE).unwrap();
        let mesh = import("/tmp/sbctl-mesh-fixture.json").unwrap();
        assert_eq!(mesh.address, "10.20.30.2/32");
        assert_eq!(mesh.peer_public_key.len(), 44);
        assert_eq!(mesh.port, 51820);
        assert_eq!(mesh.subnet, "10.20.30.0/24");
        assert!(!mesh.enabled);
    }

    #[test]
    fn rejects_configs_without_a_wireguard_endpoint() {
        std::fs::write("/tmp/sbctl-mesh-broken.json", "{\"endpoints\": []}").unwrap();
        assert!(import("/tmp/sbctl-mesh-broken.json").is_err());
        std::fs::write("/tmp/sbctl-mesh-broken.json", "{}").unwrap();
        assert!(import("/tmp/sbctl-mesh-broken.json").is_err());
    }
}
