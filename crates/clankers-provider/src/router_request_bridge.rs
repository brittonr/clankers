//! Single clankers-provider owned bridge into `clanker_router::CompletionRequest`.
//!
//! Provider/router ownership rule: this module is the only clankers-provider
//! place that translates clankers-native `AgentMessage` history into the
//! router's provider-native JSON message surface. Backends behind
//! `clanker-router` still own final provider HTTP body construction.

use std::collections::HashMap;

use chrono::Utc;
use clanker_message::transcript::AgentMessage;
use clanker_message::transcript::AssistantMessage;
use clanker_message::Content;
use clanker_message::ImageSource;
use clanker_message::transcript::MessageId;
use clanker_message::StopReason;
use clanker_message::transcript::ToolResultMessage;
use clanker_message::transcript::UserMessage;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::CompletionRequest;
use clanker_message::Usage;

const BRANCH_SUMMARY_MESSAGE_PREFIX: &str = "Branch summary";
const COMPACTION_SUMMARY_MESSAGE_PREFIX: &str = "Compaction summary";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequestBridgeInput {
    pub model: String,
    pub messages: Vec<CompletionRequestBridgeMessage>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub tools: Vec<clanker_message::ToolDefinition>,
    pub thinking: Option<clanker_message::ThinkingConfig>,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
    pub extra_params: HashMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionRequestBridgeMessageRole {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequestBridgeMessage {
    pub role: CompletionRequestBridgeMessageRole,
    pub content: Vec<Content>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub is_error: bool,
}

#[must_use]
pub fn completion_request_from_bridge_input(input: CompletionRequestBridgeInput) -> CompletionRequest {
    let messages = bridge_messages_to_agent_messages(&input.model, input.messages);
    CompletionRequest {
        model: input.model,
        messages,
        system_prompt: input.system_prompt,
        max_tokens: input.max_tokens,
        temperature: input.temperature,
        tools: input.tools,
        thinking: input.thinking,
        no_cache: input.no_cache,
        cache_ttl: input.cache_ttl,
        extra_params: input.extra_params,
    }
}

fn bridge_messages_to_agent_messages(
    default_model: &str,
    messages: Vec<CompletionRequestBridgeMessage>,
) -> Vec<AgentMessage> {
    messages
        .into_iter()
        .enumerate()
        .map(|(index, message)| bridge_message_to_agent_message(default_model, index, message))
        .collect()
}

fn bridge_message_to_agent_message(
    default_model: &str,
    index: usize,
    message: CompletionRequestBridgeMessage,
) -> AgentMessage {
    let id = MessageId::new(message.id.unwrap_or_else(|| format!("runtime-provider-{index}")));
    let timestamp = Utc::now();
    match message.role {
        CompletionRequestBridgeMessageRole::User | CompletionRequestBridgeMessageRole::System => {
            AgentMessage::User(UserMessage {
                id,
                content: message.content,
                timestamp,
            })
        }
        CompletionRequestBridgeMessageRole::Assistant => AgentMessage::Assistant(AssistantMessage {
            id,
            content: message.content,
            model: message.model.unwrap_or_else(|| default_model.to_string()),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            timestamp,
        }),
        CompletionRequestBridgeMessageRole::Tool => {
            let call_id = message
                .call_id
                .or_else(|| first_tool_result_id(&message.content))
                .unwrap_or_else(|| format!("runtime-provider-tool-{index}"));
            AgentMessage::ToolResult(ToolResultMessage {
                id,
                call_id,
                tool_name: message.tool_name.unwrap_or_else(|| "tool".to_string()),
                content: message.content,
                is_error: message.is_error,
                details: None,
                timestamp,
            })
        }
    }
}

fn first_tool_result_id(content: &[Content]) -> Option<String> {
    content.iter().find_map(|block| match block {
        Content::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
        _ => None,
    })
}

pub(crate) fn build_router_request(request: CompletionRequest) -> clanker_router::CompletionRequest {
    clanker_router::CompletionRequest {
        model: request.model,
        messages: messages_to_router_json(&request.messages),
        system_prompt: request.system_prompt,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tools: request.tools,
        thinking: request.thinking,
        no_cache: request.no_cache,
        cache_ttl: request.cache_ttl,
        extra_params: request.extra_params,
    }
}

pub(crate) fn compute_router_cache_key_from_request_projection(request: CompletionRequest) -> String {
    let router_request = build_router_request(request);
    let input = clanker_router::db::cache::CacheKeyInput {
        model: &router_request.model,
        system_prompt: router_request.system_prompt.as_deref(),
        messages: &router_request.messages,
        tools: &router_request.tools,
        temperature: router_request.temperature,
        thinking_enabled: router_request.thinking.as_ref().is_some_and(|thinking| thinking.enabled),
    };
    input.compute_key()
}

fn messages_to_router_json(messages: &[AgentMessage]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for message in messages {
        match message {
            AgentMessage::User(user) => {
                let content: Vec<serde_json::Value> = user.content.iter().map(content_to_router_json).collect();
                out.push(json!({"role": "user", "content": content}));
            }
            AgentMessage::Assistant(assistant) => {
                let content: Vec<serde_json::Value> = assistant.content.iter().map(content_to_router_json).collect();
                out.push(json!({"role": "assistant", "content": content}));
            }
            AgentMessage::ToolResult(result) => {
                let content_blocks: Vec<serde_json::Value> =
                    result.content.iter().map(content_to_router_json).collect();
                let mut block = json!({
                    "type": "tool_result",
                    "tool_use_id": result.call_id,
                    "content": content_blocks,
                });
                if result.is_error {
                    block["is_error"] = json!(true);
                }
                out.push(json!({"role": "user", "content": [block]}));
            }
            AgentMessage::BranchSummary(summary) => {
                out.push(summary_to_router_json(BRANCH_SUMMARY_MESSAGE_PREFIX, &summary.summary));
            }
            AgentMessage::CompactionSummary(summary) => {
                out.push(summary_to_router_json(COMPACTION_SUMMARY_MESSAGE_PREFIX, &summary.summary));
            }
            AgentMessage::BashExecution(_) | AgentMessage::Custom(_) => {}
        }
    }
    out
}

fn summary_to_router_json(prefix: &str, summary: &str) -> serde_json::Value {
    json!({
        "role": "user",
        "content": [{"type": "text", "text": format!("[{prefix}]\n{summary}")}],
    })
}

fn content_to_router_json(content: &Content) -> serde_json::Value {
    match content {
        Content::Text { text } => json!({"type": "text", "text": text}),
        Content::Image { source } => match source {
            ImageSource::Base64 { media_type, data } => json!({
                "type": "image",
                "source": {"type": "base64", "media_type": media_type, "data": data}
            }),
            ImageSource::Url { url } => json!({"type": "text", "text": format!("[Image URL: {}]", url)}),
        },
        Content::Thinking { thinking, signature } => {
            json!({"type": "thinking", "thinking": thinking, "signature": signature})
        }
        Content::ToolUse { id, name, input } => {
            json!({"type": "tool_use", "id": id, "name": name, "input": input})
        }
        Content::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let blocks: Vec<serde_json::Value> = content.iter().map(content_to_router_json).collect();
            let mut value = json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": blocks});
            if let Some(true) = is_error {
                value["is_error"] = json!(true);
            }
            value
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use clanker_message::transcript::AssistantMessage;
    use clanker_message::transcript::BranchSummaryMessage;
    use clanker_message::transcript::CompactionSummaryMessage;
    use clanker_message::transcript::MessageId;
    use clanker_message::StopReason;
    use clanker_message::transcript::ToolResultMessage;
    use clanker_message::transcript::UserMessage;
    use serde_json::json;

    use super::*;

    fn request(messages: Vec<AgentMessage>) -> CompletionRequest {
        CompletionRequest {
            model: "openai-codex/gpt-5.3-codex".to_string(),
            messages,
            system_prompt: Some("Be helpful".to_string()),
            max_tokens: Some(128),
            temperature: Some(0.2),
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: Some("1h".to_string()),
            extra_params: HashMap::from([("_session_id".to_string(), json!("session-router-bridge"))]),
        }
    }

    #[test]
    fn builds_router_request_with_provider_native_message_json() {
        let router_request = build_router_request(request(vec![
            AgentMessage::User(UserMessage {
                id: MessageId::new("user-1"),
                content: vec![Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: Utc::now(),
            }),
            AgentMessage::Assistant(AssistantMessage {
                id: MessageId::new("assistant-1"),
                content: vec![Content::ToolUse {
                    id: "call_1:item_1".to_string(),
                    name: "read_file".to_string(),
                    input: json!({"path":"README.md"}),
                }],
                model: "test-model".to_string(),
                usage: clanker_message::Usage::default(),
                stop_reason: StopReason::ToolUse,
                timestamp: Utc::now(),
            }),
            AgentMessage::ToolResult(ToolResultMessage {
                id: MessageId::new("tool-result-1"),
                call_id: "call_1:item_1".to_string(),
                tool_name: "read_file".to_string(),
                content: vec![Content::Text {
                    text: "contents".to_string(),
                }],
                is_error: false,
                details: None,
                timestamp: Utc::now(),
            }),
        ]));

        assert_eq!(router_request.extra_params.get("_session_id"), Some(&json!("session-router-bridge")));
        assert_eq!(
            router_request.messages[0],
            json!({
                "role": "user",
                "content": [{"type": "text", "text": "hello"}],
            })
        );
        assert_eq!(router_request.messages[1]["role"], json!("assistant"));
        assert_eq!(router_request.messages[1]["content"][0]["type"], json!("tool_use"));
        assert_eq!(router_request.messages[2]["role"], json!("user"));
        assert_eq!(router_request.messages[2]["content"][0]["type"], json!("tool_result"));
    }

    #[test]
    fn preserves_branch_and_compaction_summaries_as_user_context() {
        let router_request = build_router_request(request(vec![
            AgentMessage::BranchSummary(BranchSummaryMessage {
                id: MessageId::new("branch-1"),
                from_id: MessageId::new("user-1"),
                summary: "branch state".to_string(),
                timestamp: Utc::now(),
            }),
            AgentMessage::CompactionSummary(CompactionSummaryMessage {
                id: MessageId::new("compaction-1"),
                compacted_ids: vec![MessageId::new("user-1")],
                summary: "compact state".to_string(),
                tokens_saved: 42,
                timestamp: Utc::now(),
            }),
        ]));

        assert_eq!(router_request.messages.len(), 2);
        assert_eq!(router_request.messages[0]["role"], json!("user"));
        assert_eq!(router_request.messages[0]["content"][0]["text"], json!("[Branch summary]\nbranch state"));
        assert_eq!(router_request.messages[1]["content"][0]["text"], json!("[Compaction summary]\ncompact state"));
    }

    #[test]
    fn cache_key_uses_router_message_projection_literal() {
        let request = request(vec![
            AgentMessage::User(UserMessage {
                id: MessageId::new("user-cache"),
                content: vec![Content::Text {
                    text: "cache me".to_string(),
                }],
                timestamp: Utc::now(),
            }),
            AgentMessage::BranchSummary(BranchSummaryMessage {
                id: MessageId::new("branch-cache"),
                from_id: MessageId::new("user-cache"),
                summary: "branch cache state".to_string(),
                timestamp: Utc::now(),
            }),
        ]);
        let actual = compute_router_cache_key_from_request_projection(request);
        let expected_messages = vec![
            json!({
                "role": "user",
                "content": [{"type": "text", "text": "cache me"}],
            }),
            json!({
                "role": "user",
                "content": [{"type": "text", "text": "[Branch summary]\nbranch cache state"}],
            }),
        ];
        let expected_input = clanker_router::db::cache::CacheKeyInput {
            model: "openai-codex/gpt-5.3-codex",
            system_prompt: Some("Be helpful"),
            messages: &expected_messages,
            tools: &[],
            temperature: Some(0.2),
            thinking_enabled: false,
        };

        assert_eq!(actual, expected_input.compute_key());
    }
}
