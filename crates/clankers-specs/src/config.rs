//! Project spec config (openspec/config.yaml)

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpecConfig {
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub rules: HashMap<String, Vec<String>>,
}

impl SpecConfig {
    pub fn load(openspec_dir: &Path) -> Self {
        let path = openspec_dir.join("config.yaml");
        std::fs::read_to_string(&path).ok().and_then(|s| serde_yaml::from_str(&s).ok()).unwrap_or_default()
    }
}
