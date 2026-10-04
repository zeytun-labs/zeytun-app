use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimePhase {
    Idle,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreRuntimeStatus {
    pub phase: RuntimePhase,
    pub message: Option<String>,
    pub zeytun_core_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorePathsInfo {
    pub app_data_dir: String,
    pub resource_dir: Option<String>,
    pub core_dir: String,
    pub bin_dir: String,
    pub platform_bin_dir: String,
    pub runtime_dir: String,
    pub config_dir: String,
    pub profile_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryCheck {
    pub name: String,
    pub path: Option<String>,
    pub found: bool,
    pub executable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorePreflightReport {
    pub ok: bool,
    pub platform: String,
    pub paths: CorePathsInfo,
    pub checks: Vec<BinaryCheck>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessHistoryEntry {
    pub name: String,
    pub path: String,
    pub total_connections: u64,
    pub total_upload: u64,
    pub total_download: u64,
    pub last_seen_at: u64,
}
