use std::fs;
use std::path::PathBuf;

use serde_json::Value;

pub struct ConfigStore {
    base_dir: PathBuf,
}

impl ConfigStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn write_zeytun_core_config(
        &self,
        profile_id: &str,
        config: &Value,
    ) -> Result<PathBuf, String> {
        let dir = self.base_dir.join("zeytun-core");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let path = dir.join(format!("{}.json", profile_id));
        let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;

        fs::write(&path, json).map_err(|e| e.to_string())?;

        Ok(path)
    }
}
