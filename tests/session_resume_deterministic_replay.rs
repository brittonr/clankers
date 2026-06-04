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
use clankers_session::SessionManager;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::mpsc;

const MODEL: &str = "deterministic-resume-model";
const INITIAL_PROMPT: &str = "Look up order 42 and remember it.";
const FOLLOW_UP_PROMPT: &str = "Use the resumed session context to answer again.";
const SYSTEM_PROMPT: &str = "You are a deterministic session resume replay fixture.";
const TOOL_CALL_ID: &str = "call_resume_lookup_order_42";
const EXPECTED_RECEIPT_HASH: &str = "ef5a4d5692c8902b374bf5a901572f86b931ec7a13e4e69b084cb891e2c5a11f";

#[derive(Clone, Copy)]
enum ScriptKind {
    Initial,
    FollowUp,
}

struct ScriptedProvider {
    kind: ScriptKind,
    requests: Mutex<Vec<Value>>,
}

impl ScriptedProvider {
    fn new(kind: ScriptKind) -> Self {
        Self {
            kind,
            requests: Mutex::new(Vec::new()),
        }
    }
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

        match (self.kind, request_index) {
            (ScriptKind::Initial, 0) => stream_tool_call(tx).await,
            (ScriptKind::Initial, 1) => stream_initial_answer(tx).await,
            (ScriptKind::FollowUp, 0) => stream_follow_up_answer(tx).await,
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
                    provider: "scripted-session-resume".to_string(),
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
        "scripted-session-resume"
    }
}

struct LookupOrderTool {
    calls: Arc<Mutex<Vec<Value>>>,
    expected_session_id: String,
    definition: ToolDefinition,
}

impl LookupOrderTool {
    fn new(calls: Arc<Mutex<Vec<Value>>>, expected_session_id: String) -> Self {
        Self {
            calls,
            expected_session_id,
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
        assert_eq!(ctx.session_id(), self.expected_session_id);
        self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(params);
        ToolResult::text("order 42 status=shipped total=$19.99")
    }
}

#[tokio::test]
async fn persisted_session_resume_replay_restores_context_and_session_metadata() {
    let first = Box::pin(run_resume_replay_once()).await;
    let second = Box::pin(run_resume_replay_once()).await;

    assert_eq!(first, second, "session resume replay must be byte-stable across isolated runs");
    assert_resume_properties(&first);

    let receipt_hash = blake3::hash(serde_json::to_string(&first).expect("receipt serializes").as_bytes())
        .to_hex()
        .to_string();
    assert_eq!(receipt_hash, EXPECTED_RECEIPT_HASH, "normalized receipt changed: {receipt_hash}\n{first:#}");
}

async fn run_resume_replay_once() -> Value {
    let tmp = TempDir::new().expect("tempdir should exist");
    let cwd = tmp.path().to_string_lossy().to_string();
    let session_manager =
        SessionManager::create(tmp.path(), &cwd, MODEL, None, None, None).expect("session manager should create");
    let session_id = session_manager.session_id().to_string();
    let session_file = session_manager.file_path().to_path_buf();

    let initial_provider = Arc::new(ScriptedProvider::new(ScriptKind::Initial));
    let tool_calls = Arc::new(Mutex::new(Vec::new()));
    let initial_tool: Arc<dyn Tool> = Arc::new(LookupOrderTool::new(tool_calls.clone(), session_id.clone()));
    let initial_settings = deterministic_settings();
    let initial_agent = Agent::new_with_agent_settings(
        initial_provider.clone(),
        vec![initial_tool],
        clankers::agent_config::agent_settings_from_config(&initial_settings),
        MODEL.to_string(),
        SYSTEM_PROMPT.to_string(),
    );
    let mut initial_controller = SessionController::new(initial_agent, ControllerConfig {
        session_id: session_id.clone(),
        model: MODEL.to_string(),
        session_manager: Some(session_manager),
        ..Default::default()
    });

    initial_controller
        .handle_command(SessionCommand::Prompt {
            text: INITIAL_PROMPT.to_string(),
            images: vec![],
        })
        .await;
    let initial_events = initial_controller.take_outgoing();
    initial_controller.shutdown().await;

    let mut resumed_manager = SessionManager::open(session_file).expect("persisted session should reopen");
    assert_eq!(resumed_manager.session_id(), session_id);
    let resume_from = resumed_manager.active_leaf_id().cloned().expect("persisted session has active leaf");
    resumed_manager.record_resume(resume_from).expect("resume annotation persists");
    let resumed_context = resumed_manager.build_context().expect("resume context builds");
    let resumed_context_roles = resumed_context.iter().map(message_role).collect::<Vec<_>>();

    let follow_provider = Arc::new(ScriptedProvider::new(ScriptKind::FollowUp));
    let follow_tool: Arc<dyn Tool> = Arc::new(LookupOrderTool::new(tool_calls.clone(), session_id.clone()));
    let follow_settings = deterministic_settings();
    let mut follow_agent = Agent::new_with_agent_settings(
        follow_provider.clone(),
        vec![follow_tool],
        clankers::agent_config::agent_settings_from_config(&follow_settings),
        MODEL.to_string(),
        SYSTEM_PROMPT.to_string(),
    );
    follow_agent.seed_messages(resumed_context);
    let mut follow_controller = SessionController::new(follow_agent, ControllerConfig {
        session_id: session_id.clone(),
        model: MODEL.to_string(),
        session_manager: Some(resumed_manager),
        ..Default::default()
    });

    follow_controller
        .handle_command(SessionCommand::Prompt {
            text: FOLLOW_UP_PROMPT.to_string(),
            images: vec![],
        })
        .await;
    let follow_events = follow_controller.take_outgoing();
    follow_controller.shutdown().await;

    let final_manager =
        SessionManager::open(follow_controller.session_manager().expect("session manager").file_path().to_path_buf())
            .expect("final persisted session should reopen");
    let final_context = final_manager.build_context().expect("final context builds");

    json!({
        "session_id": "SESSION",
        "initial_requests": initial_provider.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        "follow_up_requests": follow_provider.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        "tool_calls": tool_calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        "resumed_context_roles": resumed_context_roles,
        "final_context_roles": final_context.iter().map(message_role).collect::<Vec<_>>(),
        "final_context_content": final_context.iter().map(normalize_message_content).collect::<Vec<_>>(),
        "initial_events": normalize_events(&initial_events),
        "follow_up_events": normalize_events(&follow_events),
    })
}

fn deterministic_settings() -> Settings {
    Settings {
        max_tokens: 256,
        no_cache: true,
        ..Default::default()
    }
}

fn assert_resume_properties(receipt: &Value) {
    let initial_requests = receipt["initial_requests"].as_array().expect("initial requests recorded");
    let follow_requests = receipt["follow_up_requests"].as_array().expect("follow-up requests recorded");
    assert_eq!(initial_requests.len(), 2, "initial provider request count: {receipt:#}");
    assert_eq!(follow_requests.len(), 1, "follow-up provider request count: {receipt:#}");

    assert_eq!(initial_requests[0]["extra_params"]["_session_id"], "SESSION");
    assert_eq!(initial_requests[1]["extra_params"]["_session_id"], "SESSION");
    assert_eq!(follow_requests[0]["extra_params"]["_session_id"], "SESSION");
    assert_eq!(initial_requests[0]["message_roles"], json!(["user"]));
    assert_eq!(initial_requests[1]["message_roles"], json!(["user", "assistant", "tool"]));
    assert_eq!(follow_requests[0]["message_roles"], json!(["user", "assistant", "tool", "assistant", "user"]));
    assert_eq!(receipt["resumed_context_roles"], json!(["user", "assistant", "tool", "assistant"]));
    assert_eq!(
        receipt["final_context_roles"],
        json!(["user", "assistant", "tool", "assistant", "user", "assistant"])
    );
    assert_eq!(receipt["tool_calls"], json!([{ "order_id": "42" }]));
    assert!(receipt["follow_up_events"].as_array().expect("events").iter().any(|event| {
        event == &json!({ "type": "TextDelta", "text": "Resumed context says order 42 shipped for $19.99." })
    }));
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

async fn stream_initial_answer(tx: mpsc::Sender<StreamEvent>) {
    stream_text_answer(tx, "Initial context says order 42 shipped for $19.99.", "msg-initial-final").await;
}

async fn stream_follow_up_answer(tx: mpsc::Sender<StreamEvent>) {
    stream_text_answer(tx, "Resumed context says order 42 shipped for $19.99.", "msg-follow-final").await;
}

async fn stream_text_answer(tx: mpsc::Sender<StreamEvent>, text: &str, message_id: &str) {
    send(&tx, StreamEvent::MessageStart {
        message: metadata(message_id),
    })
    .await;
    send(&tx, StreamEvent::ContentBlockStart {
        index: 0,
        content_block: Content::Text { text: String::new() },
    })
    .await;
    send(&tx, StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::TextDelta { text: text.to_string() },
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
    let mut extra_params = request.extra_params.clone();
    if extra_params.contains_key("_session_id") {
        extra_params.insert("_session_id".to_string(), json!("SESSION"));
    }
    json!({
        "model": request.model,
        "system_prompt": request.system_prompt,
        "max_tokens": request.max_tokens,
        "temperature": request.temperature,
        "tools": request.tools.iter().map(|tool| tool.name.clone()).collect::<Vec<_>>(),
        "extra_params": extra_params,
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
