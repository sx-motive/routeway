use serde_json::{json, Value};

use crate::settings::Settings;

/// Builds a sing-box 1.14 config implementing the core promise of Routeway:
/// listed domains go through the server, everything else goes direct.
///
/// The shape is deliberately conservative — a `mixed` inbound on loopback plus
/// domain-suffix routing — so the same config works whether traffic arrives from
/// the system proxy or from an app pointed at the port manually.
pub fn build_config(settings: &Settings) -> Result<Value, String> {
    let server = settings
        .server
        .as_ref()
        .ok_or_else(|| "сервер не настроен".to_string())?;

    if server.address.trim().is_empty() {
        return Err("не указан адрес сервера".into());
    }
    if server.uuid.trim().is_empty() {
        return Err("не указан UUID".into());
    }

    let domains = settings.normalized_domains();

    let mut tls = json!({
        "enabled": true,
        "server_name": if server.sni.is_empty() { server.address.clone() } else { server.sni.clone() },
        "utls": {
            "enabled": true,
            "fingerprint": if server.fingerprint.is_empty() { "chrome".to_string() } else { server.fingerprint.clone() }
        }
    });

    // REALITY is optional: without a public key this is a plain TLS endpoint.
    if !server.public_key.trim().is_empty() {
        tls["reality"] = json!({
            "enabled": true,
            "public_key": server.public_key,
            "short_id": server.short_id,
        });
    }

    let mut proxy_outbound = json!({
        "type": "vless",
        "tag": "proxy",
        "server": server.address,
        "server_port": server.port,
        "uuid": server.uuid,
        "packet_encoding": "xudp",
        "tls": tls,
    });
    if !server.flow.trim().is_empty() {
        proxy_outbound["flow"] = json!(server.flow);
    }

    // Route rules are ordered: sniff first so later rules can see the hostname,
    // then DNS hijack, then private IPs direct, then the user's domain list.
    let mut route_rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "ip_is_private": true, "outbound": "direct" }),
        // Anything arriving on the probe port is forced through the server. The UI
        // uses it to answer "is the server alive" without touching the domain list.
        json!({ "inbound": "probe", "outbound": "proxy" }),
    ];
    let mut dns_rules: Vec<Value> = Vec::new();

    if !domains.is_empty() {
        route_rules.push(json!({ "domain_suffix": domains, "outbound": "proxy" }));
        // Resolve proxied names through the tunnel so a poisoned local resolver
        // cannot answer for them.
        dns_rules.push(json!({ "domain_suffix": domains, "server": "dns-proxy" }));
    }

    Ok(json!({
        "log": { "level": "warn", "timestamp": true },
        "dns": {
            "servers": [
                { "type": "https", "tag": "dns-proxy", "server": "1.1.1.1", "detour": "proxy" },
                { "type": "udp", "tag": "dns-direct", "server": "1.1.1.1" }
            ],
            "rules": dns_rules,
            "strategy": "prefer_ipv4"
        },
        "inbounds": [
            {
                "type": "mixed",
                "tag": "in",
                "listen": "127.0.0.1",
                "listen_port": settings.listen_port
            },
            {
                "type": "mixed",
                "tag": "probe",
                "listen": "127.0.0.1",
                "listen_port": settings.listen_port + 1
            }
        ],
        "outbounds": [
            proxy_outbound,
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": route_rules,
            "final": "direct",
            "default_domain_resolver": "dns-direct"
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::build_config;
    use crate::settings::{Server, Settings};

    fn settings_with(domains: &[&str]) -> Settings {
        Settings {
            server: Some(Server {
                label: "Estonia".into(),
                address: "203.0.113.10".into(),
                port: 47443,
                uuid: "11111111-2222-3333-4444-555555555555".into(),
                flow: "xtls-rprx-vision".into(),
                sni: "www.cloudflare.com".into(),
                public_key: "pk".into(),
                short_id: "ab".into(),
                fingerprint: "chrome".into(),
            }),
            domains: domains.iter().map(|d| d.to_string()).collect(),
            ..Settings::default()
        }
    }

    #[test]
    fn routes_listed_domains_to_proxy_and_rest_direct() {
        let cfg = build_config(&settings_with(&["youtube.com"])).unwrap();
        assert_eq!(cfg["route"]["final"], "direct");
        let rules = cfg["route"]["rules"].as_array().unwrap();
        let domain_rule = rules.last().unwrap();
        assert_eq!(domain_rule["outbound"], "proxy");
        assert_eq!(domain_rule["domain_suffix"][0], "youtube.com");
    }

    #[test]
    fn empty_domain_list_still_produces_a_valid_all_direct_config() {
        let cfg = build_config(&settings_with(&[])).unwrap();
        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert!(rules.iter().all(|r| r["outbound"] != "proxy"));
        assert!(cfg["dns"]["rules"].as_array().unwrap().is_empty());
    }

    #[test]
    fn reality_block_is_omitted_without_a_public_key() {
        let mut settings = settings_with(&["youtube.com"]);
        settings.server.as_mut().unwrap().public_key = String::new();
        let cfg = build_config(&settings).unwrap();
        assert!(cfg["outbounds"][0]["tls"]["reality"].is_null());
    }

    #[test]
    fn missing_server_is_an_error() {
        let settings = Settings::default();
        assert!(build_config(&settings).is_err());
    }
}
