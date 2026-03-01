//! Model registry and provider discovery
//!
//! Wraps `clankers_router::ModelRegistry` to provide the same API while
//! bridging between clankers `Model` and router `Model` types.

use std::collections::HashMap;

use crate::provider::Model;

/// Registry of all available models across providers
#[derive(Debug, Default)]
pub struct ModelRegistry {
    inner: clankers_router::ModelRegistry,
    /// Cache of clankers Model conversions (keyed by model ID)
    cache: HashMap<String, Model>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register models from a provider
    pub fn register_models(&mut self, models: &[Model]) {
        let router_models: Vec<clankers_router::Model> = models.iter().map(|m| m.to_router_model()).collect();
        self.inner.register_models(&router_models);

        // Cache the clankers models
        for model in models {
            self.cache.insert(model.id.clone(), model.clone());
        }
    }

    /// Look up a model by ID (exact match)
    pub fn get(&self, id: &str) -> Option<&Model> {
        self.cache.get(id)
    }

    /// Look up a model by alias (partial match)
    pub fn resolve(&self, name: &str) -> Option<&Model> {
        // Try cache first (exact)
        if let Some(model) = self.cache.get(name) {
            return Some(model);
        }

        // Use router's alias + substring resolution
        if let Some(router_model) = self.inner.resolve(name) {
            return self.cache.get(&router_model.id);
        }

        None
    }

    pub fn list(&self) -> Vec<&Model> {
        let mut models: Vec<_> = self.cache.values().collect();
        models.sort_by_key(|m| &m.id);
        models
    }

    /// List models for a specific provider
    pub fn list_for_provider(&self, provider: &str) -> Vec<&Model> {
        let mut models: Vec<_> = self.cache.values().filter(|m| m.provider == provider).collect();
        models.sort_by_key(|m| &m.id);
        models
    }

    /// Get the provider name for a model ID or alias
    pub fn provider_for(&self, model_id: &str) -> Option<&str> {
        self.resolve(model_id).map(|m| m.provider.as_str())
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}
