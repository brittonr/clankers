use async_trait::async_trait;

use super::*;
use crate::streaming::StreamEvent;

struct MockProvider {
    name: String,
    models: Vec<Model>,
}

#[async_trait]
impl Provider for MockProvider {
    async fn complete(&self, _request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let _ = tx.send(StreamEvent::MessageStop).await;
        Ok(())
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn make_mock_provider(name: &str, model_ids: &[&str]) -> Arc<dyn Provider> {
    Arc::new(MockProvider {
        name: name.to_string(),
        models: model_ids
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
            .collect(),
    })
}

#[test]
fn test_register_provider() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router
        .register_provider(make_mock_provider("anthropic", &["claude-sonnet-4-5-20250514", "claude-opus-4-20250514"]));
    assert_eq!(router.provider_names(), vec!["anthropic"]);
    assert_eq!(router.list_models().len(), 2);
}

#[test]
fn test_resolve_provider_exact() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(make_mock_provider("anthropic", &["claude-sonnet-4-5-20250514"]));
    router.register_provider(make_mock_provider("openai", &["gpt-4o"]));

    let (provider, resolved) = router.resolve_provider("gpt-4o").unwrap();
    assert_eq!(provider.name(), "openai");
    assert!(resolved.is_none());
}

#[test]
fn test_resolve_provider_alias() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(make_mock_provider("anthropic", &["claude-sonnet-4-5-20250514"]));

    let (provider, resolved) = router.resolve_provider("sonnet").unwrap();
    assert_eq!(provider.name(), "anthropic");
    assert_eq!(resolved.as_deref(), Some("claude-sonnet-4-5-20250514"));
}

#[test]
fn test_resolve_provider_fallback() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(make_mock_provider("anthropic", &["claude-sonnet-4-5-20250514"]));

    // Unknown model falls back to default model's provider
    let (provider, _) = router.resolve_provider("nonexistent-model").unwrap();
    assert_eq!(provider.name(), "anthropic");
}

#[tokio::test]
async fn test_complete_routes_correctly() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(make_mock_provider("anthropic", &["claude-sonnet-4-5-20250514"]));

    let (tx, mut rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "sonnet".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    router.complete(request, tx).await.unwrap();
    let event = rx.recv().await.unwrap();
    assert!(matches!(event, StreamEvent::MessageStop));
}

#[test]
fn test_no_provider_error() {
    let router = Router::new("nonexistent");
    let result = router.resolve_provider("anything");
    assert!(result.is_err());
}

#[test]
fn test_multi_provider() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(make_mock_provider("anthropic", &["claude-sonnet-4-5-20250514"]));
    router.register_provider(make_mock_provider("openai", &["gpt-4o"]));

    assert_eq!(router.provider_names().len(), 2);
    assert_eq!(router.list_models().len(), 2);

    assert_eq!(router.resolve_model("sonnet").unwrap().provider, "anthropic");
    assert_eq!(router.resolve_model("gpt-4o").unwrap().provider, "openai");
}

// ── Fallback config tests ───────────────────────────────────────

#[test]
fn test_fallback_config_empty() {
    let config = FallbackConfig::new();
    assert!(config.chain_for("anything").is_none());
    assert!(config.chains().is_empty());
}

#[test]
fn test_fallback_config_set_chain() {
    let mut config = FallbackConfig::new();
    config.set_chain("model-a", vec!["model-b".into(), "model-c".into()]);

    let chain = config.chain_for("model-a").unwrap();
    assert_eq!(chain, &["model-b", "model-c"]);
    assert!(config.chain_for("model-b").is_none());
}

#[test]
fn test_fallback_config_remove_chain() {
    let mut config = FallbackConfig::new();
    config.set_chain("model-a", vec!["model-b".into()]);
    let removed = config.remove_chain("model-a");
    assert!(removed.is_some());
    assert!(config.chain_for("model-a").is_none());
}

#[test]
fn test_fallback_config_defaults() {
    let config = FallbackConfig::with_defaults();
    let chain = config.chain_for("claude-sonnet-4-5-20250514").unwrap();
    assert!(chain.contains(&"gpt-4o".to_string()));

    let chain = config.chain_for("gpt-4o").unwrap();
    assert!(chain.contains(&"claude-sonnet-4-5-20250514".to_string()));
}

// ── Fallback routing tests ──────────────────────────────────────

/// A provider that always fails with a retryable error.
struct FailingProvider {
    name: String,
    models: Vec<Model>,
}

#[async_trait]
impl Provider for FailingProvider {
    async fn complete(&self, _request: CompletionRequest, _tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        Err(crate::Error::provider_with_status(429, "HTTP 429: rate limited"))
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn make_failing_provider(name: &str, model_ids: &[&str]) -> Arc<dyn Provider> {
    Arc::new(FailingProvider {
        name: name.to_string(),
        models: model_ids
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
            .collect(),
    })
}

/// A provider that fails with a non-retryable error.
struct AuthFailProvider {
    name: String,
    models: Vec<Model>,
}

#[async_trait]
impl Provider for AuthFailProvider {
    async fn complete(&self, _request: CompletionRequest, _tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        Err(crate::Error::Auth {
            message: "invalid API key".to_string(),
        })
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn make_auth_fail_provider(name: &str, model_ids: &[&str]) -> Arc<dyn Provider> {
    Arc::new(AuthFailProvider {
        name: name.to_string(),
        models: model_ids
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
            .collect(),
    })
}

#[tokio::test]
async fn test_fallback_on_retryable_error() {
    let mut router = Router::new("model-a");
    router.register_provider(make_failing_provider("provider-a", &["model-a"]));
    router.register_provider(make_mock_provider("provider-b", &["model-b"]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("model-a", vec!["model-b".into()]);
    router.set_fallbacks(fallbacks);

    let (tx, mut rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    // Should succeed via fallback to model-b
    router.complete(request, tx).await.unwrap();
    let event = rx.recv().await.unwrap();
    assert!(matches!(event, StreamEvent::MessageStop));
}

#[tokio::test]
async fn test_no_fallback_on_auth_error() {
    let mut router = Router::new("model-a");
    router.register_provider(make_auth_fail_provider("provider-a", &["model-a"]));
    router.register_provider(make_mock_provider("provider-b", &["model-b"]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("model-a", vec!["model-b".into()]);
    router.set_fallbacks(fallbacks);

    let (tx, _rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    // Should fail immediately — auth errors are not retryable
    let err = router.complete(request, tx).await.unwrap_err();
    assert!(matches!(err, crate::Error::Auth { .. }));
}

#[tokio::test]
async fn test_fallback_skips_unhealthy_providers() {
    let db = RouterDb::in_memory().unwrap();
    // Put provider-b:model-b in cooldown
    db.rate_limits().record_error("provider-b", "model-b", 429, Some(300)).unwrap();

    let mut router = Router::with_db("model-a", db);
    router.register_provider(make_failing_provider("provider-a", &["model-a"]));
    router.register_provider(make_mock_provider("provider-b", &["model-b"]));
    router.register_provider(make_mock_provider("provider-c", &["model-c"]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("model-a", vec!["model-b".into(), "model-c".into()]);
    router.set_fallbacks(fallbacks);

    let (tx, mut rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    // model-a fails (retryable), model-b in cooldown, model-c succeeds
    router.complete(request, tx).await.unwrap();
    let event = rx.recv().await.unwrap();
    assert!(matches!(event, StreamEvent::MessageStop));
}

#[tokio::test]
async fn test_primary_in_cooldown_skips_to_fallback() {
    let db = RouterDb::in_memory().unwrap();
    // Put the primary in cooldown
    db.rate_limits().record_error("provider-a", "model-a", 429, Some(300)).unwrap();

    let mut router = Router::with_db("model-a", db);
    // Primary provider is fine, but rate-limited
    router.register_provider(make_mock_provider("provider-a", &["model-a"]));
    router.register_provider(make_mock_provider("provider-b", &["model-b"]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("model-a", vec!["model-b".into()]);
    router.set_fallbacks(fallbacks);

    let (tx, mut rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    // model-a skipped (cooldown), model-b succeeds
    router.complete(request, tx).await.unwrap();
    let event = rx.recv().await.unwrap();
    assert!(matches!(event, StreamEvent::MessageStop));
}

#[tokio::test]
async fn test_all_fallbacks_exhausted() {
    let mut router = Router::new("model-a");
    router.register_provider(make_failing_provider("provider-a", &["model-a"]));
    router.register_provider(make_failing_provider("provider-b", &["model-b"]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("model-a", vec!["model-b".into()]);
    router.set_fallbacks(fallbacks);

    let (tx, _rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    // Both fail → returns last error
    let err = router.complete(request, tx).await.unwrap_err();
    assert!(matches!(err, crate::Error::Provider { .. }));
}

#[tokio::test]
async fn test_no_fallback_configured_returns_error() {
    let mut router = Router::new("model-a");
    router.register_provider(make_failing_provider("provider-a", &["model-a"]));

    // No fallback chain configured
    let (tx, _rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    let err = router.complete(request, tx).await.unwrap_err();
    assert!(matches!(err, crate::Error::Provider { .. }));
}

// ── RichMockProvider for DB integration tests ────────────────────

struct RichMockProvider {
    name: String,
    models: Vec<Model>,
}

#[async_trait]
impl Provider for RichMockProvider {
    async fn complete(&self, _request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        use crate::streaming::ContentBlock;
        use crate::streaming::ContentDelta;
        use crate::streaming::MessageMetadata;
        let _ = tx
            .send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "msg-test-1".into(),
                    model: "test-model".into(),
                    role: "assistant".into(),
                },
            })
            .await;
        let _ = tx
            .send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::Text { text: String::new() },
            })
            .await;
        let _ = tx
            .send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta { text: "Hello!".into() },
            })
            .await;
        let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
        let _ = tx
            .send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: crate::provider::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
            .await;
        let _ = tx.send(StreamEvent::MessageStop).await;
        Ok(())
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn make_rich_mock(name: &str, model_id: &str) -> Arc<dyn Provider> {
    Arc::new(RichMockProvider {
        name: name.to_string(),
        models: vec![Model {
            id: model_id.to_string(),
            name: model_id.to_string(),
            provider: name.to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        }],
    })
}

fn make_db_router(model: &str) -> Router {
    let db = RouterDb::in_memory().unwrap();
    let mut router = Router::with_db(model, db);
    router.set_cache_enabled(true);
    router.register_provider(make_rich_mock("anthropic", model));
    router
}

#[tokio::test]
async fn test_complete_records_usage() {
    let router = make_db_router("test-model");
    let (tx, mut rx) = mpsc::channel(64);

    let request = CompletionRequest {
        model: "test-model".into(),
        messages: vec![serde_json::json!({"role": "user", "content": "hi"})],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };
    router.complete(request, tx).await.unwrap();
    while rx.recv().await.is_some() {}

    let db = router.db().unwrap();
    let today = db.usage().today().unwrap().unwrap();
    assert_eq!(today.requests, 1);
    assert_eq!(today.input_tokens, 10);
    assert_eq!(today.output_tokens, 5);
}

#[tokio::test]
async fn test_complete_records_request_log() {
    let router = make_db_router("test-model");
    let (tx, mut rx) = mpsc::channel(64);

    let request = CompletionRequest {
        model: "test-model".into(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };
    router.complete(request, tx).await.unwrap();
    while rx.recv().await.is_some() {}

    let db = router.db().unwrap();
    let entries = db.request_log().recent(10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].provider, "anthropic");
    assert_eq!(entries[0].model, "test-model");
    assert!(!entries[0].cache_hit);
}

#[tokio::test]
async fn test_cache_write_back_and_hit() {
    let router = make_db_router("test-model");

    // First request: cache miss → provider called → response cached
    let (tx1, mut rx1) = mpsc::channel(64);
    let request = CompletionRequest {
        model: "test-model".into(),
        messages: vec![serde_json::json!({"role": "user", "content": "cached?"})],
        system_prompt: None,
        max_tokens: None,
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };
    router.complete(request.clone(), tx1).await.unwrap();
    let mut first_events = Vec::new();
    while let Some(ev) = rx1.recv().await {
        first_events.push(ev);
    }
    assert!(!first_events.is_empty());

    // Verify cache was populated
    let db = router.db().unwrap();
    assert_eq!(db.cache().len().unwrap(), 1);

    // Second request: identical → should be a cache hit
    let (tx2, mut rx2) = mpsc::channel(64);
    router.complete(request, tx2).await.unwrap();
    let mut second_events = Vec::new();
    while let Some(ev) = rx2.recv().await {
        second_events.push(ev);
    }

    // Same events replayed from cache
    assert_eq!(first_events.len(), second_events.len());

    // Request log should show 2 entries: miss then hit
    let entries = db.request_log().recent(10).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].cache_hit);
    assert!(!entries[1].cache_hit);
}

#[tokio::test]
async fn test_fallback_records_rate_limit_error() {
    let db = RouterDb::in_memory().unwrap();
    let mut router = Router::with_db("model-a", db);
    router.register_provider(make_failing_provider("provider-a", &["model-a"]));
    router.register_provider(make_mock_provider("provider-b", &["model-b"]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("model-a", vec!["model-b".into()]);
    router.set_fallbacks(fallbacks);

    let (tx, mut rx) = mpsc::channel(10);
    let request = CompletionRequest {
        model: "model-a".to_string(),
        messages: vec![],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    router.complete(request, tx).await.unwrap();
    while rx.recv().await.is_some() {}

    // The failed provider should have a rate limit error recorded
    let db = router.db().unwrap();
    assert!(!db.rate_limits().is_healthy("provider-a", "model-a").unwrap());

    // The successful fallback should be healthy
    assert!(db.rate_limits().is_healthy("provider-b", "model-b").unwrap());
}
