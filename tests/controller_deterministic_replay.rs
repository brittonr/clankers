use std::sync::Arc;
use std::sync::Mutex;

use clanker_message::Content;
use clanker_message::Usage;
use clanker_message::streaming::ContentDelta;
use clanker_message::streaming::MessageMetadata;
use clanker_message::streaming::StreamEvent;
use clankers_agent::Agent;
use clankers_agent::Tool;
use clankers_agent::ToolContext;
use clankers_agent::ToolDefinition;
use clankers_agent::ToolResult;
use clankers_config::settings::Settings;
use clankers_controller::SessionController;
use clankers_controller::config::ControllerConfig;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_provider::CompletionRequest;
use clankers_provider::Model;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

const SESSION_ID: &str = "deterministic-controller-session-001";
const MODEL: &str = "deterministic-controller-model";
const PROMPT: &str = "Look up order 42 and answer deterministically.";
const SYSTEM_PROMPT: &str = "You are a deterministic controller replay fixture.";
const TOOL_CALL_ID: &str = "call_lookup_order_42";
const EXPECTED_RECEIPT_HASH: &str = "966821dd7fac529fee8f3b08ef7edf1021451f9c8189840e92f858940d85b68d";

#[derive(Default)]
struct ScriptedProvider {
    requests: Mutex<Vec<Value>>,
}

#[async_trait::async_trait]
impl clankers_provider::Provider for ScriptedProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<StreamEvent>,
    ) -> clankers_provider::error::Result<()> {
        let request_index = {
            let mut requests = self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            requests.push(normalize_request(&request));
            requests.len() - 1
        };

        match request_index {
            0 => stream_tool_call(tx).await,
            1 => stream_final_answer(tx).await,
            _ => {
                tx.send(StreamEvent::Error {
                    error: format!("unexpected scripted provider request {request_index}"),
                })
                .await
                .ok();
            }
        }
        Ok(())
    }

    fn models(&self) -> &[Model] {
        static MODELS: std::sync::OnceLock<Vec<Model>> = std::sync::OnceLock::new();
        MODELS
            .get_or_init(|| {
                vec![Model {
                    id: MODEL.to_string(),
                    name: MODEL.to_string(),
                    provider: "scripted-controller".to_string(),
                    max_input_tokens: 4_096,
                    max_output_tokens: 1_024,
                    supports_thinking: false,
                    supports_images: false,
                    supports_tools: true,
                    input_cost_per_mtok: None,
                    output_cost_per_mtok: None,
                }]
            })
            .as_slice()
    }

    fn name(&self) -> &str {
        "scripted-controller"
    }
}

struct LookupOrderTool {
    calls: Arc<Mutex<Vec<Value>>>,
    definition: ToolDefinition,
}

impl LookupOrderTool {
    fn new(calls: Arc<Mutex<Vec<Value>>>) -> Self {
        Self {
            calls,
            definition: ToolDefinition {
                name: "lookup_order".to_string(),
                description: "Return deterministic order details".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "order_id": { "type": "string" }
                    },
                    "required": ["order_id"]
                }),
            },
        }
    }
}

#[async_trait::async_trait]
impl Tool for LookupOrderTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        assert_eq!(ctx.call_id, TOOL_CALL_ID);
        assert_eq!(ctx.session_id(), SESSION_ID);
        self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(params);
        ToolResult::text("order 42 status=shipped total=$19.99")
    }
}

#[tokio::test]
async fn controller_replay_preserves_session_request_shape_tools_and_events() {
    let first = Box::pin(run_replay_once()).await;
    let second = Box::pin(run_replay_once()).await;

    assert_eq!(first, second, "controller replay must be byte-stable across isolated runs");
    assert_replay_properties(&first);

    let receipt_hash = blake3::hash(serde_json::to_string(&first).expect("receipt serializes").as_bytes())
        .to_hex()
        .to_string();
    assert_eq!(receipt_hash, EXPECTED_RECEIPT_HASH, "normalized receipt changed: {receipt_hash}\n{first:#}");
}

async fn run_replay_once() -> Value {
    let provider = Arc::new(ScriptedProvider::default());
    let tool_calls = Arc::new(Mutex::new(Vec::new()));
    let tool: Arc<dyn Tool> = Arc::new(LookupOrderTool::new(tool_calls.clone()));
    let settings = Settings {
        max_tokens: 256,
        no_cache: true,
        ..Default::default()
    };

    let agent = Agent::new_with_agent_settings(
        provider.clone(),
        vec![tool],
        clankers::agent_config::agent_settings_from_config(&settings),
        MODEL.to_string(),
        SYSTEM_PROMPT.to_string(),
    );
    let mut controller = SessionController::new(agent, ControllerConfig {
        session_id: SESSION_ID.to_string(),
        model: MODEL.to_string(),
        ..Default::default()
    });

    controller
        .handle_command(SessionCommand::Prompt {
            text: PROMPT.to_string(),
            images: vec![],
        })
        .await;

    json!({
        "requests": provider.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        "tool_calls": tool_calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        "events": normalize_events(&controller.take_outgoing()),
    })
}

fn assert_replay_properties(receipt: &Value) {
    let requests = receipt["requests"].as_array().expect("requests are recorded");
    let tool_calls = receipt["tool_calls"].as_array().expect("tool calls are recorded");
    let events = receipt["events"].as_array().expect("events are recorded");

    assert_eq!(requests.len(), 2, "provider request count: {receipt:#}");
    assert_eq!(requests[0]["extra_params"]["_session_id"], SESSION_ID);
    assert_eq!(requests[1]["extra_params"]["_session_id"], SESSION_ID);
    assert_eq!(requests[0]["message_roles"], json!(["user"]));
    assert_eq!(requests[1]["message_roles"], json!(["user", "assistant", "tool"]));
    assert_eq!(requests[0]["tools"], json!(["lookup_order"]));
    assert_eq!(tool_calls, &[json!({ "order_id": "42" })]);
    assert!(events.iter().any(|event| event
        == &json!({
            "type": "ToolCall",
            "tool_name": "lookup_order",
            "call_id": TOOL_CALL_ID,
            "input": { "order_id": "42" },
        })));
    assert!(events.iter().any(|event| event
        == &json!({
            "type": "ToolDone",
            "call_id": TOOL_CALL_ID,
            "text": "order 42 status=shipped total=$19.99",
            "is_error": false,
        })));
    assert!(
        events
            .iter()
            .any(|event| event == &json!({ "type": "TextDelta", "text": "Order 42 shipped for $19.99." }))
    );
    assert_eq!(events.last(), Some(&json!({ "type": "PromptDone", "error": null })));
}

async fn stream_tool_call(tx: mpsc::Sender<StreamEvent>) {
    send(&tx, StreamEvent::MessageStart {
        message: metadata("msg-tool"),
    })
    .await;
    send(&tx, StreamEvent::ContentBlockStart {
        index: 0,
        content_block: Content::ToolUse {
            id: TOOL_CALL_ID.to_string(),
            name: "lookup_order".to_string(),
            input: json!({}),
        },
    })
    .await;
    send(&tx, StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::InputJsonDelta {
            partial_json: "{\"order_id\":\"42\"}".to_string(),
        },
    })
    .await;
    send(&tx, StreamEvent::ContentBlockStop { index: 0 }).await;
    send(&tx, StreamEvent::MessageDelta {
        stop_reason: Some("tool_use".to_string()),
        usage: Usage::default(),
    })
    .await;
    send(&tx, StreamEvent::MessageStop).await;
}

async fn stream_final_answer(tx: mpsc::Sender<StreamEvent>) {
    send(&tx, StreamEvent::MessageStart {
        message: metadata("msg-final"),
    })
    .await;
    send(&tx, StreamEvent::ContentBlockStart {
        index: 0,
        content_block: Content::Text { text: String::new() },
    })
    .await;
    send(&tx, StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::TextDelta {
            text: "Order 42 shipped for $19.99.".to_string(),
        },
    })
    .await;
    send(&tx, StreamEvent::ContentBlockStop { index: 0 }).await;
    send(&tx, StreamEvent::MessageDelta {
        stop_reason: Some("stop".to_string()),
        usage: Usage::default(),
    })
    .await;
    send(&tx, StreamEvent::MessageStop).await;
}

async fn send(tx: &mpsc::Sender<StreamEvent>, event: StreamEvent) {
    tx.send(event).await.expect("scripted stream receiver stays open");
}

fn metadata(id: &str) -> MessageMetadata {
    MessageMetadata {
        id: id.to_string(),
        model: MODEL.to_string(),
        role: "assistant".to_string(),
    }
}

fn normalize_request(request: &CompletionRequest) -> Value {
    json!({
        "model": request.model,
        "system_prompt": request.system_prompt,
        "max_tokens": request.max_tokens,
        "temperature": request.temperature,
        "tools": request.tools.iter().map(|tool| tool.name.clone()).collect::<Vec<_>>(),
        "extra_params": request.extra_params,
        "message_roles": request.messages.iter().map(message_role).collect::<Vec<_>>(),
        "message_content": request.messages.iter().map(normalize_message_content).collect::<Vec<_>>(),
    })
}

fn message_role(message: &clanker_message::transcript::AgentMessage) -> &'static str {
    match message {
        clanker_message::transcript::AgentMessage::User(_) => "user",
        clanker_message::transcript::AgentMessage::Assistant(_) => "assistant",
        clanker_message::transcript::AgentMessage::ToolResult(_) => "tool",
        clanker_message::transcript::AgentMessage::BashExecution(_) => "bash",
        clanker_message::transcript::AgentMessage::Custom(_) => "custom",
        clanker_message::transcript::AgentMessage::BranchSummary(_) => "branch_summary",
        clanker_message::transcript::AgentMessage::CompactionSummary(_) => "compaction_summary",
    }
}

fn normalize_message_content(message: &clanker_message::transcript::AgentMessage) -> Value {
    match message {
        clanker_message::transcript::AgentMessage::User(user) => json!(user.content),
        clanker_message::transcript::AgentMessage::Assistant(assistant) => json!(assistant.content),
        clanker_message::transcript::AgentMessage::ToolResult(tool) => json!({
            "call_id": tool.call_id,
            "tool_name": tool.tool_name,
            "content": tool.content,
            "is_error": tool.is_error,
        }),
        _ => json!(null),
    }
}

fn normalize_events(events: &[DaemonEvent]) -> Vec<Value> {
    events
        .iter()
        .filter_map(|event| match event {
            DaemonEvent::AgentStart => Some(json!({ "type": "AgentStart" })),
            DaemonEvent::UserInput {
                text, agent_msg_count, ..
            } => Some(json!({
                "type": "UserInput",
                "text": text,
                "agent_msg_count": agent_msg_count,
            })),
            DaemonEvent::ToolCall {
                tool_name,
                call_id,
                input,
            } => Some(json!({
                "type": "ToolCall",
                "tool_name": tool_name,
                "call_id": call_id,
                "input": input,
            })),
            DaemonEvent::ToolStart { call_id, tool_name } => Some(json!({
                "type": "ToolStart",
                "call_id": call_id,
                "tool_name": tool_name,
            })),
            DaemonEvent::ToolDone {
                call_id,
                text,
                is_error,
                ..
            } => Some(json!({
                "type": "ToolDone",
                "call_id": call_id,
                "text": text,
                "is_error": is_error,
            })),
            DaemonEvent::TextDelta { text } => Some(json!({ "type": "TextDelta", "text": text })),
            DaemonEvent::AgentEnd => Some(json!({ "type": "AgentEnd" })),
            DaemonEvent::PromptDone { error } => Some(json!({ "type": "PromptDone", "error": error })),
            _ => None,
        })
        .collect()
}
