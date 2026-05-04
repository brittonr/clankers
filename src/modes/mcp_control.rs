use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpControlErrorKind {
    UnsupportedMethod,
    UnknownTool,
    InvalidRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpControlError {
    pub kind: McpControlErrorKind,
    pub message: String,
}

impl McpControlError {
    fn unsupported_method(method: impl Into<String>) -> Self {
        let method = method.into();
        Self {
            kind: McpControlErrorKind::UnsupportedMethod,
            message: format!("MCP method '{method}' is not supported by clankers session control"),
        }
    }

    fn unknown_tool(tool: impl Into<String>) -> Self {
        let tool = tool.into();
        Self {
            kind: McpControlErrorKind::UnknownTool,
            message: format!("MCP session-control tool '{tool}' is not supported"),
        }
    }

    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            kind: McpControlErrorKind::InvalidRequest,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct McpControlRequest {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct McpControlResponse {
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpControlErrorResponse>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct McpControlErrorResponse {
    pub code: i64,
    pub message: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum McpSessionEffect {
    Command(SessionCommand),
    ReadOnly { action: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct McpDispatchEvidence {
    pub submitted: bool,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub disconnected: bool,
}

impl McpDispatchEvidence {
    pub fn submitted(events: Vec<Value>, disconnected: bool) -> Self {
        Self {
            submitted: true,
            events,
            disconnected,
        }
    }

    pub fn failed() -> Self {
        Self {
            submitted: false,
            events: Vec::new(),
            disconnected: false,
        }
    }
}

impl From<bool> for McpDispatchEvidence {
    fn from(submitted: bool) -> Self {
        if submitted {
            Self::submitted(Vec::new(), false)
        } else {
            Self::failed()
        }
    }
}

pub fn metadata_for_status(status: &str) -> Value {
    serde_json::json!({
        "source": "mcp_session_control",
        "transport": "stdio",
        "status": status,
    })
}

pub fn supported_tools() -> Vec<Value> {
    vec![
        tool_definition(
            "send_prompt",
            "Submit a prompt through SessionCommand::Prompt",
            serde_json::json!({
                "type": "object",
                "required": ["text"],
                "properties": {
                    "text": {"type": "string"}
                }
            }),
        ),
        tool_definition(
            "interrupt",
            "Abort the active turn through SessionCommand::Abort",
            serde_json::json!({"type": "object"}),
        ),
        tool_definition(
            "set_thinking_level",
            "Set the session thinking level",
            serde_json::json!({
                "type": "object",
                "required": ["level"],
                "properties": {"level": {"type": "string"}}
            }),
        ),
        tool_definition(
            "set_disabled_tools",
            "Replace the disabled tool list for the session",
            serde_json::json!({
                "type": "object",
                "required": ["tools"],
                "properties": {"tools": {"type": "array", "items": {"type": "string"}}}
            }),
        ),
        tool_definition(
            "set_capabilities",
            "Restrict or clear active session capabilities",
            serde_json::json!({
                "type": "object",
                "properties": {"capabilities": {"type": ["array", "null"], "items": {"type": "string"}}}
            }),
        ),
        tool_definition(
            "approve_confirmation",
            "Approve a pending bash confirmation",
            serde_json::json!({
                "type": "object",
                "required": ["request_id"],
                "properties": {"request_id": {"type": "string"}}
            }),
        ),
        tool_definition(
            "deny_confirmation",
            "Deny a pending bash confirmation",
            serde_json::json!({
                "type": "object",
                "required": ["request_id"],
                "properties": {"request_id": {"type": "string"}}
            }),
        ),
        tool_definition(
            "compact_history",
            "Compact conversation history through SessionCommand::CompactHistory",
            serde_json::json!({"type": "object"}),
        ),
        tool_definition(
            "session_status",
            "Read current session-control bridge status without mutation",
            serde_json::json!({"type": "object"}),
        ),
        tool_definition(
            "session_history",
            "Request conversation history replay through SessionCommand::ReplayHistory",
            serde_json::json!({"type": "object"}),
        ),
    ]
}

fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

pub fn effect_for_tool_call(name: &str, args: &Value) -> Result<McpSessionEffect, McpControlError> {
    match name {
        "send_prompt" => Ok(McpSessionEffect::Command(SessionCommand::Prompt {
            text: required_string(args, "text")?,
            images: Vec::new(),
        })),
        "interrupt" => Ok(McpSessionEffect::Command(SessionCommand::Abort)),
        "set_thinking_level" => Ok(McpSessionEffect::Command(SessionCommand::SetThinkingLevel {
            level: required_string(args, "level")?,
        })),
        "set_disabled_tools" => Ok(McpSessionEffect::Command(SessionCommand::SetDisabledTools {
            tools: required_string_array(args, "tools")?,
        })),
        "set_capabilities" => Ok(McpSessionEffect::Command(SessionCommand::SetCapabilities {
            capabilities: optional_string_array(args, "capabilities")?,
        })),
        "approve_confirmation" => Ok(McpSessionEffect::Command(SessionCommand::ConfirmBash {
            request_id: required_string(args, "request_id")?,
            approved: true,
        })),
        "deny_confirmation" => Ok(McpSessionEffect::Command(SessionCommand::ConfirmBash {
            request_id: required_string(args, "request_id")?,
            approved: false,
        })),
        "compact_history" => Ok(McpSessionEffect::Command(SessionCommand::CompactHistory)),
        "session_status" => Ok(McpSessionEffect::ReadOnly {
            action: "session_status".to_string(),
        }),
        "session_history" => Ok(McpSessionEffect::Command(SessionCommand::ReplayHistory)),
        other => Err(McpControlError::unknown_tool(other)),
    }
}

fn required_string(args: &Value, key: &str) -> Result<String, McpControlError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| McpControlError::invalid_request(format!("missing or blank string argument '{key}'")))
}

fn required_string_array(args: &Value, key: &str) -> Result<Vec<String>, McpControlError> {
    optional_string_array(args, key)?
        .ok_or_else(|| McpControlError::invalid_request(format!("missing string array argument '{key}'")))
}

fn optional_string_array(args: &Value, key: &str) -> Result<Option<Vec<String>>, McpControlError> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let array = value.as_array().ok_or_else(|| {
        McpControlError::invalid_request(format!("argument '{key}' must be an array of strings or null"))
    })?;
    let mut values = Vec::with_capacity(array.len());
    for item in array {
        let Some(text) = item.as_str() else {
            return Err(McpControlError::invalid_request(format!("argument '{key}' must contain only strings")));
        };
        values.push(text.to_string());
    }
    Ok(Some(values))
}

impl McpControlResponse {
    fn ok(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, err: McpControlError) -> Self {
        let code = match err.kind {
            McpControlErrorKind::UnsupportedMethod => -32601,
            McpControlErrorKind::UnknownTool => -32602,
            McpControlErrorKind::InvalidRequest => -32600,
        };
        Self {
            id,
            result: None,
            error: Some(McpControlErrorResponse {
                code,
                message: err.message,
                data: serde_json::json!({
                    "source": "mcp_session_control",
                    "transport": "stdio",
                    "status": "error",
                }),
            }),
        }
    }
}

pub fn handle_request(request: McpControlRequest, session_id: Option<&str>) -> McpControlResponse {
    let mut dispatch = |_command: SessionCommand| McpDispatchEvidence::submitted(Vec::new(), false);
    handle_request_inner(request, session_id, false, &mut dispatch)
}

pub fn handle_request_with_dispatch<F>(
    request: McpControlRequest,
    session_id: Option<&str>,
    dispatch: &mut F,
) -> McpControlResponse
where
    F: FnMut(SessionCommand) -> bool,
{
    let mut evidence_dispatch = |command: SessionCommand| McpDispatchEvidence::from(dispatch(command));
    handle_request_inner(request, session_id, true, &mut evidence_dispatch)
}

pub fn handle_request_with_evidence_dispatch<F>(
    request: McpControlRequest,
    session_id: Option<&str>,
    dispatch: &mut F,
) -> McpControlResponse
where
    F: FnMut(SessionCommand) -> McpDispatchEvidence,
{
    handle_request_inner(request, session_id, true, dispatch)
}

fn handle_request_inner<F>(
    request: McpControlRequest,
    session_id: Option<&str>,
    should_dispatch: bool,
    dispatch: &mut F,
) -> McpControlResponse
where
    F: FnMut(SessionCommand) -> McpDispatchEvidence,
{
    match request.method.as_str() {
        "initialize" => McpControlResponse::ok(
            request.id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "clankers-mcp-session-control", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"tools": {}, "resources": {}},
                "metadata": metadata_for_status("ok"),
            }),
        ),
        "tools/list" => McpControlResponse::ok(
            request.id,
            serde_json::json!({
                "tools": supported_tools(),
                "metadata": metadata_for_status("ok"),
            }),
        ),
        "tools/call" => handle_tools_call(request.id, &request.params, session_id, should_dispatch, dispatch),
        other => McpControlResponse::error(request.id, McpControlError::unsupported_method(other)),
    }
}

fn handle_tools_call<F>(
    id: Value,
    params: &Value,
    session_id: Option<&str>,
    should_dispatch: bool,
    dispatch: &mut F,
) -> McpControlResponse
where
    F: FnMut(SessionCommand) -> McpDispatchEvidence,
{
    let tool_name = match params.get("name").and_then(Value::as_str) {
        Some(name) if !name.trim().is_empty() => name,
        _ => return McpControlResponse::error(id, McpControlError::invalid_request("missing tool name")),
    };
    let args = params.get("arguments").unwrap_or(&Value::Null);
    match effect_for_tool_call(tool_name, args) {
        Ok(effect) => match tool_result(tool_name, effect, session_id, should_dispatch, dispatch) {
            Ok(result) => McpControlResponse::ok(id, result),
            Err(err) => McpControlResponse::error(id, err),
        },
        Err(err) => McpControlResponse::error(id, err),
    }
}

fn tool_result<F>(
    tool_name: &str,
    effect: McpSessionEffect,
    session_id: Option<&str>,
    should_dispatch: bool,
    dispatch: &mut F,
) -> Result<Value, McpControlError>
where
    F: FnMut(SessionCommand) -> McpDispatchEvidence,
{
    let (status, command_value, read_only, evidence) = match effect {
        McpSessionEffect::Command(command) => {
            let evidence = if should_dispatch {
                let evidence = dispatch(command.clone());
                if !evidence.submitted {
                    return Err(McpControlError::invalid_request("failed to submit session command"));
                }
                evidence
            } else {
                McpDispatchEvidence::submitted(Vec::new(), false)
            };
            ("accepted", serde_json::to_value(command).expect("SessionCommand serializes"), Value::Null, evidence)
        }
        McpSessionEffect::ReadOnly { action } => (
            "ok",
            Value::Null,
            serde_json::json!({"action": action}),
            McpDispatchEvidence::submitted(Vec::new(), false),
        ),
    };
    Ok(serde_json::json!({
        "content": [{"type": "text", "text": format!("clankers MCP session-control {tool_name}: {status}")}],
        "receipt": {
            "source": "mcp_session_control",
            "transport": "stdio",
            "session_id": session_id,
            "action": tool_name,
            "status": status,
            "command": command_value,
            "read_only": read_only,
            "evidence": {
                "event_count": evidence.events.len(),
                "events": evidence.events,
                "disconnected": evidence.disconnected,
            },
        }
    }))
}

pub fn summarize_daemon_event(event: &DaemonEvent) -> Value {
    match event {
        DaemonEvent::AgentStart => serde_json::json!({"type": "AgentStart"}),
        DaemonEvent::AgentEnd => serde_json::json!({"type": "AgentEnd"}),
        DaemonEvent::ContentBlockStart { is_thinking } => {
            serde_json::json!({"type": "ContentBlockStart", "is_thinking": is_thinking})
        }
        DaemonEvent::ContentBlockStop => serde_json::json!({"type": "ContentBlockStop"}),
        DaemonEvent::TextDelta { text } => serde_json::json!({"type": "TextDelta", "text_len": text.chars().count()}),
        DaemonEvent::ThinkingDelta { text } => {
            serde_json::json!({"type": "ThinkingDelta", "text_len": text.chars().count()})
        }
        DaemonEvent::ToolCall { tool_name, call_id, .. } => {
            serde_json::json!({"type": "ToolCall", "tool_name": tool_name, "call_id": call_id})
        }
        DaemonEvent::ToolStart { call_id, tool_name } => {
            serde_json::json!({"type": "ToolStart", "tool_name": tool_name, "call_id": call_id})
        }
        DaemonEvent::ToolOutput { call_id, text, images } => {
            serde_json::json!({"type": "ToolOutput", "call_id": call_id, "text_len": text.chars().count(), "image_count": images.len()})
        }
        DaemonEvent::ToolProgressUpdate { call_id, .. } => {
            serde_json::json!({"type": "ToolProgressUpdate", "call_id": call_id})
        }
        DaemonEvent::ToolChunk {
            call_id,
            content,
            content_type,
        } => {
            serde_json::json!({"type": "ToolChunk", "call_id": call_id, "content_type": content_type, "content_len": content.chars().count()})
        }
        DaemonEvent::ToolDone {
            call_id,
            text,
            images,
            is_error,
        } => {
            serde_json::json!({"type": "ToolDone", "call_id": call_id, "text_len": text.chars().count(), "image_count": images.len(), "is_error": is_error})
        }
        DaemonEvent::UserInput {
            text,
            agent_msg_count,
            timestamp,
        } => {
            serde_json::json!({"type": "UserInput", "text_len": text.chars().count(), "agent_msg_count": agent_msg_count, "timestamp": timestamp})
        }
        DaemonEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => {
            serde_json::json!({"type": "SessionCompaction", "compacted_count": compacted_count, "tokens_saved": tokens_saved})
        }
        DaemonEvent::UsageUpdate {
            input_tokens,
            output_tokens,
            cache_read,
            model,
        } => {
            serde_json::json!({"type": "UsageUpdate", "input_tokens": input_tokens, "output_tokens": output_tokens, "cache_read": cache_read, "model": model})
        }
        DaemonEvent::ModelChanged { from, to, reason } => {
            serde_json::json!({"type": "ModelChanged", "from": from, "to": to, "reason": reason})
        }
        DaemonEvent::ConfirmRequest {
            request_id,
            command,
            working_dir,
        } => {
            serde_json::json!({"type": "ConfirmRequest", "request_id": request_id, "command_len": command.chars().count(), "working_dir": working_dir})
        }
        DaemonEvent::TodoRequest { request_id, .. } => {
            serde_json::json!({"type": "TodoRequest", "request_id": request_id})
        }
        DaemonEvent::SessionInfo {
            session_id,
            model,
            available_models,
            active_account,
            disabled_tools,
            auto_test_command,
            ..
        } => {
            serde_json::json!({"type": "SessionInfo", "session_id": session_id, "model": model, "available_models": available_models, "active_account": active_account, "disabled_tools": disabled_tools, "auto_test_command": auto_test_command})
        }
        DaemonEvent::SystemPromptResponse { prompt } => {
            serde_json::json!({"type": "SystemPromptResponse", "prompt_len": prompt.chars().count()})
        }
        DaemonEvent::SubagentStarted { id, name, task, pid } => {
            serde_json::json!({"type": "SubagentStarted", "id": id, "name": name, "task_len": task.chars().count(), "pid": pid})
        }
        DaemonEvent::SubagentOutput { id, line } => {
            serde_json::json!({"type": "SubagentOutput", "id": id, "line_len": line.chars().count()})
        }
        DaemonEvent::SubagentDone { id } => serde_json::json!({"type": "SubagentDone", "id": id}),
        DaemonEvent::SubagentError { id, message } => {
            serde_json::json!({"type": "SubagentError", "id": id, "message_len": message.chars().count()})
        }
        DaemonEvent::Capabilities { capabilities } => {
            serde_json::json!({"type": "Capabilities", "capabilities": capabilities})
        }
        DaemonEvent::ToolBlocked {
            call_id,
            tool_name,
            reason,
        } => serde_json::json!({"type": "ToolBlocked", "call_id": call_id, "tool_name": tool_name, "reason": reason}),
        DaemonEvent::ToolList { tools } => {
            serde_json::json!({"type": "ToolList", "tool_count": tools.len(), "tools": tools.iter().map(|tool| tool.name.clone()).collect::<Vec<_>>()})
        }
        DaemonEvent::DisabledToolsChanged { tools } => {
            serde_json::json!({"type": "DisabledToolsChanged", "tools": tools})
        }
        DaemonEvent::ThinkingLevelChanged { from, to } => {
            serde_json::json!({"type": "ThinkingLevelChanged", "from": from, "to": to})
        }
        DaemonEvent::LoopStatus {
            active,
            iteration,
            max_iterations,
            break_condition,
        } => {
            serde_json::json!({"type": "LoopStatus", "active": active, "iteration": iteration, "max_iterations": max_iterations, "break_condition": break_condition})
        }
        DaemonEvent::AutoTestChanged { enabled, command } => {
            serde_json::json!({"type": "AutoTestChanged", "enabled": enabled, "command": command})
        }
        DaemonEvent::CostUpdate {
            total_cost_usd,
            total_input_tokens,
            total_output_tokens,
        } => {
            serde_json::json!({"type": "CostUpdate", "total_cost_usd": total_cost_usd, "total_input_tokens": total_input_tokens, "total_output_tokens": total_output_tokens})
        }
        DaemonEvent::SystemMessage { text, is_error } => {
            serde_json::json!({"type": "SystemMessage", "text_len": text.chars().count(), "is_error": is_error})
        }
        DaemonEvent::PromptDone { error } => {
            serde_json::json!({"type": "PromptDone", "error": error.as_ref().map(|text| text.chars().count())})
        }
        DaemonEvent::PluginWidget { plugin, widget } => {
            serde_json::json!({"type": "PluginWidget", "plugin": plugin, "has_widget": widget.is_some()})
        }
        DaemonEvent::PluginStatus { plugin, text, color } => {
            serde_json::json!({"type": "PluginStatus", "plugin": plugin, "text_len": text.as_ref().map(|text| text.chars().count()), "color": color})
        }
        DaemonEvent::PluginNotify { plugin, message, level } => {
            serde_json::json!({"type": "PluginNotify", "plugin": plugin, "message_len": message.chars().count(), "level": level})
        }
        DaemonEvent::PluginList { plugins } => serde_json::json!({"type": "PluginList", "plugin_count": plugins.len()}),
        DaemonEvent::ScheduleFire {
            schedule_id,
            schedule_name,
            fire_count,
            ..
        } => {
            serde_json::json!({"type": "ScheduleFire", "schedule_id": schedule_id, "schedule_name": schedule_name, "fire_count": fire_count})
        }
        DaemonEvent::HistoryBlock { block } => {
            serde_json::json!({"type": "HistoryBlock", "block_kind": block.get("role").and_then(Value::as_str).unwrap_or("unknown"), "block_bytes": block.to_string().len()})
        }
        DaemonEvent::HistoryEnd => serde_json::json!({"type": "HistoryEnd"}),
    }
}

pub fn handle_json_line(line: &str, session_id: Option<&str>) -> Result<String, serde_json::Error> {
    let request: McpControlRequest = serde_json::from_str(line)?;
    let response = handle_request(request, session_id);
    serde_json::to_string(&response)
}

pub fn handle_json_line_with_dispatch<F>(
    line: &str,
    session_id: Option<&str>,
    dispatch: &mut F,
) -> Result<String, serde_json::Error>
where
    F: FnMut(SessionCommand) -> bool,
{
    let request: McpControlRequest = serde_json::from_str(line)?;
    let response = handle_request_with_dispatch(request, session_id, dispatch);
    serde_json::to_string(&response)
}

pub fn handle_json_line_with_evidence_dispatch<F>(
    line: &str,
    session_id: Option<&str>,
    dispatch: &mut F,
) -> Result<String, serde_json::Error>
where
    F: FnMut(SessionCommand) -> McpDispatchEvidence,
{
    let request: McpControlRequest = serde_json::from_str(line)?;
    let response = handle_request_with_evidence_dispatch(request, session_id, dispatch);
    serde_json::to_string(&response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_prompt_tool_to_session_command() {
        let effect = effect_for_tool_call("send_prompt", &serde_json::json!({"text": "hello"})).expect("effect");
        assert_eq!(
            effect,
            McpSessionEffect::Command(SessionCommand::Prompt {
                text: "hello".to_string(),
                images: Vec::new(),
            })
        );
    }

    #[test]
    fn maps_confirmation_tools_to_normal_confirm_command() {
        let approved =
            effect_for_tool_call("approve_confirmation", &serde_json::json!({"request_id": "req-1"})).expect("effect");
        let denied =
            effect_for_tool_call("deny_confirmation", &serde_json::json!({"request_id": "req-1"})).expect("effect");
        assert_eq!(
            approved,
            McpSessionEffect::Command(SessionCommand::ConfirmBash {
                request_id: "req-1".to_string(),
                approved: true,
            })
        );
        assert_eq!(
            denied,
            McpSessionEffect::Command(SessionCommand::ConfirmBash {
                request_id: "req-1".to_string(),
                approved: false,
            })
        );
    }

    #[test]
    fn rejects_private_unknown_tools() {
        let err = effect_for_tool_call("mutate_tui_app", &Value::Null).expect_err("unknown tools are rejected");
        assert_eq!(err.kind, McpControlErrorKind::UnknownTool);
    }

    #[test]
    fn tools_call_returns_receipt_with_command_not_raw_secret_metadata() {
        let line = r#"{"id":1,"method":"tools/call","params":{"name":"send_prompt","arguments":{"text":"secret-ish prompt"}}}"#;
        let response = handle_json_line(line, Some("sess-1")).expect("json response");
        let value: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(value["result"]["receipt"]["source"], "mcp_session_control");
        assert_eq!(value["result"]["receipt"]["session_id"], "sess-1");
        assert_eq!(value["result"]["receipt"]["command"]["Prompt"]["text"], "secret-ish prompt");
        assert!(value["result"]["receipt"].get("environment").is_none());
    }

    #[test]
    fn dispatch_path_submits_normal_session_command() {
        let line = r#"{"id":9,"method":"tools/call","params":{"name":"interrupt","arguments":{}}}"#;
        let mut submitted = Vec::new();
        let mut dispatch = |cmd: SessionCommand| {
            submitted.push(cmd);
            true
        };

        let response = handle_json_line_with_dispatch(line, Some("sess-1"), &mut dispatch).expect("json response");
        let value: Value = serde_json::from_str(&response).expect("response json");

        assert_eq!(submitted, vec![SessionCommand::Abort]);
        assert_eq!(value["result"]["receipt"]["status"], "accepted");
        assert_eq!(value["result"]["receipt"]["command"], serde_json::json!("Abort"));
    }

    #[test]
    fn failed_dispatch_returns_error_receipt() {
        let line = r#"{"id":10,"method":"tools/call","params":{"name":"compact_history","arguments":{}}}"#;
        let mut dispatch = |_cmd: SessionCommand| false;

        let response = handle_json_line_with_dispatch(line, Some("sess-1"), &mut dispatch).expect("json response");
        let value: Value = serde_json::from_str(&response).expect("response json");

        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["source"], "mcp_session_control");
    }

    #[test]
    fn initializes_and_lists_tools() {
        let init = handle_request(
            McpControlRequest {
                id: serde_json::json!(1),
                method: "initialize".to_string(),
                params: Value::Null,
            },
            None,
        );
        assert!(init.error.is_none());
        let listed = handle_request(
            McpControlRequest {
                id: serde_json::json!(2),
                method: "tools/list".to_string(),
                params: Value::Null,
            },
            None,
        );
        let tools = listed.result.expect("tools result")["tools"].as_array().expect("tools array").clone();
        assert!(tools.iter().any(|tool| tool["name"] == "send_prompt"));
        assert!(tools.iter().any(|tool| tool["name"] == "approve_confirmation"));
        assert!(tools.iter().any(|tool| tool["name"] == "session_history"));
    }

    #[test]
    fn evidence_dispatch_attaches_summarized_daemon_events() {
        let line =
            r#"{"id":11,"method":"tools/call","params":{"name":"set_thinking_level","arguments":{"level":"high"}}}"#;
        let mut dispatch = |_cmd: SessionCommand| {
            McpDispatchEvidence::submitted(
                vec![serde_json::json!({"type": "ThinkingLevelChanged", "from": "off", "to": "high"})],
                false,
            )
        };

        let response =
            handle_json_line_with_evidence_dispatch(line, Some("sess-1"), &mut dispatch).expect("json response");
        let value: Value = serde_json::from_str(&response).expect("response json");

        assert_eq!(value["result"]["receipt"]["evidence"]["event_count"], 1);
        assert_eq!(value["result"]["receipt"]["evidence"]["events"][0]["type"], "ThinkingLevelChanged");
    }

    #[test]
    fn daemon_event_summary_omits_raw_prompt_and_history_text() {
        let user_event = DaemonEvent::UserInput {
            text: "sensitive prompt text".to_string(),
            agent_msg_count: 1,
            timestamp: "2026-05-04T00:00:00Z".to_string(),
        };
        let history_event = DaemonEvent::HistoryBlock {
            block: serde_json::json!({"role": "user", "content": "sensitive history text"}),
        };

        let user_summary = summarize_daemon_event(&user_event);
        let history_summary = summarize_daemon_event(&history_event);

        assert_eq!(user_summary["type"], "UserInput");
        assert_eq!(user_summary["text_len"], 21);
        assert!(user_summary.to_string().find("sensitive prompt text").is_none());
        assert_eq!(history_summary["type"], "HistoryBlock");
        assert_eq!(history_summary["block_kind"], "user");
        assert!(history_summary.to_string().find("sensitive history text").is_none());
    }
}
