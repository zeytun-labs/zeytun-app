//! DNS endpoint parsing shared by validation and config compilation.
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsTransport {
    Udp,
    Tcp,
    Tls,
    Https,
    Quic,
}

impl DnsTransport {
    fn scheme(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::Tls => "tls",
            Self::Https => "https",
            Self::Quic => "quic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsEndpoint {
    pub transport: DnsTransport,
    pub host: String,
    pub port: Option<u16>,
    pub path: Option<String>,
    explicit_scheme: bool,
}

impl DnsEndpoint {
    pub fn parse(address: &str) -> Result<Self, String> {
        let address = address.trim();
        if address.is_empty()
            || address.chars().any(|c| c.is_whitespace() || c.is_control())
            || address.contains(['\\', '#', '@'])
        {
            return Err("DNS server address is empty or contains invalid characters".into());
        }
        let (transport, rest, explicit_scheme) =
            if let Some((scheme, rest)) = address.split_once("://") {
                let transport = match scheme.to_ascii_lowercase().as_str() {
                    "udp" => DnsTransport::Udp,
                    "tcp" => DnsTransport::Tcp,
                    "tls" => DnsTransport::Tls,
                    "https" => DnsTransport::Https,
                    "quic" => DnsTransport::Quic,
                    _ => return Err("DNS protocol must be UDP, TCP, DoT, DoH, or QUIC".into()),
                };
                (transport, rest, true)
            } else {
                (DnsTransport::Udp, address, false)
            };
        let split = rest.find(['/', '?']).unwrap_or(rest.len());
        let authority = &rest[..split];
        let suffix = &rest[split..];
        if !suffix.is_empty() && transport != DnsTransport::Https {
            return Err("Only DNS over HTTPS servers can include a path or query".into());
        }
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, after) = bracketed
                .split_once(']')
                .ok_or("Invalid bracketed DNS server IPv6 address")?;
            let ip = host
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| "Invalid DNS server IPv6 address")?;
            let port = if after.is_empty() {
                None
            } else {
                Some(parse_port(
                    after.strip_prefix(':').ok_or("Invalid DNS server port")?,
                )?)
            };
            (ip.to_string(), port)
        } else if let Ok(ip) = authority.parse::<IpAddr>() {
            (ip.to_string(), None)
        } else {
            let (host, port) = match authority.rsplit_once(':') {
                Some((host, port)) => (host, Some(parse_port(port)?)),
                None => (authority, None),
            };
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            if host.is_empty()
                || host.len() > 253
                || !host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && label
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && label
                            .as_bytes()
                            .last()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err("DNS server must have a valid hostname, IPv4, or IPv6 address".into());
            }
            // A malformed IPv4 must not be treated as a hostname.
            if host.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                && host.parse::<IpAddr>().is_err()
            {
                return Err("Invalid DNS server IPv4 address".into());
            }
            (host, port)
        };
        if host
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_unspecified() || ip.is_multicast())
        {
            return Err("DNS server must be a unicast address".into());
        }
        let path = (transport == DnsTransport::Https).then(|| {
            if suffix.is_empty() {
                "/dns-query".into()
            } else if suffix.starts_with('?') {
                format!("/dns-query{suffix}")
            } else {
                suffix.into()
            }
        });
        Ok(Self {
            transport,
            host,
            port,
            path,
            explicit_scheme,
        })
    }

    pub fn canonical(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let port = self.port.map(|p| format!(":{p}")).unwrap_or_default();
        let scheme = if self.explicit_scheme {
            format!("{}://", self.transport.scheme())
        } else {
            String::new()
        };
        format!(
            "{scheme}{host}{port}{}",
            self.path.as_deref().unwrap_or_default()
        )
    }
}

fn parse_port(port: &str) -> Result<u16, String> {
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err("DNS server port must be between 1 and 65535".into());
    }
    port.parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
        .ok_or_else(|| "DNS server port must be between 1 and 65535".into())
}

pub fn normalize_profile_dns(profile: &mut crate::core::dto::ZeytunCore) -> Result<(), String> {
    let dns = profile
        .dns
        .get_or_insert_with(crate::core::dto::DnsConfig::with_default_server);
    if dns.mode.is_some() {
        dns.migrate_legacy();
    }
    let mut tags = std::collections::HashSet::new();
    for server in dns.servers.as_deref_mut().unwrap_or_default() {
        if server.tag.trim().is_empty() || server.tag.trim() != server.tag {
            return Err(
                "DNS server tag must be nonempty and have no surrounding whitespace".into(),
            );
        }
        if server.tag.starts_with("__dns-direct-")
            || matches!(
                server.tag.as_str(),
                "local-dns"
                    | "local-hosts"
                    | "fakeip"
                    | "builtin-fallback-resolver"
                    | "block"
                    | "auto-dns-proxy"
                    | "auto-dns-direct"
            )
        {
            return Err(format!(
                "DNS server tag `{}` is reserved for DNS hosts or built-in resolvers",
                server.tag
            ));
        }
        if !tags.insert(server.tag.clone()) {
            return Err("Duplicate DNS server tag".into());
        }
        server.name = server.name.trim().to_string();
        if server.name.is_empty() {
            return Err("DNS server name must not be empty".into());
        }
        server.address = DnsEndpoint::parse(&server.address)?.canonical();
        server.detour = server
            .detour
            .take()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());
        if server
            .detour
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case("direct"))
        {
            server.detour = Some("direct".into());
        }
    }
    if let Some(final_server) = dns.final_server.as_deref().filter(|s| !s.is_empty()) {
        if final_server != "local-dns" && !tags.contains(final_server) {
            return Err("Final DNS resolver is not a configured DNS server".into());
        }
    }
    for server in dns.servers.as_deref().unwrap_or_default() {
        if let Some(detour) = server
            .detour
            .as_deref()
            .filter(|d| *d != "direct" && *d != "follow-mode")
        {
            if !profile.policies.iter().any(|p| p.tag == detour)
                && !profile.proxies.iter().any(|p| p.tag == detour)
            {
                return Err("DNS server proxy route is not a configured policy or proxy".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_endpoints_preserve_ports_paths_and_queries() {
        for (input, expected) in [
            (" 1.1.1.1:5353 ", "1.1.1.1:5353"),
            ("HTTPS://DNS.Google:443", "https://dns.google:443/dns-query"),
            (
                "https://dns.example/query?key=abc",
                "https://dns.example/query?key=abc",
            ),
            ("2001:4860:4860::8888", "[2001:4860:4860::8888]"),
            (
                "tls://[2001:4860:4860::8888]:853",
                "tls://[2001:4860:4860::8888]:853",
            ),
        ] {
            assert_eq!(DnsEndpoint::parse(input).unwrap().canonical(), expected);
        }
    }

    #[test]
    fn invalid_endpoints_are_rejected_without_echoing_credentials() {
        for input in [
            "",
            "https://user:secret@dns.example/dns-query",
            "https://dns.example/#frag",
            "https://dns.example\\path",
            "http://dns.example",
            "1.1.1.1:0",
            "1.1.1.1:65536",
            "999.1.1.1",
            "0.0.0.0",
            "udp://dns.example/query",
            "tcp://dns.example?key=x",
            "[::1",
            "dns.example:",
            "dns example",
            "-bad.example",
        ] {
            let err = DnsEndpoint::parse(input).unwrap_err();
            assert!(!err.contains("secret"));
        }
    }
}
