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

#[cfg(feature = "fs")]
impl SpecConfig {
    pub fn load(openspec_dir: &Path) -> Self {
        let path = openspec_dir.join("config.yaml");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = SpecConfig::default();
        assert!(config.schema.is_none());
        assert!(config.context.is_none());
        assert!(config.rules.is_empty());
    }

    #[cfg(all(test, feature = "fs"))]
    mod fs_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_load_config_missing() {
            let dir = TempDir::new().expect("failed to create temp dir");
            let config = SpecConfig::load(dir.path());
            assert!(config.schema.is_none());
        }

        #[test]
        fn test_load_config_yaml() {
            let dir = TempDir::new().expect("failed to create temp dir");
            let yaml = r#"
schema: custom
context: |
  This is the project context
rules:
  formatting:
    - Use snake_case
    - 80 char lines
"#;
            std::fs::write(dir.path().join("config.yaml"), yaml).expect("failed to write config");

            let config = SpecConfig::load(dir.path());
            assert_eq!(config.schema.as_deref(), Some("custom"));
            assert!(config.context.as_ref().unwrap().contains("project context"));
            assert!(config.rules.contains_key("formatting"));
            assert_eq!(config.rules["formatting"].len(), 2);
        }
    }
}
