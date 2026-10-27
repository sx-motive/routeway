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
        .ok_or_else(|| t!("err.no_server"))?;

    if server.address.trim().is_empty() {
        return Err(t!("err.no_address"));
    }
    if server.uuid.trim().is_empty() {
        return Err(t!("err.no_uuid"));
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
    let mut route_rules = vec![json!({ "action": "sniff" })];
    if settings.tun {
        // Windows broadcasts NetBIOS, mDNS and multicast on every adapter; under
        // auto_route that chatter would otherwise churn through the core.
        route_rules.push(json!({ "network": ["udp"], "port": [135, 137, 138, 139, 5353], "action": "reject" }));
        route_rules.push(json!({ "ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject" }));
    }
    route_rules.extend([
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "ip_is_private": true, "outbound": "direct" }),
        // Anything arriving on the probe port is forced through the server. The UI
        // uses it to answer "is the server alive" without touching the domain list.
        json!({ "inbound": "probe", "outbound": "proxy" }),
    ]);
    let mut dns_rules: Vec<Value> = Vec::new();

    let direct = settings.normalized_direct();
    if !direct.is_empty() {
        route_rules.push(json!({ "domain_suffix": direct, "outbound": "direct" }));
    }

    // Process rules come before domain rules: "this app always goes through the
    // server" is a stronger statement than "this domain does".
    let (process_names, process_paths) = settings.process_rules();
    if !process_names.is_empty() {
        route_rules.push(json!({ "process_name": process_names, "outbound": "proxy" }));
    }
    if !process_paths.is_empty() {
        route_rules.push(json!({ "process_path": process_paths, "outbound": "proxy" }));
    }

    if !domains.is_empty() {
        route_rules.push(json!({ "domain_suffix": domains, "outbound": "proxy" }));
        // Resolve proxied names through the tunnel so a poisoned local resolver
        // cannot answer for them.
        dns_rules.push(json!({ "domain_suffix": domains, "server": "dns-proxy" }));
    }

    let mut inbounds = vec![
        json!({
            "type": "mixed",
            "tag": "in",
            "listen": "127.0.0.1",
            "listen_port": settings.listen_port
        }),
        json!({
            "type": "mixed",
            "tag": "probe",
            "listen": "127.0.0.1",
            "listen_port": settings.listen_port + 1
        }),
    ];
    if settings.tun {
        inbounds.push(json!({
            "type": "tun",
            "tag": "tun",
            "address": ["172.19.0.1/30", "fdfe:dcba:9876::1/126"],
            "mtu": 9000,
            "auto_route": true,
            // strict_route would also firewall off DNS servers reached through
            // other adapters, which breaks name resolution inside corporate VPNs.
            "strict_route": false,
            "stack": "mixed"
        }));
    }

    // In TUN mode apps resolve through us and pick whatever family we answer
    // with. Servers without IPv6 egress would silently fail AAAA-first clients.
    let dns_strategy = if settings.tun { "ipv4_only" } else { "prefer_ipv4" };
    // In proxy mode keep the network's working DNS (including VPN/corporate
    // resolvers), rather than making every direct connection depend on UDP/53
    // to Cloudflare. TUN must use an external resolver to avoid DNS recursion.
    let direct_dns = if settings.tun {
        json!({ "type": "udp", "tag": "dns-direct", "server": "1.1.1.1" })
    } else {
        json!({ "type": "local", "tag": "dns-direct" })
    };

    Ok(json!({
        // Info carries the per-connection process and destination that explain a
        // failure, but logs every connection, so it is opt-in.
        "log": { "level": if settings.verbose_log { "info" } else { "warning" }, "timestamp": true },
        "dns": {
            "servers": [
                { "type": "https", "tag": "dns-proxy", "server": "1.1.1.1", "detour": "proxy" },
                direct_dns
            ],
            "rules": dns_rules,
            // Unmatched DNS must not depend on the remote server being up.
            "final": "dns-direct",
            "strategy": dns_strategy
        },
        "inbounds": inbounds,
        "outbounds": [
            proxy_outbound,
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": route_rules,
            "final": "direct",
            "default_domain_resolver": "dns-direct",
            "find_process": true,
            // Direct and server-bound sockets must bind the physical interface,
            // otherwise auto_route sends them straight back into the tunnel.
            "auto_detect_interface": settings.tun
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
    fn empty_domain_list_sends_no_user_traffic_through_the_server() {
        let cfg = build_config(&settings_with(&[])).unwrap();
        let rules = cfg["route"]["rules"].as_array().unwrap();

        // The probe inbound is allowed to reach the server — it exists precisely
        // to test the server. Nothing arriving on the main inbound may.
        let user_rules_to_proxy: Vec<_> = rules
            .iter()
            .filter(|r| r["outbound"] == "proxy" && r["inbound"] != "probe")
            .collect();
        assert!(user_rules_to_proxy.is_empty(), "{user_rules_to_proxy:?}");

        assert_eq!(cfg["route"]["final"], "direct");
        assert!(cfg["dns"]["rules"].as_array().unwrap().is_empty());
    }

    #[test]
    fn probe_inbound_always_reaches_the_server() {
        let cfg = build_config(&settings_with(&["youtube.com"])).unwrap();
        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds[1]["tag"], "probe");
        assert_eq!(
            inbounds[1]["listen_port"].as_u64().unwrap(),
            inbounds[0]["listen_port"].as_u64().unwrap() + 1
        );
        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert!(rules
            .iter()
            .any(|r| r["inbound"] == "probe" && r["outbound"] == "proxy"));
    }

    #[test]
    fn reality_block_is_omitted_without_a_public_key() {
        let mut settings = settings_with(&["youtube.com"]);
        settings.server.as_mut().unwrap().public_key = String::new();
        let cfg = build_config(&settings).unwrap();
        assert!(cfg["outbounds"][0]["tls"]["reality"].is_null());
    }

    #[test]
    fn process_rules_split_by_shape_and_outrank_domains() {
        let mut settings = settings_with(&["youtube.com"]);
        settings.processes = vec!["codex".into(), "/usr/bin/curl".into(), "claude".into()];
        let cfg = build_config(&settings).unwrap();

        assert_eq!(cfg["route"]["find_process"], true);
        let rules = cfg["route"]["rules"].as_array().unwrap();

        let by_name = rules.iter().position(|r| !r["process_name"].is_null()).unwrap();
        let by_path = rules.iter().position(|r| !r["process_path"].is_null()).unwrap();
        let by_domain = rules.iter().position(|r| !r["domain_suffix"].is_null()).unwrap();

        assert_eq!(rules[by_name]["process_name"][0], "codex");
        assert_eq!(rules[by_name]["process_name"][1], "claude");
        assert_eq!(rules[by_path]["process_path"][0], "/usr/bin/curl");
        assert!(by_name < by_domain && by_path < by_domain);
    }

    #[test]
    fn direct_domains_outrank_process_and_domain_rules() {
        let mut settings = settings_with(&["argustelecom.ru"]);
        settings.processes = vec!["claude.exe".into()];
        settings.direct = vec!["jira.argustelecom.ru".into()];
        let cfg = build_config(&settings).unwrap();
        let rules = cfg["route"]["rules"].as_array().unwrap();

        let direct = rules
            .iter()
            .position(|r| r["outbound"] == "direct" && !r["domain_suffix"].is_null())
            .unwrap();
        let by_name = rules.iter().position(|r| !r["process_name"].is_null()).unwrap();
        let proxied = rules
            .iter()
            .position(|r| r["outbound"] == "proxy" && !r["domain_suffix"].is_null())
            .unwrap();

        assert_eq!(rules[direct]["domain_suffix"][0], "jira.argustelecom.ru");
        assert!(direct < by_name && direct < proxied);
    }

    #[test]
    fn process_lookup_is_on_so_the_log_can_name_apps() {
        let cfg = build_config(&settings_with(&["youtube.com"])).unwrap();
        assert_eq!(cfg["route"]["find_process"], true);
        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert!(rules.iter().all(|r| r["process_name"].is_null()));
    }

    #[test]
    fn per_connection_logging_is_opt_in() {
        let mut settings = settings_with(&["youtube.com"]);
        assert_eq!(build_config(&settings).unwrap()["log"]["level"], "warning");
        settings.verbose_log = true;
        assert_eq!(build_config(&settings).unwrap()["log"]["level"], "info");
    }

    #[test]
    fn tun_rejects_broadcast_chatter_before_dns_hijack() {
        let mut settings = settings_with(&["youtube.com"]);
        let proxy_rules = build_config(&settings).unwrap()["route"]["rules"].clone();
        assert!(proxy_rules.as_array().unwrap().iter().all(|r| r["action"] != "reject"));

        settings.tun = true;
        let cfg = build_config(&settings).unwrap();
        let rules = cfg["route"]["rules"].as_array().unwrap();
        let hijack = rules.iter().position(|r| r["action"] == "hijack-dns").unwrap();
        let rejects: Vec<usize> = rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r["action"] == "reject")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(rejects.len(), 2);
        assert!(rejects.iter().all(|&i| i < hijack));
        assert!(rules.iter().any(|r| r["port"].as_array().is_some_and(|p| p.contains(&5353.into()))));
        assert!(rules.iter().any(|r| r["ip_cidr"][0] == "224.0.0.0/3"));
    }

    #[test]
    fn proxy_mode_has_no_tun_inbound() {
        let cfg = build_config(&settings_with(&["youtube.com"])).unwrap();
        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert!(inbounds.iter().all(|i| i["type"] != "tun"));
        assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
        assert_eq!(cfg["route"]["auto_detect_interface"], false);
        assert_eq!(cfg["dns"]["final"], "dns-direct");
        assert_eq!(cfg["dns"]["servers"][1]["type"], "local");
    }

    #[test]
    fn tun_mode_captures_everything_but_keeps_the_split() {
        let mut settings = settings_with(&["youtube.com"]);
        settings.tun = true;
        let cfg = build_config(&settings).unwrap();

        let inbounds = cfg["inbounds"].as_array().unwrap();
        let tun = inbounds.iter().find(|i| i["type"] == "tun").unwrap();
        assert_eq!(tun["auto_route"], true);
        assert_eq!(tun["strict_route"], false);
        assert_eq!(cfg["dns"]["strategy"], "ipv4_only");
        assert_eq!(cfg["route"]["auto_detect_interface"], true);
        assert_eq!(cfg["dns"]["final"], "dns-direct");
        assert_eq!(cfg["dns"]["servers"][1]["type"], "udp");

        assert!(inbounds.iter().any(|i| i["tag"] == "in"));
        assert!(inbounds.iter().any(|i| i["tag"] == "probe"));

        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["action"], "sniff");
        assert!(rules.iter().any(|r| r["action"] == "hijack-dns"));
        assert_eq!(cfg["route"]["final"], "direct");
    }

    #[test]
    fn missing_server_is_an_error() {
        let settings = Settings::default();
        assert!(build_config(&settings).is_err());
    }
}
