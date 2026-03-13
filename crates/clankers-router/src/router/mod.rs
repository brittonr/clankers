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
use crate::model_switch::ModelSwitchReason;
use crate::model_switch::ModelSwitchTracker;
use crate::multi::MultiRequest;
use crate::multi::MultiResult;
use crate::multi::MultiStrategy;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::Usage;
use crate::registry::ModelRegistry;
use crate::streaming::StreamEvent;
use crate::streaming::TaggedStreamEvent;

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
    /// Model switch tracker (active model + switch history + usage stats)
    switch_tracker: ModelSwitchTracker,
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
        let model: String = default_model.into();
        Self {
            providers: HashMap::new(),
            registry: ModelRegistry::new(),
            switch_tracker: ModelSwitchTracker::new(model.clone()),
            default_model: model,
            db: None,
            cache_enabled: false,
            fallbacks: FallbackConfig::new(),
        }
    }

    /// Create a new router backed by a persistent database.
    pub fn with_db(default_model: impl Into<String>, db: RouterDb) -> Self {
        let model: String = default_model.into();
        Self {
            providers: HashMap::new(),
            registry: ModelRegistry::new(),
            switch_tracker: ModelSwitchTracker::new(model.clone()),
            default_model: model,
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

    // ── Multi-model dispatch ────────────────────────────────────────

    /// Send the same request to multiple models simultaneously.
    ///
    /// Each model is resolved through the registry (aliases work) and
    /// dispatched to its provider. The `MultiStrategy` controls how
    /// results are collected:
    ///
    /// - `Race` — first success wins, remaining tasks are cancelled
    /// - `All`  — fan out to all and collect every response
    /// - `Fastest(n)` — return after `n` models succeed
    ///
    /// Usage, rate limits, and request logs are recorded for each model
    /// that completes (success or failure).
    pub async fn complete_multi(&self, multi_req: MultiRequest) -> Result<MultiResult> {
        let models = &multi_req.models;
        if models.is_empty() {
            return Err(crate::Error::Config {
                message: "multi-model request has no target models".into(),
            });
        }

        info!("multi-model dispatch: {} models, strategy={}", models.len(), multi_req.strategy);

        // ── Spawn one provider task per model ───────────────────────
        let mut tasks = Vec::with_capacity(models.len());

        for model_name in models {
            // Resolve the model to a concrete ID and provider
            let resolved_id =
                self.registry.resolve(model_name).map(|m| m.id.clone()).unwrap_or_else(|| model_name.clone());

            let (_provider, provider_name) = match self.resolve_provider_for_model(&resolved_id) {
                Some(p) => p,
                None => {
                    warn!("multi-model: no provider for {model_name}, skipping");
                    continue;
                }
            };

            // Build per-model request
            let mut req = multi_req.request.clone();
            req.model.clone_from(&resolved_id);

            // Create a channel for this model's stream
            let (tx, rx) = mpsc::channel::<StreamEvent>(256);

            // Wrap provider in Arc so we can send it into the spawned task
            let provider_name_owned = provider_name.clone();
            let model_id = resolved_id.clone();

            // We need to get an Arc<dyn Provider> to move into the task
            let provider_arc = self.providers.get(&provider_name).cloned();

            let handle = tokio::spawn(async move {
                if let Some(provider) = provider_arc {
                    provider.complete(req, tx).await
                } else {
                    Err(crate::Error::NoProvider { model: model_id })
                }
            });

            tasks.push((resolved_id, provider_name_owned, rx, handle));
        }

        if tasks.is_empty() {
            return Err(crate::Error::Config {
                message: "multi-model: no models could be resolved to providers".into(),
            });
        }

        // ── Dispatch with the chosen strategy ───────────────────────
        let result = match multi_req.strategy {
            MultiStrategy::Race => crate::multi::dispatch_race(tasks).await,
            MultiStrategy::All => crate::multi::dispatch_all(tasks).await,
            MultiStrategy::Fastest(n) => crate::multi::dispatch_fastest(tasks, n).await,
        };

        // ── Record usage/logs for each completed model ──────────────
        if let Some(ref db) = self.db {
            for resp in &result.responses {
                if resp.is_ok() {
                    let model_def = self.registry.resolve(&resp.model);
                    let cost =
                        model_def.and_then(|m| m.estimate_cost(resp.usage.input_tokens, resp.usage.output_tokens));
                    let req_usage = RequestUsage::from_provider_usage(&resp.provider, &resp.model, &resp.usage, cost);
                    let _ = db.usage().record(&req_usage);

                    let total_tokens = (resp.usage.input_tokens + resp.usage.output_tokens) as u64;
                    let _ = db.rate_limits().record_success(&resp.provider, &resp.model, total_tokens);

                    let entry = LogEntry::success(
                        &resp.provider,
                        &resp.model,
                        None,
                        resp.usage.input_tokens as u64,
                        resp.usage.output_tokens as u64,
                        resp.duration_ms,
                    )
                    .with_cost(cost.unwrap_or(0.0));
                    let _ = db.request_log().append(&entry);
                } else if let Some(ref err) = resp.error {
                    let entry = LogEntry::error(&resp.provider, &resp.model, resp.duration_ms, err);
                    let _ = db.request_log().append(&entry);
                }
            }
        }

        Ok(result)
    }

    /// Stream a multi-model race to a single channel with tagged events.
    ///
    /// Unlike `complete_multi()` which collects all events, this method
    /// streams [`TaggedStreamEvent`]s as they arrive from the winning model.
    /// Only the race winner's events are forwarded; losers are cancelled.
    pub async fn complete_race_streaming(
        &self,
        request: CompletionRequest,
        models: Vec<String>,
        tx: mpsc::Sender<TaggedStreamEvent>,
    ) -> Result<()> {
        let multi_req = MultiRequest {
            request,
            models,
            strategy: MultiStrategy::Race,
        };

        let result = self.complete_multi(multi_req).await?;

        if let Some(winner) = result.winning_response() {
            for event in &winner.events {
                let tagged = TaggedStreamEvent::new(winner.model.clone(), winner.provider.clone(), event.clone());
                if tx.send(tagged).await.is_err() {
                    break;
                }
            }
            Ok(())
        } else {
            Err(crate::Error::NoProvider {
                model: "all models failed in race".into(),
            })
        }
    }

    // ── Quorum dispatch ───────────────────────────────────────────────

    /// Fan out the same prompt to a quorum of models/replicas and determine
    /// a consensus result.
    ///
    /// This builds on [`complete_multi`](Self::complete_multi) for the fan-out
    /// phase and then applies the configured [`ConsensusStrategy`] to pick
    /// a winner:
    ///
    /// - `Unanimous` / `Majority` — cluster by text similarity
    /// - `Judge` — make a second LLM call to evaluate candidates
    /// - `Collect` — return all responses, no winner
    ///
    /// For the `Judge` strategy, an additional completion request is sent
    /// to the configured judge model. The judge's token usage is included
    /// in `QuorumResult::total_usage`.
    pub async fn complete_quorum(
        &self,
        quorum_req: crate::quorum::QuorumRequest,
    ) -> Result<crate::quorum::QuorumResult> {
        use crate::quorum::*;

        if quorum_req.targets.is_empty() {
            return Err(crate::Error::Config {
                message: "quorum request has no targets".into(),
            });
        }

        let slot_count = quorum_req.targets.len();
        info!("quorum dispatch: {} slots, consensus={}", slot_count, quorum_req.consensus);

        // ── Build per-slot requests and fan out ─────────────────────
        let mut tasks: Vec<crate::multi::ProviderTask> = Vec::with_capacity(slot_count);

        for (i, slot) in quorum_req.targets.slots.iter().enumerate() {
            let resolved_id =
                self.registry.resolve(&slot.model).map(|m| m.id.clone()).unwrap_or_else(|| slot.model.clone());

            let (_provider, provider_name) = match self.resolve_provider_for_model(&resolved_id) {
                Some(p) => p,
                None => {
                    warn!("quorum slot {i}: no provider for {}, skipping", slot.model);
                    continue;
                }
            };

            let mut req = quorum_req.request.clone();
            req.model.clone_from(&resolved_id);
            if let Some(temp) = slot.temperature {
                req.temperature = Some(temp);
            }

            let (tx, rx) = mpsc::channel::<StreamEvent>(256);
            let provider_arc = self.providers.get(&provider_name).cloned();
            let model_id = resolved_id.clone();
            let label = slot.label.clone().unwrap_or_else(|| resolved_id.clone());

            let handle = tokio::spawn(async move {
                if let Some(provider) = provider_arc {
                    provider.complete(req, tx).await
                } else {
                    Err(crate::Error::NoProvider { model: model_id })
                }
            });

            tasks.push((label, provider_name.clone(), rx, handle));
        }

        if tasks.is_empty() {
            return Err(crate::Error::Config {
                message: "quorum: no targets could be resolved to providers".into(),
            });
        }

        // ── Collect all responses ───────────────────────────────────
        let multi_result = crate::multi::dispatch_all(tasks).await;
        let responses = multi_result.responses;

        // ── Record usage for fan-out responses ──────────────────────
        if let Some(ref db) = self.db {
            for resp in &responses {
                if resp.is_ok() {
                    // Resolve back to the real model ID for DB recording
                    let real_model = self.registry.resolve(&resp.model).map(|m| m.id.as_str()).unwrap_or(&resp.model);
                    let model_def = self.registry.resolve(real_model);
                    let cost =
                        model_def.and_then(|m| m.estimate_cost(resp.usage.input_tokens, resp.usage.output_tokens));
                    let req_usage = RequestUsage::from_provider_usage(&resp.provider, real_model, &resp.usage, cost);
                    let _ = db.usage().record(&req_usage);
                }
            }
        }

        // ── Apply consensus strategy ────────────────────────────────
        let successful_count = responses.iter().filter(|r| r.is_ok()).count();
        let min_agree = quorum_req.min_agree;

        let (winner_index, agreeing_count, agreement, judge_reasoning) = match &quorum_req.consensus {
            ConsensusStrategy::Unanimous { similarity_threshold } => {
                let (w, a, ag) = evaluate_unanimous(&responses, *similarity_threshold, min_agree);
                (w, a, ag, None)
            }
            ConsensusStrategy::Majority { similarity_threshold } => {
                let (w, a, ag) = evaluate_majority(&responses, *similarity_threshold, min_agree);
                (w, a, ag, None)
            }
            ConsensusStrategy::Judge { judge_model, criteria } => {
                self.run_judge_consensus(&quorum_req.request, &responses, judge_model, criteria).await
            }
            ConsensusStrategy::Collect => {
                // No consensus; pick the first successful response
                let first_ok = responses.iter().position(|r| r.is_ok()).unwrap_or(0);
                (first_ok, successful_count, 0.0, None)
            }
        };

        let quorum_met = agreeing_count >= min_agree;

        // ── Compute total usage (fan-out + optional judge) ──────────
        let mut total_usage = Usage::default();
        for r in &responses {
            total_usage.input_tokens += r.usage.input_tokens;
            total_usage.output_tokens += r.usage.output_tokens;
            total_usage.cache_creation_input_tokens += r.usage.cache_creation_input_tokens;
            total_usage.cache_read_input_tokens += r.usage.cache_read_input_tokens;
        }

        let winner = responses.get(winner_index).cloned().unwrap_or_else(|| {
            responses.first().cloned().unwrap_or_else(|| crate::multi::MultiResponse {
                model: "none".into(),
                provider: "none".into(),
                events: vec![],
                usage: Usage::default(),
                duration_ms: 0,
                error: Some("no responses".into()),
            })
        });

        info!(
            "quorum result: winner={} agreeing={}/{} agreement={:.0}% met={}",
            winner.model,
            agreeing_count,
            successful_count,
            agreement * 100.0,
            quorum_met
        );

        Ok(QuorumResult {
            winner,
            winner_index,
            all_responses: responses,
            agreeing_count,
            agreement,
            quorum_met,
            consensus: quorum_req.consensus,
            judge_reasoning,
            total_usage,
        })
    }

    /// Run the Judge consensus: send all candidate responses to a judge model
    /// and parse its verdict.
    async fn run_judge_consensus(
        &self,
        original_request: &CompletionRequest,
        responses: &[crate::multi::MultiResponse],
        judge_model: &str,
        criteria: &str,
    ) -> (usize, usize, f64, Option<String>) {
        use crate::quorum::*;

        // Build the candidates list (only successful responses)
        let ok_indices: Vec<usize> = responses.iter().enumerate().filter(|(_, r)| r.is_ok()).map(|(i, _)| i).collect();

        if ok_indices.is_empty() {
            return (0, 0, 0.0, Some("no successful responses to judge".into()));
        }

        // Collect texts first, then build candidate tuples with references
        let texts: Vec<String> = ok_indices.iter().map(|&i| responses[i].text()).collect();
        let candidates: Vec<(usize, &str, &str)> = ok_indices
            .iter()
            .enumerate()
            .map(|(display_idx, &resp_idx)| {
                (display_idx, responses[resp_idx].model.as_str(), texts[display_idx].as_str())
            })
            .collect();

        // Extract the original user prompt from the request messages
        let user_prompt = original_request
            .messages
            .last()
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("[prompt not available]");

        let judge_prompt = build_judge_prompt(user_prompt, &candidates, criteria);

        // Send the judge request
        let resolved_judge =
            self.registry.resolve(judge_model).map(|m| m.id.clone()).unwrap_or_else(|| judge_model.to_string());

        let judge_request = CompletionRequest {
            model: resolved_judge.clone(),
            messages: vec![serde_json::json!({"role": "user", "content": judge_prompt})],
            system_prompt: Some("You are a careful evaluator of LLM responses. Always respond with valid JSON.".into()),
            max_tokens: Some(1024),
            temperature: Some(0.0),
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: HashMap::new(),
        };

        let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);
        let judge_result = self.complete(judge_request, tx).await;

        // Collect judge response text
        let mut judge_text = String::new();
        while let Some(event) = rx.recv().await {
            if let StreamEvent::ContentBlockDelta {
                delta: crate::streaming::ContentDelta::TextDelta { text },
                ..
            } = event
            {
                judge_text.push_str(&text);
            }
        }

        if let Err(e) = judge_result {
            warn!("judge model failed: {e}");
            // Fall back to majority
            let (w, a, ag) = evaluate_majority(responses, 0.7, 0);
            return (w, a, ag, Some(format!("judge failed: {e}, fell back to majority")));
        }

        // Parse the verdict
        match parse_judge_response(&judge_text) {
            Some((winner_display_idx, reasoning, agreement)) => {
                // Map display index back to response index
                let winner_resp_idx = ok_indices.get(winner_display_idx).copied().unwrap_or(ok_indices[0]);
                let agreeing = ((agreement * ok_indices.len() as f64).round() as usize).max(1);
                (winner_resp_idx, agreeing, agreement, Some(reasoning))
            }
            None => {
                warn!("failed to parse judge response: {judge_text}");
                let (w, a, ag) = evaluate_majority(responses, 0.7, 0);
                (w, a, ag, Some("judge parse failed, fell back to majority".to_string()))
            }
        }
    }

    // ── Model switching ─────────────────────────────────────────────

    /// Switch the active model with tracking.
    ///
    /// Records the switch in the tracker with a reason and returns
    /// the previous model ID, or `None` if it was already active.
    pub fn switch_model(&mut self, model: impl Into<String>, reason: ModelSwitchReason) -> Option<String> {
        let model = model.into();
        // Also resolve aliases so we track the canonical ID
        let resolved = self.registry.resolve(&model).map(|m| m.id.clone()).unwrap_or(model);

        let old = self.switch_tracker.switch(resolved.clone(), reason);
        if old.is_some() {
            self.default_model = resolved;
        }
        old
    }

    /// Switch back to the previously active model.
    ///
    /// Returns `None` if there's no previous model to switch back to.
    pub fn switch_back(&mut self) -> Option<String> {
        let old = self.switch_tracker.switch_back()?;
        self.default_model = self.switch_tracker.current_model().to_string();
        Some(old)
    }

    /// Get the currently active model (from the switch tracker).
    pub fn active_model(&self) -> &str {
        self.switch_tracker.current_model()
    }

    /// Get a reference to the model switch tracker.
    pub fn switch_tracker(&self) -> &ModelSwitchTracker {
        &self.switch_tracker
    }

    /// Get a mutable reference to the model switch tracker.
    pub fn switch_tracker_mut(&mut self) -> &mut ModelSwitchTracker {
        &mut self.switch_tracker
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
mod tests;
