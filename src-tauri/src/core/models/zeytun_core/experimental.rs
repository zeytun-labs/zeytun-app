use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Experimental {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_file: Option<CacheFile>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clash_api: Option<ClashApi>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v2ray_api: Option<V2RayApi>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_fakeip: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdrc_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_dns: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClashApi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_controller: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ui: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ui_download_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ui_download_detour: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_control_allow_origin: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_control_allow_private_network: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct V2RayApi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<V2RayApiStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct V2RayApiStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbounds: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbounds: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<String>>,
}
