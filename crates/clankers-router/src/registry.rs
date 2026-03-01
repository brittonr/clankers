//! Model registry and provider discovery
//!
//! Maintains a catalog of all available models across all registered providers,
//! with alias resolution and capability-based filtering.

use std::collections::HashMap;

use crate::model::Model;
use crate::model::ModelAliases;

/// Registry of all available models across providers
#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: HashMap<String, Model>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register models from a provider
    pub fn register_models(&mut self, models: &[Model]) {
        for model in models {
            self.models.insert(model.id.clone(), model.clone());
        }
    }

    /// Look up a model by exact ID
    pub fn get(&self, id: &str) -> Option<&Model> {
        self.models.get(id)
    }

    /// Resolve a model by alias, exact ID, or substring match.
    ///
    /// Priority:
    /// 1. Exact ID match
    /// 2. Known alias (e.g. "sonnet" → "claude-sonnet-4-5-20250514")
    /// 3. Case-insensitive substring match
    pub fn resolve(&self, name: &str) -> Option<&Model> {
        // 1. Exact match
        if let Some(model) = self.models.get(name) {
            return Some(model);
        }

        // 2. Alias resolution
        if let Some(resolved_id) = ModelAliases::resolve(name)
            && let Some(model) = self.models.get(resolved_id)
        {
            return Some(model);
        }

        // 3. Substring match
        let lower = name.to_lowercase();
        self.models.values().find(|m| m.id.to_lowercase().contains(&lower))
    }

    /// List all registered models, sorted by ID
    pub fn list(&self) -> Vec<&Model> {
        let mut models: Vec<_> = self.models.values().collect();
        models.sort_by_key(|m| &m.id);
        models
    }

    /// List models for a specific provider
    pub fn list_for_provider(&self, provider: &str) -> Vec<&Model> {
        let mut models: Vec<_> = self.models.values().filter(|m| m.provider == provider).collect();
        models.sort_by_key(|m| &m.id);
        models
    }

    /// Find models that support a specific capability
    pub fn with_capability(&self, thinking: bool, images: bool) -> Vec<&Model> {
        self.models
            .values()
            .filter(|m| (!thinking || m.supports_thinking) && (!images || m.supports_images))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Get the provider name for a model ID
    pub fn provider_for(&self, model_id: &str) -> Option<&str> {
        // Try direct lookup
        if let Some(model) = self.get(model_id) {
            return Some(&model.provider);
        }
        // Try alias
        if let Some(resolved) = ModelAliases::resolve(model_id)
            && let Some(model) = self.get(resolved)
        {
            return Some(&model.provider);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_models() -> Vec<Model> {
        vec![
            Model {
                id: "claude-sonnet-4-5-20250514".into(),
                name: "Claude Sonnet 4.5".into(),
                provider: "anthropic".into(),
                max_input_tokens: 200_000,
                max_output_tokens: 16_384,
                supports_thinking: true,
                supports_images: true,
                supports_tools: true,
                input_cost_per_mtok: Some(3.0),
                output_cost_per_mtok: Some(15.0),
            },
            Model {
                id: "gpt-4o".into(),
                name: "GPT-4o".into(),
                provider: "openai".into(),
                max_input_tokens: 128_000,
                max_output_tokens: 16_384,
                supports_thinking: false,
                supports_images: true,
                supports_tools: true,
                input_cost_per_mtok: Some(5.0),
                output_cost_per_mtok: Some(15.0),
            },
        ]
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = ModelRegistry::new();
        reg.register_models(&test_models());
        assert_eq!(reg.len(), 2);
        assert!(reg.get("claude-sonnet-4-5-20250514").is_some());
        assert!(reg.get("gpt-4o").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_resolve_alias() {
        let mut reg = ModelRegistry::new();
        reg.register_models(&test_models());
        let model = reg.resolve("sonnet").unwrap();
        assert_eq!(model.id, "claude-sonnet-4-5-20250514");
    }

    #[test]
    fn test_resolve_substring() {
        let mut reg = ModelRegistry::new();
        reg.register_models(&test_models());
        let model = reg.resolve("4o").unwrap();
        assert_eq!(model.id, "gpt-4o");
    }

    #[test]
    fn test_list_for_provider() {
        let mut reg = ModelRegistry::new();
        reg.register_models(&test_models());
        assert_eq!(reg.list_for_provider("anthropic").len(), 1);
        assert_eq!(reg.list_for_provider("openai").len(), 1);
        assert_eq!(reg.list_for_provider("unknown").len(), 0);
    }

    #[test]
    fn test_provider_for() {
        let mut reg = ModelRegistry::new();
        reg.register_models(&test_models());
        assert_eq!(reg.provider_for("gpt-4o"), Some("openai"));
        assert_eq!(reg.provider_for("sonnet"), Some("anthropic"));
        assert_eq!(reg.provider_for("unknown"), None);
    }
}
