use crate::core::models::proxy::{Protocol, ProxyServer};

#[derive(Debug, Clone)]
pub struct SupportResult {
    pub supported: bool,
    pub reason: Option<String>,
}

pub struct SupportResolver;

impl SupportResolver {
    pub fn resolve(&self, _server: &ProxyServer) -> SupportResult {
        SupportResult {
            supported: true,
            reason: None,
        }
    }

    pub fn normalize(&self, server: &ProxyServer) -> (ProxyServer, Option<String>) {
        let mut server = server.clone();

        let warning = match &mut server.protocol {
            Protocol::Vless(vless) => {
                normalize_transport_in_protocol(&mut vless.network, &server.name)
            }
            Protocol::Vmess(vmess) => {
                normalize_transport_in_protocol(&mut vmess.network, &server.name)
            }
            Protocol::Trojan(trojan) => {
                normalize_transport_in_protocol(&mut trojan.network, &server.name)
            }
            _ => None,
        };

        (server, warning)
    }
}

fn normalize_transport_in_protocol(network: &mut String, name: &str) -> Option<String> {
    let old = network.clone();
    match old.to_ascii_lowercase().as_str() {
        "xray-http" | "xray_http" | "splithttp" | "split-http" | "split_http" => {
            *network = "xhttp".to_string();
            Some(format!(
                "proxy '{name}': transport '{old}' normalized to 'xhttp'"
            ))
        }
        _ => None,
    }
}
