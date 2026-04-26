//! LLM provider abstraction
//!
//! This module defines the core provider trait and types for interacting with
//! language model APIs. It supports streaming responses, multiple content types,
//! tool use, and extended thinking modes.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::error::Result;

pub mod error;
pub use error_classifier::ClassifiedError;
pub use error_classifier::FailoverReason;
pub use error_classifier::classify_api_error;
pub use error_classifier::classify_transport_error;
pub use error_classifier::recovery_hints;

pub mod anthropic;
pub mod auth;
pub mod credential_manager;
pub mod discovery;
pub mod error_classifier;
pub mod message;
pub mod openai_codex;
/// Model registry — re-exported from `clanker-router`.
pub use clanker_router::registry;
/// Retry logic — re-exported from `clanker-router`.
pub use clanker_router::retry;
pub mod router;
pub mod rpc_provider;
pub mod streaming;

/// Provider trait for LLM API implementations.
///
/// Each provider (Anthropic, OpenAI, etc.) implements this trait to expose
/// a unified interface for model completion requests.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a completion request and stream the response via the provided channel.
    ///
    /// The provider should send [`StreamEvent`](streaming::StreamEvent) items as they arrive,
    /// and close the channel when the response is complete.
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<streaming::StreamEvent>) -> Result<()>;

    /// Returns the list of models supported by this provider.
    fn models(&self) -> &[Model];

    /// Returns the provider's unique name (e.g., "anthropic", "openai").
    fn name(&self) -> &str;

    /// Reload credentials from disk (e.g. after a fresh `/login`).
    ///
    /// Default implementation is a no-op. Providers with a `CredentialManager`
    /// override this to re-read the auth store and update in-memory state.
    async fn reload_credentials(&self) {}
}

// Re-export Model from clanker-router (canonical definition)
pub use clanker_router::Model;

/// Request for a model completion.
///
/// Contains all parameters needed to invoke a model, including messages,
/// system prompt, sampling parameters, and tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model identifier to use
    pub model: String,

    /// Conversation messages (user, assistant, tool results, etc.)
    pub messages: Vec<message::AgentMessage>,

    /// System prompt (provider-dependent placement)
    pub system_prompt: Option<String>,

    /// Maximum tokens to generate (None = model default)
    pub max_tokens: Option<usize>,

    /// Sampling temperature (typically 0.0-1.0)
    pub temperature: Option<f64>,

    /// Available tools for the model to call
    pub tools: Vec<ToolDefinition>,

    /// Extended thinking configuration (if supported)
    pub thinking: Option<ThinkingConfig>,

    /// Disable prompt caching (skip cache_control breakpoints)
    #[serde(default)]
    pub no_cache: bool,

    /// Cache TTL override (e.g. "1h" for 1-hour cache). None = default 5m ephemeral.
    #[serde(default)]
    pub cache_ttl: Option<String>,

    /// Extra provider-specific parameters passed through verbatim.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_params: HashMap<String, serde_json::Value>,
}

// Re-export ThinkingConfig from clanker-message (canonical definition)
pub use clanker_message::ThinkingConfig;
// Re-export ToolDefinition from clanker-message (canonical definition)
pub use clanker_message::ToolDefinition;
// ThinkingLevel re-exported from clanker-tui-types (canonical definition).
pub use clanker_tui_types::ThinkingLevel;

/// Extension: convert ThinkingLevel to provider-specific ThinkingConfig.
pub fn thinking_level_to_config(level: ThinkingLevel) -> Option<ThinkingConfig> {
    if level.is_enabled() {
        Some(ThinkingConfig {
            enabled: true,
            budget_tokens: level.budget_tokens().map(|tokens| tokens as usize),
        })
    } else {
        None
    }
}

// Re-export Usage from clanker-message and Cost from clanker-router.
pub use clanker_message::Usage;
pub use clanker_router::provider::Cost;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use serde_json::Value;
    use serde_json::json;

    use super::CompletionRequest;
    use super::ThinkingConfig;
    use super::ToolDefinition;
    use super::Usage;
    use crate::message::AgentMessage;
    use crate::message::Content;
    use crate::message::MessageId;
    use crate::message::UserMessage;
    use crate::streaming::ContentDelta;
    use crate::streaming::MessageMetadata;
    use crate::streaming::StreamDelta;

    const TEST_MAX_TOKENS: usize = 256;
    const TEST_TEMPERATURE: f64 = 0.1;
    const TEST_THINKING_BUDGET_TOKENS: usize = 1024;
    const TEST_INPUT_TOKENS: usize = 11;
    const TEST_OUTPUT_TOKENS: usize = 7;
    const TEST_CACHE_CREATION_TOKENS: usize = 3;
    const TEST_CACHE_READ_TOKENS: usize = 5;

    const MAX_STREAM_DELTA_SCAN_FILES: usize = 512;
    const STREAM_DELTA_TYPE_NAME: &str = "StreamDelta";

    fn stream_delta_definition_needles() -> [String; 3] {
        [
            format!("pub type {STREAM_DELTA_TYPE_NAME}"),
            format!("pub struct {STREAM_DELTA_TYPE_NAME}"),
            format!("pub enum {STREAM_DELTA_TYPE_NAME}"),
        ]
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn assert_completion_request_inventory(path: &str, expected_count: usize) {
        let source = std::fs::read_to_string(workspace_root().join(path))
            .unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        let occurrences: Vec<usize> = source.match_indices("CompletionRequest {").map(|(idx, _)| idx).collect();
        assert_eq!(occurrences.len(), expected_count, "unexpected CompletionRequest constructor count in {path}");

        for start in occurrences {
            let snippet = source[start..].lines().take(40).collect::<Vec<_>>().join("\n");
            assert!(
                snippet.contains("extra_params"),
                "CompletionRequest constructor missing extra_params in {path}:\n{snippet}"
            );
        }
    }

    fn provider_request(extra_params: HashMap<String, Value>) -> CompletionRequest {
        CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![AgentMessage::User(UserMessage {
                id: MessageId::new("test-user"),
                content: vec![Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: chrono::Utc::now(),
            })],
            system_prompt: Some("Be helpful".to_string()),
            max_tokens: Some(TEST_MAX_TOKENS),
            temperature: Some(TEST_TEMPERATURE),
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({"type": "object"}),
            }],
            thinking: Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(TEST_THINKING_BUDGET_TOKENS),
            }),
            no_cache: true,
            cache_ttl: Some("1h".to_string()),
            extra_params,
        }
    }

    fn router_request(extra_params: HashMap<String, Value>) -> clanker_router::CompletionRequest {
        clanker_router::CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![json!({
                "role": "user",
                "content": [{"type": "text", "text": "hello"}],
            })],
            system_prompt: Some("Be helpful".to_string()),
            max_tokens: Some(TEST_MAX_TOKENS),
            temperature: Some(TEST_TEMPERATURE),
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({"type": "object"}),
            }],
            thinking: Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(TEST_THINKING_BUDGET_TOKENS),
            }),
            no_cache: true,
            cache_ttl: Some("1h".to_string()),
            extra_params,
        }
    }

    fn assert_same_type<T>(_left: &T, _right: &T) {}

    fn rust_files_under(root: PathBuf) -> Vec<PathBuf> {
        let mut stack = vec![root];
        let mut files = Vec::new();
        while let Some(path) = stack.pop() {
            let metadata = std::fs::metadata(&path)
                .unwrap_or_else(|error| panic!("failed to read metadata for {}: {error}", path.display()));
            if metadata.is_dir() {
                for entry in std::fs::read_dir(&path)
                    .unwrap_or_else(|error| panic!("failed to read dir {}: {error}", path.display()))
                {
                    let entry = entry.expect("directory entry should be readable");
                    stack.push(entry.path());
                    assert!(
                        stack.len() + files.len() <= MAX_STREAM_DELTA_SCAN_FILES,
                        "StreamDelta source scan exceeded bounded file count"
                    );
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
        files
    }

    fn assert_no_stream_delta_definition_under(path: &str) {
        for source_path in rust_files_under(workspace_root().join(path)) {
            let source = std::fs::read_to_string(&source_path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
            for needle in stream_delta_definition_needles() {
                assert!(
                    !source.contains(&needle),
                    "{} must not define independent StreamDelta via {needle}",
                    source_path.display()
                );
            }
        }
    }

    fn expected_shared_request_shape() -> Value {
        json!({
            "model": "test-model",
            "system_prompt": "Be helpful",
            "max_tokens": TEST_MAX_TOKENS,
            "temperature": TEST_TEMPERATURE,
            "tools": [{
                "name": "read",
                "description": "Read a file",
                "input_schema": {"type": "object"},
            }],
            "thinking": {
                "enabled": true,
                "budget_tokens": TEST_THINKING_BUDGET_TOKENS,
            },
            "no_cache": true,
            "cache_ttl": "1h",
            "extra_params": {
                "_session_id": "session-parity-1",
                "verbosity": "medium",
            },
        })
    }

    fn shared_field_projection(value: &Value) -> Value {
        fn field(value: &Value, name: &str) -> Value {
            value.get(name).cloned().unwrap_or(Value::Null)
        }

        json!({
            "model": field(value, "model"),
            "system_prompt": field(value, "system_prompt"),
            "max_tokens": field(value, "max_tokens"),
            "temperature": field(value, "temperature"),
            "tools": field(value, "tools"),
            "thinking": field(value, "thinking"),
            "no_cache": field(value, "no_cache"),
            "cache_ttl": field(value, "cache_ttl"),
            "extra_params": field(value, "extra_params"),
        })
    }

    #[test]
    fn router_and_provider_contract_paths_resolve_to_message_types() {
        let canonical_usage = clanker_message::Usage {
            input_tokens: TEST_INPUT_TOKENS,
            output_tokens: TEST_OUTPUT_TOKENS,
            cache_creation_input_tokens: TEST_CACHE_CREATION_TOKENS,
            cache_read_input_tokens: TEST_CACHE_READ_TOKENS,
        };
        let router_provider_usage: clanker_router::provider::Usage = canonical_usage.clone();
        let router_root_usage: clanker_router::Usage = canonical_usage.clone();
        let provider_usage: Usage = canonical_usage.clone();
        assert_same_type(&canonical_usage, &router_provider_usage);
        assert_same_type(&canonical_usage, &router_root_usage);
        assert_same_type(&canonical_usage, &provider_usage);

        let canonical_tool = clanker_message::ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object"}),
        };
        let router_tool: clanker_router::provider::ToolDefinition = canonical_tool.clone();
        let provider_tool: ToolDefinition = canonical_tool.clone();
        assert_same_type(&canonical_tool, &router_tool);
        assert_same_type(&canonical_tool, &provider_tool);

        let canonical_thinking = clanker_message::ThinkingConfig {
            enabled: true,
            budget_tokens: Some(TEST_THINKING_BUDGET_TOKENS),
        };
        let router_provider_thinking: clanker_router::provider::ThinkingConfig = canonical_thinking.clone();
        let router_root_thinking: clanker_router::ThinkingConfig = canonical_thinking.clone();
        let provider_thinking: ThinkingConfig = canonical_thinking.clone();
        assert_same_type(&canonical_thinking, &router_provider_thinking);
        assert_same_type(&canonical_thinking, &router_root_thinking);
        assert_same_type(&canonical_thinking, &provider_thinking);

        let canonical_metadata = clanker_message::MessageMetadata {
            id: "msg_1".to_string(),
            model: "test-model".to_string(),
            role: "assistant".to_string(),
        };
        let router_metadata: clanker_router::streaming::MessageMetadata = canonical_metadata.clone();
        let provider_metadata: MessageMetadata = canonical_metadata.clone();
        assert_same_type(&canonical_metadata, &router_metadata);
        assert_same_type(&canonical_metadata, &provider_metadata);

        let canonical_delta = clanker_message::ContentDelta::TextDelta {
            text: "hello".to_string(),
        };
        let router_delta: clanker_router::streaming::ContentDelta = canonical_delta.clone();
        let provider_delta: ContentDelta = canonical_delta.clone();
        let stream_delta: clanker_message::StreamDelta = canonical_delta.clone();
        assert_same_type(&canonical_delta, &router_delta);
        assert_same_type(&canonical_delta, &provider_delta);
        assert_same_type(&canonical_delta, &stream_delta);
    }

    #[test]
    fn router_and_provider_do_not_define_independent_stream_delta() {
        assert_no_stream_delta_definition_under("crates/clanker-router/src");
        assert_no_stream_delta_definition_under("crates/clankers-provider/src");
    }

    #[test]
    fn moved_contract_json_shapes_match_inline_golden() {
        let usage = Usage {
            input_tokens: TEST_INPUT_TOKENS,
            output_tokens: TEST_OUTPUT_TOKENS,
            cache_creation_input_tokens: TEST_CACHE_CREATION_TOKENS,
            cache_read_input_tokens: TEST_CACHE_READ_TOKENS,
        };
        assert_eq!(
            serde_json::to_value(usage).expect("usage should serialize"),
            json!({
                "input_tokens": TEST_INPUT_TOKENS,
                "output_tokens": TEST_OUTPUT_TOKENS,
                "cache_creation_input_tokens": TEST_CACHE_CREATION_TOKENS,
                "cache_read_input_tokens": TEST_CACHE_READ_TOKENS,
            })
        );

        let tool = ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object"}),
        };
        assert_eq!(
            serde_json::to_value(tool).expect("tool should serialize"),
            json!({
                "name": "read",
                "description": "Read a file",
                "input_schema": {"type": "object"},
            })
        );

        let thinking = ThinkingConfig {
            enabled: true,
            budget_tokens: Some(TEST_THINKING_BUDGET_TOKENS),
        };
        assert_eq!(
            serde_json::to_value(thinking).expect("thinking config should serialize"),
            json!({
                "enabled": true,
                "budget_tokens": TEST_THINKING_BUDGET_TOKENS,
            })
        );

        let metadata = MessageMetadata {
            id: "msg_1".to_string(),
            model: "test-model".to_string(),
            role: "assistant".to_string(),
        };
        assert_eq!(
            serde_json::to_value(metadata).expect("message metadata should serialize"),
            json!({
                "id": "msg_1",
                "model": "test-model",
                "role": "assistant",
            })
        );

        let delta = ContentDelta::InputJsonDelta {
            partial_json: "{\"path\"".to_string(),
        };
        assert_eq!(
            serde_json::to_value(delta).expect("content delta should serialize"),
            json!({
                "type": "InputJsonDelta",
                "partial_json": "{\"path\"",
            })
        );

        let stream_delta: StreamDelta = ContentDelta::TextDelta {
            text: "hello".to_string(),
        };
        assert_eq!(
            serde_json::to_value(stream_delta).expect("stream delta should serialize"),
            json!({
                "type": "TextDelta",
                "text": "hello",
            })
        );
    }

    #[test]
    fn provider_request_shared_fields_match_inline_golden() {
        let extra_params = HashMap::from([
            ("_session_id".to_string(), json!("session-parity-1")),
            ("verbosity".to_string(), json!("medium")),
        ]);
        let provider_json =
            serde_json::to_value(provider_request(extra_params)).expect("provider request should serialize");

        assert_eq!(shared_field_projection(&provider_json), expected_shared_request_shape());
    }

    #[test]
    fn completion_request_constructor_inventory_requires_extra_params() {
        let inventory = [
            ("crates/clankers-agent/src/turn/execution.rs", 1usize),
            ("src/modes/agent_task.rs", 1usize),
            ("src/worktree/llm_resolver.rs", 1usize),
            ("crates/clankers-provider/src/router.rs", 21usize),
            ("crates/clankers-provider/src/rpc_provider.rs", 4usize),
        ];

        for (path, count) in inventory {
            assert_completion_request_inventory(path, count);
        }
    }

    #[test]
    fn provider_and_router_request_shared_schema_fields_stay_in_parity() {
        let extra_params = HashMap::from([
            ("_session_id".to_string(), json!("session-parity-1")),
            ("verbosity".to_string(), json!("medium")),
        ]);
        let provider_json =
            serde_json::to_value(provider_request(extra_params.clone())).expect("provider request should serialize");
        let router_json = serde_json::to_value(router_request(extra_params)).expect("router request should serialize");

        assert_eq!(
            shared_field_projection(&provider_json),
            shared_field_projection(&router_json),
            "provider/router CompletionRequest shared fields drifted"
        );
    }

    #[test]
    fn provider_and_router_request_omit_empty_extra_params_consistently() {
        let provider_json =
            serde_json::to_value(provider_request(HashMap::new())).expect("provider request should serialize");
        let router_json =
            serde_json::to_value(router_request(HashMap::new())).expect("router request should serialize");

        assert!(provider_json.get("extra_params").is_none());
        assert!(router_json.get("extra_params").is_none());
        assert_eq!(
            shared_field_projection(&provider_json),
            shared_field_projection(&router_json),
            "provider/router empty extra_params serialization drifted"
        );
    }
}
