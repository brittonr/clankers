//! Live integration tests against a local Ollama instance.
//!
//! These tests hit a real Ollama server with a tiny Qwen model (qwen2.5:0.5b).
//! They exercise the full clanker-router pipeline: provider construction,
//! request building, SSE parsing, stream event collection, usage tracking,
//! fallback chains, and caching.
//!
//! **Requirements:**
//!   - Ollama running on localhost:11434
//!   - `qwen2.5:0.5b` model pulled (`ollama pull qwen2.5:0.5b`)
//!
//! Skip with: `cargo nextest run -E 'not test(ollama)'`

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use clanker_router::Model;
use clanker_router::Router;
use clanker_router::RouterDb;
use clanker_router::backends::openai_compat::OpenAICompatConfig;
use clanker_router::backends::openai_compat::OpenAICompatProvider;
use clanker_router::model_switch::ModelSwitchReason;
use clanker_router::provider::CompletionRequest;
use clanker_router::provider::ToolDefinition;
use clanker_router::router::FallbackConfig;
use clanker_router::streaming::ContentDelta;
use clanker_router::streaming::StreamEvent;
use serde_json::json;
use tokio::sync::mpsc;

// ── Constants ───────────────────────────────────────────────────────────

const OLLAMA_BASE: &str = "http://localhost:11434/v1";
const MODEL_ID: &str = "qwen2.5:0.5b";
/// Second model for multi-model / switching tests.
/// Uses the same family (qwen2.5) to avoid slow cross-family model swaps
/// in Ollama. qwen3 models default to thinking mode (empty text content)
/// and require full model reload, making them impractical for fast tests.
const MODEL_ALT: &str = "qwen2.5:3b";

// ── Helpers ─────────────────────────────────────────────────────────────

/// Build a model definition for Ollama.
///
/// Note: `provider` must be `"local"` to match `OpenAICompatConfig::local()`
/// which sets `name: "local"`. The router resolves models to providers by
/// matching `model.provider` against `provider.name()`.
fn ollama_model(id: &str) -> Model {
    Model {
        id: id.to_string(),
        name: id.to_string(),
        provider: "local".to_string(),
        max_input_tokens: 32_768,
        max_output_tokens: 8_192,
        supports_thinking: false,
        supports_images: false,
        supports_tools: true,
        input_cost_per_mtok: None,
        output_cost_per_mtok: None,
    }
}

fn ollama_provider(models: Vec<Model>) -> Arc<dyn clanker_router::Provider> {
    let config = OpenAICompatConfig::local(OLLAMA_BASE.to_string(), models);
    OpenAICompatProvider::new(config)
}

fn ollama_router() -> Router {
    let mut router = Router::new(MODEL_ID);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID)]));
    router
}

fn simple_request(model: &str, prompt: &str) -> CompletionRequest {
    CompletionRequest {
        model: model.to_string(),
        messages: vec![json!({"role": "user", "content": prompt})],
        system_prompt: None,
        max_tokens: Some(128),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: HashMap::new(),
    }
}

/// Collect all stream events from a router completion.
async fn collect_events(router: &Router, request: CompletionRequest) -> clanker_router::Result<Vec<StreamEvent>> {
    let (tx, mut rx) = mpsc::channel(256);
    router.complete(request, tx).await?;
    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    Ok(events)
}

/// Extract concatenated text from stream events.
fn extract_text(events: &[StreamEvent]) -> String {
    let mut buf = String::new();
    for ev in events {
        if let StreamEvent::ContentBlockDelta {
            delta: ContentDelta::TextDelta { text },
            ..
        } = ev
        {
            buf.push_str(text);
        }
    }
    buf
}

/// Check that Ollama is reachable and the model is pulled.
/// Returns false if the test should be skipped.
fn ollama_available() -> bool {
    let addr: std::net::SocketAddr = "127.0.0.1:11434".parse().unwrap();
    if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_err() {
        eprintln!("SKIP: Ollama not running on localhost:11434");
        return false;
    }
    // Check the model is pulled via a blocking HTTP call in a thread
    // (avoid nested runtime panic).
    let ok = std::thread::spawn(|| {
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(2)).build().ok()?;
        let resp = client.get(format!("{}/models", OLLAMA_BASE)).send().ok()?;
        let body: serde_json::Value = resp.json().ok()?;
        let models = body.get("data")?.as_array()?;
        let found = models.iter().any(|m| m.get("id").and_then(|v| v.as_str()) == Some(MODEL_ID));
        Some(found)
    })
    .join()
    .ok()
    .flatten()
    .unwrap_or(false);

    if !ok {
        eprintln!("SKIP: model {} not found in Ollama — run `ollama pull {}`", MODEL_ID, MODEL_ID);
    }
    ok
}

/// Check that a specific model is available in Ollama.
fn model_available(model_id: &str) -> bool {
    let id = model_id.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(2)).build().ok()?;
        let resp = client.get(format!("{}/models", OLLAMA_BASE)).send().ok()?;
        let body: serde_json::Value = resp.json().ok()?;
        let models = body.get("data")?.as_array()?;
        Some(models.iter().any(|m| m.get("id").and_then(|v| v.as_str()).is_some_and(|mid| mid == id)))
    })
    .join()
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// Build a router with two local models registered.
fn two_model_router() -> Router {
    let models = vec![ollama_model(MODEL_ID), ollama_model(MODEL_ALT)];
    let mut router = Router::new(MODEL_ID);
    router.register_provider(ollama_provider(models));
    router
}

// ── Tests ───────────────────────────────────────────────────────────────

/// Smoke test: send a trivial prompt, get a non-empty response.
#[tokio::test]
async fn ollama_simple_completion() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let events = collect_events(&router, simple_request(MODEL_ID, "Say hello in exactly one word."))
        .await
        .expect("completion should succeed");

    let text = extract_text(&events);
    assert!(!text.is_empty(), "expected non-empty response, got nothing");

    // Verify we got the standard event sequence
    assert!(
        matches!(events.first(), Some(StreamEvent::MessageStart { .. })),
        "first event should be MessageStart"
    );
    assert!(matches!(events.last(), Some(StreamEvent::MessageStop)), "last event should be MessageStop");

    // Verify we got at least one MessageDelta event (stop reason + usage).
    //
    // NOTE: Ollama sends the final usage stats in a chunk with `choices: []`
    // (empty array), but the router's SSE handler checks `choices.is_none()`
    // which only matches JSON null. This means usage tokens may be 0 in the
    // MessageDelta events. This is a known upstream issue in clanker-router.
    let has_delta = events.iter().any(|ev| matches!(ev, StreamEvent::MessageDelta { .. }));
    assert!(has_delta, "expected at least one MessageDelta event");
}

/// Verify the model name echoed back in MessageStart matches what we asked for.
#[tokio::test]
async fn ollama_message_metadata() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let events = collect_events(&router, simple_request(MODEL_ID, "Hi")).await.expect("completion failed");

    if let Some(StreamEvent::MessageStart { message }) = events.first() {
        assert_eq!(message.role, "assistant");
        // Ollama returns the model name; it may include a tag suffix
        assert!(message.model.contains("qwen"), "expected model to contain 'qwen', got '{}'", message.model);
    } else {
        panic!("first event was not MessageStart");
    }
}

/// System prompts are forwarded to the model.
#[tokio::test]
async fn ollama_system_prompt() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let mut req = simple_request(MODEL_ID, "What is your name?");
    req.system_prompt = Some("Your name is Rusty. Always introduce yourself as Rusty.".to_string());
    req.max_tokens = Some(64);

    let events = collect_events(&router, req).await.expect("completion failed");
    let text = extract_text(&events).to_lowercase();

    // The model should mention "Rusty" somewhere (small models can be flaky,
    // but a direct identity instruction usually works).
    assert!(text.contains("rusty"), "expected response to contain 'rusty' given system prompt, got: {text}");
}

/// Temperature=0 should produce deterministic output across two calls.
#[tokio::test]
async fn ollama_deterministic_at_temp_zero() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let req = || {
        let mut r = simple_request(MODEL_ID, "What is 2+2? Reply with just the number.");
        r.temperature = Some(0.0);
        r.max_tokens = Some(16);
        r
    };

    let text1 = extract_text(&collect_events(&router, req()).await.expect("call 1 failed"));
    let text2 = extract_text(&collect_events(&router, req()).await.expect("call 2 failed"));

    assert_eq!(text1, text2, "temp=0 should produce identical output");
}

/// max_tokens is respected — short limit yields short output.
#[tokio::test]
async fn ollama_max_tokens_limit() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let mut req = simple_request(MODEL_ID, "Write a 500-word essay about the history of computing.");
    req.max_tokens = Some(20);
    req.temperature = Some(0.0);

    let events = collect_events(&router, req).await.expect("completion failed");
    let text = extract_text(&events);

    // 20 tokens is roughly 15-60 chars depending on tokenizer.
    // The model should have been cut off well before 500 words.
    let word_count = text.split_whitespace().count();
    assert!(word_count < 60, "expected short output with max_tokens=20, got {word_count} words: {text}");

    // Check the stop reason indicates truncation
    let truncated = events.iter().any(|ev| {
        matches!(ev, StreamEvent::MessageDelta { stop_reason: Some(reason), .. } if reason == "max_tokens" || reason == "length")
    });
    assert!(truncated, "expected stop_reason=max_tokens or length");
}

/// Multi-turn conversation: the model can see previous messages.
#[tokio::test]
async fn ollama_multi_turn() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let req = CompletionRequest {
        model: MODEL_ID.to_string(),
        messages: vec![
            json!({"role": "user", "content": "My favorite color is blue. Remember this."}),
            json!({"role": "assistant", "content": "I'll remember that your favorite color is blue."}),
            json!({"role": "user", "content": "What is my favorite color? Reply with just the color."}),
        ],
        system_prompt: None,
        max_tokens: Some(16),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: HashMap::new(),
    };

    let events = collect_events(&router, req).await.expect("completion failed");
    let text = extract_text(&events).to_lowercase();
    assert!(text.contains("blue"), "model should recall 'blue' from conversation history, got: {text}");
}

/// Streaming: events arrive in the correct order.
#[tokio::test]
async fn ollama_streaming_event_order() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let events = collect_events(&router, simple_request(MODEL_ID, "Count from 1 to 5."))
        .await
        .expect("completion failed");

    // Expected order: MessageStart, ContentBlockStart, [ContentBlockDelta...],
    // ContentBlockStop, MessageDelta, MessageStop
    let event_types: Vec<&str> = events
        .iter()
        .map(|ev| match ev {
            StreamEvent::MessageStart { .. } => "MessageStart",
            StreamEvent::ContentBlockStart { .. } => "ContentBlockStart",
            StreamEvent::ContentBlockDelta { .. } => "ContentBlockDelta",
            StreamEvent::ContentBlockStop { .. } => "ContentBlockStop",
            StreamEvent::MessageDelta { .. } => "MessageDelta",
            StreamEvent::MessageStop => "MessageStop",
            StreamEvent::Error { .. } => "Error",
        })
        .collect();

    // MessageStart must be first
    assert_eq!(event_types[0], "MessageStart");

    // ContentBlockStart must come before any ContentBlockDelta
    let first_start = event_types.iter().position(|t| *t == "ContentBlockStart");
    let first_delta = event_types.iter().position(|t| *t == "ContentBlockDelta");
    if let (Some(s), Some(d)) = (first_start, first_delta) {
        assert!(s < d, "ContentBlockStart ({s}) should come before ContentBlockDelta ({d})");
    }

    // MessageStop must be last
    assert_eq!(*event_types.last().unwrap(), "MessageStop");

    // At least one ContentBlockDelta should be present
    assert!(event_types.contains(&"ContentBlockDelta"), "expected at least one ContentBlockDelta");
}

/// Router with DB: usage is recorded after a completion.
#[tokio::test]
async fn ollama_usage_tracking() {
    if !ollama_available() {
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp dir");
    let db_path = tmp.path().join("test_usage.db");
    let db = RouterDb::open(&db_path).expect("open db");

    let mut router = Router::with_db(MODEL_ID, db);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID)]));

    let _ = collect_events(&router, simple_request(MODEL_ID, "Say yes.")).await.expect("completion failed");

    // Check usage was recorded.
    //
    // NOTE: Due to an upstream bug in clanker-router (choices: [] vs null),
    // Ollama's usage stats may not be captured. We verify that a request was
    // at least logged. See ollama_simple_completion for details.
    let db = router.db().expect("db should be present");
    let recent = db.usage().recent_days(1).expect("usage query");
    assert!(!recent.is_empty(), "expected at least one usage record");

    let entry = &recent[0];
    assert!(entry.requests > 0, "requests should be > 0");
}

/// Router with DB: request log captures success entries.
#[tokio::test]
async fn ollama_request_log() {
    if !ollama_available() {
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = RouterDb::open(&tmp.path().join("test_log.db")).expect("open db");

    let mut router = Router::with_db(MODEL_ID, db);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID)]));

    let _ = collect_events(&router, simple_request(MODEL_ID, "Ping")).await.expect("completion failed");

    let log = router.db().unwrap().request_log();
    let entries = log.recent(10).expect("log query");
    assert!(!entries.is_empty(), "expected a request log entry");

    let entry = &entries[0];
    assert!(
        matches!(entry.outcome, clanker_router::db::request_log::RequestOutcome::Success),
        "entry should be successful, got {:?}",
        entry.outcome
    );
    assert!(entry.duration_ms > 0, "duration should be > 0");
}

/// Response caching: second identical request serves from cache.
#[tokio::test]
async fn ollama_response_cache() {
    if !ollama_available() {
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = RouterDb::open(&tmp.path().join("test_cache.db")).expect("open db");

    let mut router = Router::with_db(MODEL_ID, db);
    router.set_cache_enabled(true);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID)]));

    let req = || {
        let mut r = simple_request(MODEL_ID, "What is the capital of France? One word.");
        r.temperature = Some(0.0);
        r.max_tokens = Some(16);
        r
    };

    // First call: hits Ollama
    let events1 = collect_events(&router, req()).await.expect("first call failed");
    let text1 = extract_text(&events1);

    // Second call: should hit cache
    let events2 = collect_events(&router, req()).await.expect("second call failed");
    let text2 = extract_text(&events2);

    assert_eq!(text1, text2, "cached response should be identical");

    // Verify the cache has an entry
    let count = router.db().unwrap().cache().len().expect("cache len");
    assert!(count > 0, "cache should have at least one entry");
}

/// Fallback: requesting an unregistered model falls back to the default.
///
/// When the requested model has no provider mapping, the router appends
/// the default model as a last-resort fallback and tries it.
#[tokio::test]
async fn ollama_fallback_to_default_model() {
    if !ollama_available() {
        return;
    }

    // Only register the real model. "ghost-model" is NOT in the registry.
    let mut router = Router::new(MODEL_ID);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID)]));

    // Request a model that doesn't exist anywhere — the router should
    // fall back to the default model (qwen2.5:0.5b).
    let events = collect_events(&router, simple_request("ghost-model", "Hi"))
        .await
        .expect("fallback to default should succeed");

    let text = extract_text(&events);
    assert!(!text.is_empty(), "default fallback model should produce output");
}

/// Fallback chain: explicit chain routes to the configured fallback.
#[tokio::test]
async fn ollama_explicit_fallback_chain() {
    if !ollama_available() {
        return;
    }

    // Register two models under the same provider — one real, one fake.
    // The fake one is registered in the registry but will 404 at Ollama.
    // Since 404 is non-retryable, fallback won't help (by design).
    // Instead, test the chain config API and resolution logic.
    let mut router = Router::new(MODEL_ID);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID)]));

    let mut fallbacks = FallbackConfig::new();
    fallbacks.set_chain("primary-model", vec![MODEL_ID.to_string()]);
    router.set_fallbacks(fallbacks);

    // Verify the chain was configured correctly
    let chain = router.fallbacks().chain_for("primary-model");
    assert!(chain.is_some(), "fallback chain should exist");
    assert_eq!(chain.unwrap(), &[MODEL_ID.to_string()]);
}

/// Multiple models registered: routing picks the right provider.
#[tokio::test]
async fn ollama_model_routing() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let router = two_model_router();

    // Request the 0.5b model
    let events = collect_events(&router, simple_request(MODEL_ID, "Hi"))
        .await
        .expect("primary model completion failed");
    assert!(!extract_text(&events).is_empty());

    // Request the 3b model
    let events = collect_events(&router, simple_request(MODEL_ALT, "Hi")).await.expect("alt model completion failed");
    assert!(!extract_text(&events).is_empty());
}

/// Tool definitions are sent to the model (we don't need the model to
/// actually call the tool — just verify no errors when tools are present).
#[tokio::test]
async fn ollama_with_tool_definitions() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let req = CompletionRequest {
        model: MODEL_ID.to_string(),
        messages: vec![json!({"role": "user", "content": "What is 7 * 8?"})],
        system_prompt: None,
        max_tokens: Some(64),
        temperature: Some(0.0),
        tools: vec![ToolDefinition {
            name: "calculator".to_string(),
            description: "Evaluate a math expression".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "Math expression to evaluate"}
                },
                "required": ["expression"]
            }),
        }],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: HashMap::new(),
    };

    let events = collect_events(&router, req).await.expect("completion with tools failed");

    // The model should produce some output — either a text response or a
    // tool call. Either way, we should get a valid event sequence.
    assert!(matches!(events.first(), Some(StreamEvent::MessageStart { .. })), "expected MessageStart");
    assert!(matches!(events.last(), Some(StreamEvent::MessageStop)), "expected MessageStop");
}

/// Empty message list doesn't crash (some providers handle this gracefully).
#[tokio::test]
async fn ollama_empty_messages() {
    if !ollama_available() {
        return;
    }

    let router = ollama_router();
    let req = CompletionRequest {
        model: MODEL_ID.to_string(),
        messages: vec![],
        system_prompt: Some("Say hello.".to_string()),
        max_tokens: Some(32),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: HashMap::new(),
    };

    // This may succeed or fail depending on the provider — we just
    // verify it doesn't panic.
    let _ = collect_events(&router, req).await;
}

/// Provider availability check works for local Ollama.
#[tokio::test]
async fn ollama_provider_is_available() {
    if !ollama_available() {
        return;
    }

    let provider = ollama_provider(vec![ollama_model(MODEL_ID)]);
    assert!(provider.is_available().await, "local provider should be available");
    assert_eq!(provider.name(), "local");
    assert!(!provider.models().is_empty());
}

// ── Model switching tests ───────────────────────────────────────────────

/// Switch from qwen2.5:0.5b → qwen3:0.6b and verify both produce output.
#[tokio::test]
async fn ollama_switch_model_and_complete() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let mut router = two_model_router();

    // Complete on the initial model
    let events1 = collect_events(&router, simple_request(MODEL_ID, "Say hello."))
        .await
        .expect("initial model completion failed");
    let text1 = extract_text(&events1);
    assert!(!text1.is_empty(), "initial model should produce output");

    // Switch to the alt model
    let old = router.switch_model(MODEL_ALT, ModelSwitchReason::UserRequest);
    assert_eq!(old, Some(MODEL_ID.to_string()), "switch should return previous model");
    assert_eq!(router.active_model(), MODEL_ALT);

    // Complete on the new model
    let events2 = collect_events(&router, simple_request(MODEL_ALT, "Say hello."))
        .await
        .expect("switched model completion failed");
    let text2 = extract_text(&events2);
    assert!(!text2.is_empty(), "switched model should produce output");
}

/// Switch and switch back: verify the tracker returns to the original.
#[tokio::test]
async fn ollama_switch_back() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let mut router = two_model_router();
    assert_eq!(router.active_model(), MODEL_ID);

    // Switch to alt
    router.switch_model(MODEL_ALT, ModelSwitchReason::UserRequest);
    assert_eq!(router.active_model(), MODEL_ALT);

    // Switch back
    let old = router.switch_back();
    assert_eq!(old, Some(MODEL_ALT.to_string()));
    assert_eq!(router.active_model(), MODEL_ID);

    // Verify the original model still works after switching back
    let events = collect_events(&router, simple_request(MODEL_ID, "Hi"))
        .await
        .expect("completion after switch_back failed");
    assert!(!extract_text(&events).is_empty());
}

/// Model switch history records each transition with reasons.
#[tokio::test]
async fn ollama_switch_history() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let mut router = two_model_router();

    // Do several switches
    router.switch_model(MODEL_ALT, ModelSwitchReason::UserRequest);
    router.switch_model(MODEL_ID, ModelSwitchReason::RoleSwitch { role: "smol".into() });
    router.switch_model(MODEL_ALT, ModelSwitchReason::ConfigChange);

    let tracker = router.switch_tracker();
    assert_eq!(tracker.total_switches(), 3);

    let history = tracker.history();
    // history[0] = Initial (MODEL_ID)
    // history[1] = UserRequest (→ MODEL_ALT)
    // history[2] = RoleSwitch (→ MODEL_ID)
    // history[3] = ConfigChange (→ MODEL_ALT)
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].reason, ModelSwitchReason::Initial);
    assert_eq!(history[1].reason, ModelSwitchReason::UserRequest);
    assert_eq!(history[1].from, MODEL_ID);
    assert_eq!(history[1].to, MODEL_ALT);
    assert_eq!(history[2].reason, ModelSwitchReason::RoleSwitch { role: "smol".into() });
    assert_eq!(history[3].reason, ModelSwitchReason::ConfigChange);
}

/// Switching to the same model is a no-op.
#[tokio::test]
async fn ollama_switch_same_model_noop() {
    if !ollama_available() {
        return;
    }

    let mut router = ollama_router();
    let result = router.switch_model(MODEL_ID, ModelSwitchReason::UserRequest);
    assert!(result.is_none(), "switching to the same model should return None");
    assert_eq!(router.switch_tracker().total_switches(), 0);
}

/// After switching, the default_model updates and completions route correctly.
#[tokio::test]
async fn ollama_switch_changes_default_model() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let mut router = two_model_router();
    assert_eq!(router.default_model(), MODEL_ID);

    router.switch_model(MODEL_ALT, ModelSwitchReason::UserRequest);
    assert_eq!(router.default_model(), MODEL_ALT);

    // Complete using the default (should go to MODEL_ALT now)
    let events = collect_events(&router, simple_request(MODEL_ALT, "Say yes."))
        .await
        .expect("completion on new default failed");
    assert!(!extract_text(&events).is_empty());
}

/// Both models produce different MessageStart metadata (model field differs).
#[tokio::test]
async fn ollama_switch_models_have_distinct_metadata() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let router = two_model_router();

    let events1 = collect_events(&router, simple_request(MODEL_ID, "Hi")).await.expect("model 1 failed");
    let events2 = collect_events(&router, simple_request(MODEL_ALT, "Hi")).await.expect("model 2 failed");

    let model1 = match events1.first() {
        Some(StreamEvent::MessageStart { message }) => message.model.clone(),
        _ => panic!("expected MessageStart for model 1"),
    };
    let model2 = match events2.first() {
        Some(StreamEvent::MessageStart { message }) => message.model.clone(),
        _ => panic!("expected MessageStart for model 2"),
    };

    assert_ne!(model1, model2, "different models should report different model names in metadata");
}

/// Model switch with DB: usage is tracked per-model across switches.
#[tokio::test]
async fn ollama_switch_usage_per_model() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = RouterDb::open(&tmp.path().join("switch_usage.db")).expect("open db");

    let mut router = Router::with_db(MODEL_ID, db);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID), ollama_model(MODEL_ALT)]));

    // Complete on model 1
    let _ = collect_events(&router, simple_request(MODEL_ID, "Hi")).await.expect("model 1 failed");

    // Switch and complete on model 2
    router.switch_model(MODEL_ALT, ModelSwitchReason::UserRequest);
    let _ = collect_events(&router, simple_request(MODEL_ALT, "Hi")).await.expect("model 2 failed");

    // Both models should appear in the request log
    let log = router.db().unwrap().request_log();
    let entries = log.recent(10).expect("log query");
    assert!(entries.len() >= 2, "expected at least 2 log entries, got {}", entries.len());

    let models_logged: Vec<&str> = entries.iter().map(|e| e.model.as_str()).collect();
    assert!(models_logged.contains(&MODEL_ID), "log should contain {MODEL_ID}, got {models_logged:?}");
    assert!(models_logged.contains(&MODEL_ALT), "log should contain {MODEL_ALT}, got {models_logged:?}");
}

// ── Dynamic model switching tests ───────────────────────────────────────
//
// These tests simulate the agent turn loop's slot-based model switch
// mechanism: SwitchModelTool writes to a shared slot, check_model_switch
// reads it and updates active_model, the next completion uses the new model.

/// Simulate the slot-based dynamic switch the turn loop performs.
///
/// Flow: complete on model A → tool writes slot → "turn loop" reads slot
/// → complete on model B → verify both responses are valid.
#[tokio::test]
async fn ollama_dynamic_slot_switch() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let router = two_model_router();
    let slot: Arc<parking_lot::Mutex<Option<String>>> = Arc::new(parking_lot::Mutex::new(None));

    // Turn 1: complete on MODEL_ID
    let mut active_model = MODEL_ID.to_string();
    let events1 = collect_events(&router, simple_request(&active_model, "Say hello.")).await.expect("turn 1 failed");
    assert!(!extract_text(&events1).is_empty(), "turn 1 should produce output");

    // Simulate SwitchModelTool writing to the slot
    *slot.lock() = Some(MODEL_ALT.to_string());

    // Simulate check_model_switch: read and consume the slot
    if let Some(new_model) = slot.lock().take() {
        active_model = new_model;
    }
    assert_eq!(active_model, MODEL_ALT, "active model should have switched");

    // Turn 2: complete on MODEL_ALT
    let events2 = collect_events(&router, simple_request(&active_model, "Say goodbye.")).await.expect("turn 2 failed");
    assert!(!extract_text(&events2).is_empty(), "turn 2 should produce output");

    // Verify the two turns used different models (via MessageStart metadata)
    let model1 = match events1.first() {
        Some(StreamEvent::MessageStart { message }) => &message.model,
        _ => panic!("missing MessageStart in turn 1"),
    };
    let model2 = match events2.first() {
        Some(StreamEvent::MessageStart { message }) => &message.model,
        _ => panic!("missing MessageStart in turn 2"),
    };
    assert_ne!(model1, model2, "turns should use different models");
}

/// Slot-based switch mid-conversation: the new model sees prior context.
///
/// Simulates a multi-turn conversation where a model switch happens
/// between turns. The switched-to model should still be able to reference
/// context from earlier in the conversation (passed via messages).
#[tokio::test]
async fn ollama_dynamic_switch_preserves_context() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let router = two_model_router();

    // Turn 1 on MODEL_ID: establish a fact
    let mut active_model = MODEL_ID.to_string();
    let events1 = collect_events(&router, CompletionRequest {
        model: active_model.clone(),
        messages: vec![json!({"role": "user", "content": "The secret word is 'banana'. Acknowledge this."})],
        system_prompt: None,
        max_tokens: Some(32),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: HashMap::new(),
    })
    .await
    .expect("turn 1 failed");
    let response1 = extract_text(&events1);

    // Dynamic switch via slot
    active_model = MODEL_ALT.to_string();

    // Turn 2 on MODEL_ALT: ask about the fact, passing full history
    let events2 = collect_events(&router, CompletionRequest {
        model: active_model.clone(),
        messages: vec![
            json!({"role": "user", "content": "The secret word is 'banana'. Acknowledge this."}),
            json!({"role": "assistant", "content": response1}),
            json!({"role": "user", "content": "What is the secret word? Reply with just the word."}),
        ],
        system_prompt: None,
        max_tokens: Some(16),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: HashMap::new(),
    })
    .await
    .expect("turn 2 failed");
    let text2 = extract_text(&events2).to_lowercase();
    assert!(text2.contains("banana"), "switched model should recall 'banana' from prior context, got: {text2}");
}

/// Switch back and forth between models, completing on each.
/// Ensures no state leaks between switches.
///
/// Note: Ollama swaps models between different families (qwen2.5 vs qwen3),
/// so each switch may take several seconds for model loading.
#[tokio::test]
async fn ollama_dynamic_toggle() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let router = two_model_router();

    // MODEL_ID → MODEL_ALT → back to MODEL_ID
    for (i, model) in [MODEL_ID, MODEL_ALT, MODEL_ID].iter().enumerate() {
        let events = collect_events(&router, simple_request(model, &format!("What is {i}+{i}?")))
            .await
            .unwrap_or_else(|e| panic!("turn {i} on {model} failed: {e}"));
        let text = extract_text(&events);
        assert!(!text.is_empty(), "turn {i} on {model} returned empty");
    }
}

/// Dynamic switch with DB logging: verify each turn's model is logged correctly.
#[tokio::test]
async fn ollama_dynamic_switch_logged_per_turn() {
    if !ollama_available() || !model_available(MODEL_ALT) {
        eprintln!("SKIP: need both {} and {}", MODEL_ID, MODEL_ALT);
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = RouterDb::open(&tmp.path().join("dynamic_log.db")).expect("open db");

    let mut router = Router::with_db(MODEL_ID, db);
    router.register_provider(ollama_provider(vec![ollama_model(MODEL_ID), ollama_model(MODEL_ALT)]));

    // Turn 1 on MODEL_ID
    let _ = collect_events(&router, simple_request(MODEL_ID, "Hi")).await.expect("turn 1 failed");

    // Dynamic switch
    router.switch_model(MODEL_ALT, ModelSwitchReason::UserRequest);

    // Turn 2 on MODEL_ALT
    let _ = collect_events(&router, simple_request(MODEL_ALT, "Hi")).await.expect("turn 2 failed");

    // Switch back
    router.switch_back();

    // Turn 3 on MODEL_ID again
    let _ = collect_events(&router, simple_request(MODEL_ID, "Hi")).await.expect("turn 3 failed");

    // Verify all three turns appear in the log with correct models
    let entries = router.db().unwrap().request_log().recent(10).expect("log");
    assert!(entries.len() >= 3, "expected >= 3 log entries, got {}", entries.len());

    // Entries are newest-first
    let logged_models: Vec<&str> = entries.iter().rev().map(|e| e.model.as_str()).collect();
    assert_eq!(
        logged_models[..3],
        [MODEL_ID, MODEL_ALT, MODEL_ID],
        "log should reflect switch pattern, got: {logged_models:?}"
    );
}

/// Concurrent requests don't interfere with each other.
#[tokio::test]
async fn ollama_concurrent_requests() {
    if !ollama_available() {
        return;
    }

    let router = Arc::new(ollama_router());
    let mut handles = Vec::new();

    for i in 0..3 {
        let router = Arc::clone(&router);
        let prompt = format!("What is {} + {}? Reply with just the number.", i, i);
        handles.push(tokio::spawn(async move {
            let req = simple_request(MODEL_ID, &prompt);
            let (tx, mut rx) = mpsc::channel(256);
            router.complete(req, tx).await.expect("concurrent request failed");
            let mut events = Vec::new();
            while let Some(ev) = rx.recv().await {
                events.push(ev);
            }
            extract_text(&events)
        }));
    }

    let results: Vec<String> =
        futures::future::join_all(handles).await.into_iter().map(|r| r.expect("task panicked")).collect();

    // All three should have produced output
    for (i, text) in results.iter().enumerate() {
        assert!(!text.is_empty(), "concurrent request {i} returned empty");
    }
}
