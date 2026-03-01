//! Model router — routes completion requests to the right provider
//!
//! The router manages multiple provider backends and routes requests based on:
//! - Model ID → provider mapping (from the registry)
//! - Model roles (default, smol, slow, plan, commit, review)
//! - Credential availability
//! - Fallback chains (when primary model is rate-limited or fails)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::db::RouterDb;
use crate::db::cache::CacheKeyInput;
use crate::db::request_log::LogEntry;
use crate::db::usage::RequestUsage;
use crate::error::Result;
use crate::model::Model;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::Usage;
use crate::registry::ModelRegistry;
use crate::streaming::StreamEvent;

// ── Fallback configuration ──────────────────────────────────────────────

/// Configuration for model fallback chains.
///
/// When a primary model is rate-limited or returns a retryable error,
/// the router tries fallback models in order until one succeeds.
#[derive(Debug, Clone, Default)]
pub struct FallbackConfig {
    /// Per-model fallback chains: model_id → [fallback_model_ids]
    chains: HashMap<String, Vec<String>>,
}

impl FallbackConfig {
    /// Create an empty fallback configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with sensible defaults for well-known models.
    pub fn with_defaults() -> Self {
        let mut chains = HashMap::new();

        // Anthropic → OpenAI → DeepSeek
        for anthropic in &[
            "claude-sonnet-4-5-20250514",
            "claude-opus-4-20250514",
            "claude-opus-4-6-20250610",
            "claude-haiku-4-5-20250514",
        ] {
            chains.insert(anthropic.to_string(), vec!["gpt-4o".to_string(), "deepseek-chat".to_string()]);
        }

        // OpenAI → Anthropic → DeepSeek
        for openai in &["gpt-4o", "gpt-4o-mini", "o3", "o3-mini"] {
            chains.insert(openai.to_string(), vec![
                "claude-sonnet-4-5-20250514".to_string(),
                "deepseek-chat".to_string(),
            ]);
        }

        // DeepSeek → Anthropic → OpenAI
        chains
            .insert("deepseek-chat".to_string(), vec!["claude-sonnet-4-5-20250514".to_string(), "gpt-4o".to_string()]);

        Self { chains }
    }

    /// Set a fallback chain for a specific model.
    pub fn set_chain(&mut self, model: impl Into<String>, fallbacks: Vec<String>) {
        self.chains.insert(model.into(), fallbacks);
    }

    /// Get the fallback chain for a model, if any.
    pub fn chain_for(&self, model: &str) -> Option<&[String]> {
        self.chains.get(model).map(|v| v.as_slice())
    }

    /// Remove the fallback chain for a model.
    pub fn remove_chain(&mut self, model: &str) -> Option<Vec<String>> {
        self.chains.remove(model)
    }

    /// List all configured model fallback chains.
    pub fn chains(&self) -> &HashMap<String, Vec<String>> {
        &self.chains
    }
}

// ── Router ──────────────────────────────────────────────────────────────

/// The main router that dispatches completion requests to providers.
pub struct Router {
    /// Registered provider backends
    providers: HashMap<String, Arc<dyn Provider>>,
    /// Model registry (populated from all providers)
    registry: ModelRegistry,
    /// Default model ID to use when none is specified
    default_model: String,
    /// Persistent database for usage, rate limits, cache, request log
    db: Option<RouterDb>,
    /// Whether response caching is enabled
    cache_enabled: bool,
    /// Fallback chain configuration
    fallbacks: FallbackConfig,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .field("default_model", &self.default_model)
            .field("cache_enabled", &self.cache_enabled)
            .field("model_count", &self.registry.len())
            .finish()
    }
}

impl Router {
    /// Create a new empty router.
    pub fn new(default_model: impl Into<String>) -> Self {
        Self {
            providers: HashMap::new(),
            registry: ModelRegistry::new(),
            default_model: default_model.into(),
            db: None,
            cache_enabled: false,
            fallbacks: FallbackConfig::new(),
        }
    }

    /// Create a new router backed by a persistent database.
    pub fn with_db(default_model: impl Into<String>, db: RouterDb) -> Self {
        Self {
            providers: HashMap::new(),
            registry: ModelRegistry::new(),
            default_model: default_model.into(),
            db: Some(db),
            cache_enabled: false,
            fallbacks: FallbackConfig::new(),
        }
    }

    /// Enable or disable response caching.
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
    }

    /// Set the fallback configuration.
    pub fn set_fallbacks(&mut self, fallbacks: FallbackConfig) {
        self.fallbacks = fallbacks;
    }

    /// Get a mutable reference to the fallback config for in-place edits.
    pub fn fallbacks_mut(&mut self) -> &mut FallbackConfig {
        &mut self.fallbacks
    }

    /// Get a reference to the fallback config.
    pub fn fallbacks(&self) -> &FallbackConfig {
        &self.fallbacks
    }

    /// Get a reference to the database, if configured.
    pub fn db(&self) -> Option<&RouterDb> {
        self.db.as_ref()
    }

    /// Register a provider backend.
    ///
    /// All models from the provider are added to the registry.
    pub fn register_provider(&mut self, provider: Arc<dyn Provider>) {
        let name = provider.name().to_string();
        self.registry.register_models(provider.models());
        self.providers.insert(name, provider);
    }

    /// Start a background task that periodically evicts expired cache entries.
    ///
    /// The task runs every 5 minutes and removes stale responses. Call this
    /// once after the router is fully configured. The returned `JoinHandle`
    /// can be dropped (the task will continue) or aborted to stop cleanup.
    pub fn start_cache_eviction(&self) -> Option<tokio::task::JoinHandle<()>> {
        let db = self.db.clone()?;
        if !self.cache_enabled {
            return None;
        }
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                match db.cache().evict_expired() {
                    Ok(0) => {}
                    Ok(n) => debug!("cache eviction: removed {n} expired entries"),
                    Err(e) => warn!("cache eviction failed: {e}"),
                }
            }
        }))
    }

    /// Route a completion request to the appropriate provider and stream the response.
    ///
    /// Resolution order:
    /// 1. Check response cache (if enabled)
    /// 2. Resolve model → provider from the registry
    /// 3. Check rate-limit health; if unhealthy, skip to fallbacks
    /// 4. Try the primary provider
    /// 5. On retryable failure, try fallback models in order
    /// 6. Record usage, request log, and rate-limit state
    pub async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let original_model = request.model.clone();

        // ── Cache lookup (before any provider call) ─────────────────
        if let Some(cache_result) = self.try_cache_read(&request, &tx).await {
            return cache_result;
        }

        // ── Build the list of models to try ─────────────────────────
        let resolved_primary =
            self.registry.resolve(&request.model).map(|m| m.id.clone()).unwrap_or_else(|| request.model.clone());

        let mut models_to_try = vec![resolved_primary.clone()];

        // Append fallback models (only those we actually have providers for)
        if let Some(fallbacks) = self.fallbacks.chain_for(&resolved_primary) {
            for fb in fallbacks {
                if self.registry.resolve(fb).is_some() && !models_to_try.contains(fb) {
                    models_to_try.push(fb.clone());
                }
            }
        }

        // If the primary model has no provider, add the default model
        // as a last-resort fallback (unless already in the list).
        if self.resolve_provider_for_model(&resolved_primary).is_none() {
            let default_resolved = self
                .registry
                .resolve(&self.default_model)
                .map(|m| m.id.clone())
                .unwrap_or_else(|| self.default_model.clone());
            if !models_to_try.contains(&default_resolved) {
                models_to_try.push(default_resolved);
            }
        }

        // ── Try each model in order ─────────────────────────────────
        let mut last_error: Option<crate::Error> = None;

        for (idx, model_id) in models_to_try.iter().enumerate() {
            let is_fallback = idx > 0;

            // Resolve to provider
            let (provider, provider_name) = match self.resolve_provider_for_model(model_id) {
                Some(p) => p,
                None => continue,
            };

            // Check rate-limit health
            if let Some(ref db) = self.db
                && let Ok(false) = db.rate_limits().is_healthy(&provider_name, model_id)
            {
                if is_fallback {
                    debug!("fallback {provider_name}:{model_id} also in cooldown, skipping");
                } else {
                    info!("{provider_name}:{model_id} in cooldown, trying fallbacks");
                }
                continue;
            }

            if is_fallback {
                info!(
                    "falling back to {provider_name}:{model_id} \
                     (primary {} failed)",
                    original_model
                );
            }

            // Build a per-attempt request with the resolved model
            let mut attempt_request = request.clone();
            attempt_request.model = model_id.clone();

            // Try the provider
            match self.try_complete(provider, &provider_name, &original_model, model_id, attempt_request, &tx).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let retryable = e.is_retryable();
                    warn!("{provider_name}:{model_id} failed: {e}{}", if retryable { " (retryable)" } else { "" });

                    // Record error for rate-limit tracking
                    if let Some(ref db) = self.db {
                        let status = e.status_code();
                        if let Some(status) = status {
                            let _ = db.rate_limits().record_error(&provider_name, model_id, status, None);
                        }
                    }

                    if !retryable {
                        // Non-retryable errors (auth, bad request, etc.)
                        // stop immediately — fallbacks won't help
                        return Err(e);
                    }

                    last_error = Some(e);
                    // Continue to next fallback
                }
            }
        }

        // All models exhausted
        Err(last_error.unwrap_or_else(|| crate::Error::NoProvider { model: original_model }))
    }

    // ── Internal: single provider attempt ───────────────────────────

    /// Try to complete a request against a single provider.
    /// Handles cache write-back, usage recording, and request logging.
    async fn try_complete(
        &self,
        provider: &dyn Provider,
        provider_name: &str,
        original_model: &str,
        resolved_model: &str,
        request: CompletionRequest,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<()> {
        let start = Instant::now();
        let model_id = resolved_model.to_string();

        // ── Cache key ───────────────────────────────────────────────
        let cache_key: Option<String> = if self.cache_enabled && self.db.is_some() {
            let input = CacheKeyInput {
                model: &model_id,
                system_prompt: request.system_prompt.as_deref(),
                messages: &request.messages,
                tools: &request.tools,
                temperature: request.temperature,
                thinking_enabled: request.thinking.as_ref().map(|t| t.enabled).unwrap_or(false),
            };
            Some(input.compute_key())
        } else {
            None
        };

        // ── Collect stream events for cache + usage tracking ────────
        let should_cache = cache_key.is_some();
        let (inner_tx, mut inner_rx) = if should_cache || self.db.is_some() {
            let (itx, irx) = mpsc::channel::<StreamEvent>(256);
            (Some(itx), Some(irx))
        } else {
            (None, None)
        };

        let provider_tx = inner_tx.unwrap_or_else(|| tx.clone());

        // Send request to provider
        let result = provider.complete(request, provider_tx).await;

        // ── Forward events + collect for recording ──────────────────
        let mut collected_events = Vec::new();
        let mut usage = Usage::default();
        let mut stop_reason: Option<String> = None;

        if let Some(ref mut rx) = inner_rx {
            while let Some(event) = rx.recv().await {
                if let StreamEvent::MessageDelta {
                    stop_reason: sr,
                    usage: u,
                } = &event
                {
                    if let Some(reason) = sr {
                        stop_reason = Some(reason.clone());
                    }
                    usage.input_tokens += u.input_tokens;
                    usage.output_tokens += u.output_tokens;
                    usage.cache_creation_input_tokens += u.cache_creation_input_tokens;
                    usage.cache_read_input_tokens += u.cache_read_input_tokens;
                }
                if should_cache {
                    collected_events.push(event.clone());
                }
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // ── Record to database ──────────────────────────────────────
        if let Some(ref db) = self.db {
            match &result {
                Ok(()) => {
                    // Usage
                    let model_def = self.registry.resolve(&model_id);
                    let cost = model_def.and_then(|m| m.estimate_cost(usage.input_tokens, usage.output_tokens));
                    let req_usage = RequestUsage::from_provider_usage(provider_name, &model_id, &usage, cost);
                    let _ = db.usage().record(&req_usage);

                    // Rate limits: record success
                    let total_tokens = (usage.input_tokens + usage.output_tokens) as u64;
                    let _ = db.rate_limits().record_success(provider_name, &model_id, total_tokens);

                    // Request log
                    let resolved = if model_id != original_model {
                        Some(model_id.as_str())
                    } else {
                        None
                    };
                    let entry = LogEntry::success(
                        provider_name,
                        original_model,
                        resolved,
                        usage.input_tokens as u64,
                        usage.output_tokens as u64,
                        duration_ms,
                    )
                    .with_cache_tokens(usage.cache_creation_input_tokens as u64, usage.cache_read_input_tokens as u64)
                    .with_cost(cost.unwrap_or(0.0))
                    .with_stop_reason(stop_reason.as_deref().unwrap_or("unknown"));
                    let _ = db.request_log().append(&entry);

                    // Cache write-back
                    if let Some(ref key) = cache_key
                        && !collected_events.is_empty()
                    {
                        let entry = db.cache().build_entry(
                            key,
                            provider_name,
                            &model_id,
                            collected_events,
                            usage.input_tokens as u64,
                            usage.output_tokens as u64,
                        );
                        match db.cache().put(&entry) {
                            Ok(()) => debug!("cached response for {model_id} (key={key:.12}…)"),
                            Err(e) => warn!("failed to cache response: {e}"),
                        }
                    }
                }
                Err(e) => {
                    // Log error
                    let entry = LogEntry::error(provider_name, original_model, duration_ms, &e.to_string());
                    let _ = db.request_log().append(&entry);
                }
            }
        }

        result
    }

    // ── Internal: cache read ────────────────────────────────────────

    /// Check if we have a cached response for this request.
    /// Returns `Some(Ok(()))` on cache hit, `None` on miss.
    async fn try_cache_read(&self, request: &CompletionRequest, tx: &mpsc::Sender<StreamEvent>) -> Option<Result<()>> {
        if !self.cache_enabled {
            return None;
        }
        let db = self.db.as_ref()?;

        let resolved_model =
            self.registry.resolve(&request.model).map(|m| m.id.clone()).unwrap_or_else(|| request.model.clone());

        let input = CacheKeyInput {
            model: &resolved_model,
            system_prompt: request.system_prompt.as_deref(),
            messages: &request.messages,
            tools: &request.tools,
            temperature: request.temperature,
            thinking_enabled: request.thinking.as_ref().map(|t| t.enabled).unwrap_or(false),
        };
        let key = input.compute_key();

        let cached = db.cache().get(&key).ok()??;
        debug!("cache hit for {resolved_model} (key={key:.12}…)");
        let _ = db.cache().record_hit(&key);

        for event in &cached.events {
            if tx.send(event.clone()).await.is_err() {
                break;
            }
        }

        // Log the hit
        let provider_name = self.registry.provider_for(&resolved_model).unwrap_or("unknown");
        let entry = LogEntry::success(
            provider_name,
            &request.model,
            if resolved_model != request.model {
                Some(resolved_model.as_str())
            } else {
                None
            },
            cached.input_tokens,
            cached.output_tokens,
            0,
        )
        .with_cache_hit(true);
        let _ = db.request_log().append(&entry);

        Some(Ok(()))
    }

    // ── Internal: resolve model → provider ──────────────────────────

    /// Resolve a model ID to a provider backend.
    /// Returns `(provider, provider_name)` or `None` if not found.
    fn resolve_provider_for_model(&self, model: &str) -> Option<(&dyn Provider, String)> {
        // 1. Registry lookup
        if let Some(registered) = self.registry.resolve(model)
            && let Some(provider) = self.providers.get(&registered.provider)
        {
            return Some((provider.as_ref(), registered.provider.clone()));
        }

        // 2. Provider prefix (e.g. "openai/gpt-4o")
        if let Some((provider_name, _)) = model.split_once('/')
            && let Some(provider) = self.providers.get(provider_name)
        {
            return Some((provider.as_ref(), provider_name.to_string()));
        }

        None
    }

    /// Resolve a model identifier to a provider backend (public API).
    ///
    /// Returns the provider and optionally the resolved model ID
    /// (if an alias was used). Falls back to the default model.
    pub fn resolve_provider(&self, model: &str) -> Result<(&dyn Provider, Option<String>)> {
        // 1. Try to find the model in the registry
        if let Some(registered) = self.registry.resolve(model)
            && let Some(provider) = self.providers.get(&registered.provider)
        {
            let resolved = if registered.id != model {
                Some(registered.id.clone())
            } else {
                None
            };
            return Ok((provider.as_ref(), resolved));
        }

        // 2. Try matching provider name prefix (e.g. "openai/gpt-4o")
        if let Some((provider_name, _model_id)) = model.split_once('/')
            && let Some(provider) = self.providers.get(provider_name)
        {
            return Ok((provider.as_ref(), None));
        }

        // 3. Fall back to default model's provider
        if let Some(default) = self.registry.resolve(&self.default_model)
            && let Some(provider) = self.providers.get(&default.provider)
        {
            return Ok((provider.as_ref(), Some(self.default_model.clone())));
        }

        // 4. Fall back to first available provider
        if let Some(provider) = self.providers.values().next() {
            return Ok((provider.as_ref(), None));
        }

        Err(crate::Error::NoProvider {
            model: model.to_string(),
        })
    }

    /// Get the model registry.
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    /// Get a mutable reference to the registry.
    pub fn registry_mut(&mut self) -> &mut ModelRegistry {
        &mut self.registry
    }

    /// Get a provider by name.
    pub fn provider(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// List all registered provider names.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }

    /// Get the default model ID.
    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    /// Whether caching is enabled.
    pub fn cache_enabled(&self) -> bool {
        self.cache_enabled
    }

    /// Set the default model ID.
    pub fn set_default_model(&mut self, model: impl Into<String>) {
        self.default_model = model.into();
    }

    /// Reload credentials for all providers.
    pub async fn reload_credentials(&self) {
        for provider in self.providers.values() {
            provider.reload_credentials().await;
        }
    }

    /// List all available models.
    pub fn list_models(&self) -> Vec<&Model> {
        self.registry.list()
    }

    /// Resolve a model name/alias to a full model definition.
    pub fn resolve_model(&self, name: &str) -> Option<&Model> {
        self.registry.resolve(name)
    }
}

#[cfg(test)]
mod tests {
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
        router.register_provider(make_mock_provider("anthropic", &[
            "claude-sonnet-4-5-20250514",
            "claude-opus-4-20250514",
        ]));
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
        };

        router.complete(request, tx).await.unwrap();
        while rx.recv().await.is_some() {}

        // The failed provider should have a rate limit error recorded
        let db = router.db().unwrap();
        assert!(!db.rate_limits().is_healthy("provider-a", "model-a").unwrap());

        // The successful fallback should be healthy
        assert!(db.rate_limits().is_healthy("provider-b", "model-b").unwrap());
    }
}
