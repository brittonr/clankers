use clanker_message::Content;
use clanker_message::ToolDefinition;
use clankers_engine::EmbeddableEngine;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineEffect;
use clankers_engine::EngineInput;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineModelResponse;
use clankers_engine::EnginePromptSubmission;
use clankers_engine::EngineRejection;
use clankers_engine::EngineState;
use clankers_engine::EngineToolCall;
use clankers_engine::EngineTurnRequest;
use clankers_engine::reduce;
use serde_json::Value;
use serde_json::json;

const MINIMAL_TOOL_TURN_FIXTURE: &str = include_str!("fixtures/minimal_tool_turn.json");
const TOOL_FAILURE_TURN_FIXTURE: &str = include_str!("fixtures/tool_failure_turn.json");
const DEFAULT_MODEL_REQUEST_SLOT_BUDGET: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayReceipt {
    transcript_hash: String,
    events_hash: String,
    provider_requests_hash: String,
    tool_results_hash: String,
    receipt_hash: String,
    normalized: Value,
}

#[test]
fn deterministic_tool_turn_replay_is_byte_stable() {
    assert_replay_fixture_is_byte_stable(&fixture(MINIMAL_TOOL_TURN_FIXTURE));
}

#[test]
fn deterministic_tool_failure_replay_is_byte_stable() {
    let receipt = assert_replay_fixture_is_byte_stable(&fixture(TOOL_FAILURE_TURN_FIXTURE));
    let tool_result = &receipt.normalized["tool_results"][0];

    assert_eq!(tool_result["is_error"], true);
    assert_eq!(tool_result["error"], "fixture missing: fixtures/missing.txt");
}

fn assert_replay_fixture_is_byte_stable(fixture: &Value) -> ReplayReceipt {
    let first = replay_fixture(fixture);
    let second = replay_fixture(fixture);

    assert_eq!(first.normalized, second.normalized, "normalized replay output must be identical");
    assert_eq!(first, second, "BLAKE3-bound replay receipt must be stable");
    assert_eq!(
        first.transcript_hash,
        fixture_hash(fixture, "transcript"),
        "actual transcript hash: {}",
        first.transcript_hash
    );
    assert_eq!(first.events_hash, fixture_hash(fixture, "events"), "actual events hash: {}", first.events_hash);
    assert_eq!(
        first.provider_requests_hash,
        fixture_hash(fixture, "provider_requests"),
        "actual provider request hash: {}",
        first.provider_requests_hash
    );
    assert_eq!(
        first.tool_results_hash,
        fixture_hash(fixture, "tool_results"),
        "actual tool result hash: {}",
        first.tool_results_hash
    );
    assert_eq!(first.receipt_hash, fixture_hash(fixture, "receipt"), "actual receipt hash: {}", first.receipt_hash);

    first
}

#[test]
fn deterministic_replay_reports_tool_correlation_mismatches() {
    let (state, _request, tool_call) = state_waiting_for_fixture_tool(&fixture(MINIMAL_TOOL_TURN_FIXTURE));

    let mismatch = reduce(&state, &EngineInput::ToolCompleted {
        call_id: EngineCorrelationId("wrong-tool-call".to_string()),
        result: vec![Content::Text {
            text: "uncorrelated".to_string(),
        }],
    });

    assert_eq!(mismatch.rejection, Some(EngineRejection::CorrelationMismatch));
    assert!(mismatch.effects.is_empty(), "mismatched tool feedback must not emit follow-up effects");
    assert_eq!(
        tool_correlation_diagnostic(&tool_call, &EngineCorrelationId("wrong-tool-call".to_string())),
        "tool correlation mismatch: expected tool-call-001 for deterministic_read, got wrong-tool-call"
    );
}

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture must be valid JSON")
}

fn replay_fixture(fixture: &Value) -> ReplayReceipt {
    let mut provider = ScriptedProvider::new(fixture);
    let tools = ScriptedTools::new(fixture);
    let mut events = Vec::new();
    let mut tool_results = Vec::new();

    let mut engine = EmbeddableEngine::new();
    let submit = engine.submit_turn(EngineTurnRequest {
        submission: submission_from_fixture(fixture),
    });
    record_effects(&submit.outcome.effects, &mut events);
    let initial_request = request_model_effect(&submit.outcome.effects).clone();
    let mut state = submit.outcome.next_state;

    let first_response = provider.complete(&initial_request);
    let tool_plan = reduce(&state, &EngineInput::ModelCompleted {
        request_id: initial_request.request_id.clone(),
        response: first_response,
    });
    record_effects(&tool_plan.effects, &mut events);
    let tool_call = execute_tool_effect(&tool_plan.effects).clone();
    state = tool_plan.next_state;

    let tool_output = tools.execute(&tool_call);
    tool_results.push(normalize_tool_result(&tool_call, &tool_output));
    let after_tool = reduce(&state, &EngineInput::ToolCompleted {
        call_id: tool_call.call_id.clone(),
        result: tool_output.content,
    });
    record_effects(&after_tool.effects, &mut events);
    let follow_up_request = request_model_effect(&after_tool.effects).clone();
    state = after_tool.next_state;

    let final_response = provider.complete(&follow_up_request);
    let terminal = reduce(&state, &EngineInput::ModelCompleted {
        request_id: follow_up_request.request_id.clone(),
        response: final_response,
    });
    record_effects(&terminal.effects, &mut events);

    let transcript = normalize_messages(&terminal.next_state.messages);
    let provider_requests = Value::Array(provider.recorded_requests);
    let tool_results = Value::Array(tool_results);
    let events = Value::Array(events);
    let normalized = json!({
        "fixture": fixture["name"],
        "transcript": transcript,
        "events": events,
        "provider_requests": provider_requests,
        "tool_results": tool_results,
    });
    let receipt_hash = stable_hash(&normalized);

    ReplayReceipt {
        transcript_hash: stable_hash(&normalized["transcript"]),
        events_hash: stable_hash(&normalized["events"]),
        provider_requests_hash: stable_hash(&normalized["provider_requests"]),
        tool_results_hash: stable_hash(&normalized["tool_results"]),
        receipt_hash,
        normalized,
    }
}

fn state_waiting_for_fixture_tool(fixture: &Value) -> (EngineState, EngineModelRequest, EngineToolCall) {
    let mut provider = ScriptedProvider::new(fixture);
    let submitted = reduce(&EngineState::new(), &EngineInput::submit_user_prompt(submission_from_fixture(fixture)));
    let request = request_model_effect(&submitted.effects).clone();
    let response = provider.complete(&request);
    let tool_plan = reduce(&submitted.next_state, &EngineInput::ModelCompleted {
        request_id: request.request_id.clone(),
        response,
    });
    let tool_call = execute_tool_effect(&tool_plan.effects).clone();
    (tool_plan.next_state, request, tool_call)
}

struct ScriptedProvider<'a> {
    steps: &'a [Value],
    next_step: usize,
    recorded_requests: Vec<Value>,
}

impl<'a> ScriptedProvider<'a> {
    fn new(fixture: &'a Value) -> Self {
        Self {
            steps: fixture["provider_script"].as_array().expect("provider_script must be an array"),
            next_step: 0,
            recorded_requests: Vec::new(),
        }
    }

    fn complete(&mut self, request: &EngineModelRequest) -> EngineModelResponse {
        let step = self.steps.get(self.next_step).expect("scripted provider step must exist");
        self.next_step += 1;

        assert_eq!(request.request_id.0, string_at(step, "expect_request_id"));
        assert_eq!(request.session_id, string_at(step, "expect_session_id"));
        assert_eq!(message_roles(&request.messages), string_array_at(step, "expect_message_roles"));
        assert_eq!(request.tools.len(), 1, "fixture request must expose one tool schema");

        self.recorded_requests.push(normalize_request(request));
        model_response_from_value(&step["response"])
    }
}

struct ScriptedTools<'a> {
    script: &'a Value,
}

struct ToolOutput {
    content: Vec<Content>,
    is_error: bool,
    error: Option<String>,
}

impl<'a> ScriptedTools<'a> {
    fn new(fixture: &'a Value) -> Self {
        Self {
            script: &fixture["tool_script"],
        }
    }

    fn execute(&self, call: &EngineToolCall) -> ToolOutput {
        let step = &self.script[&call.call_id.0];
        assert!(step.is_object(), "tool call {} must be scripted", call.call_id.0);
        assert_eq!(call.tool_name, string_at(step, "name"));
        assert_eq!(call.input, step["input"]);
        ToolOutput {
            content: content_array_from_value(&step["result"]),
            is_error: step["is_error"].as_bool().unwrap_or(false),
            error: step["error"].as_str().map(ToString::to_string),
        }
    }
}

fn submission_from_fixture(fixture: &Value) -> EnginePromptSubmission {
    EnginePromptSubmission {
        messages: vec![EngineMessage {
            role: EngineMessageRole::User,
            content: vec![Content::Text {
                text: string_at(fixture, "user"),
            }],
        }],
        model: string_at(fixture, "model"),
        system_prompt: string_at(fixture, "system_prompt"),
        max_tokens: Some(fixture["max_tokens"].as_u64().expect("max_tokens must be integer") as usize),
        temperature: None,
        thinking: None,
        tools: vec![ToolDefinition {
            name: string_at(&fixture["tool"], "name"),
            description: string_at(&fixture["tool"], "description"),
            input_schema: fixture["tool"]["input_schema"].clone(),
        }],
        no_cache: true,
        cache_ttl: None,
        session_id: string_at(fixture, "session_id"),
        model_request_slot_budget: DEFAULT_MODEL_REQUEST_SLOT_BUDGET,
    }
}

fn model_response_from_value(value: &Value) -> EngineModelResponse {
    EngineModelResponse {
        output: content_array_from_value(&value["content"]),
        stop_reason: serde_json::from_value(value["stop_reason"].clone()).expect("stop_reason must decode"),
    }
}

fn content_array_from_value(value: &Value) -> Vec<Content> {
    serde_json::from_value(value.clone()).expect("content fixture must decode")
}

fn request_model_effect(effects: &[EngineEffect]) -> &EngineModelRequest {
    effects
        .iter()
        .find_map(|effect| match effect {
            EngineEffect::RequestModel(request) => Some(request),
            _ => None,
        })
        .expect("expected RequestModel effect")
}

fn execute_tool_effect(effects: &[EngineEffect]) -> &EngineToolCall {
    effects
        .iter()
        .find_map(|effect| match effect {
            EngineEffect::ExecuteTool(call) => Some(call),
            _ => None,
        })
        .expect("expected ExecuteTool effect")
}

fn record_effects(effects: &[EngineEffect], events: &mut Vec<Value>) {
    for effect in effects {
        match effect {
            EngineEffect::RequestModel(request) => events.push(json!({
                "type": "request_model",
                "request_id": request.request_id.0,
                "message_roles": message_roles(&request.messages),
            })),
            EngineEffect::ExecuteTool(call) => events.push(json!({
                "type": "execute_tool",
                "call_id": call.call_id.0,
                "tool_name": call.tool_name,
                "input": call.input,
            })),
            EngineEffect::ScheduleRetry { request_id, delay } => events.push(json!({
                "type": "schedule_retry",
                "request_id": request_id.0,
                "delay_ms": delay.as_millis(),
            })),
            EngineEffect::EmitEvent(event) => events.push(json!({
                "type": "engine_event",
                "event": format!("{event:?}"),
            })),
        }
    }
}

fn normalize_request(request: &EngineModelRequest) -> Value {
    json!({
        "request_id": request.request_id.0,
        "session_id": request.session_id,
        "model": request.model,
        "system_prompt": request.system_prompt,
        "max_tokens": request.max_tokens,
        "no_cache": request.no_cache,
        "message_roles": message_roles(&request.messages),
        "messages": normalize_messages(&request.messages),
        "tools": request.tools.iter().map(|tool| json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.input_schema,
        })).collect::<Vec<_>>(),
    })
}

fn normalize_messages(messages: &[EngineMessage]) -> Value {
    Value::Array(
        messages
            .iter()
            .map(|message| {
                json!({
                    "role": role_name(&message.role),
                    "content": message.content,
                })
            })
            .collect(),
    )
}

fn normalize_tool_result(call: &EngineToolCall, result: &ToolOutput) -> Value {
    json!({
        "call_id": call.call_id.0,
        "tool_name": call.tool_name,
        "result": result.content,
        "is_error": result.is_error,
        "error": result.error,
    })
}

fn message_roles(messages: &[EngineMessage]) -> Vec<String> {
    messages.iter().map(|message| role_name(&message.role).to_string()).collect()
}

fn role_name(role: &EngineMessageRole) -> &'static str {
    match role {
        EngineMessageRole::User => "user",
        EngineMessageRole::Assistant => "assistant",
        EngineMessageRole::Tool => "tool",
    }
}

fn stable_hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("normalized replay value must serialize");
    blake3::hash(&bytes).to_hex().to_string()
}

fn fixture_hash(fixture: &Value, name: &str) -> String {
    string_at(&fixture["expected_hashes"], name)
}

fn string_at(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_else(|| panic!("{key} must be a string")).to_string()
}

fn string_array_at(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} must be an array"))
        .iter()
        .map(|item| item.as_str().unwrap_or_else(|| panic!("{key} entries must be strings")).to_string())
        .collect()
}

fn tool_correlation_diagnostic(expected: &EngineToolCall, actual: &EngineCorrelationId) -> String {
    format!(
        "tool correlation mismatch: expected {} for {}, got {}",
        expected.call_id.0, expected.tool_name, actual.0
    )
}
