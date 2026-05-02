//! Deterministic fake provider for local/e2e testing.
//!
//! Enabled with `CLANKERS_FAKE_PROVIDER=1`. It streams through the same
//! provider contract as real backends, but never performs network or auth work.

use std::sync::OnceLock;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

use crate::CompletionRequest;
use crate::Model;
use crate::Provider;
use crate::Usage;
use crate::error::Result;
use crate::message::AgentMessage;
use crate::message::Content;
use crate::streaming::ContentDelta;
use crate::streaming::MessageMetadata;
use crate::streaming::StreamEvent;

pub const ENV_FAKE_PROVIDER: &str = "CLANKERS_FAKE_PROVIDER";
const FAKE_MODEL_ID: &str = "clankers-fake";

/// Returns true when deterministic fake-provider mode is requested.
pub fn fake_provider_enabled() -> bool {
    std::env::var(ENV_FAKE_PROVIDER)
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

/// Provider used by the e2e harness to avoid live credentials and model drift.
pub struct FakeProvider;

impl FakeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FakeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for FakeProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let action = choose_action(&request);
        match action {
            FakeAction::Text(text) => send_text_response(&tx, &request.model, &text).await,
            FakeAction::Tool { call_id, name, input } => {
                send_tool_response(&tx, &request.model, &call_id, &name, input).await
            }
        }
        Ok(())
    }

    fn models(&self) -> &[Model] {
        static MODELS: OnceLock<Vec<Model>> = OnceLock::new();
        MODELS.get_or_init(|| {
            vec![Model {
                id: FAKE_MODEL_ID.to_string(),
                name: "Clankers Fake".to_string(),
                provider: "fake".to_string(),
                max_input_tokens: 128_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_images: false,
                supports_tools: true,
                input_cost_per_mtok: Some(0.0),
                output_cost_per_mtok: Some(0.0),
            }]
        })
    }

    fn name(&self) -> &str {
        "fake"
    }
}

enum FakeAction {
    Text(String),
    Tool {
        call_id: String,
        name: String,
        input: Value,
    },
}

fn choose_action(request: &CompletionRequest) -> FakeAction {
    let prompt = latest_user_text(&request.messages).to_ascii_lowercase();
    let tool_results = tool_results(&request.messages);

    if prompt.contains("write tool") && prompt.contains("edit tool") {
        return choose_write_edit_action(&prompt, &tool_results);
    }

    if let Some((_, tool_name)) = tool_results.last() {
        return FakeAction::Text(final_text_for_tool(&prompt, tool_name).to_string());
    }

    if prompt.contains("bash tool") {
        return tool_call("fake_bash_1", "bash", json!({"command": "echo CLANKERS_TOOL_TEST_OK"}));
    }
    if prompt.contains("write tool") {
        let path = extract_tmp_path(&prompt).unwrap_or_else(|| "/tmp/clankers-e2e-write-test-fake".to_string());
        return tool_call("fake_write_1", "write", json!({"path": path, "content": "hello world"}));
    }
    if prompt.contains("edit tool") {
        let path = extract_tmp_path(&prompt).unwrap_or_else(|| "/tmp/clankers-e2e-write-test-fake".to_string());
        return tool_call("fake_edit_1", "edit", json!({"path": path, "old_text": "world", "new_text": "clankers"}));
    }
    if prompt.contains("read tool") {
        let path = extract_tmp_path(&prompt).unwrap_or_else(|| "Cargo.toml".to_string());
        return tool_call("fake_read_1", "read", json!({"path": path}));
    }
    if prompt.contains("ls tool") {
        return tool_call("fake_ls_1", "ls", json!({"path": "."}));
    }
    if prompt.contains("grep tool") {
        return tool_call("fake_grep_1", "grep", json!({"pattern": "fn main", "path": "src/"}));
    }
    if prompt.contains("find tool") {
        return tool_call("fake_find_1", "find", json!({"pattern": "mod.rs", "path": "src/"}));
    }
    if prompt.contains("exactly one word") && prompt.contains("yes") {
        return FakeAction::Text("yes".to_string());
    }

    FakeAction::Text("hello from clankers fake provider".to_string())
}

fn choose_write_edit_action(prompt: &str, tool_results: &[(String, String)]) -> FakeAction {
    let path = extract_tmp_path(prompt).unwrap_or_else(|| "/tmp/clankers-e2e-write-test-fake".to_string());
    match tool_results.last().map(|(_, tool_name)| tool_name.as_str()) {
        None => tool_call("fake_write_1", "write", json!({"path": path, "content": "hello world"})),
        Some("write") => {
            tool_call("fake_edit_1", "edit", json!({"path": path, "old_text": "world", "new_text": "clankers"}))
        }
        Some("edit") => tool_call("fake_read_after_edit_1", "read", json!({"path": path})),
        Some(_) => FakeAction::Text("hello clankers".to_string()),
    }
}

fn tool_call(call_id: &str, name: &str, input: Value) -> FakeAction {
    FakeAction::Tool {
        call_id: call_id.to_string(),
        name: name.to_string(),
        input,
    }
}

fn latest_user_text(messages: &[AgentMessage]) -> String {
    messages
        .iter()
        .rev()
        .find_map(|message| match message {
            AgentMessage::User(user) => Some(content_text(&user.content)),
            _ => None,
        })
        .unwrap_or_default()
}

fn content_text(content: &[Content]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_results(messages: &[AgentMessage]) -> Vec<(String, String)> {
    messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::ToolResult(result) => Some((result.call_id.clone(), result.tool_name.clone())),
            _ => None,
        })
        .collect()
}

fn final_text_for_tool(prompt: &str, tool_name: &str) -> &'static str {
    if prompt.contains("bash tool") || tool_name == "bash" {
        "CLANKERS_TOOL_TEST_OK"
    } else if prompt.contains("edit tool") || tool_name == "edit" {
        "hello clankers"
    } else if prompt.contains("read tool") || tool_name == "read" {
        if prompt.contains("/tmp/clankers-e2e-write-test-") {
            "hello clankers"
        } else {
            "clankers"
        }
    } else if prompt.contains("ls tool") || tool_name == "ls" {
        "Cargo.toml src"
    } else if prompt.contains("grep tool") || tool_name == "grep" {
        "fn main"
    } else if prompt.contains("find tool") || tool_name == "find" {
        "src/tools/mod.rs"
    } else {
        "ok"
    }
}

fn extract_tmp_path(prompt: &str) -> Option<String> {
    let start = prompt.find("/tmp/clankers-e2e-write-test-")?;
    let tail = &prompt[start..];
    let end = tail
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | '.' | ','))
        .unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

async fn send_text_response(tx: &mpsc::Sender<StreamEvent>, model: &str, text: &str) {
    let _ = tx
        .send(StreamEvent::MessageStart {
            message: MessageMetadata {
                id: "fake-message".to_string(),
                model: model.to_string(),
                role: "assistant".to_string(),
            },
        })
        .await;
    let _ = tx
        .send(StreamEvent::ContentBlockStart {
            index: 0,
            content_block: Content::Text { text: String::new() },
        })
        .await;
    let _ = tx
        .send(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta { text: text.to_string() },
        })
        .await;
    let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
    let _ = tx
        .send(StreamEvent::MessageDelta {
            stop_reason: Some("end_turn".to_string()),
            usage: Usage::default(),
        })
        .await;
    let _ = tx.send(StreamEvent::MessageStop).await;
}

async fn send_tool_response(tx: &mpsc::Sender<StreamEvent>, model: &str, call_id: &str, name: &str, input: Value) {
    let input_json = input.to_string();
    let _ = tx
        .send(StreamEvent::MessageStart {
            message: MessageMetadata {
                id: format!("fake-message-{call_id}"),
                model: model.to_string(),
                role: "assistant".to_string(),
            },
        })
        .await;
    let _ = tx
        .send(StreamEvent::ContentBlockStart {
            index: 0,
            content_block: Content::ToolUse {
                id: call_id.to_string(),
                name: name.to_string(),
                input: json!({}),
            },
        })
        .await;
    let _ = tx
        .send(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: input_json,
            },
        })
        .await;
    let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
    let _ = tx
        .send(StreamEvent::MessageDelta {
            stop_reason: Some("tool_use".to_string()),
            usage: Usage::default(),
        })
        .await;
    let _ = tx.send(StreamEvent::MessageStop).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageId;
    use crate::message::UserMessage;

    fn request(prompt: &str, messages: Vec<AgentMessage>) -> CompletionRequest {
        let mut all_messages = vec![AgentMessage::User(UserMessage {
            id: MessageId::new("user-1"),
            content: vec![Content::Text {
                text: prompt.to_string(),
            }],
            timestamp: chrono::Utc::now(),
        })];
        all_messages.extend(messages);
        CompletionRequest {
            model: FAKE_MODEL_ID.to_string(),
            messages: all_messages,
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: Vec::new(),
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: Default::default(),
        }
    }

    fn tool_result(tool_name: &str) -> AgentMessage {
        AgentMessage::ToolResult(crate::message::ToolResultMessage {
            id: MessageId::new(format!("result-{tool_name}")),
            call_id: format!("call-{tool_name}"),
            tool_name: tool_name.to_string(),
            content: vec![Content::Text { text: "ok".to_string() }],
            is_error: false,
            details: None,
            timestamp: chrono::Utc::now(),
        })
    }

    #[test]
    fn chooses_simple_yes_response() {
        match choose_action(&request("Reply with exactly one word: yes", Vec::new())) {
            FakeAction::Text(text) => assert_eq!(text, "yes"),
            FakeAction::Tool { .. } => panic!("expected text response"),
        }
    }

    #[test]
    fn chooses_bash_tool_then_final_text() {
        match choose_action(&request("Use the bash tool to run: echo CLANKERS_TOOL_TEST_OK", Vec::new())) {
            FakeAction::Tool { name, input, .. } => {
                assert_eq!(name, "bash");
                assert_eq!(input["command"], "echo CLANKERS_TOOL_TEST_OK");
            }
            FakeAction::Text(_) => panic!("expected tool call"),
        }

        match choose_action(&request("Use the bash tool to run: echo CLANKERS_TOOL_TEST_OK", vec![tool_result("bash")]))
        {
            FakeAction::Text(text) => assert_eq!(text, "CLANKERS_TOOL_TEST_OK"),
            FakeAction::Tool { .. } => panic!("expected final text"),
        }
    }

    #[test]
    fn sequences_write_edit_read() {
        let prompt = "Use the write tool to create the file /tmp/clankers-e2e-write-test-123 with content 'hello world'. Then use the edit tool to replace 'world' with 'clankers'. Then use the read tool to read it back.";
        match choose_action(&request(prompt, Vec::new())) {
            FakeAction::Tool { name, input, .. } => {
                assert_eq!(name, "write");
                assert_eq!(input["path"], "/tmp/clankers-e2e-write-test-123");
            }
            FakeAction::Text(_) => panic!("expected write tool call"),
        }
        match choose_action(&request(prompt, vec![tool_result("write")])) {
            FakeAction::Tool { name, input, .. } => {
                assert_eq!(name, "edit");
                assert_eq!(input["old_text"], "world");
            }
            FakeAction::Text(_) => panic!("expected edit tool call"),
        }
        match choose_action(&request(prompt, vec![tool_result("write"), tool_result("edit")])) {
            FakeAction::Tool { name, input, .. } => {
                assert_eq!(name, "read");
                assert_eq!(input["path"], "/tmp/clankers-e2e-write-test-123");
            }
            FakeAction::Text(_) => panic!("expected read tool call"),
        }
    }
}
