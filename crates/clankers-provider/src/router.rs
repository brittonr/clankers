//! Multi-provider router
//!
//! Wraps multiple `Provider` backends and routes completion requests
//! to the right one based on model ID, aliases, and availability.
//!
//! When a [`RouterDb`] is attached via [`RouterProvider::with_db`],
//! the router caches responses for deterministic requests (temperature
//! = 0 or unset). Cache keys are SHA-256 hashes of the normalized
//! request (model + messages + system prompt + tools + temperature +
//! thinking). Hits replay the stored stream events without making a
//! provider call.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clanker_router::RouterDb;
use clanker_router::db::cache::CacheKeyInput;
use clanker_router::router::FallbackConfig;
use clanker_router::retry::RetryConfig;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::CompletionRequest;
use crate::Model;
use crate::Provider;
use crate::error::Result;
use crate::registry::ModelRegistry;
use crate::streaming::StreamEvent;

/// Record of a single provider attempt during fallback routing.
#[derive(Debug, Clone)]
pub struct ProviderAttempt {
    pub provider_name: String,
    pub model_id: String,
    pub status: Option<u16>,
    pub message: String,
    /// Whether this provider was skipped (e.g. cooldown) rather than called.
    pub skipped: bool,
}

// ── Adapter for clanker_router providers ───────────────────────────────────

/// Wraps a `clanker_router::Provider` to implement `clankers::provider::Provider`.
///
/// This adapter converts between the two CompletionRequest formats
/// (clankers uses AgentMessage, router uses serde_json::Value).
pub struct RouterCompatAdapter {
    inner: std::sync::Arc<dyn clanker_router::Provider>,
    models_cache: Vec<Model>,
}

impl RouterCompatAdapter {
    pub fn new(inner: std::sync::Arc<dyn clanker_router::Provider>) -> Self {
        let models_cache = inner.models().to_vec();
        Self { inner, models_cache }
    }
}

#[async_trait]
impl Provider for RouterCompatAdapter {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // Convert clankers CompletionRequest → router CompletionRequest.
        // The only real conversion is AgentMessage → serde_json::Value for messages.
        // ToolDefinition is the same type (re-exported from clanker-router).
        let router_request = clanker_router::CompletionRequest {
            model: request.model,
            messages: request.messages.iter().filter_map(|m| serde_json::to_value(m).ok()).collect(),
            system_prompt: request.system_prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: request.tools,
            thinking: request.thinking,
            no_cache: request.no_cache,
            cache_ttl: request.cache_ttl,
            extra_params: request.extra_params,
        };

        // Create a channel for router StreamEvents and convert via From impl
        let (router_tx, mut router_rx) = mpsc::channel(64);

        let tx_clone = tx.clone();
        let translate_handle = tokio::spawn(async move {
            while let Some(event) = router_rx.recv().await {
                if tx_clone.send(StreamEvent::from(event)).await.is_err() {
                    break;
                }
            }
        });

        let result = self.inner.complete(router_request, router_tx).await;

        // Wait for translation to finish
        translate_handle.await.ok();

        result.map_err(crate::error::ProviderError::from)
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

/// Multi-provider router that implements the `Provider` trait.
///
/// Routes requests to the appropriate backend based on model ID.
/// Falls back to the default provider when a model isn't found.
///
/// When a [`RouterDb`] is attached, deterministic requests (temperature
/// = 0 or unset, `no_cache` = false) are cached in redb. Identical
/// requests replay stored stream events without calling the provider.
pub struct RouterProvider {
    /// Named provider backends
    providers: HashMap<String, Arc<dyn Provider>>,
    /// Model registry (populated from all providers)
    registry: ModelRegistry,
    /// Default provider name (first registered, usually anthropic)
    default_provider: String,
    /// All models from all providers
    all_models: Vec<Model>,
    /// Persistent database for response caching (optional)
    db: Option<RouterDb>,
    /// Per-model fallback chains
    fallbacks: FallbackConfig,
    /// Exponential backoff retry configuration
    retry_config: RetryConfig,
}

impl RouterProvider {
    /// Create a new router from a list of (name, provider) pairs.
    ///
    /// The first provider in the list becomes the default.
    /// Response caching is disabled (no database attached).
    pub fn new(providers: Vec<(String, Arc<dyn Provider>)>) -> Self {
        let mut registry = ModelRegistry::new();
        let mut all_models = Vec::new();
        let mut provider_map = HashMap::new();
        let mut default_provider = String::new();

        for (name, provider) in providers {
            if default_provider.is_empty() {
                default_provider.clone_from(&name);
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
            db: None,
            fallbacks: FallbackConfig::with_defaults(),
            retry_config: RetryConfig::default(),
        }
    }

    /// Create a new router with a persistent database for response caching.
    ///
    /// Starts a background task that evicts expired cache entries every
    /// 5 minutes.
    #[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "retry loop; bounded by provider count"))]
    pub fn with_db(providers: Vec<(String, Arc<dyn Provider>)>, db: RouterDb) -> Self {
        let mut this = Self::new(providers);

        // Start background cache eviction
        let db_clone = db.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
                loop {
                    interval.tick().await;
                    match db_clone.cache().evict_expired() {
                        Ok(0) => {}
                        Ok(n) => debug!("cache eviction: removed {n} expired entries"),
                        Err(e) => warn!("cache eviction failed: {e}"),
                    }
                }
            });
        }

        this.db = Some(db);
        this
    }

    /// Resolve a model identifier to a provider.
    ///
    /// Resolution order:
    /// 1. Exact model ID → provider from registry
    /// 2. Alias resolution (e.g. "sonnet", "gpt-4o")
    /// 3. Provider prefix (e.g. "openai/gpt-4o")
    /// 4. Default provider
    #[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "invariant: default_provider always exists in providers map"))]
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

        // 4. Default provider (always exists if RouterProvider was constructed with ≥1 provider)
        let provider = self
            .providers
            .get(&self.default_provider)
            .expect("RouterProvider invariant: default_provider must exist in providers map");
        (provider.as_ref(), None)
    }

    /// Get the model registry
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    /// Number of registered providers
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Configure per-model fallback chains
    pub fn with_fallbacks(mut self, fallbacks: FallbackConfig) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// Configure exponential backoff retry
    pub fn with_retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Try a provider with cache write-back on success.
    async fn try_provider(
        &self,
        provider: &dyn Provider,
        request: &CompletionRequest,
        tx: &mpsc::Sender<StreamEvent>,
        cache_key: Option<&str>,
    ) -> Result<()> {
        if cache_key.is_some() {
            // Intercept the stream to collect events for cache write-back
            let (inner_tx, mut inner_rx) = mpsc::channel::<StreamEvent>(256);

            let provider_name = provider.name().to_string();
            let model_id = request.model.clone();
            let result = provider.complete(request.clone(), inner_tx).await;

            let mut collected = Vec::new();
            let mut input_tokens = 0u64;
            let mut output_tokens = 0u64;

            while let Some(event) = inner_rx.recv().await {
                if let StreamEvent::MessageDelta { usage, .. } = &event {
                    input_tokens += usage.input_tokens as u64;
                    output_tokens += usage.output_tokens as u64;
                }
                collected.push(event.clone());
                if tx.send(event).await.is_err() {
                    break;
                }
            }

            // Write to cache on success
            if result.is_ok()
                && !collected.is_empty()
                && let (Some(key), Some(db)) = (cache_key, &self.db)
            {
                let router_events: Vec<clanker_router::streaming::StreamEvent> =
                    collected.into_iter().map(Into::into).collect();
                let entry =
                    db.cache()
                        .build_entry(key, &provider_name, &model_id, router_events, input_tokens, output_tokens);
                match db.cache().put(&entry) {
                    Ok(()) => debug!("cached response for {model_id} (key={key:.12}…)"),
                    Err(e) => warn!("cache write failed: {e}"),
                }
            }

            result
        } else {
            // No caching — stream directly
            provider.complete(request.clone(), tx.clone()).await
        }
    }
}

#[async_trait]
impl Provider for RouterProvider {
    async fn complete(&self, mut request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let (_, resolved_id) = self.resolve(&request.model);
        let original_model = request.model.clone();
        if let Some(ref id) = resolved_id {
            request.model = id.clone();
        }

        // ── Response cache lookup ───────────────────────────────────
        let cache_key = self.compute_cache_key(&request);

        if let Some(ref key) = cache_key
            && let Some(ref db) = self.db
        {
            match db.cache().get(key) {
                Ok(Some(cached)) => {
                    debug!(
                        "cache hit for {} (key={:.12}…, hits={})",
                        request.model, key, cached.hit_count
                    );
                    db.cache().record_hit(key).ok();
                    for event in cached.events {
                        if tx.send(StreamEvent::from(event)).await.is_err() {
                            break;
                        }
                    }
                    return Ok(());
                }
                Ok(None) => {} // cache miss, proceed to provider
                Err(e) => warn!("cache read failed: {e}"),
            }
        }

        // ── Build models to try: [primary, ...fallbacks] ────────────
        let mut models_to_try = vec![request.model.clone()];
        if let Some(chain) = self.fallbacks.chain_for(&request.model) {
            for fb in chain {
                if !models_to_try.contains(fb) {
                    models_to_try.push(fb.clone());
                }
            }
        }

        let mut last_error = None;
        let mut attempts: Vec<ProviderAttempt> = Vec::new();

        for (idx, model_id) in models_to_try.iter().enumerate() {
            let is_fallback = idx > 0;
            let (provider, _) = self.resolve(model_id);
            
            // Check rate-limit health
            if let Some(ref db) = self.db
                && let Ok(false) = db.rate_limits().is_healthy(provider.name(), model_id)
            {
                attempts.push(ProviderAttempt {
                    provider_name: provider.name().to_string(),
                    model_id: model_id.clone(),
                    status: None,
                    message: "in cooldown".to_string(),
                    skipped: true,
                });
                if is_fallback {
                    debug!("fallback {}:{} in cooldown, skipping", provider.name(), model_id);
                } else {
                    info!("{}:{} in cooldown, trying fallbacks", provider.name(), model_id);
                }
                continue;
            }

            if is_fallback {
                info!("falling back to {}:{}", provider.name(), model_id);
            } else if let Some(ref resolved_id) = resolved_id {
                info!("Routing '{}' → '{}' via {}", original_model, resolved_id, provider.name());
            } else {
                info!("Routing '{}' via {}", original_model, provider.name());
            }

            let mut attempt_request = request.clone();
            attempt_request.model = model_id.clone();

            // Try with cache write-back (reuse existing pattern)
            match self.try_provider(provider, &attempt_request, &tx, cache_key.as_deref()).await {
                Ok(()) => {
                    // Record success
                    if let Some(ref db) = self.db {
                        db.rate_limits().record_success(provider.name(), model_id, 0).ok();
                    }
                    return Ok(());
                }
                Err(e) => {
                    let is_retryable = e.is_retryable();
                    let status = e.status_code();
                    warn!("{}:{} failed: {}{}", provider.name(), model_id, e, if is_retryable { " (is_retryable)" } else { "" });

                    attempts.push(ProviderAttempt {
                        provider_name: provider.name().to_string(),
                        model_id: model_id.clone(),
                        status,
                        message: e.message.clone(),
                        skipped: false,
                    });

                    if let Some(ref db) = self.db {
                        let record_status = status.unwrap_or(500);
                        db.rate_limits().record_error(provider.name(), model_id, record_status, None).ok();
                    }

                    if !is_retryable {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        // All providers exhausted — build a summary of what was tried
        Err(build_exhaustion_error(&attempts, last_error))
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

/// Build a `ProviderError` summarizing all failed provider attempts.
///
/// The summary includes per-provider details (name, model, status, reason)
/// and preserves the last HTTP status code for retryability classification.
fn build_exhaustion_error(
    attempts: &[ProviderAttempt],
    last_error: Option<crate::error::ProviderError>,
) -> crate::error::ProviderError {
    if attempts.is_empty() {
        return last_error.unwrap_or_else(|| crate::error::provider_err("All providers exhausted"));
    }

    let mut lines = Vec::with_capacity(attempts.len() + 1);
    lines.push("All providers exhausted:".to_string());
    for attempt in attempts {
        let status_str = match (attempt.skipped, attempt.status) {
            (true, _) => "skipped".to_string(),
            (false, Some(code)) => code.to_string(),
            (false, None) => "err".to_string(),
        };
        lines.push(format!(
            "  {}:{} \u{2192} {} {}",
            attempt.provider_name, attempt.model_id, status_str, attempt.message,
        ));
    }
    let summary = lines.join("\n");

    // Preserve the last attempted provider's status code
    let last_status = attempts
        .iter()
        .rev()
        .find(|a| !a.skipped)
        .and_then(|a| a.status)
        .or_else(|| last_error.as_ref().and_then(|e| e.status));

    match last_status {
        Some(status) => crate::error::provider_err_with_status(status, summary),
        None => crate::error::provider_err(summary),
    }
}

impl RouterProvider {
    /// Compute a cache key for a request, if caching is appropriate.
    ///
    /// Returns `None` when:
    /// - No database is attached
    /// - `no_cache` is set on the request
    /// - Temperature is > 0 (non-deterministic responses)
    fn compute_cache_key(&self, request: &CompletionRequest) -> Option<String> {
        self.db.as_ref()?;

        if request.no_cache {
            return None;
        }

        // Skip caching for non-deterministic requests
        if let Some(temp) = request.temperature
            && temp > 0.0
        {
            return None;
        }

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect();

        let input = CacheKeyInput {
            model: &request.model,
            system_prompt: request.system_prompt.as_deref(),
            messages: &messages,
            tools: &request.tools,
            temperature: request.temperature,
            thinking_enabled: request.thinking.as_ref().is_some_and(|t| t.enabled),
        };

        Some(input.compute_key())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::streaming::StreamEvent;

    // Minimal mock provider for testing
    struct MockProvider {
        name_str: String,
        models_list: Vec<Model>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(&self, _request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
            tx.send(StreamEvent::MessageStop).await.ok();
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
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        router.complete(request, tx).await.expect("router should complete successfully");
        let event = rx.recv().await.expect("should receive stream event");
        assert!(matches!(event, StreamEvent::MessageStop));
    }

    #[tokio::test]
    async fn test_router_reload_credentials() {
        let router = RouterProvider::new(vec![mock("anthropic", &["claude-sonnet-4-5-20250514"])]);
        // Should not panic
        router.reload_credentials().await;
    }

    // ── Cache tests ─────────────────────────────────────────────────

    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    fn test_db() -> RouterDb {
        let dir = tempfile::TempDir::new().unwrap();
        // Leak the TempDir so it lives for the duration of the test.
        // (redb holds an open fd, so the dir must outlive the db.)
        let path = dir.path().join("test_cache.db");
        std::mem::forget(dir);
        RouterDb::open(&path).unwrap()
    }

    /// Mock provider that counts how many times `complete` is called.
    struct CountingProvider {
        name_str: String,
        models_list: Vec<Model>,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Provider for CountingProvider {
        async fn complete(&self, _request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            tx
                .send(StreamEvent::MessageStart {
                    message: clanker_router::streaming::MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await.ok();
            tx
                .send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: clankers_message::message::Content::Text { text: String::new() },
                })
                .await.ok();
            tx
                .send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: clanker_router::streaming::ContentDelta::TextDelta {
                        text: "Hello!".into(),
                    },
                })
                .await.ok();
            tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx
                .send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: clanker_router::Usage {
                        input_tokens: 100,
                        output_tokens: 20,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await.ok();
            tx.send(StreamEvent::MessageStop).await.ok();
            Ok(())
        }
        fn models(&self) -> &[Model] {
            &self.models_list
        }
        fn name(&self) -> &str {
            &self.name_str
        }
    }

    fn counting_mock(name: &str, model_ids: &[&str]) -> (String, Arc<dyn Provider>, Arc<AtomicUsize>) {
        let call_count = Arc::new(AtomicUsize::new(0));
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
            Arc::new(CountingProvider {
                name_str: name.to_string(),
                models_list: models,
                call_count: call_count.clone(),
            }),
            call_count,
        )
    }

    fn make_user_msg(text: &str) -> clankers_message::message::AgentMessage {
        use clankers_message::message::*;
        AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg"),
            content: vec![Content::Text { text: text.into() }],
            timestamp: chrono::Utc::now(),
        })
    }

    fn test_request(model: &str) -> CompletionRequest {
        CompletionRequest {
            model: model.to_string(),
            messages: vec![make_user_msg("What is 2+2?")],
            system_prompt: Some("You are helpful.".into()),
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_cache_hit_skips_provider() {
        let db = test_db();
        let (name, provider, call_count) = counting_mock("test", &["test-model"]);
        let router = RouterProvider::with_db(vec![(name, provider)], db);

        let request = test_request("test-model");

        // First call: cache miss, provider called
        let (tx1, mut rx1) = mpsc::channel(64);
        router.complete(request.clone(), tx1).await.unwrap();
        let mut events1 = Vec::new();
        while let Some(e) = rx1.recv().await {
            events1.push(e);
        }
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert!(!events1.is_empty());

        // Second call: cache hit, provider NOT called
        let (tx2, mut rx2) = mpsc::channel(64);
        router.complete(request, tx2).await.unwrap();
        let mut events2 = Vec::new();
        while let Some(e) = rx2.recv().await {
            events2.push(e);
        }
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // still 1
        assert_eq!(events1.len(), events2.len());
    }

    #[tokio::test]
    async fn test_cache_skipped_with_no_cache() {
        let db = test_db();
        let (name, provider, call_count) = counting_mock("test", &["test-model"]);
        let router = RouterProvider::with_db(vec![(name, provider)], db);

        let mut request = test_request("test-model");
        request.no_cache = true;

        // Both calls hit the provider (caching disabled per-request)
        let (tx1, _rx1) = mpsc::channel(64);
        router.complete(request.clone(), tx1).await.unwrap();
        let (tx2, _rx2) = mpsc::channel(64);
        router.complete(request, tx2).await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_cache_skipped_with_nonzero_temperature() {
        let db = test_db();
        let (name, provider, call_count) = counting_mock("test", &["test-model"]);
        let router = RouterProvider::with_db(vec![(name, provider)], db);

        let mut request = test_request("test-model");
        request.temperature = Some(0.7);

        // Both calls hit the provider (non-deterministic)
        let (tx1, _rx1) = mpsc::channel(64);
        router.complete(request.clone(), tx1).await.unwrap();
        let (tx2, _rx2) = mpsc::channel(64);
        router.complete(request, tx2).await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_cache_works_with_temp_zero() {
        let db = test_db();
        let (name, provider, call_count) = counting_mock("test", &["test-model"]);
        let router = RouterProvider::with_db(vec![(name, provider)], db);

        let mut request = test_request("test-model");
        request.temperature = Some(0.0);

        let (tx1, _rx1) = mpsc::channel(64);
        router.complete(request.clone(), tx1).await.unwrap();
        let (tx2, _rx2) = mpsc::channel(64);
        router.complete(request, tx2).await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 1); // second was cached
    }

    #[tokio::test]
    async fn test_no_cache_without_db() {
        // Router without a db — caching is implicitly disabled
        let (name, provider, call_count) = counting_mock("test", &["test-model"]);
        let router = RouterProvider::new(vec![(name, provider)]);

        let request = test_request("test-model");

        let (tx1, _rx1) = mpsc::channel(64);
        router.complete(request.clone(), tx1).await.unwrap();
        let (tx2, _rx2) = mpsc::channel(64);
        router.complete(request, tx2).await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_compute_cache_key_deterministic() {
        let db = test_db();
        let router = RouterProvider::with_db(vec![mock("test", &["test-model"])], db);

        let req = test_request("test-model");
        let key1 = router.compute_cache_key(&req);
        let key2 = router.compute_cache_key(&req);

        assert!(key1.is_some());
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_compute_cache_key_none_when_no_db() {
        let router = RouterProvider::new(vec![mock("test", &["test-model"])]);
        assert!(router.compute_cache_key(&test_request("test-model")).is_none());
    }

    #[test]
    fn test_compute_cache_key_none_when_no_cache_set() {
        let db = test_db();
        let router = RouterProvider::with_db(vec![mock("test", &["test-model"])], db);

        let mut req = test_request("test-model");
        req.no_cache = true;
        assert!(router.compute_cache_key(&req).is_none());
    }

    #[test]
    fn test_compute_cache_key_none_for_high_temperature() {
        let db = test_db();
        let router = RouterProvider::with_db(vec![mock("test", &["test-model"])], db);

        let mut req = test_request("test-model");
        req.temperature = Some(0.5);
        assert!(router.compute_cache_key(&req).is_none());
    }

    #[test]
    fn test_different_messages_different_keys() {
        let db = test_db();
        let router = RouterProvider::with_db(vec![mock("test", &["test-model"])], db);

        let req1 = test_request("test-model");
        let mut req2 = test_request("test-model");
        req2.messages = vec![make_user_msg("What is 3+3?")];

        let key1 = router.compute_cache_key(&req1).unwrap();
        let key2 = router.compute_cache_key(&req2).unwrap();
        assert_ne!(key1, key2);
    }

    // ── Fallback and retry tests ────────────────────────────────────

    /// Mock provider that always fails with a configurable error
    struct FailingProvider {
        name_str: String,
        models_list: Vec<Model>,
        error_message: String,
        /// HTTP status code — if set, creates a status-bearing error
        status: Option<u16>,
    }

    #[async_trait]
    impl Provider for FailingProvider {
        async fn complete(&self, _request: CompletionRequest, _tx: mpsc::Sender<StreamEvent>) -> Result<()> {
            match self.status {
                Some(status) => Err(crate::error::provider_err_with_status(status, &self.error_message)),
                None => Err(crate::error::provider_err(&self.error_message)),
            }
        }
        fn models(&self) -> &[Model] {
            &self.models_list
        }
        fn name(&self) -> &str {
            &self.name_str
        }
    }

    fn failing_mock(name: &str, model_ids: &[&str], error_msg: &str) -> (String, Arc<dyn Provider>) {
        failing_mock_with_status(name, model_ids, error_msg, None)
    }

    fn failing_mock_with_status(name: &str, model_ids: &[&str], error_msg: &str, status: Option<u16>) -> (String, Arc<dyn Provider>) {
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
            Arc::new(FailingProvider {
                name_str: name.to_string(),
                models_list: models,
                error_message: error_msg.to_string(),
                status,
            }),
        )
    }

    #[tokio::test]
    async fn test_fallback_on_retryable_error() {
        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain("primary-model", vec!["fallback-model".to_string()]);

        let (name1, provider1) = failing_mock_with_status("primary", &["primary-model"], "rate limited", Some(429));
        let (name2, provider2, call_count) = counting_mock("fallback", &["fallback-model"]);

        let router = RouterProvider::new(vec![(name1, provider1), (name2, provider2)])
            .with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let result = router.complete(request, tx).await;
        
        // Should succeed via fallback
        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // fallback was called
    }

    #[tokio::test]
    async fn test_no_fallback_on_non_retryable_error() {
        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain("primary-model", vec!["fallback-model".to_string()]);

        let (name1, provider1) = failing_mock("primary", &["primary-model"], "Invalid API key");
        let (name2, provider2, call_count) = counting_mock("fallback", &["fallback-model"]);

        let router = RouterProvider::new(vec![(name1, provider1), (name2, provider2)])
            .with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let result = router.complete(request, tx).await;
        
        // Should fail immediately
        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 0); // fallback was NOT called
    }

    #[tokio::test]
    async fn test_rate_limit_health_skip() {
        let db = test_db();
        
        // Record a failure to put the provider in cooldown
        db.rate_limits().record_error("primary", "primary-model", 429, None).ok();

        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain("primary-model", vec!["fallback-model".to_string()]);

        let (name1, provider1, primary_count) = counting_mock("primary", &["primary-model"]);
        let (name2, provider2, fallback_count) = counting_mock("fallback", &["fallback-model"]);

        let router = RouterProvider::with_db(
            vec![(name1, provider1), (name2, provider2)], 
            db
        ).with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let result = router.complete(request, tx).await;
        
        // Should skip primary (in cooldown) and use fallback
        assert!(result.is_ok());
        assert_eq!(primary_count.load(Ordering::SeqCst), 0); // primary skipped
        assert_eq!(fallback_count.load(Ordering::SeqCst), 1); // fallback used
    }

    #[test]
    fn test_fallback_config_wired() {
        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain("model-a", vec!["model-b".to_string(), "model-c".to_string()]);

        let router = RouterProvider::new(vec![mock("test", &["model-a", "model-b", "model-c"])])
            .with_fallbacks(fallback_config);

        // Check that the chain is properly configured
        let chain = router.fallbacks.chain_for("model-a");
        assert!(chain.is_some());
        assert_eq!(chain.unwrap(), &["model-b", "model-c"]);
    }

    // ── All fallbacks exhausted ─────────────────────────────────────

    #[tokio::test]
    async fn test_all_fallbacks_exhausted() {
        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain(
            "primary-model",
            vec!["fallback-a".to_string(), "fallback-b".to_string()],
        );

        let (name1, provider1) =
            failing_mock_with_status("p1", &["primary-model"], "rate limited", Some(429));
        let (name2, provider2) =
            failing_mock_with_status("p2", &["fallback-a"], "overloaded", Some(529));
        let (name3, provider3) =
            failing_mock_with_status("p3", &["fallback-b"], "overloaded", Some(529));

        let router = RouterProvider::new(vec![(name1, provider1), (name2, provider2), (name3, provider3)])
            .with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let err = router.complete(request, tx).await.unwrap_err();

        // Should be the last error from the chain
        assert!(
            err.message.contains("529") || err.message.contains("overloaded"),
            "expected last fallback error, got: {}",
            err.message
        );
    }

    // ── All models in cooldown (primary + fallbacks) ────────────────

    #[tokio::test]
    async fn test_all_models_in_cooldown() {
        let db = test_db();

        // Put primary and all fallbacks in cooldown
        db.rate_limits().record_error("p1", "primary-model", 429, None).ok();
        db.rate_limits().record_error("p2", "fallback-model", 429, None).ok();

        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain("primary-model", vec!["fallback-model".to_string()]);

        let (name1, provider1, primary_count) = counting_mock("p1", &["primary-model"]);
        let (name2, provider2, fallback_count) = counting_mock("p2", &["fallback-model"]);

        let router = RouterProvider::with_db(vec![(name1, provider1), (name2, provider2)], db)
            .with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let err = router.complete(request, tx).await.unwrap_err();

        // Neither provider was called — both skipped due to cooldown
        assert_eq!(primary_count.load(Ordering::SeqCst), 0);
        assert_eq!(fallback_count.load(Ordering::SeqCst), 0);
        assert!(
            err.message.contains("All providers exhausted"),
            "got: {}",
            err.message
        );
    }

    // ── Empty router panics on resolve (defensive) ──────────────────

    #[test]
    fn test_empty_router_provider_count() {
        // Empty providers list: technically usable for listing models (returns 0),
        // but resolve() will panic. This is a known invariant — callers must
        // supply at least one provider.
        let router = RouterProvider::new(vec![]);
        assert_eq!(router.provider_count(), 0);
        assert!(router.models().is_empty());
    }

    // ── RouterCompatAdapter ─────────────────────────────────────────

    /// Minimal mock of `clanker_router::Provider` (the external trait)
    struct MockRouterProvider {
        name_str: String,
        models_list: Vec<Model>,
    }

    #[async_trait]
    impl clanker_router::Provider for MockRouterProvider {
        async fn complete(
            &self,
            _request: clanker_router::CompletionRequest,
            tx: mpsc::Sender<clanker_router::streaming::StreamEvent>,
        ) -> std::result::Result<(), clanker_router::Error> {
            tx
                .send(clanker_router::streaming::StreamEvent::MessageStart {
                    message: clanker_router::streaming::MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await.ok();
            tx
                .send(clanker_router::streaming::StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: clanker_router::streaming::ContentBlock::Text {
                        text: String::new(),
                    },
                })
                .await.ok();
            tx
                .send(clanker_router::streaming::StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: clanker_router::streaming::ContentDelta::TextDelta {
                        text: "Hello from router provider".into(),
                    },
                })
                .await.ok();
            tx
                .send(clanker_router::streaming::StreamEvent::ContentBlockStop { index: 0 })
                .await.ok();
            tx
                .send(clanker_router::streaming::StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: clanker_router::Usage {
                        input_tokens: 10,
                        output_tokens: 5,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await.ok();
            tx
                .send(clanker_router::streaming::StreamEvent::MessageStop)
                .await.ok();
            Ok(())
        }

        fn models(&self) -> &[Model] {
            &self.models_list
        }

        fn name(&self) -> &str {
            &self.name_str
        }
    }

    fn mock_router_provider(name: &str, model_ids: &[&str]) -> std::sync::Arc<dyn clanker_router::Provider> {
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

        std::sync::Arc::new(MockRouterProvider {
            name_str: name.to_string(),
            models_list: models,
        })
    }

    #[test]
    fn test_compat_adapter_name() {
        let inner = mock_router_provider("my-backend", &["model-a"]);
        let adapter = RouterCompatAdapter::new(inner);
        assert_eq!(adapter.name(), "my-backend");
    }

    #[test]
    fn test_compat_adapter_models() {
        let inner = mock_router_provider("backend", &["model-a", "model-b"]);
        let adapter = RouterCompatAdapter::new(inner);
        assert_eq!(adapter.models().len(), 2);
        assert_eq!(adapter.models()[0].id, "model-a");
    }

    #[tokio::test]
    async fn test_compat_adapter_complete() {
        let inner = mock_router_provider("backend", &["test-model"]);
        let adapter = RouterCompatAdapter::new(inner);

        let (tx, mut rx) = mpsc::channel(64);
        let request = CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![make_user_msg("Hello")],
            system_prompt: Some("Be helpful".into()),
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        adapter.complete(request, tx).await.unwrap();

        let mut events = Vec::new();
        while let Some(e) = rx.recv().await {
            events.push(e);
        }

        // Should have translated all events from router format to clankers format
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStart { .. })));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::ContentBlockDelta { .. })));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    struct CapturingRouterProvider {
        captured: Mutex<Option<clanker_router::CompletionRequest>>,
        models_list: Vec<Model>,
    }

    #[async_trait]
    impl clanker_router::Provider for CapturingRouterProvider {
        async fn complete(
            &self,
            request: clanker_router::CompletionRequest,
            tx: mpsc::Sender<clanker_router::streaming::StreamEvent>,
        ) -> std::result::Result<(), clanker_router::Error> {
            *self.captured.lock().expect("capture lock poisoned") = Some(request);
            tx.send(clanker_router::streaming::StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[Model] {
            &self.models_list
        }

        fn name(&self) -> &str {
            "capturing"
        }
    }

    #[tokio::test]
    async fn test_compat_adapter_preserves_session_id_extra_param() {
        let inner = Arc::new(CapturingRouterProvider {
            captured: Mutex::new(None),
            models_list: vec![Model {
                id: "test-model".to_string(),
                name: "test-model".to_string(),
                provider: "capturing".to_string(),
                max_input_tokens: 200_000,
                max_output_tokens: 16_384,
                supports_thinking: true,
                supports_images: true,
                supports_tools: true,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
            }],
        });
        let adapter = RouterCompatAdapter::new(inner.clone());

        let (tx, _rx) = mpsc::channel(4);
        let request = CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![make_user_msg("Hello")],
            system_prompt: Some("Be helpful".into()),
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::from([("_session_id".to_string(), json!("session-local-1"))]),
        };

        adapter.complete(request, tx).await.unwrap();

        let captured = inner
            .captured
            .lock()
            .expect("capture lock poisoned")
            .take()
            .expect("router request should be captured");
        assert_eq!(captured.extra_params.get("_session_id"), Some(&json!("session-local-1")));
    }

    #[tokio::test]
    async fn test_compat_adapter_reload() {
        let inner = mock_router_provider("backend", &["test-model"]);
        let adapter = RouterCompatAdapter::new(inner);
        // Should not panic
        adapter.reload_credentials().await;
    }

    // ── Failing RouterCompatAdapter ─────────────────────────────────

    struct FailingRouterProvider;

    #[async_trait]
    impl clanker_router::Provider for FailingRouterProvider {
        async fn complete(
            &self,
            _request: clanker_router::CompletionRequest,
            _tx: mpsc::Sender<clanker_router::streaming::StreamEvent>,
        ) -> std::result::Result<(), clanker_router::Error> {
            Err(clanker_router::Error::Provider {
                message: "backend exploded".into(),
                status: Some(500),
            })
        }

        fn models(&self) -> &[Model] {
            &[]
        }

        fn name(&self) -> &str {
            "failing"
        }
    }

    #[tokio::test]
    async fn test_compat_adapter_error_propagation() {
        let adapter = RouterCompatAdapter::new(std::sync::Arc::new(FailingRouterProvider));

        let (tx, _rx) = mpsc::channel(64);
        let request = CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let err = adapter.complete(request, tx).await.unwrap_err();
        assert!(err.message.contains("backend exploded"), "got: {}", err.message);
    }

    // ── Router with_retry / with_fallbacks builder ──────────────────

    #[test]
    fn test_with_retry_configurable() {
        let router = RouterProvider::new(vec![mock("test", &["model-a"])])
            .with_retry(RetryConfig::default());
        assert_eq!(router.provider_count(), 1);
    }

    // ── Resolve edge cases ──────────────────────────────────────────

    #[test]
    fn test_resolve_unknown_prefix_falls_to_default() {
        let router = RouterProvider::new(vec![
            mock("anthropic", &["claude-sonnet"]),
            mock("openai", &["gpt-4o"]),
        ]);

        // "google/gemini" — prefix "google" doesn't match any provider
        let (provider, _) = router.resolve("google/gemini");
        assert_eq!(provider.name(), "anthropic"); // falls back to default
    }

    #[test]
    fn test_resolve_prefix_exact_model() {
        let router = RouterProvider::new(vec![
            mock("anthropic", &["claude-sonnet"]),
            mock("openai", &["gpt-4o"]),
        ]);

        // "openai/gpt-4o" matches prefix → openai provider
        let (provider, _) = router.resolve("openai/gpt-4o");
        assert_eq!(provider.name(), "openai");
    }

    // ── Cache key edge cases ────────────────────────────────────────

    #[test]
    fn test_cache_key_same_model_different_system_prompts() {
        let db = test_db();
        let router = RouterProvider::with_db(vec![mock("test", &["model-a"])], db);

        let mut req1 = test_request("model-a");
        req1.system_prompt = Some("You are a pirate.".into());

        let mut req2 = test_request("model-a");
        req2.system_prompt = Some("You are a robot.".into());

        let key1 = router.compute_cache_key(&req1).unwrap();
        let key2 = router.compute_cache_key(&req2).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_key_with_tools_differs() {
        let db = test_db();
        let router = RouterProvider::with_db(vec![mock("test", &["model-a"])], db);

        let req1 = test_request("model-a");

        let mut req2 = test_request("model-a");
        req2.tools = vec![clanker_router::provider::ToolDefinition {
            name: "bash".into(),
            description: "Run bash".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }];

        let key1 = router.compute_cache_key(&req1).unwrap();
        let key2 = router.compute_cache_key(&req2).unwrap();
        assert_ne!(key1, key2);
    }

    // ── Duplicate fallback models deduplicated ──────────────────────

    #[tokio::test]
    async fn test_fallback_chain_deduplicates() {
        // If fallback chain contains the primary model, it shouldn't be tried twice
        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain(
            "model-a",
            vec!["model-a".to_string(), "model-b".to_string()],
        );

        let (name2, provider2, count_b) = counting_mock("test-b", &["model-b"]);

        // Provider that fails for model-a with is_retryable error
        let (name1_fail, provider1_fail) =
            failing_mock_with_status("test-a", &["model-a"], "fail", Some(429));

        let router = RouterProvider::new(vec![(name1_fail, provider1_fail), (name2, provider2)])
            .with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "model-a".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let result = router.complete(request, tx).await;

        assert!(result.is_ok());
        // model-b should have been called once (fallback succeeded)
        assert_eq!(count_b.load(Ordering::SeqCst), 1);
    }

    // ── Provider failure summary tests ──────────────────────────────

    #[tokio::test]
    async fn test_exhaustion_summary_two_providers() {
        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain(
            "primary-model",
            vec!["fallback-model".to_string()],
        );

        let (name1, provider1) =
            failing_mock_with_status("anthropic", &["primary-model"], "rate limited", Some(429));
        let (name2, provider2) =
            failing_mock_with_status("openai", &["fallback-model"], "overloaded", Some(529));

        let router = RouterProvider::new(vec![(name1, provider1), (name2, provider2)])
            .with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let err = router.complete(request, tx).await.unwrap_err();

        // Summary should list both providers
        assert!(err.message.contains("anthropic:primary-model"), "missing primary in summary: {}", err.message);
        assert!(err.message.contains("openai:fallback-model"), "missing fallback in summary: {}", err.message);
        assert!(err.message.contains("429"), "missing 429 status: {}", err.message);
        assert!(err.message.contains("529"), "missing 529 status: {}", err.message);
        // Last status should be from last attempted provider
        assert_eq!(err.status, Some(529));
    }

    #[tokio::test]
    async fn test_exhaustion_summary_single_provider() {
        let (name1, provider1) =
            failing_mock_with_status("anthropic", &["the-model"], "internal error", Some(500));

        let router = RouterProvider::new(vec![(name1, provider1)]);

        let request = CompletionRequest {
            model: "the-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let err = router.complete(request, tx).await.unwrap_err();

        assert!(err.message.contains("anthropic:the-model"), "missing provider in summary: {}", err.message);
        assert!(err.message.contains("500"), "missing status: {}", err.message);
        assert_eq!(err.status, Some(500));
    }

    #[tokio::test]
    async fn test_exhaustion_summary_cooldown_skipped() {
        let db = test_db();

        // Put primary in cooldown
        db.rate_limits().record_error("p1", "primary-model", 429, None).ok();

        let mut fallback_config = FallbackConfig::with_defaults();
        fallback_config.set_chain("primary-model", vec!["fallback-model".to_string()]);

        let (name1, provider1, _) = counting_mock("p1", &["primary-model"]);
        let (name2, provider2) =
            failing_mock_with_status("p2", &["fallback-model"], "overloaded", Some(529));

        let router = RouterProvider::with_db(
            vec![(name1, provider1), (name2, provider2)],
            db,
        ).with_fallbacks(fallback_config);

        let request = CompletionRequest {
            model: "primary-model".to_string(),
            messages: vec![make_user_msg("test")],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, _rx) = mpsc::channel(10);
        let err = router.complete(request, tx).await.unwrap_err();

        // Summary should include the cooldown-skipped provider
        assert!(err.message.contains("in cooldown"), "missing cooldown indicator: {}", err.message);
        assert!(err.message.contains("p1:primary-model"), "missing skipped provider: {}", err.message);
        assert!(err.message.contains("p2:fallback-model"), "missing fallback: {}", err.message);
    }
}
