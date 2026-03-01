//! Multi-provider router
//!
//! Wraps multiple `Provider` backends and routes completion requests
//! to the right one based on model ID, aliases, and availability.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::provider::Model;
use crate::provider::Provider;
use crate::provider::registry::ModelRegistry;
use crate::provider::streaming::StreamEvent;

// ── Adapter for clankers_router providers ───────────────────────────────────

/// Wraps a `clankers_router::Provider` to implement `clankers::provider::Provider`.
///
/// This adapter converts between the two CompletionRequest formats
/// (clankers uses AgentMessage, router uses serde_json::Value).
pub struct RouterCompatAdapter {
    inner: std::sync::Arc<dyn clankers_router::Provider>,
    models_cache: Vec<Model>,
}

impl RouterCompatAdapter {
    pub fn new(inner: std::sync::Arc<dyn clankers_router::Provider>) -> Self {
        let models_cache = inner.models().iter().map(Model::from_router_model).collect();
        Self { inner, models_cache }
    }
}

#[async_trait]
impl Provider for RouterCompatAdapter {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // Convert clankers CompletionRequest → router CompletionRequest
        let router_request = clankers_router::CompletionRequest {
            model: request.model,
            messages: request.messages.iter().filter_map(|m| serde_json::to_value(m).ok()).collect(),
            system_prompt: request.system_prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: request
                .tools
                .into_iter()
                .map(|t| clankers_router::provider::ToolDefinition {
                    name: t.name,
                    description: t.description,
                    input_schema: t.input_schema,
                })
                .collect(),
            thinking: request.thinking.map(|t| clankers_router::ThinkingConfig {
                enabled: t.enabled,
                budget_tokens: t.budget_tokens,
            }),
        };

        // Create a channel for router StreamEvents and translate them
        let (router_tx, mut router_rx) = mpsc::channel(64);

        let tx_clone = tx.clone();
        let translate_handle = tokio::spawn(async move {
            while let Some(event) = router_rx.recv().await {
                let clankers_event = translate_stream_event(event);
                if tx_clone.send(clankers_event).await.is_err() {
                    break;
                }
            }
        });

        let result = self.inner.complete(router_request, router_tx).await;

        // Wait for translation to finish
        let _ = translate_handle.await;

        result.map_err(|e| crate::error::Error::Provider { message: e.to_string() })
    }

    fn models(&self) -> &[Model] {
        &self.models_cache
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    async fn reload_credentials(&self) {
        self.inner.reload_credentials().await;
    }
}

/// Translate a clankers_router StreamEvent → clankers StreamEvent
fn translate_stream_event(event: clankers_router::streaming::StreamEvent) -> StreamEvent {
    use clankers_router::streaming as router_stream;

    use crate::provider::message::Content;
    use crate::provider::streaming as clankers_stream;

    match event {
        router_stream::StreamEvent::MessageStart { message } => StreamEvent::MessageStart {
            message: clankers_stream::MessageMetadata {
                id: message.id,
                model: message.model,
                role: message.role,
            },
        },
        router_stream::StreamEvent::ContentBlockStart { index, content_block } => {
            let block = match content_block {
                router_stream::ContentBlock::Text { text } => Content::Text { text },
                router_stream::ContentBlock::Thinking { thinking } => Content::Thinking { thinking },
                router_stream::ContentBlock::ToolUse { id, name, input } => Content::ToolUse { id, name, input },
            };
            StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            }
        }
        router_stream::StreamEvent::ContentBlockDelta { index, delta } => {
            let d = match delta {
                router_stream::ContentDelta::TextDelta { text } => clankers_stream::ContentDelta::TextDelta { text },
                router_stream::ContentDelta::ThinkingDelta { thinking } => {
                    clankers_stream::ContentDelta::ThinkingDelta { thinking }
                }
                router_stream::ContentDelta::InputJsonDelta { partial_json } => {
                    clankers_stream::ContentDelta::InputJsonDelta { partial_json }
                }
            };
            StreamEvent::ContentBlockDelta { index, delta: d }
        }
        router_stream::StreamEvent::ContentBlockStop { index } => StreamEvent::ContentBlockStop { index },
        router_stream::StreamEvent::MessageDelta { stop_reason, usage } => StreamEvent::MessageDelta {
            stop_reason,
            usage: crate::provider::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_creation_input_tokens: usage.cache_creation_input_tokens,
                cache_read_input_tokens: usage.cache_read_input_tokens,
            },
        },
        router_stream::StreamEvent::MessageStop => StreamEvent::MessageStop,
        router_stream::StreamEvent::Error { error } => StreamEvent::Error { error },
    }
}

/// Multi-provider router that implements the `Provider` trait.
///
/// Routes requests to the appropriate backend based on model ID.
/// Falls back to the default provider when a model isn't found.
pub struct RouterProvider {
    /// Named provider backends
    providers: HashMap<String, Arc<dyn Provider>>,
    /// Model registry (populated from all providers)
    registry: ModelRegistry,
    /// Default provider name (first registered, usually anthropic)
    default_provider: String,
    /// All models from all providers
    all_models: Vec<Model>,
}

impl RouterProvider {
    /// Create a new router from a list of (name, provider) pairs.
    ///
    /// The first provider in the list becomes the default.
    pub fn new(providers: Vec<(String, Arc<dyn Provider>)>) -> Self {
        let mut registry = ModelRegistry::new();
        let mut all_models = Vec::new();
        let mut provider_map = HashMap::new();
        let mut default_provider = String::new();

        for (name, provider) in providers {
            if default_provider.is_empty() {
                default_provider = name.clone();
            }

            let models = provider.models();
            registry.register_models(models);
            all_models.extend_from_slice(models);

            info!("Registered provider '{}' with {} models", name, models.len());
            provider_map.insert(name, provider);
        }

        Self {
            providers: provider_map,
            registry,
            default_provider,
            all_models,
        }
    }

    /// Resolve a model identifier to a provider.
    ///
    /// Resolution order:
    /// 1. Exact model ID → provider from registry
    /// 2. Alias resolution (e.g. "sonnet", "gpt-4o")
    /// 3. Provider prefix (e.g. "openai/gpt-4o")
    /// 4. Default provider
    fn resolve(&self, model: &str) -> (&dyn Provider, Option<String>) {
        // 1-2. Registry lookup (handles exact + alias + substring)
        if let Some(registered) = self.registry.resolve(model)
            && let Some(provider) = self.providers.get(&registered.provider)
        {
            let resolved_id = if registered.id != model {
                Some(registered.id.clone())
            } else {
                None
            };
            return (provider.as_ref(), resolved_id);
        }

        // 3. Provider prefix: "openai/gpt-4o" → provider="openai", model="gpt-4o"
        if let Some((provider_name, _)) = model.split_once('/')
            && let Some(provider) = self.providers.get(provider_name)
        {
            return (provider.as_ref(), None);
        }

        // 4. Default provider
        if let Some(provider) = self.providers.get(&self.default_provider) {
            return (provider.as_ref(), None);
        }

        // Should never happen if we have at least one provider
        unreachable!("RouterProvider has no providers")
    }

    /// Get the model registry
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    /// List all registered provider names
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }

    /// Get a specific provider by name
    pub fn get_provider(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// Number of registered providers
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

#[async_trait]
impl Provider for RouterProvider {
    async fn complete(&self, mut request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let (provider, resolved_id) = self.resolve(&request.model);

        if let Some(id) = resolved_id {
            info!("Routing '{}' → '{}' via {}", request.model, id, provider.name());
            request.model = id;
        } else {
            info!("Routing '{}' via {}", request.model, provider.name());
        }

        provider.complete(request, tx).await
    }

    fn models(&self) -> &[Model] {
        &self.all_models
    }

    fn name(&self) -> &str {
        "router"
    }

    async fn reload_credentials(&self) {
        for provider in self.providers.values() {
            provider.reload_credentials().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::streaming::StreamEvent;

    // Minimal mock provider for testing
    struct MockProvider {
        name_str: String,
        models_list: Vec<Model>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(&self, _request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
            let _ = tx.send(StreamEvent::MessageStop).await;
            Ok(())
        }
        fn models(&self) -> &[Model] {
            &self.models_list
        }
        fn name(&self) -> &str {
            &self.name_str
        }
    }

    fn mock(name: &str, model_ids: &[&str]) -> (String, Arc<dyn Provider>) {
        let models: Vec<Model> = model_ids
            .iter()
            .map(|id| Model {
                id: id.to_string(),
                name: id.to_string(),
                provider: name.to_string(),
                max_input_tokens: 200_000,
                max_output_tokens: 16_384,
                supports_thinking: true,
                supports_images: true,
                supports_tools: true,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
            })
            .collect();

        (
            name.to_string(),
            Arc::new(MockProvider {
                name_str: name.to_string(),
                models_list: models,
            }),
        )
    }

    #[test]
    fn test_router_resolve_exact() {
        let router = RouterProvider::new(vec![
            mock("anthropic", &["claude-sonnet-4-5-20250514"]),
            mock("openai", &["gpt-4o"]),
        ]);

        let (provider, _) = router.resolve("gpt-4o");
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_router_resolve_alias() {
        let router = RouterProvider::new(vec![mock("anthropic", &["claude-sonnet-4-5-20250514"])]);

        let (provider, resolved) = router.resolve("sonnet");
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(resolved.as_deref(), Some("claude-sonnet-4-5-20250514"));
    }

    #[test]
    fn test_router_resolve_prefix() {
        let router = RouterProvider::new(vec![
            mock("anthropic", &["claude-sonnet-4-5-20250514"]),
            mock("openai", &["gpt-4o"]),
        ]);

        let (provider, _) = router.resolve("openai/gpt-4o-custom");
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_router_resolve_fallback() {
        let router = RouterProvider::new(vec![mock("anthropic", &["claude-sonnet-4-5-20250514"])]);

        let (provider, _) = router.resolve("unknown-model");
        assert_eq!(provider.name(), "anthropic"); // falls back to default
    }

    #[test]
    fn test_router_all_models() {
        let router = RouterProvider::new(vec![
            mock("anthropic", &["claude-sonnet-4-5-20250514"]),
            mock("openai", &["gpt-4o", "gpt-4o-mini"]),
        ]);

        assert_eq!(router.models().len(), 3);
        assert_eq!(router.provider_count(), 2);
    }

    #[tokio::test]
    async fn test_router_complete_routes() {
        let router = RouterProvider::new(vec![
            mock("anthropic", &["claude-sonnet-4-5-20250514"]),
            mock("openai", &["gpt-4o"]),
        ]);

        let (tx, mut rx) = mpsc::channel(10);
        let request = CompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
        };

        router.complete(request, tx).await.unwrap();
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, StreamEvent::MessageStop));
    }

    #[tokio::test]
    async fn test_router_reload_credentials() {
        let router = RouterProvider::new(vec![mock("anthropic", &["claude-sonnet-4-5-20250514"])]);
        // Should not panic
        router.reload_credentials().await;
    }
}
