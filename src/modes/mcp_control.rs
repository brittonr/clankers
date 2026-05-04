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
    let mut dispatch = |_command: SessionCommand| true;
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
    handle_request_inner(request, session_id, true, dispatch)
}

fn handle_request_inner<F>(
    request: McpControlRequest,
    session_id: Option<&str>,
    should_dispatch: bool,
    dispatch: &mut F,
) -> McpControlResponse
where
    F: FnMut(SessionCommand) -> bool,
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
    F: FnMut(SessionCommand) -> bool,
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
    F: FnMut(SessionCommand) -> bool,
{
    let (status, command_value, read_only) = match effect {
        McpSessionEffect::Command(command) => {
            if should_dispatch && !dispatch(command.clone()) {
                return Err(McpControlError::invalid_request("failed to submit session command"));
            }
            ("accepted", serde_json::to_value(command).expect("SessionCommand serializes"), Value::Null)
        }
        McpSessionEffect::ReadOnly { action } => ("ok", Value::Null, serde_json::json!({"action": action})),
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
        }
    }))
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
    }
}
