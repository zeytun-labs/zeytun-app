use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    pub rule_type: RuleType,
    pub value: String,
    pub action: RuleAction,
    pub used: usize,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleType {
    #[serde(rename = "DOMAIN")]
    Domain,

    #[serde(rename = "DOMAIN_SUFFIC")]
    DomainSuffix,

    #[serde(rename = "DOMAIN_KEYWORD")]
    DomainKeyword,

    #[serde(rename = "DOMAIN_REGEX")]
    DomainRegex,

    #[serde(rename = "IP_CIDR")]
    IpCidr,

    #[serde(rename = "IN_PORT")]
    InPort,

    #[serde(rename = "DEST_PORT")]
    DestPort,

    #[serde(rename = "GEO_IP")]
    GeoIp,

    #[serde(rename = "GEO_SITE")]
    GeoSite,

    #[serde(rename = "PROCESS_NAME")]
    ProcessName,

    #[serde(rename = "PROCESS_PATH")]
    ProcessPath,

    #[serde(rename = "PROCESS_PATH_REGEX")]
    ProcessPathRegex,

    #[serde(rename = "PROTOCOL")]
    Protocol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuleAction {
    #[serde(flatten)]
    pub action: RuleActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RuleActionType {
    Route(RouteAction),
    Reject(RejectAction),
    Bypass,
    HijackDns,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAction {
    pub outbound: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<RejectMethod>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_drop: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RejectMethod {
    Default,
    Drop,
    Reply,
}
