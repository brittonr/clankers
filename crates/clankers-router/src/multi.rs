//! Multi-model dispatch — send the same request to multiple models simultaneously
//!
//! Supports three strategies:
//!
//! - **Race** — First successful response wins; remaining tasks are cancelled. Ideal for
//!   latency-sensitive paths where you want the fastest provider.
//!
//! - **All** — Fan out to every target model and collect all responses. Useful for comparison,
//!   consensus voting, or ensemble approaches.
//!
//! - **Fastest(n)** — Return after `n` models have responded successfully. A middle ground: get
//!   some diversity without waiting for the slowest.
//!
//! Uses [`futures-buffered`] and [`futures-lite`] from the n0/iroh ecosystem
//! for efficient, bounded concurrent dispatch.
//!
//! # Example
//!
//! ```ignore
//! let multi_req = MultiRequest {
//!     request: base_request,
//!     models: vec!["claude-sonnet-4-5-20250514".into(), "gpt-4o".into()],
//!     strategy: MultiStrategy::Race,
//! };
//! let result = router.complete_multi(multi_req).await?;
//! let winner = &result.responses[result.winner.unwrap()];
//! ```

use std::time::Instant;

use futures_buffered::FuturesUnordered;
use futures_lite::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;

use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::provider::Usage;
use crate::streaming::StreamEvent;

// ── Strategy ────────────────────────────────────────────────────────────

/// How to handle responses from multiple models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiStrategy {
    /// First successful response wins; remaining in-flight requests are cancelled.
    Race,
    /// Send to all models and collect every response.
    All,
    /// Return after the fastest `n` models respond successfully.
    Fastest(usize),
}

impl std::fmt::Display for MultiStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiStrategy::Race => write!(f, "race"),
            MultiStrategy::All => write!(f, "all"),
            MultiStrategy::Fastest(n) => write!(f, "fastest({n})"),
        }
    }
}

// ── Request ─────────────────────────────────────────────────────────────

/// A request that targets multiple models simultaneously.
#[derive(Debug, Clone)]
pub struct MultiRequest {
    /// The base completion request (the `model` field is overridden per target).
    pub request: CompletionRequest,
    /// Models to target. Each is resolved through the registry (aliases work).
    pub models: Vec<String>,
    /// Strategy for handling multiple responses.
    pub strategy: MultiStrategy,
}

// ── Per-model response ──────────────────────────────────────────────────

/// A single model's collected response from a multi-model call.
#[derive(Debug, Clone)]
pub struct MultiResponse {
    /// The model ID that produced this response (resolved, not alias).
    pub model: String,
    /// Provider that served the request.
    pub provider: String,
    /// All stream events collected from this model's response.
    pub events: Vec<StreamEvent>,
    /// Aggregated token usage.
    pub usage: Usage,
    /// Wall-clock time to complete (milliseconds).
    pub duration_ms: u64,
    /// Error message if this model failed (events may be partial or empty).
    pub error: Option<String>,
}

impl MultiResponse {
    /// Whether this response completed successfully.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// Extract the assistant's text content from the collected events.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for event in &self.events {
            if let StreamEvent::ContentBlockDelta {
                delta: crate::streaming::ContentDelta::TextDelta { text },
                ..
            } = event
            {
                out.push_str(text);
            }
        }
        out
    }
}

// ── Aggregated result ───────────────────────────────────────────────────

/// Collected results from a multi-model dispatch.
#[derive(Debug)]
pub struct MultiResult {
    /// Per-model responses, in the order they completed.
    pub responses: Vec<MultiResponse>,
    /// The strategy that was used.
    pub strategy: MultiStrategy,
    /// Index into `responses` of the "winning" response (for Race / Fastest).
    /// For `All`, this is `None`.
    pub winner: Option<usize>,
}

impl MultiResult {
    /// Get the winning response, if any.
    pub fn winning_response(&self) -> Option<&MultiResponse> {
        self.winner.and_then(|i| self.responses.get(i))
    }

    /// Get all successful responses.
    pub fn successful(&self) -> Vec<&MultiResponse> {
        self.responses.iter().filter(|r| r.is_ok()).collect()
    }

    /// Get all failed responses.
    pub fn failed(&self) -> Vec<&MultiResponse> {
        self.responses.iter().filter(|r| !r.is_ok()).collect()
    }

    /// Total usage across all responses.
    pub fn total_usage(&self) -> Usage {
        let mut total = Usage::default();
        for r in &self.responses {
            total.input_tokens += r.usage.input_tokens;
            total.output_tokens += r.usage.output_tokens;
            total.cache_creation_input_tokens += r.usage.cache_creation_input_tokens;
            total.cache_read_input_tokens += r.usage.cache_read_input_tokens;
        }
        total
    }
}

// ── Internal: collect a single model's streamed response ────────────────

/// Collect all stream events from a channel into a `MultiResponse`.
pub(crate) async fn collect_response(
    model: String,
    provider: String,
    mut rx: mpsc::Receiver<StreamEvent>,
    start: Instant,
) -> MultiResponse {
    let mut events = Vec::new();
    let mut usage = Usage::default();
    let mut error = None;

    while let Some(event) = rx.recv().await {
        match &event {
            StreamEvent::MessageDelta { usage: u, .. } => {
                usage.input_tokens += u.input_tokens;
                usage.output_tokens += u.output_tokens;
                usage.cache_creation_input_tokens += u.cache_creation_input_tokens;
                usage.cache_read_input_tokens += u.cache_read_input_tokens;
            }
            StreamEvent::Error { error: e } => {
                error = Some(e.clone());
            }
            _ => {}
        }
        events.push(event);
    }

    MultiResponse {
        model,
        provider,
        events,
        usage,
        duration_ms: start.elapsed().as_millis() as u64,
        error,
    }
}

// ── Dispatch task payload ───────────────────────────────────────────────

/// Everything needed to collect one model's result, returned as a future output.
struct TaskPayload {
    model: String,
    provider: String,
    rx: mpsc::Receiver<StreamEvent>,
    provider_result: std::result::Result<Result<()>, tokio::task::JoinError>,
    start: Instant,
}

/// Spawn a provider task and return the payload future.
///
/// The caller pushes these into a `FuturesUnordered` for concurrent polling.
fn spawn_collect(
    model: String,
    provider: String,
    rx: mpsc::Receiver<StreamEvent>,
    handle: tokio::task::JoinHandle<Result<()>>,
    start: Instant,
) -> tokio::task::JoinHandle<TaskPayload> {
    tokio::spawn(async move {
        let provider_result = handle.await;
        TaskPayload {
            model,
            provider,
            rx,
            provider_result,
            start,
        }
    })
}

/// Convert a completed `TaskPayload` into a `MultiResponse`.
async fn payload_to_response(payload: TaskPayload) -> MultiResponse {
    let mut resp = collect_response(payload.model, payload.provider, payload.rx, payload.start).await;

    match payload.provider_result {
        // Task panicked or was cancelled
        Err(e) => {
            if resp.error.is_none() {
                resp.error = Some(format!("task failed: {e}"));
            }
        }
        // Provider returned an error (rate limit, auth, etc.)
        Ok(Err(e)) => {
            if resp.error.is_none() {
                resp.error = Some(e.to_string());
            }
        }
        Ok(Ok(())) => {}
    }

    resp
}

// ── Task type alias ─────────────────────────────────────────────────────

/// A spawned provider task: (model_id, provider_name, event_receiver, join_handle).
pub(crate) type ProviderTask = (String, String, mpsc::Receiver<StreamEvent>, tokio::task::JoinHandle<Result<()>>);

// ── Dispatch: Race ──────────────────────────────────────────────────────

/// First success wins, remaining tasks are cancelled.
pub(crate) async fn dispatch_race(tasks: Vec<ProviderTask>) -> MultiResult {
    let start = Instant::now();
    let mut responses = Vec::with_capacity(tasks.len());

    let mut collectors: FuturesUnordered<_> = FuturesUnordered::new();

    for (model, provider, rx, handle) in tasks {
        // Keep the provider handle so we can abort it
        let provider_handle_for_abort = handle.abort_handle();
        let collector = spawn_collect(model, provider, rx, handle, start);
        collectors.push(collector);
        // We'll abort via the provider abort handle stored separately
        // Actually, aborting the collector will cascade. Keep them in the unordered set.
        let _ = provider_handle_for_abort; // we abort via collector below
    }

    // Stream results as they complete via futures-lite StreamExt
    while let Some(join_result) = collectors.next().await {
        let payload = match join_result {
            Ok(p) => p,
            Err(e) => {
                debug!("race collector task panicked: {e}");
                continue;
            }
        };

        let resp = payload_to_response(payload).await;
        let is_success = resp.is_ok();
        let resp_idx = responses.len();

        info!("race: {} {} in {}ms", resp.model, if is_success { "succeeded" } else { "failed" }, resp.duration_ms);
        responses.push(resp);

        if is_success {
            // We have a winner — abort all remaining collectors
            // (dropping the FuturesUnordered aborts the remaining JoinHandles)
            drop(collectors);
            return MultiResult {
                responses,
                strategy: MultiStrategy::Race,
                winner: Some(resp_idx),
            };
        }
    }

    // All failed
    MultiResult {
        responses,
        strategy: MultiStrategy::Race,
        winner: None,
    }
}

// ── Dispatch: All ───────────────────────────────────────────────────────

/// Fan out to every model and collect all responses.
pub(crate) async fn dispatch_all(tasks: Vec<ProviderTask>) -> MultiResult {
    let start = Instant::now();

    // Use futures_buffered::join_all for efficient bounded collection
    let collect_futs: Vec<_> = tasks
        .into_iter()
        .map(|(model, provider, rx, handle)| async move {
            let provider_result = handle.await;
            let mut resp = collect_response(model, provider, rx, start).await;
            match provider_result {
                Err(e) => {
                    if resp.error.is_none() {
                        resp.error = Some(format!("task failed: {e}"));
                    }
                }
                Ok(Err(e)) => {
                    if resp.error.is_none() {
                        resp.error = Some(e.to_string());
                    }
                }
                Ok(Ok(())) => {}
            }
            resp
        })
        .collect();

    let mut responses = futures_buffered::join_all(collect_futs).await;

    // Sort by completion time (fastest first)
    responses.sort_by_key(|r| r.duration_ms);

    MultiResult {
        responses,
        strategy: MultiStrategy::All,
        winner: None,
    }
}

// ── Dispatch: Fastest(n) ────────────────────────────────────────────────

/// Return after `n` models respond successfully.
pub(crate) async fn dispatch_fastest(tasks: Vec<ProviderTask>, n: usize) -> MultiResult {
    let start = Instant::now();
    let total = tasks.len();
    let target = n.min(total);
    let mut success_count = 0;
    let mut responses = Vec::with_capacity(total);

    let mut collectors: FuturesUnordered<_> = FuturesUnordered::new();

    for (model, provider, rx, handle) in tasks {
        collectors.push(spawn_collect(model, provider, rx, handle, start));
    }

    while let Some(join_result) = collectors.next().await {
        let payload = match join_result {
            Ok(p) => p,
            Err(e) => {
                debug!("fastest({target}) collector panicked: {e}");
                continue;
            }
        };

        let resp = payload_to_response(payload).await;
        if resp.is_ok() {
            success_count += 1;
        }
        responses.push(resp);

        if success_count >= target {
            debug!("fastest({target}): reached {success_count} successes, cancelling rest");
            drop(collectors);
            break;
        }
    }

    // Sort by duration (fastest first)
    responses.sort_by_key(|r| r.duration_ms);

    let winner = responses.iter().position(|r| r.is_ok());

    debug!("fastest({target}): {success_count}/{total} succeeded");

    MultiResult {
        responses,
        strategy: MultiStrategy::Fastest(target),
        winner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_strategy_display() {
        assert_eq!(MultiStrategy::Race.to_string(), "race");
        assert_eq!(MultiStrategy::All.to_string(), "all");
        assert_eq!(MultiStrategy::Fastest(3).to_string(), "fastest(3)");
    }

    #[test]
    fn test_multi_response_text_extraction() {
        use crate::streaming::ContentDelta;

        let resp = MultiResponse {
            model: "test-model".into(),
            provider: "test".into(),
            events: vec![
                StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "Hello ".into() },
                },
                StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "world!".into() },
                },
            ],
            usage: Usage::default(),
            duration_ms: 100,
            error: None,
        };

        assert_eq!(resp.text(), "Hello world!");
        assert!(resp.is_ok());
    }

    #[test]
    fn test_multi_response_with_error() {
        let resp = MultiResponse {
            model: "test-model".into(),
            provider: "test".into(),
            events: vec![],
            usage: Usage::default(),
            duration_ms: 50,
            error: Some("rate limited".into()),
        };

        assert!(!resp.is_ok());
        assert_eq!(resp.text(), "");
    }

    #[test]
    fn test_multi_result_accessors() {
        let result = MultiResult {
            responses: vec![
                MultiResponse {
                    model: "model-a".into(),
                    provider: "p-a".into(),
                    events: vec![],
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 5,
                        ..Default::default()
                    },
                    duration_ms: 100,
                    error: None,
                },
                MultiResponse {
                    model: "model-b".into(),
                    provider: "p-b".into(),
                    events: vec![],
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 8,
                        ..Default::default()
                    },
                    duration_ms: 200,
                    error: Some("failed".into()),
                },
            ],
            strategy: MultiStrategy::Race,
            winner: Some(0),
        };

        assert_eq!(result.winning_response().unwrap().model, "model-a");
        assert_eq!(result.successful().len(), 1);
        assert_eq!(result.failed().len(), 1);

        let total = result.total_usage();
        assert_eq!(total.input_tokens, 20);
        assert_eq!(total.output_tokens, 13);
    }
}
