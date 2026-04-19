//! External model catalog — load model definitions from a JSON config file
//!
//! Replaces the hardcoded model lists in each backend with a user-editable
//! JSON file so new models don't require recompilation. Falls back to the
//! compiled-in defaults when no config file is present.
//!
//! # File format
//!
//! ```json
//! {
//!   "models": [
//!     {
//!       "id": "gpt-4.1",
//!       "name": "GPT-4.1",
//!       "provider": "openai",
//!       "max_input_tokens": 1048576,
//!       "max_output_tokens": 32768,
//!       "supports_thinking": false,
//!       "supports_images": true,
//!       "supports_tools": true,
//!       "input_cost_per_mtok": 2.0,
//!       "output_cost_per_mtok": 8.0
//!     }
//!   ],
//!   "aliases": {
//!     "4.1": "gpt-4.1",
//!     "o4-mini": "o4-mini-2025-04-16"
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::model::Model;

/// Serializable model catalog — the on-disk format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCatalog {
    /// Additional or replacement model definitions.
    /// Models with the same `id` as a built-in model override it.
    #[serde(default)]
    pub models: Vec<Model>,

    /// Extra aliases beyond the hardcoded ones.
    /// Maps short name → full model ID.
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

impl ModelCatalog {
    /// Load a catalog from a JSON file, returning `None` if the file
    /// doesn't exist (not an error — config is optional).
    pub fn load(path: &Path) -> Option<Self> {
        if !path.exists() {
            debug!("no model catalog at {}, using defaults", path.display());
            return None;
        }

        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("failed to read model catalog {}: {e}", path.display());
                return None;
            }
        };

        match serde_json::from_str::<ModelCatalog>(&contents) {
            Ok(catalog) => {
                info!(
                    "loaded model catalog: {} models, {} aliases from {}",
                    catalog.models.len(),
                    catalog.aliases.len(),
                    path.display()
                );
                Some(catalog)
            }
            Err(e) => {
                warn!("failed to parse model catalog {}: {e}", path.display());
                None
            }
        }
    }

    /// Save the catalog to a JSON file (pretty-printed).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Merge catalog models into a mutable model list.
    /// Catalog entries override built-in entries with the same `id`.
    pub fn apply_to(&self, models: &mut Vec<Model>) {
        for catalog_model in &self.models {
            if let Some(existing) = models.iter_mut().find(|m| m.id == catalog_model.id) {
                *existing = catalog_model.clone();
                debug!("catalog override: {}", catalog_model.id);
            } else {
                models.push(catalog_model.clone());
                debug!("catalog add: {}", catalog_model.id);
            }
        }
    }

    /// Generate a default catalog file from built-in model lists
    /// (useful for `clanker-router init` or first-run).
    pub fn from_builtin_models(models: &[Model]) -> Self {
        Self {
            models: models.to_vec(),
            aliases: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_roundtrip() {
        let catalog = ModelCatalog {
            models: vec![Model {
                id: "test-model".into(),
                name: "Test".into(),
                provider: "test".into(),
                max_input_tokens: 128_000,
                max_output_tokens: 16_384,
                supports_thinking: false,
                supports_images: true,
                supports_tools: true,
                input_cost_per_mtok: Some(1.0),
                output_cost_per_mtok: Some(2.0),
            }],
            aliases: HashMap::from([("test".into(), "test-model".into())]),
        };

        let json = serde_json::to_string_pretty(&catalog).unwrap();
        let parsed: ModelCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].id, "test-model");
        assert_eq!(parsed.aliases["test"], "test-model");
    }

    #[test]
    fn test_catalog_apply_override() {
        let mut models = vec![Model {
            id: "gpt-4o".into(),
            name: "GPT-4o (old)".into(),
            provider: "openai".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(5.0),
            output_cost_per_mtok: Some(15.0),
        }];

        let catalog = ModelCatalog {
            models: vec![
                // Override existing
                Model {
                    id: "gpt-4o".into(),
                    name: "GPT-4o (updated)".into(),
                    provider: "openai".into(),
                    max_input_tokens: 128_000,
                    max_output_tokens: 16_384,
                    supports_thinking: false,
                    supports_images: true,
                    supports_tools: true,
                    input_cost_per_mtok: Some(2.5),
                    output_cost_per_mtok: Some(10.0),
                },
                // Add new
                Model {
                    id: "gpt-4.1".into(),
                    name: "GPT-4.1".into(),
                    provider: "openai".into(),
                    max_input_tokens: 1_048_576,
                    max_output_tokens: 32_768,
                    supports_thinking: false,
                    supports_images: true,
                    supports_tools: true,
                    input_cost_per_mtok: Some(2.0),
                    output_cost_per_mtok: Some(8.0),
                },
            ],
            aliases: Default::default(),
        };

        catalog.apply_to(&mut models);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].name, "GPT-4o (updated)");
        assert_eq!(models[0].input_cost_per_mtok, Some(2.5));
        assert_eq!(models[1].id, "gpt-4.1");
    }

    #[test]
    fn test_catalog_file_roundtrip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let catalog = ModelCatalog {
            models: vec![Model {
                id: "test".into(),
                name: "Test".into(),
                provider: "test".into(),
                max_input_tokens: 8192,
                max_output_tokens: 4096,
                supports_thinking: false,
                supports_images: false,
                supports_tools: false,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
            }],
            aliases: Default::default(),
        };

        catalog.save(tmp.path()).unwrap();
        let loaded = ModelCatalog::load(tmp.path()).unwrap();
        assert_eq!(loaded.models.len(), 1);
        assert_eq!(loaded.models[0].id, "test");
    }

    #[test]
    fn test_catalog_missing_file() {
        assert!(ModelCatalog::load(Path::new("/nonexistent/models.json")).is_none());
    }
}
