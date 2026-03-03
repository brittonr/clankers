//! Integration tests for clankers-router
//!
//! Tests cross-module interactions, edge cases, and scenarios not covered
//! by unit tests in individual modules.

use std::sync::Arc;

use async_trait::async_trait;
use clankers_router::Router;
use clankers_router::auth::AuthStore;
use clankers_router::auth::LegacyOAuthCredentials;
use clankers_router::auth::StoredCredential;
use clankers_router::auth::env_var_for_provider;
use clankers_router::auth::is_oauth_token;
use clankers_router::auth::resolve_credential;
use clankers_router::error::Error;
use clankers_router::model::Model;
use clankers_router::model::ModelAliases;
use clankers_router::model_switch::ModelSwitchReason;
use clankers_router::multi::MultiRequest;
use clankers_router::multi::MultiStrategy;
use clankers_router::quorum::ConsensusStrategy;
use clankers_router::quorum::QuorumRequest;
use clankers_router::quorum::QuorumTarget;
use clankers_router::provider::CompletionRequest;
use clankers_router::provider::Provider;
use clankers_router::provider::ThinkingConfig;
use clankers_router::provider::ToolDefinition;
use clankers_router::provider::Usage;
use clankers_router::registry::ModelRegistry;
use clankers_router::retry::RetryConfig;
use clankers_router::retry::is_retryable_error;
use clankers_router::retry::is_retryable_status;
use clankers_router::retry::parse_retry_after;
use clankers_router::streaming::ContentBlock;
use clankers_router::streaming::ContentDelta;
use clankers_router::streaming::MessageMetadata;
use clankers_router::streaming::StreamEvent;
use clankers_router::streaming::TaggedStreamEvent;
use serde_json::json;
use tokio::sync::mpsc;

// ── Test helpers ────────────────────────────────────────────────────────

fn make_model(id: &str, provider: &str) -> Model {
    Model {
        id: id.to_string(),
        name: id.to_string(),
        provider: provider.to_string(),
        max_input_tokens: 200_000,
        max_output_tokens: 16_384,
        supports_thinking: true,
        supports_images: true,
        supports_tools: true,
        input_cost_per_mtok: Some(3.0),
        output_cost_per_mtok: Some(15.0),
    }
}

fn make_model_with_caps(id: &str, provider: &str, thinking: bool, images: bool, tools: bool) -> Model {
    Model {
        id: id.to_string(),
        name: id.to_string(),
        provider: provider.to_string(),
        max_input_tokens: 128_000,
        max_output_tokens: 8_192,
        supports_thinking: thinking,
        supports_images: images,
        supports_tools: tools,
        input_cost_per_mtok: None,
        output_cost_per_mtok: None,
    }
}

struct MockProvider {
    name: String,
    models: Vec<Model>,
    /// If set, complete() returns this error
    fail_with: Option<String>,
}

impl MockProvider {
    fn new(name: &str, models: Vec<Model>) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
            models,
            fail_with: None,
        })
    }

    fn failing(name: &str, models: Vec<Model>, error: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
            models,
            fail_with: Some(error.to_string()),
        })
    }
}

#[async_trait]
impl Provider for MockProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> clankers_router::Result<()> {
        if let Some(ref err) = self.fail_with {
            return Err(Error::Provider {
                message: err.clone(),
                status: None,
            });
        }

        // Echo the model name back via MessageStart so tests can verify routing
        let _ = tx
            .send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "test-id".to_string(),
                    model: request.model.clone(),
                    role: "assistant".to_string(),
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
                delta: ContentDelta::TextDelta {
                    text: format!("Hello from {}", self.name),
                },
            })
            .await;
        let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
        let _ = tx
            .send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".to_string()),
                usage: Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    ..Default::default()
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

fn simple_request(model: &str) -> CompletionRequest {
    CompletionRequest {
        model: model.to_string(),
        messages: vec![json!({"role": "user", "content": "hello"})],
        system_prompt: None,
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        extra_params: Default::default(),
    }
}

async fn collect_events(router: &Router, request: CompletionRequest) -> clankers_router::Result<Vec<StreamEvent>> {
    let (tx, mut rx) = mpsc::channel(32);
    router.complete(request, tx).await?;
    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    Ok(events)
}

// ═══════════════════════════════════════════════════════════════════════
// Router tests
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_router_routes_to_correct_provider_by_exact_id() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router
        .register_provider(MockProvider::new("anthropic", vec![make_model("claude-sonnet-4-5-20250514", "anthropic")]));
    router.register_provider(MockProvider::new("openai", vec![make_model("gpt-4o", "openai")]));

    let events = collect_events(&router, simple_request("gpt-4o")).await.unwrap();

    // Verify it went to the right provider
    if let StreamEvent::MessageStart { message } = &events[0] {
        assert_eq!(message.model, "gpt-4o");
    } else {
        panic!("Expected MessageStart");
    }
}

#[tokio::test]
async fn test_router_routes_alias_to_correct_provider() {
    let mut router = Router::new("gpt-4o");
    router
        .register_provider(MockProvider::new("anthropic", vec![make_model("claude-sonnet-4-5-20250514", "anthropic")]));
    router.register_provider(MockProvider::new("openai", vec![make_model("gpt-4o", "openai")]));

    // "sonnet" alias → claude-sonnet-4-5-20250514 → anthropic
    let events = collect_events(&router, simple_request("sonnet")).await.unwrap();

    if let StreamEvent::MessageStart { message } = &events[0] {
        assert_eq!(message.model, "claude-sonnet-4-5-20250514");
    } else {
        panic!("Expected MessageStart");
    }
}

#[tokio::test]
async fn test_router_provider_prefix_routing() {
    let mut router = Router::new("gpt-4o");
    router.register_provider(MockProvider::new("openai", vec![make_model("gpt-4o", "openai")]));

    // "openai/some-new-model" should route to the openai provider
    let events = collect_events(&router, simple_request("openai/some-new-model")).await.unwrap();

    if let StreamEvent::MessageStart { message } = &events[0] {
        assert_eq!(message.model, "openai/some-new-model");
    } else {
        panic!("Expected MessageStart");
    }
}

#[tokio::test]
async fn test_router_unknown_model_falls_back_to_default() {
    let mut router = Router::new("gpt-4o");
    router.register_provider(MockProvider::new("openai", vec![make_model("gpt-4o", "openai")]));

    let events = collect_events(&router, simple_request("totally-unknown-model")).await.unwrap();

    // Should fall back to the default model
    if let StreamEvent::MessageStart { message } = &events[0] {
        assert_eq!(message.model, "gpt-4o");
    } else {
        panic!("Expected MessageStart");
    }
}

#[tokio::test]
async fn test_router_no_providers_returns_error() {
    let router = Router::new("nonexistent");
    let (tx, _rx) = mpsc::channel(32);
    let result = router.complete(simple_request("anything"), tx).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("no provider"), "Error was: {}", err);
}

#[tokio::test]
async fn test_router_provider_failure_propagates() {
    let mut router = Router::new("test-model");
    router.register_provider(MockProvider::failing(
        "failing",
        vec![make_model("test-model", "failing")],
        "API exploded",
    ));

    let (tx, _rx) = mpsc::channel(32);
    let result = router.complete(simple_request("test-model"), tx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("API exploded"));
}

#[test]
fn test_router_set_default_model() {
    let mut router = Router::new("model-a");
    assert_eq!(router.default_model(), "model-a");
    router.set_default_model("model-b");
    assert_eq!(router.default_model(), "model-b");
}

#[test]
fn test_router_provider_lookup() {
    let mut router = Router::new("gpt-4o");
    router.register_provider(MockProvider::new("openai", vec![make_model("gpt-4o", "openai")]));

    assert!(router.provider("openai").is_some());
    assert!(router.provider("nonexistent").is_none());
}

#[tokio::test]
async fn test_router_reload_credentials() {
    let mut router = Router::new("model-a");
    router.register_provider(MockProvider::new("prov", vec![make_model("model-a", "prov")]));
    // Should not panic even with no credentials to reload
    router.reload_credentials().await;
}

#[test]
fn test_router_resolve_model() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router
        .register_provider(MockProvider::new("anthropic", vec![make_model("claude-sonnet-4-5-20250514", "anthropic")]));

    assert!(router.resolve_model("sonnet").is_some());
    assert!(router.resolve_model("claude-sonnet-4-5-20250514").is_some());
    assert!(router.resolve_model("nonexistent-xyz-abc").is_none());
}

#[tokio::test]
async fn test_router_complete_full_event_stream() {
    let mut router = Router::new("test-model");
    router.register_provider(MockProvider::new("test", vec![make_model("test-model", "test")]));

    let events = collect_events(&router, simple_request("test-model")).await.unwrap();

    // Verify we get the full event sequence
    assert!(matches!(&events[0], StreamEvent::MessageStart { .. }));
    assert!(matches!(&events[1], StreamEvent::ContentBlockStart { .. }));
    assert!(matches!(&events[2], StreamEvent::ContentBlockDelta { .. }));
    assert!(matches!(&events[3], StreamEvent::ContentBlockStop { .. }));
    assert!(matches!(&events[4], StreamEvent::MessageDelta { .. }));
    assert!(matches!(&events[5], StreamEvent::MessageStop));

    // Verify usage in MessageDelta
    if let StreamEvent::MessageDelta { usage, stop_reason } = &events[4] {
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens(), 150);
        assert_eq!(stop_reason.as_deref(), Some("end_turn"));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Registry tests — extended
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_registry_empty() {
    let reg = ModelRegistry::new();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
    assert!(reg.list().is_empty());
    assert!(reg.get("anything").is_none());
    assert!(reg.resolve("anything").is_none());
}

#[test]
fn test_registry_with_capability_filtering() {
    let mut reg = ModelRegistry::new();
    reg.register_models(&[
        make_model_with_caps("m1", "p", true, true, true),
        make_model_with_caps("m2", "p", false, true, true),
        make_model_with_caps("m3", "p", true, false, true),
        make_model_with_caps("m4", "p", false, false, false),
    ]);

    // Only thinking models
    let thinking = reg.with_capability(true, false);
    assert_eq!(thinking.len(), 2); // m1, m3

    // Only image models
    let images = reg.with_capability(false, true);
    assert_eq!(images.len(), 2); // m1, m2

    // Both thinking + images
    let both = reg.with_capability(true, true);
    assert_eq!(both.len(), 1); // m1

    // No filter
    let all = reg.with_capability(false, false);
    assert_eq!(all.len(), 4);
}

#[test]
fn test_registry_overwrite_model() {
    let mut reg = ModelRegistry::new();
    reg.register_models(&[make_model("m1", "provider-a")]);
    assert_eq!(reg.get("m1").unwrap().provider, "provider-a");

    // Re-register same ID with different provider
    reg.register_models(&[make_model("m1", "provider-b")]);
    assert_eq!(reg.get("m1").unwrap().provider, "provider-b");
    assert_eq!(reg.len(), 1);
}

#[test]
fn test_registry_case_insensitive_substring() {
    let mut reg = ModelRegistry::new();
    reg.register_models(&[make_model("Claude-Sonnet-4-5-20250514", "anthropic")]);

    // Case-insensitive substring
    assert!(reg.resolve("claude").is_some());
    assert!(reg.resolve("CLAUDE").is_some());
    assert!(reg.resolve("SONNET").is_some());
}

#[test]
fn test_registry_list_sorted() {
    let mut reg = ModelRegistry::new();
    reg.register_models(&[
        make_model("z-model", "p"),
        make_model("a-model", "p"),
        make_model("m-model", "p"),
    ]);

    let list = reg.list();
    assert_eq!(list[0].id, "a-model");
    assert_eq!(list[1].id, "m-model");
    assert_eq!(list[2].id, "z-model");
}

#[test]
fn test_registry_provider_for_via_alias() {
    let mut reg = ModelRegistry::new();
    reg.register_models(&[make_model("claude-sonnet-4-5-20250514", "anthropic")]);

    assert_eq!(reg.provider_for("sonnet"), Some("anthropic"));
    assert_eq!(reg.provider_for("claude-sonnet-4-5-20250514"), Some("anthropic"));
}

// ═══════════════════════════════════════════════════════════════════════
// Model tests — extended
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_model_estimate_cost_zero_tokens() {
    let model = Model {
        id: "test".into(),
        name: "Test".into(),
        provider: "test".into(),
        max_input_tokens: 100_000,
        max_output_tokens: 8_000,
        supports_thinking: false,
        supports_images: false,
        supports_tools: false,
        input_cost_per_mtok: Some(3.0),
        output_cost_per_mtok: Some(15.0),
    };
    let cost = model.estimate_cost(0, 0).unwrap();
    assert!((cost - 0.0).abs() < 0.001);
}

#[test]
fn test_model_estimate_cost_large_tokens() {
    let model = Model {
        id: "test".into(),
        name: "Test".into(),
        provider: "test".into(),
        max_input_tokens: 200_000,
        max_output_tokens: 16_384,
        supports_thinking: false,
        supports_images: false,
        supports_tools: false,
        input_cost_per_mtok: Some(3.0),
        output_cost_per_mtok: Some(15.0),
    };
    // 10M input, 1M output
    let cost = model.estimate_cost(10_000_000, 1_000_000).unwrap();
    assert!((cost - 45.0).abs() < 0.001); // 30.0 + 15.0
}

#[test]
fn test_model_aliases_all_variants() {
    // Anthropic aliases
    assert!(ModelAliases::resolve("sonnet").is_some());
    assert!(ModelAliases::resolve("claude-sonnet").is_some());
    assert!(ModelAliases::resolve("claude-sonnet-4-5").is_some());
    assert!(ModelAliases::resolve("opus").is_some());
    assert!(ModelAliases::resolve("claude-opus").is_some());
    assert!(ModelAliases::resolve("claude-opus-4").is_some());
    assert!(ModelAliases::resolve("opus-4-6").is_some());
    assert!(ModelAliases::resolve("claude-opus-4-6").is_some());
    assert!(ModelAliases::resolve("haiku").is_some());
    assert!(ModelAliases::resolve("claude-haiku").is_some());
    assert!(ModelAliases::resolve("claude-haiku-4-5").is_some());

    // OpenAI aliases
    assert_eq!(ModelAliases::resolve("gpt-4o"), Some("gpt-4o"));
    assert_eq!(ModelAliases::resolve("4o"), Some("gpt-4o"));
    assert_eq!(ModelAliases::resolve("gpt-4o-mini"), Some("gpt-4o-mini"));
    assert_eq!(ModelAliases::resolve("4o-mini"), Some("gpt-4o-mini"));
    assert_eq!(ModelAliases::resolve("o1"), Some("o1"));
    assert_eq!(ModelAliases::resolve("o1-mini"), Some("o1-mini"));
    assert_eq!(ModelAliases::resolve("o3"), Some("o3"));
    assert_eq!(ModelAliases::resolve("o3-mini"), Some("o3-mini"));

    // Google aliases
    assert!(ModelAliases::resolve("gemini-pro").is_some());
    assert!(ModelAliases::resolve("gemini-2.5-pro").is_some());
    assert!(ModelAliases::resolve("gemini-flash").is_some());
    assert!(ModelAliases::resolve("gemini-2.5-flash").is_some());

    // DeepSeek aliases
    assert_eq!(ModelAliases::resolve("deepseek"), Some("deepseek-chat"));
    assert_eq!(ModelAliases::resolve("deepseek-v3"), Some("deepseek-chat"));
    assert_eq!(ModelAliases::resolve("deepseek-r1"), Some("deepseek-reasoner"));

    // Unknown → None
    assert_eq!(ModelAliases::resolve("totally-unknown"), None);
    assert_eq!(ModelAliases::resolve(""), None);
}

// ═══════════════════════════════════════════════════════════════════════
// Auth tests — extended
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_auth_store_load_nonexistent_file() {
    let store = AuthStore::load(std::path::Path::new("/nonexistent/path/auth.json"));
    assert!(store.configured_providers().is_empty());
}

#[test]
fn test_auth_store_legacy_migration_no_overwrite() {
    // If v2 already has an "anthropic" default account, legacy should NOT overwrite
    // We test this by writing a JSON file with both v2 and legacy fields, then loading it
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    let json = r#"{
        "version": 2,
        "providers": {
            "anthropic": {
                "active_account": "default",
                "accounts": {
                    "default": {
                        "credential_type": "api_key",
                        "api_key": "existing-key"
                    }
                }
            }
        },
        "anthropic": {
            "access": "legacy-token",
            "refresh": "legacy-refresh",
            "expires": 9999999999999
        }
    }"#;
    std::fs::write(&path, json).unwrap();

    let store = AuthStore::load(&path);

    // Existing v2 credential should NOT be overwritten by legacy migration
    assert_eq!(store.active_credential("anthropic").unwrap().token(), "existing-key");
}

#[test]
fn test_stored_credential_api_key_properties() {
    let cred = StoredCredential::ApiKey {
        api_key: "sk-test-123".into(),
        label: Some("My key".into()),
    };
    assert_eq!(cred.token(), "sk-test-123");
    assert!(!cred.is_oauth());
    assert!(!cred.is_expired());
    assert!(cred.refresh_token().is_none());
    assert_eq!(cred.label(), Some("My key"));
}

#[test]
fn test_stored_credential_oauth_expired() {
    let cred = StoredCredential::OAuth {
        access_token: "oat-old".into(),
        refresh_token: "ort-old".into(),
        expires_at_ms: 0, // expired long ago
        label: None,
    };
    assert!(cred.is_expired());
    assert!(cred.is_oauth());
    assert_eq!(cred.refresh_token(), Some("ort-old"));
    assert_eq!(cred.label(), None);
}

#[test]
fn test_stored_credential_oauth_not_expired() {
    let cred = StoredCredential::OAuth {
        access_token: "oat-new".into(),
        refresh_token: "ort-new".into(),
        expires_at_ms: i64::MAX,
        label: Some("Fresh".into()),
    };
    assert!(!cred.is_expired());
    assert!(cred.is_oauth());
}

#[test]
fn test_auth_store_multiple_accounts() {
    let mut store = AuthStore::default();

    store.set_credential("anthropic", "personal", StoredCredential::ApiKey {
        api_key: "personal-key".into(),
        label: Some("Personal".into()),
    });
    store.set_credential("anthropic", "work", StoredCredential::ApiKey {
        api_key: "work-key".into(),
        label: Some("Work".into()),
    });
    store.set_credential("anthropic", "test", StoredCredential::ApiKey {
        api_key: "test-key".into(),
        label: None,
    });

    let accounts = store.list_accounts("anthropic");
    assert_eq!(accounts.len(), 3);

    // First account set is auto-activated
    assert_eq!(store.active_credential("anthropic").unwrap().token(), "personal-key");

    // Switch and verify
    assert!(store.switch_account("anthropic", "work"));
    assert_eq!(store.active_credential("anthropic").unwrap().token(), "work-key");

    // Remove active account — should auto-switch
    assert!(store.remove_account("anthropic", "work"));
    assert!(store.active_credential("anthropic").is_some());
}

#[test]
fn test_auth_store_remove_last_account() {
    let mut store = AuthStore::default();
    store.set_credential("openai", "only", StoredCredential::ApiKey {
        api_key: "k".into(),
        label: None,
    });
    assert!(store.remove_account("openai", "only"));
    assert!(store.active_credential("openai").is_none());
}

#[test]
fn test_auth_store_remove_nonexistent() {
    let mut store = AuthStore::default();
    assert!(!store.remove_account("openai", "nonexistent"));
    assert!(!store.switch_account("openai", "nonexistent"));
}

#[test]
fn test_auth_store_credential_for_specific_account() {
    let mut store = AuthStore::default();
    store.set_credential("openai", "a", StoredCredential::ApiKey {
        api_key: "key-a".into(),
        label: None,
    });
    store.set_credential("openai", "b", StoredCredential::ApiKey {
        api_key: "key-b".into(),
        label: None,
    });

    assert_eq!(store.credential_for("openai", "a").unwrap().token(), "key-a");
    assert_eq!(store.credential_for("openai", "b").unwrap().token(), "key-b");
    assert!(store.credential_for("openai", "c").is_none());
    assert!(store.credential_for("nonexistent", "a").is_none());
}

#[test]
fn test_auth_store_list_accounts_nonexistent_provider() {
    let store = AuthStore::default();
    assert!(store.list_accounts("nonexistent").is_empty());
}

#[test]
fn test_auth_store_anthropic_legacy_sync() {
    // Setting an OAuth credential for anthropic should also update the legacy field
    let mut store = AuthStore::default();
    store.set_credential("anthropic", "default", StoredCredential::OAuth {
        access_token: "oat-sync".into(),
        refresh_token: "ort-sync".into(),
        expires_at_ms: 999,
        label: None,
    });

    let legacy = store.anthropic.as_ref().unwrap();
    assert_eq!(legacy.access, "oat-sync");
    assert_eq!(legacy.refresh, "ort-sync");
    assert_eq!(legacy.expires, 999);
}

#[test]
fn test_auth_store_anthropic_api_key_no_legacy_sync() {
    // Setting an API key for anthropic should NOT create a legacy entry
    let mut store = AuthStore::default();
    store.set_credential("anthropic", "default", StoredCredential::ApiKey {
        api_key: "sk-ant-test".into(),
        label: None,
    });
    assert!(store.anthropic.is_none());
}

#[test]
fn test_auth_store_summary_empty() {
    let store = AuthStore::default();
    assert!(store.summary().contains("No credentials configured"));
}

#[test]
fn test_auth_store_summary_with_expired() {
    let mut store = AuthStore::default();
    store.set_credential("anthropic", "default", StoredCredential::OAuth {
        access_token: "oat".into(),
        refresh_token: "ort".into(),
        expires_at_ms: 0,
        label: None,
    });
    let summary = store.summary();
    assert!(summary.contains("(expired)"));
}

#[test]
fn test_auth_store_save_load_preserves_all_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");

    let mut store = AuthStore::default();
    store.set_credential("anthropic", "default", StoredCredential::OAuth {
        access_token: "oat-test".into(),
        refresh_token: "ort-test".into(),
        expires_at_ms: 1234567890,
        label: Some("Test OAuth".into()),
    });
    store.set_credential("openai", "work", StoredCredential::ApiKey {
        api_key: "sk-work".into(),
        label: Some("Work key".into()),
    });
    store.save(&path).unwrap();

    let loaded = AuthStore::load(&path);

    let ant_cred = loaded.active_credential("anthropic").unwrap();
    assert!(ant_cred.is_oauth());
    assert_eq!(ant_cred.token(), "oat-test");
    assert_eq!(ant_cred.label(), Some("Test OAuth"));

    let oai_cred = loaded.active_credential("openai").unwrap();
    assert!(!oai_cred.is_oauth());
    assert_eq!(oai_cred.token(), "sk-work");
    assert_eq!(oai_cred.label(), Some("Work key"));
}

#[test]
fn test_auth_store_save_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep").join("nested").join("auth.json");

    let store = AuthStore::default();
    store.save(&path).unwrap();
    assert!(path.exists());
}

#[test]
fn test_is_oauth_token() {
    assert!(is_oauth_token("sk-ant-oat-abc123"));
    assert!(!is_oauth_token("sk-ant-api-abc123"));
    assert!(!is_oauth_token("sk-openai-key"));
    assert!(!is_oauth_token(""));
}

#[test]
fn test_env_var_for_provider_all() {
    let providers_with_vars = [
        ("anthropic", "ANTHROPIC_API_KEY"),
        ("openai", "OPENAI_API_KEY"),
        ("openrouter", "OPENROUTER_API_KEY"),
        ("google", "GOOGLE_API_KEY"),
        ("gemini", "GOOGLE_API_KEY"),
        ("mistral", "MISTRAL_API_KEY"),
        ("groq", "GROQ_API_KEY"),
        ("deepseek", "DEEPSEEK_API_KEY"),
        ("together", "TOGETHER_API_KEY"),
        ("fireworks", "FIREWORKS_API_KEY"),
        ("perplexity", "PERPLEXITY_API_KEY"),
        ("cohere", "COHERE_API_KEY"),
        ("xai", "XAI_API_KEY"),
        ("grok", "XAI_API_KEY"),
    ];

    for (provider, expected_var) in providers_with_vars {
        assert_eq!(env_var_for_provider(provider), Some(expected_var), "Failed for provider: {}", provider);
    }

    assert_eq!(env_var_for_provider("custom-provider"), None);
}

#[test]
fn test_resolve_credential_empty_override() {
    let store = AuthStore::default();
    // Empty string override should be skipped
    let cred = resolve_credential("openai", Some(""), &store, None);
    assert!(cred.is_none());
}

#[test]
fn test_resolve_credential_oauth_override() {
    let store = AuthStore::default();
    let cred = resolve_credential("anthropic", Some("sk-ant-oat-override"), &store, None).unwrap();
    assert!(cred.is_oauth());
    assert_eq!(cred.token(), "sk-ant-oat-override");
}

#[test]
fn test_resolve_credential_fallback_store() {
    let primary = AuthStore::default();
    let mut fallback = AuthStore::default();
    fallback.set_credential("openai", "default", StoredCredential::ApiKey {
        api_key: "fallback-key".into(),
        label: None,
    });

    let cred = resolve_credential("openai", None, &primary, Some(&fallback)).unwrap();
    assert_eq!(cred.token(), "fallback-key");
}

#[test]
fn test_resolve_credential_primary_over_fallback() {
    let mut primary = AuthStore::default();
    primary.set_credential("openai", "default", StoredCredential::ApiKey {
        api_key: "primary-key".into(),
        label: None,
    });
    let mut fallback = AuthStore::default();
    fallback.set_credential("openai", "default", StoredCredential::ApiKey {
        api_key: "fallback-key".into(),
        label: None,
    });

    let cred = resolve_credential("openai", None, &primary, Some(&fallback)).unwrap();
    assert_eq!(cred.token(), "primary-key");
}

#[test]
fn test_legacy_oauth_expired() {
    let legacy = LegacyOAuthCredentials {
        access: "old".into(),
        refresh: "old".into(),
        expires: 0,
    };
    assert!(legacy.is_expired());

    let legacy_fresh = LegacyOAuthCredentials {
        access: "new".into(),
        refresh: "new".into(),
        expires: i64::MAX,
    };
    assert!(!legacy_fresh.is_expired());
}

// ═══════════════════════════════════════════════════════════════════════
// Retry tests — extended
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_retry_config_custom() {
    let config = RetryConfig {
        max_retries: 5,
        initial_backoff: std::time::Duration::from_millis(100),
        max_backoff: std::time::Duration::from_secs(10),
        multiplier: 3.0,
        jitter: false,
    };
    assert_eq!(config.backoff_for(0), std::time::Duration::from_millis(100));
    assert_eq!(config.backoff_for(1), std::time::Duration::from_millis(300));
    assert_eq!(config.backoff_for(2), std::time::Duration::from_millis(900));
}

#[test]
fn test_retryable_status_all() {
    for status in [429, 500, 502, 503, 529] {
        assert!(is_retryable_status(status), "Status {} should be retryable", status);
    }
    for status in [200, 201, 204, 400, 401, 403, 404, 422] {
        assert!(!is_retryable_status(status), "Status {} should NOT be retryable", status);
    }
}

#[test]
fn test_retryable_error_messages() {
    assert!(is_retryable_error("rate limit exceeded"));
    assert!(is_retryable_error("Server is overloaded"));
    assert!(is_retryable_error("request timeout"));
    assert!(is_retryable_error("connection reset by peer"));
    assert!(is_retryable_error("Connection Refused"));
    assert!(is_retryable_error("temporarily unavailable"));

    assert!(!is_retryable_error("invalid api key"));
    assert!(!is_retryable_error("model not found"));
    assert!(!is_retryable_error(""));
}

#[test]
fn test_parse_retry_after_edge_cases() {
    assert_eq!(parse_retry_after("0"), Some(std::time::Duration::from_secs(0)));
    assert_eq!(parse_retry_after("  30  "), Some(std::time::Duration::from_secs(30)));
    assert_eq!(parse_retry_after(""), None);
    assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), None); // HTTP-date not supported
    assert_eq!(parse_retry_after("-1"), None); // negative
}

// ═══════════════════════════════════════════════════════════════════════
// Streaming types tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_stream_event_serialization_roundtrip() {
    let events = vec![
        StreamEvent::MessageStart {
            message: MessageMetadata {
                id: "msg-123".into(),
                model: "gpt-4o".into(),
                role: "assistant".into(),
            },
        },
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::Text { text: String::new() },
        },
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta { text: "Hello".into() },
        },
        StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::ThinkingDelta {
                thinking: "Let me think...".into(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 2,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"key"#.into(),
            },
        },
        StreamEvent::ContentBlockStart {
            index: 1,
            content_block: ContentBlock::Thinking {
                thinking: String::new(),
            },
        },
        StreamEvent::ContentBlockStart {
            index: 2,
            content_block: ContentBlock::ToolUse {
                id: "call-1".into(),
                name: "bash".into(),
                input: json!({}),
            },
        },
        StreamEvent::ContentBlockStop { index: 0 },
        StreamEvent::MessageDelta {
            stop_reason: Some("end_turn".into()),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: 10,
                cache_read_input_tokens: 20,
            },
        },
        StreamEvent::MessageStop,
        StreamEvent::Error {
            error: "something went wrong".into(),
        },
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let deserialized: StreamEvent = serde_json::from_str(&json).unwrap();

        // Re-serialize and compare JSON
        let json2 = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(json, json2);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Usage tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_usage_default() {
    let usage = Usage::default();
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.cache_creation_input_tokens, 0);
    assert_eq!(usage.cache_read_input_tokens, 0);
    assert_eq!(usage.total_tokens(), 0);
}

#[test]
fn test_usage_total_tokens() {
    let usage = Usage {
        input_tokens: 500,
        output_tokens: 200,
        cache_creation_input_tokens: 100,
        cache_read_input_tokens: 50,
    };
    assert_eq!(usage.total_tokens(), 700);
}

// ═══════════════════════════════════════════════════════════════════════
// Error type tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_error_display() {
    let errors: Vec<Error> = vec![
        Error::Provider {
            message: "boom".into(),
            status: None,
        },
        Error::Auth {
            message: "bad creds".into(),
        },
        Error::Streaming {
            message: "stream broke".into(),
        },
        Error::NoProvider { model: "gpt-5".into() },
        Error::Config {
            message: "bad config".into(),
        },
    ];

    assert!(errors[0].to_string().contains("boom"));
    assert!(errors[1].to_string().contains("bad creds"));
    assert!(errors[2].to_string().contains("stream broke"));
    assert!(errors[3].to_string().contains("gpt-5"));
    assert!(errors[4].to_string().contains("bad config"));
}

#[test]
fn test_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(err.to_string().contains("file not found"));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn test_error_from_json() {
    let json_err = serde_json::from_str::<serde_json::Value>("{{bad").unwrap_err();
    let err: Error = json_err.into();
    assert!(err.to_string().contains("JSON"));
    assert!(std::error::Error::source(&err).is_some());
}

// ═══════════════════════════════════════════════════════════════════════
// CompletionRequest / Provider types tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_completion_request_with_all_fields() {
    let req = CompletionRequest {
        model: "gpt-4o".into(),
        messages: vec![
            json!({"role": "system", "content": "Be helpful"}),
            json!({"role": "user", "content": "Hi"}),
        ],
        system_prompt: Some("System prompt".into()),
        max_tokens: Some(4096),
        temperature: Some(0.5),
        tools: vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }],
        thinking: Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(10_000),
        }),
        extra_params: Default::default(),
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&req).unwrap();
    let req2: CompletionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(req2.model, "gpt-4o");
    assert_eq!(req2.messages.len(), 2);
    assert_eq!(req2.max_tokens, Some(4096));
    assert_eq!(req2.temperature, Some(0.5));
    assert_eq!(req2.tools.len(), 1);
    assert_eq!(req2.tools[0].name, "read_file");
    assert!(req2.thinking.as_ref().unwrap().enabled);
    assert_eq!(req2.thinking.as_ref().unwrap().budget_tokens, Some(10_000));
}

// ═══════════════════════════════════════════════════════════════════════
// OpenAI compat backend — build request edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_openai_compat_provider_creation() {
    use clankers_router::backends::openai_compat::OpenAICompatConfig;
    use clankers_router::backends::openai_compat::OpenAICompatProvider;

    let config = OpenAICompatConfig::openai("sk-test".into());
    let provider = OpenAICompatProvider::new(config);
    assert_eq!(provider.name(), "openai");
    assert!(!provider.models().is_empty());
}

#[tokio::test]
async fn test_openai_compat_provider_is_available() {
    use clankers_router::backends::openai_compat::OpenAICompatConfig;
    use clankers_router::backends::openai_compat::OpenAICompatProvider;

    let config = OpenAICompatConfig::openai("sk-test".into());
    let provider = OpenAICompatProvider::new(config);
    // Default is_available returns true
    assert!(provider.is_available().await);
}

// ═══════════════════════════════════════════════════════════════════════
// Auth store JSON parsing edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_auth_store_parse_empty_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    std::fs::write(&path, "{}").unwrap();

    let store = AuthStore::load(&path);
    assert!(store.configured_providers().is_empty());
}

#[test]
fn test_auth_store_parse_malformed_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    std::fs::write(&path, "not json at all").unwrap();

    let store = AuthStore::load(&path);
    // Should gracefully return default
    assert!(store.configured_providers().is_empty());
}

#[test]
fn test_auth_store_parse_v1_legacy_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    let json = r#"{
        "anthropic": {
            "access": "legacy-access-token",
            "refresh": "legacy-refresh-token",
            "expires": 9999999999999
        }
    }"#;
    std::fs::write(&path, json).unwrap();

    let store = AuthStore::load(&path);
    let cred = store.active_credential("anthropic").unwrap();
    assert_eq!(cred.token(), "legacy-access-token");
    assert!(cred.is_oauth());
}

// ═══════════════════════════════════════════════════════════════════════
// Router with many providers
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_router_with_many_providers() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");

    let providers = vec![
        ("anthropic", vec!["claude-sonnet-4-5-20250514", "claude-opus-4-20250514"]),
        ("openai", vec!["gpt-4o", "gpt-4o-mini", "o3"]),
        ("deepseek", vec!["deepseek-chat", "deepseek-reasoner"]),
        ("groq", vec!["llama-3.3-70b-versatile"]),
    ];

    for (name, model_ids) in &providers {
        let models: Vec<Model> = model_ids.iter().map(|id| make_model(id, name)).collect();
        router.register_provider(MockProvider::new(name, models));
    }

    assert_eq!(router.provider_names().len(), 4);
    assert_eq!(router.list_models().len(), 8);

    // Route to each provider
    for (name, model_ids) in &providers {
        let events = collect_events(&router, simple_request(model_ids[0])).await.unwrap();
        if let StreamEvent::ContentBlockDelta {
            delta: ContentDelta::TextDelta { text },
            ..
        } = &events[2]
        {
            assert!(text.contains(name), "Expected text from {}, got: {}", name, text);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Error status code tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_error_provider_with_status() {
    let err = Error::provider_with_status(429, "rate limited");
    assert_eq!(err.status_code(), Some(429));
    assert!(err.is_retryable());
    assert!(err.to_string().contains("rate limited"));
}

#[test]
fn test_error_provider_with_status_not_retryable() {
    let err = Error::provider_with_status(401, "unauthorized");
    assert_eq!(err.status_code(), Some(401));
    assert!(!err.is_retryable());
}

#[test]
fn test_error_provider_without_status_falls_back_to_message() {
    let err = Error::Provider {
        message: "HTTP 503: service unavailable".into(),
        status: None,
    };
    assert_eq!(err.status_code(), Some(503));
    assert!(err.is_retryable());
}

#[test]
fn test_error_provider_status_overrides_message() {
    // If both are present, the structured status wins
    let err = Error::Provider {
        message: "HTTP 200: ok".into(),
        status: Some(429),
    };
    assert_eq!(err.status_code(), Some(429));
    assert!(err.is_retryable());
}

// ═══════════════════════════════════════════════════════════════════════
// Retry jitter tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_retry_jitter_produces_varied_values() {
    let config = RetryConfig::default();
    assert!(config.jitter);

    let values: Vec<std::time::Duration> = (0..50).map(|_| config.backoff_for(1)).collect();

    // With jitter, not all values should be identical
    let all_same = values.windows(2).all(|w| w[0] == w[1]);
    assert!(!all_same, "50 jittered values should not all be identical");

    // All should be bounded by max_backoff
    for v in &values {
        assert!(*v <= config.max_backoff);
        assert!(*v > std::time::Duration::ZERO);
    }
}

#[test]
fn test_retry_deterministic_has_no_jitter() {
    let config = RetryConfig::deterministic();
    assert!(!config.jitter);

    let v1 = config.backoff_for(2);
    let v2 = config.backoff_for(2);
    assert_eq!(v1, v2, "deterministic mode should produce identical values");
}

// ═══════════════════════════════════════════════════════════════════════
// Circuit breaker (rate_limits) tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_circuit_breaker_state_transitions() {
    use clankers_router::db::rate_limits::CircuitState;
    use clankers_router::db::rate_limits::RateLimitState;

    let mut state = RateLimitState::new("test", "model");
    assert_eq!(state.effective_circuit(), CircuitState::Closed);
    assert!(state.is_healthy());

    // Error → Open
    state.record_error(429, None);
    assert_eq!(state.circuit, CircuitState::Open);
    assert!(!state.is_healthy()); // Open + cooling down

    // Success → Closed
    state.record_success(100);
    assert_eq!(state.circuit, CircuitState::Closed);
    assert!(state.is_healthy());
}

#[test]
fn test_circuit_breaker_halfopen_on_cooldown_expiry() {
    use clankers_router::db::rate_limits::CircuitState;
    use clankers_router::db::rate_limits::RateLimitState;

    let mut state = RateLimitState::new("test", "model");
    state.record_error(429, Some(0)); // 0-second cooldown → already expired
    assert_eq!(state.circuit, CircuitState::Open);
    // Cooldown expired → effective state is HalfOpen
    assert_eq!(state.effective_circuit(), CircuitState::HalfOpen);
    assert!(state.is_healthy()); // HalfOpen allows probe
}

// ═══════════════════════════════════════════════════════════════════════
// Cache eviction tests (integration level)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_cache_eviction_cleans_expired_entries() {
    use clankers_router::db::RouterDb;
    use clankers_router::db::cache::ResponseCache;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache-test.db");
    let db = RouterDb::open(&path).unwrap();

    // Create entries with -1s TTL (immediately expired)
    let cache = ResponseCache::with_ttl(&db, -1);
    for i in 0..5 {
        let entry = cache.build_entry(&format!("key-{i}"), "anthropic", "sonnet", vec![], 10, 5);
        cache.put(&entry).unwrap();
    }

    // Entries are in DB but expired
    assert_eq!(cache.len().unwrap(), 5);
    // get() returns None for expired
    assert!(cache.get("key-0").unwrap().is_none());

    // Evict
    let removed = cache.evict_expired().unwrap();
    assert_eq!(removed, 5);
    assert_eq!(cache.len().unwrap(), 0);
}

// ── Multi-model dispatch tests ──────────────────────────────────────────

/// Helper: a slow mock provider that sleeps before responding.
struct SlowMockProvider {
    name: String,
    models: Vec<Model>,
    delay_ms: u64,
}

impl SlowMockProvider {
    fn new(name: &str, models: Vec<Model>, delay_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
            models,
            delay_ms,
        })
    }
}

#[async_trait]
impl Provider for SlowMockProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<StreamEvent>,
    ) -> clankers_router::Result<()> {
        tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;

        let _ = tx
            .send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "slow-id".into(),
                    model: request.model.clone(),
                    role: "assistant".into(),
                },
            })
            .await;
        let _ = tx
            .send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta {
                    text: format!("from {}", self.name),
                },
            })
            .await;
        let _ = tx
            .send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    ..Default::default()
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

#[tokio::test]
async fn test_multi_race_picks_first_success() {
    let mut router = Router::new("model-fast");
    // "fast" provider responds after 10ms, "slow" after 200ms
    router.register_provider(SlowMockProvider::new(
        "fast-provider",
        vec![make_model("model-fast", "fast-provider")],
        10,
    ));
    router.register_provider(SlowMockProvider::new(
        "slow-provider",
        vec![make_model("model-slow", "slow-provider")],
        200,
    ));

    let multi_req = MultiRequest {
        request: simple_request("ignored"),
        models: vec!["model-fast".into(), "model-slow".into()],
        strategy: MultiStrategy::Race,
    };

    let result = router.complete_multi(multi_req).await.unwrap();
    assert!(result.winner.is_some(), "race should have a winner");

    let winner = result.winning_response().unwrap();
    assert_eq!(winner.model, "model-fast");
    assert!(winner.is_ok());
    assert!(winner.text().contains("from fast-provider"));
}

#[tokio::test]
async fn test_multi_all_collects_every_response() {
    let mut router = Router::new("model-a");
    router.register_provider(SlowMockProvider::new(
        "provider-a",
        vec![make_model("model-a", "provider-a")],
        10,
    ));
    router.register_provider(SlowMockProvider::new(
        "provider-b",
        vec![make_model("model-b", "provider-b")],
        20,
    ));

    let multi_req = MultiRequest {
        request: simple_request("ignored"),
        models: vec!["model-a".into(), "model-b".into()],
        strategy: MultiStrategy::All,
    };

    let result = router.complete_multi(multi_req).await.unwrap();
    assert!(result.winner.is_none(), "All strategy should have no winner");
    assert_eq!(result.responses.len(), 2);
    assert_eq!(result.successful().len(), 2);

    // Both should have content
    let texts: Vec<String> = result.responses.iter().map(|r| r.text()).collect();
    assert!(texts.iter().any(|t| t.contains("provider-a")));
    assert!(texts.iter().any(|t| t.contains("provider-b")));

    // Total usage is aggregated
    let total = result.total_usage();
    assert_eq!(total.input_tokens, 20); // 10 + 10
    assert_eq!(total.output_tokens, 10); // 5 + 5
}

#[tokio::test]
async fn test_multi_fastest_returns_after_n() {
    let mut router = Router::new("model-a");
    router.register_provider(SlowMockProvider::new(
        "provider-a",
        vec![make_model("model-a", "provider-a")],
        10,
    ));
    router.register_provider(SlowMockProvider::new(
        "provider-b",
        vec![make_model("model-b", "provider-b")],
        20,
    ));
    router.register_provider(SlowMockProvider::new(
        "provider-c",
        vec![make_model("model-c", "provider-c")],
        500,
    ));

    let multi_req = MultiRequest {
        request: simple_request("ignored"),
        models: vec!["model-a".into(), "model-b".into(), "model-c".into()],
        strategy: MultiStrategy::Fastest(2),
    };

    let result = router.complete_multi(multi_req).await.unwrap();
    assert!(result.winner.is_some());
    // At least 2 successful responses
    assert!(result.successful().len() >= 2);
}

#[tokio::test]
async fn test_multi_race_with_one_failing() {
    let mut router = Router::new("model-a");
    router.register_provider(MockProvider::failing(
        "failing-provider",
        vec![make_model("model-fail", "failing-provider")],
        "boom",
    ));
    router.register_provider(SlowMockProvider::new(
        "ok-provider",
        vec![make_model("model-ok", "ok-provider")],
        10,
    ));

    let multi_req = MultiRequest {
        request: simple_request("ignored"),
        models: vec!["model-fail".into(), "model-ok".into()],
        strategy: MultiStrategy::Race,
    };

    let result = router.complete_multi(multi_req).await.unwrap();
    assert!(result.winner.is_some());
    assert_eq!(result.winning_response().unwrap().model, "model-ok");
    assert_eq!(result.failed().len(), 1);
}

#[tokio::test]
async fn test_multi_empty_models_returns_error() {
    let router = Router::new("model-a");
    let multi_req = MultiRequest {
        request: simple_request("ignored"),
        models: vec![],
        strategy: MultiStrategy::All,
    };

    let err = router.complete_multi(multi_req).await.unwrap_err();
    assert!(matches!(err, Error::Config { .. }));
}

#[tokio::test]
async fn test_multi_records_usage_to_db() {
    use clankers_router::db::RouterDb;

    let dir = tempfile::tempdir().unwrap();
    let db = RouterDb::open(&dir.path().join("multi-usage.db")).unwrap();
    let mut router = Router::with_db("model-a", db);
    router.register_provider(SlowMockProvider::new(
        "provider-a",
        vec![make_model("model-a", "provider-a")],
        5,
    ));
    router.register_provider(SlowMockProvider::new(
        "provider-b",
        vec![make_model("model-b", "provider-b")],
        5,
    ));

    let multi_req = MultiRequest {
        request: simple_request("ignored"),
        models: vec!["model-a".into(), "model-b".into()],
        strategy: MultiStrategy::All,
    };

    router.complete_multi(multi_req).await.unwrap();

    let db = router.db().unwrap();
    let today = db.usage().today().unwrap().unwrap();
    // Two models each with 10 input + 5 output
    assert_eq!(today.requests, 2);
    assert_eq!(today.input_tokens, 20);
    assert_eq!(today.output_tokens, 10);

    let log = db.request_log().recent(10).unwrap();
    assert_eq!(log.len(), 2);
}

#[tokio::test]
async fn test_multi_race_streaming() {
    let mut router = Router::new("model-fast");
    router.register_provider(SlowMockProvider::new(
        "fast-provider",
        vec![make_model("model-fast", "fast-provider")],
        10,
    ));
    router.register_provider(SlowMockProvider::new(
        "slow-provider",
        vec![make_model("model-slow", "slow-provider")],
        200,
    ));

    let (tx, mut rx) = mpsc::channel::<TaggedStreamEvent>(64);

    router
        .complete_race_streaming(
            simple_request("ignored"),
            vec!["model-fast".into(), "model-slow".into()],
            tx,
        )
        .await
        .unwrap();

    // Should receive tagged events from the winner
    let mut got_events = false;
    while let Some(tagged) = rx.recv().await {
        assert_eq!(tagged.model, "model-fast");
        assert_eq!(tagged.provider, "fast-provider");
        got_events = true;
    }
    assert!(got_events);
}

// ── Model switch tracking tests ─────────────────────────────────────────

#[test]
fn test_router_switch_model() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(MockProvider::new(
        "anthropic",
        vec![make_model("claude-sonnet-4-5-20250514", "anthropic")],
    ));
    router.register_provider(MockProvider::new(
        "openai",
        vec![make_model("gpt-4o", "openai")],
    ));

    assert_eq!(router.active_model(), "claude-sonnet-4-5-20250514");

    let old = router.switch_model("gpt-4o", ModelSwitchReason::UserRequest);
    assert_eq!(old, Some("claude-sonnet-4-5-20250514".to_string()));
    assert_eq!(router.active_model(), "gpt-4o");
    assert_eq!(router.default_model(), "gpt-4o");
}

#[test]
fn test_router_switch_model_resolves_alias() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(MockProvider::new(
        "anthropic",
        vec![make_model("claude-sonnet-4-5-20250514", "anthropic")],
    ));
    router.register_provider(MockProvider::new(
        "openai",
        vec![make_model("gpt-4o", "openai")],
    ));

    // "sonnet" alias should resolve to the full ID
    let old = router.switch_model("sonnet", ModelSwitchReason::UserRequest);
    assert_eq!(old, None); // same model, noop
    assert_eq!(router.active_model(), "claude-sonnet-4-5-20250514");

    // Switch to a different one via alias
    let old = router.switch_model("4o", ModelSwitchReason::UserRequest);
    assert_eq!(old, Some("claude-sonnet-4-5-20250514".to_string()));
    assert_eq!(router.active_model(), "gpt-4o");
}

#[test]
fn test_router_switch_back() {
    let mut router = Router::new("claude-sonnet-4-5-20250514");
    router.register_provider(MockProvider::new(
        "anthropic",
        vec![make_model("claude-sonnet-4-5-20250514", "anthropic")],
    ));
    router.register_provider(MockProvider::new(
        "openai",
        vec![make_model("gpt-4o", "openai")],
    ));

    router.switch_model("gpt-4o", ModelSwitchReason::UserRequest);
    assert_eq!(router.active_model(), "gpt-4o");

    let old = router.switch_back();
    assert_eq!(old, Some("gpt-4o".to_string()));
    assert_eq!(router.active_model(), "claude-sonnet-4-5-20250514");
}

#[test]
fn test_router_switch_tracker_history() {
    let mut router = Router::new("model-a");
    router.register_provider(MockProvider::new("p", vec![
        make_model("model-a", "p"),
        make_model("model-b", "p"),
        make_model("model-c", "p"),
    ]));

    router.switch_model("model-b", ModelSwitchReason::UserRequest);
    router.switch_model("model-c", ModelSwitchReason::RoleSwitch {
        role: "smol".into(),
    });

    let tracker = router.switch_tracker();
    assert_eq!(tracker.total_switches(), 2);

    let history = tracker.history();
    assert_eq!(history.len(), 3); // initial + 2 switches
    assert_eq!(history[1].from, "model-a");
    assert_eq!(history[1].to, "model-b");
    assert_eq!(history[1].reason, ModelSwitchReason::UserRequest);
    assert_eq!(history[2].from, "model-b");
    assert_eq!(history[2].to, "model-c");
    assert_eq!(
        history[2].reason,
        ModelSwitchReason::RoleSwitch {
            role: "smol".into()
        }
    );
}

#[test]
fn test_router_switch_same_model_noop() {
    let mut router = Router::new("model-a");
    router.register_provider(MockProvider::new("p", vec![make_model("model-a", "p")]));

    let old = router.switch_model("model-a", ModelSwitchReason::UserRequest);
    assert!(old.is_none());
    assert_eq!(router.switch_tracker().total_switches(), 0);
}

#[test]
fn test_tagged_stream_event() {
    let event = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::TextDelta {
            text: "hello".into(),
        },
    };

    let tagged = TaggedStreamEvent::new("gpt-4o", "openai", event.clone());
    assert_eq!(tagged.model, "gpt-4o");
    assert_eq!(tagged.provider, "openai");

    // into_inner unwraps
    let inner = tagged.into_inner();
    assert!(matches!(
        inner,
        StreamEvent::ContentBlockDelta {
            delta: ContentDelta::TextDelta { .. },
            ..
        }
    ));
}

// ── Quorum dispatch tests ───────────────────────────────────────────────

/// A deterministic mock that always returns a specific text.
struct TextMockProvider {
    name: String,
    models: Vec<Model>,
    response_text: String,
}

impl TextMockProvider {
    fn new(name: &str, model_id: &str, text: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
            models: vec![make_model(model_id, name)],
            response_text: text.to_string(),
        })
    }
}

#[async_trait]
impl Provider for TextMockProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<StreamEvent>,
    ) -> clankers_router::Result<()> {
        let _ = tx
            .send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "q-id".into(),
                    model: request.model.clone(),
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
                delta: ContentDelta::TextDelta {
                    text: self.response_text.clone(),
                },
            })
            .await;
        let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
        let _ = tx
            .send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    ..Default::default()
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

#[tokio::test]
async fn test_quorum_majority_cross_model() {
    let mut router = Router::new("model-a");
    // Two agree ("42"), one disagrees (completely different text)
    router.register_provider(TextMockProvider::new("p-a", "model-a", "the answer is 42"));
    router.register_provider(TextMockProvider::new("p-b", "model-b", "the answer is 42"));
    router.register_provider(TextMockProvider::new("p-c", "model-c", "I cannot determine the result from the given information"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::models(["model-a", "model-b", "model-c"]),
        consensus: ConsensusStrategy::Majority {
            similarity_threshold: 0.7,
        },
        min_agree: 2,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    assert_eq!(result.all_responses.len(), 3);
    assert_eq!(result.agreeing_count, 2);
    assert!(result.quorum_met);
    assert!(result.winner.text().contains("42"));
    assert!(result.agreement > 0.5);
}

#[tokio::test]
async fn test_quorum_unanimous_all_agree() {
    let mut router = Router::new("model-a");
    router.register_provider(TextMockProvider::new("p-a", "model-a", "yes"));
    router.register_provider(TextMockProvider::new("p-b", "model-b", "yes"));
    router.register_provider(TextMockProvider::new("p-c", "model-c", "yes"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::models(["model-a", "model-b", "model-c"]),
        consensus: ConsensusStrategy::Unanimous {
            similarity_threshold: 0.8,
        },
        min_agree: 3,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    assert_eq!(result.agreeing_count, 3);
    assert!(result.quorum_met);
    assert!((result.agreement - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_quorum_unanimous_broken() {
    let mut router = Router::new("model-a");
    router.register_provider(TextMockProvider::new("p-a", "model-a", "yes definitely"));
    router.register_provider(TextMockProvider::new("p-b", "model-b", "no absolutely not"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::models(["model-a", "model-b"]),
        consensus: ConsensusStrategy::Unanimous {
            similarity_threshold: 0.8,
        },
        min_agree: 2,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    assert_eq!(result.agreeing_count, 1); // unanimity broken
    assert!(!result.quorum_met);
}

#[tokio::test]
async fn test_quorum_replicas_same_model() {
    let mut router = Router::new("model-a");
    // Same provider, same model, queried 3 times
    router.register_provider(TextMockProvider::new("p-a", "model-a", "the answer is 42"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::replicas("model-a", 3),
        consensus: ConsensusStrategy::Majority {
            similarity_threshold: 0.7,
        },
        min_agree: 2,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    assert_eq!(result.all_responses.len(), 3);
    // All replicas return the same text
    assert_eq!(result.agreeing_count, 3);
    assert!(result.quorum_met);
}

#[tokio::test]
async fn test_quorum_collect_no_consensus() {
    let mut router = Router::new("model-a");
    router.register_provider(TextMockProvider::new("p-a", "model-a", "alpha"));
    router.register_provider(TextMockProvider::new("p-b", "model-b", "beta"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::models(["model-a", "model-b"]),
        consensus: ConsensusStrategy::Collect,
        min_agree: 0,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    assert_eq!(result.all_responses.len(), 2);
    // Collect has no winner logic; agreement is 0
    assert!((result.agreement - 0.0).abs() < f64::EPSILON);
    // quorum_met is true because min_agree=0 and agreeing_count >= 0
    assert!(result.quorum_met);
}

#[tokio::test]
async fn test_quorum_with_failures() {
    let mut router = Router::new("model-a");
    router.register_provider(TextMockProvider::new("p-a", "model-a", "the answer is 42"));
    router.register_provider(MockProvider::failing(
        "p-fail",
        vec![make_model("model-fail", "p-fail")],
        "crash",
    ));
    router.register_provider(TextMockProvider::new("p-c", "model-c", "the answer is 42"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::models(["model-a", "model-fail", "model-c"]),
        consensus: ConsensusStrategy::Majority {
            similarity_threshold: 0.7,
        },
        min_agree: 2,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    assert_eq!(result.all_responses.len(), 3);
    // 2 succeed with matching text, 1 failed
    assert_eq!(result.agreeing_count, 2);
    assert!(result.quorum_met);
    assert!(result.winner.is_ok());
}

#[tokio::test]
async fn test_quorum_empty_targets_error() {
    let router = Router::new("model-a");
    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget { slots: vec![] },
        consensus: ConsensusStrategy::Collect,
        min_agree: 0,
    };

    let err = router.complete_quorum(quorum_req).await.unwrap_err();
    assert!(matches!(err, Error::Config { .. }));
}

#[tokio::test]
async fn test_quorum_temperature_spread() {
    // Verify the temperature spread builder works end-to-end
    let target = QuorumTarget::replicas("model-a", 5).with_temperature_spread(0.0, 1.0);
    assert_eq!(target.slots.len(), 5);
    assert!((target.slots[0].temperature.unwrap() - 0.0).abs() < f64::EPSILON);
    assert!((target.slots[2].temperature.unwrap() - 0.5).abs() < f64::EPSILON);
    assert!((target.slots[4].temperature.unwrap() - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_quorum_total_usage() {
    let mut router = Router::new("model-a");
    router.register_provider(TextMockProvider::new("p-a", "model-a", "hello"));
    router.register_provider(TextMockProvider::new("p-b", "model-b", "hello"));

    let quorum_req = QuorumRequest {
        request: simple_request("ignored"),
        targets: QuorumTarget::models(["model-a", "model-b"]),
        consensus: ConsensusStrategy::Collect,
        min_agree: 0,
    };

    let result = router.complete_quorum(quorum_req).await.unwrap();
    // Each mock returns 10 input + 5 output
    assert_eq!(result.total_usage.input_tokens, 20);
    assert_eq!(result.total_usage.output_tokens, 10);
}
