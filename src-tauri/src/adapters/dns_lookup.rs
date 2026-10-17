//! DNS diagnostic results. System lookups deliberately use the OS resolver,
//! rather than reading resolv.conf or sending a packet to a guessed DNS server.
use serde::{Deserialize, Serialize};
use std::{net::ToSocketAddrs, time::Instant};

use super::clash_api::ClashDnsQueryResponse;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DnsLookupTrace {
    pub server: Option<String>,
    pub transport: Option<String>,
    pub rule: Option<String>,
    pub rule_index: Option<usize>,
    pub outbound: Option<String>,
    pub cache_hit: Option<bool>,
    pub source: Option<String>,
    pub selection: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: String,
    pub data: String,
    pub ttl: Option<u32>,
    pub section: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DnsLookupResult {
    pub resolved_ips: Vec<String>,
    pub server_used: String,
    pub latency_ms: u64,
    pub status: Option<u16>,
    pub record_type: String,
    pub ttl: Option<u32>,
    pub records: Vec<DnsRecord>,
    pub trace: Option<DnsLookupTrace>,
}

pub fn normalize_record_type(record_type: &str, system: bool) -> Result<String, String> {
    let record_type = record_type.trim().to_ascii_uppercase();
    if !matches!(record_type.as_str(), "A" | "AAAA" | "CNAME") {
        return Err("DNS record type must be A, AAAA, or CNAME".into());
    }
    if system && record_type == "CNAME" {
        return Err("System DNS comparison supports A and AAAA; the OS resolver does not expose CNAME records".into());
    }
    Ok(record_type)
}

fn record_name(record_type: u16) -> String {
    match record_type {
        1 => "A".into(),
        2 => "NS".into(),
        5 => "CNAME".into(),
        6 => "SOA".into(),
        12 => "PTR".into(),
        15 => "MX".into(),
        16 => "TXT".into(),
        28 => "AAAA".into(),
        33 => "SRV".into(),
        41 => "OPT".into(),
        65 => "HTTPS".into(),
        other => format!("TYPE{other}"),
    }
}

pub fn core_result(
    response: ClashDnsQueryResponse,
    record_type: String,
    latency_ms: u64,
) -> DnsLookupResult {
    let mut resolved_ips = Vec::new();
    let mut ttl = None;
    let mut records = Vec::new();
    for (section, answers) in [
        ("answer", response.answer),
        ("authority", response.authority),
        ("additional", response.additional),
    ] {
        for answer in answers {
            if section == "answer" {
                ttl = Some(ttl.map_or(answer.ttl, |value: u32| value.min(answer.ttl)));
                if matches!(answer.record_type, 1 | 28) {
                    if let Ok(ip) = answer.data.trim().parse::<std::net::IpAddr>() {
                        let ip = ip.to_string();
                        if !resolved_ips.contains(&ip) {
                            resolved_ips.push(ip);
                        }
                    }
                }
            }
            records.push(DnsRecord {
                name: answer.name,
                record_type: record_name(answer.record_type),
                data: answer.data.trim().into(),
                ttl: Some(answer.ttl),
                section,
            });
        }
    }
    DnsLookupResult {
        resolved_ips,
        server_used: response.server,
        latency_ms,
        status: Some(response.status),
        record_type,
        ttl,
        records,
        trace: response.trace,
    }
}

pub fn system_result(domain: &str, record_type: &str) -> Result<DnsLookupResult, String> {
    let started = Instant::now();
    let addresses = (domain, 0)
        .to_socket_addrs()
        .map_err(|e| format!("System DNS lookup failed: {e}"))?;
    let mut resolved_ips = Vec::new();
    for addr in addresses {
        let ip = addr.ip();
        if (record_type == "A" && ip.is_ipv4()) || (record_type == "AAAA" && ip.is_ipv6()) {
            let ip = ip.to_string();
            if !resolved_ips.contains(&ip) {
                resolved_ips.push(ip);
            }
        }
    }
    let records = resolved_ips
        .iter()
        .map(|ip| DnsRecord {
            name: domain.into(),
            record_type: record_type.into(),
            data: ip.clone(),
            ttl: None,
            section: "answer",
        })
        .collect();
    Ok(DnsLookupResult {
        resolved_ips,
        server_used: "System resolver".into(),
        latency_ms: started.elapsed().as_millis() as u64,
        // getaddrinfo does not expose wire RCODE, TTL, CNAME chains or upstream.
        status: None,
        record_type: record_type.into(),
        ttl: None,
        records,
        trace: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_record_type_and_system_capabilities() {
        assert_eq!(normalize_record_type(" aaaa ", false).unwrap(), "AAAA");
        assert!(normalize_record_type("CNAME", true).is_err());
        assert!(normalize_record_type("A&server=other", false).is_err());
    }

    #[test]
    fn preserves_ipv6_cname_authority_and_runtime_trace() {
        let response: ClashDnsQueryResponse = serde_json::from_value(serde_json::json!({
            "Status": 0, "Server": "google",
            "Answer": [
                {"name":"app.example.","type":5,"TTL":60,"data":"edge.example."},
                {"name":"edge.example.","type":28,"TTL":30,"data":"2001:db8::1"}
            ],
            "Authority":[{"name":"example.","type":2,"TTL":120,"data":"ns.example."}],
            "Trace":{"server":"google","transport":"https","cache_hit":true,"rule_index":0,"selection":"rule"}
        })).unwrap();
        let result = core_result(response, "AAAA".into(), 5);
        assert_eq!(result.resolved_ips, ["2001:db8::1"]);
        assert_eq!(result.ttl, Some(30));
        assert_eq!(result.records.len(), 3);
        assert_eq!(result.records[0].record_type, "CNAME");
        assert_eq!(result.records[2].section, "authority");
        assert_eq!(result.trace.unwrap().cache_hit, Some(true));
    }

    #[test]
    fn system_lookup_does_not_invent_dns_packet_metadata() {
        let result = system_result("localhost", "A").unwrap();
        assert!(!result.resolved_ips.is_empty());
        assert!(result
            .resolved_ips
            .iter()
            .all(|ip| ip.parse::<std::net::Ipv4Addr>().unwrap().is_loopback()));
        assert!(result.status.is_none());
        assert!(result.ttl.is_none());
        assert!(result.trace.is_none());
        assert!(result
            .records
            .iter()
            .all(|r| r.ttl.is_none() && r.record_type == "A"));
    }

    #[test]
    fn older_core_response_keeps_attribution_unknown() {
        let response =
            serde_json::from_value(serde_json::json!({"Status":3,"Server":"internal"})).unwrap();
        let result = core_result(response, "A".into(), 1);
        assert!(result.trace.is_none());
        assert!(result.records.is_empty());
        assert_eq!(result.status, Some(3));
    }
}
