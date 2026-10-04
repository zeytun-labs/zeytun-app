use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    pub disabled: bool,
    pub level: LogLevel,
    pub output: String,
    pub timestamp: bool,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            disabled: false,
            level: LogLevel::Info,
            output: "stdout".to_string(),
            timestamp: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Panic,
}
