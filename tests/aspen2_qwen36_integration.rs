//! Live integration tests against aspen2's Lemonade OpenAI-compatible Qwen 3.6 endpoint.
//!
//! These tests are intentionally self-skipping so normal CI and offline runs stay safe.
//! Defaults:
//!   - `ASPEN2_QWEN36_BASE_URL=http://aspen2:13305/v1`
//!   - `ASPEN2_QWEN36_MODEL=user.Qwen3.6-35B-A3B`
//!
//! Skip explicitly with: `cargo nextest run -E 'not test(aspen2_qwen36)'`

use std::collections::HashMap;
use std::time::Duration;

use clanker_router::Model;
use clanker_router::Router;
use clanker_router::backends::openai_compat::OpenAICompatConfig;
use clanker_router::backends::openai_compat::OpenAICompatProvider;
use clanker_router::provider::CompletionRequest;
use clanker_router::streaming::ContentDelta;
use clanker_router::streaming::StreamEvent;
use serde_json::json;
use tokio::sync::mpsc;

const DEFAULT_BASE_URL: &str = "http://aspen2:13305/v1";
const DEFAULT_MODEL_ID: &str = "user.Qwen3.6-35B-A3B";

fn base_url() -> String {
    std::env::var("ASPEN2_QWEN36_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
}

fn model_id() -> String {
    std::env::var("ASPEN2_QWEN36_MODEL").unwrap_or_else(|_| DEFAULT_MODEL_ID.to_string())
}

fn aspen2_qwen36_model(id: &str) -> Model {
    Model {
        id: id.to_string(),
        name: "Qwen 3.6 35B-A3B (aspen2)".to_string(),
        provider: "local".to_string(),
        max_input_tokens: 131_072,
        max_output_tokens: 8_192,
        supports_thinking: true,
        supports_images: false,
        supports_tools: true,
        input_cost_per_mtok: None,
        output_cost_per_mtok: None,
    }
}

fn aspen2_qwen36_available(base_url: &str, model_id: &str) -> bool {
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let model_id_for_probe = model_id.to_string();
    let ok = std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build().ok()?;
        let resp = client.get(models_url).send().ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: serde_json::Value = resp.json().ok()?;
        let models = body.get("data")?.as_array()?;
        Some(models.iter().any(|m| m.get("id").and_then(|v| v.as_str()) == Some(model_id_for_probe.as_str())))
    })
    .join()
    .ok()
    .flatten()
    .unwrap_or(false);

    if !ok {
        eprintln!("SKIP: aspen2 Qwen 3.6 endpoint/model unavailable at {base_url} ({model_id})");
    }
    ok
}

fn aspen2_qwen36_router(base_url: String, model_id: &str) -> Router {
    let mut router = Router::new(model_id);
    let config = OpenAICompatConfig::local(base_url, vec![aspen2_qwen36_model(model_id)]);
    router.register_provider(OpenAICompatProvider::new(config));
    router
}

fn qwen_reasoning_request(model: &str) -> CompletionRequest {
    CompletionRequest {
        model: model.to_string(),
        messages: vec![json!({
            "role": "user",
            "content": "Think briefly, then answer with exactly: OK"
        })],
        system_prompt: None,
        max_tokens: Some(96),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: true,
        cache_ttl: None,
        extra_params: HashMap::new(),
    }
}

async fn collect_events(router: &Router, request: CompletionRequest) -> clanker_router::Result<Vec<StreamEvent>> {
    let (tx, mut rx) = mpsc::channel(256);
    router.complete(request, tx).await?;
    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    Ok(events)
}

#[tokio::test]
async fn aspen2_qwen36_streams_reasoning_or_text() {
    let base_url = base_url();
    let model_id = model_id();
    if !aspen2_qwen36_available(&base_url, &model_id) {
        return;
    }

    let router = aspen2_qwen36_router(base_url, &model_id);
    let events =
        tokio::time::timeout(Duration::from_secs(120), collect_events(&router, qwen_reasoning_request(&model_id)))
            .await
            .expect("aspen2 Qwen 3.6 completion timed out")
            .expect("aspen2 Qwen 3.6 completion should succeed");

    assert!(
        matches!(events.first(), Some(StreamEvent::MessageStart { .. })),
        "first event should be MessageStart; events: {events:?}"
    );
    assert!(
        matches!(events.last(), Some(StreamEvent::MessageStop)),
        "last event should be MessageStop; events: {events:?}"
    );

    let has_reasoning = events.iter().any(|ev| {
        matches!(
            ev,
            StreamEvent::ContentBlockDelta {
                delta: ContentDelta::ThinkingDelta { thinking },
                ..
            } if !thinking.is_empty()
        )
    });
    let has_text = events.iter().any(|ev| {
        matches!(
            ev,
            StreamEvent::ContentBlockDelta {
                delta: ContentDelta::TextDelta { text },
                ..
            } if !text.is_empty()
        )
    });

    assert!(
        has_reasoning || has_text,
        "expected non-empty reasoning or text delta from aspen2 Qwen 3.6; events: {events:?}"
    );
}
