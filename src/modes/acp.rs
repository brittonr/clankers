use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

const ACP_SOURCE: &str = "acp_ide_integration";
const ACP_TRANSPORT: &str = "stdio";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpAdapterErrorKind {
    UnsupportedMethod,
    InvalidRequest,
    MissingSession,
    MissingPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAdapterError {
    pub kind: AcpAdapterErrorKind,
    pub method: String,
    pub message: String,
}

impl AcpAdapterError {
    pub fn unsupported_method(method: impl Into<String>) -> Self {
        let method = method.into();
        Self {
            kind: AcpAdapterErrorKind::UnsupportedMethod,
            message: format!("ACP method '{method}' is not supported by this adapter"),
            method,
        }
    }

    pub fn invalid_request(method: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: AcpAdapterErrorKind::InvalidRequest,
            method: method.into(),
            message: message.into(),
        }
    }

    pub fn missing_session(method: impl Into<String>) -> Self {
        let method = method.into();
        Self {
            kind: AcpAdapterErrorKind::MissingSession,
            message: "ACP prompt requests require a session_id created by session/new or supplied by the editor"
                .to_string(),
            method,
        }
    }

    pub fn missing_prompt(method: impl Into<String>) -> Self {
        let method = method.into();
        Self {
            kind: AcpAdapterErrorKind::MissingPrompt,
            message: "ACP prompt requests require a non-empty prompt".to_string(),
            method,
        }
    }
}

pub fn validate_method(method: &str) -> Result<(), AcpAdapterError> {
    match method {
        "initialize" | "session/new" | "session/prompt" => Ok(()),
        other => Err(AcpAdapterError::unsupported_method(other)),
    }
}

pub fn metadata_for_method(method: &str, status: &str) -> Value {
    serde_json::json!({
        "source": ACP_SOURCE,
        "transport": ACP_TRANSPORT,
        "method": method,
        "status": status,
    })
}

pub fn bind_session(params: &Value) -> Result<AcpSessionReceipt, AcpAdapterError> {
    let mode = if params.get("attach").and_then(Value::as_bool).unwrap_or(false)
        || params.get("session_id").and_then(Value::as_str).is_some()
    {
        AcpSessionMode::Attach
    } else {
        AcpSessionMode::New
    };
    let model = params
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(ToOwned::to_owned);
    let session_id = match mode {
        AcpSessionMode::New => params
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("acp-{}", Uuid::new_v4())),
        AcpSessionMode::Attach => params
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| AcpAdapterError::invalid_request("session/new", "attach mode requires session_id"))?,
    };
    Ok(AcpSessionReceipt {
        source: ACP_SOURCE,
        transport: ACP_TRANSPORT,
        session_id,
        status: "bound",
        mode,
        model,
    })
}

pub fn accept_prompt(params: &Value) -> Result<AcpPromptReceipt, AcpAdapterError> {
    let session_id = params
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AcpAdapterError::missing_session("session/prompt"))?;
    let prompt = params
        .get("prompt")
        .or_else(|| params.get("text"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
        .ok_or_else(|| AcpAdapterError::missing_prompt("session/prompt"))?;
    Ok(AcpPromptReceipt {
        source: ACP_SOURCE,
        transport: ACP_TRANSPORT,
        session_id: session_id.to_string(),
        status: "accepted",
        prompt_bytes: prompt.len(),
        prompt_sha256: sha256_hex(prompt.as_bytes()),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AcpRequest {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpSessionMode {
    New,
    Attach,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSessionBindingRequest {
    pub mode: AcpSessionMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcpSessionReceipt {
    pub source: &'static str,
    pub transport: &'static str,
    pub session_id: String,
    pub status: &'static str,
    pub mode: AcpSessionMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcpPromptReceipt {
    pub source: &'static str,
    pub transport: &'static str,
    pub session_id: String,
    pub status: &'static str,
    pub prompt_bytes: usize,
    pub prompt_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcpEditorCapabilities {
    pub sessions: bool,
    pub prompts: bool,
    pub cancellation: bool,
    pub history_replay: bool,
    pub terminals: bool,
    pub diffs: bool,
    pub multi_workspace: bool,
    pub tool_activity: bool,
}

impl Default for AcpEditorCapabilities {
    fn default() -> Self {
        Self {
            sessions: true,
            prompts: true,
            cancellation: false,
            history_replay: false,
            terminals: false,
            diffs: false,
            multi_workspace: false,
            tool_activity: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AcpResponse {
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AcpErrorResponse>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AcpErrorResponse {
    pub code: i64,
    pub message: String,
    pub data: Value,
}

impl AcpResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, err: AcpAdapterError) -> Self {
        let code = match err.kind {
            AcpAdapterErrorKind::UnsupportedMethod => -32004,
            AcpAdapterErrorKind::InvalidRequest => -32600,
            AcpAdapterErrorKind::MissingSession => -32010,
            AcpAdapterErrorKind::MissingPrompt => -32602,
        };
        Self {
            id,
            result: None,
            error: Some(AcpErrorResponse {
                code,
                message: err.message,
                data: serde_json::json!({
                    "source": ACP_SOURCE,
                    "method": err.method,
                    "status": "unsupported",
                }),
            }),
        }
    }
}

pub fn handle_request(request: AcpRequest) -> AcpResponse {
    if let Err(err) = validate_method(&request.method) {
        return AcpResponse::error(request.id, err);
    }

    let result = match request.method.as_str() {
        "initialize" => serde_json::json!({
            "protocol": "acp",
            "server": "clankers",
            "capabilities": AcpEditorCapabilities::default(),
            "metadata": metadata_for_method("initialize", "ok"),
        }),
        "session/new" => match bind_session(&request.params) {
            Ok(receipt) => serde_json::json!({
                "session": {
                    "id": receipt.session_id,
                    "status": receipt.status,
                    "mode": receipt.mode,
                },
                "metadata": receipt,
            }),
            Err(err) => return AcpResponse::error(request.id, err),
        },
        "session/prompt" => match accept_prompt(&request.params) {
            Ok(receipt) => serde_json::json!({
                "accepted": true,
                "session": { "id": receipt.session_id },
                "metadata": receipt,
            }),
            Err(err) => return AcpResponse::error(request.id, err),
        },
        _ => unreachable!("method was validated"),
    };

    AcpResponse::ok(request.id, result)
}

pub fn handle_json_line(line: &str) -> Result<String, serde_json::Error> {
    let (response, _) = handle_json_line_with_metadata(line)?;
    Ok(response)
}

pub fn handle_json_line_with_metadata(line: &str) -> Result<(String, Value), serde_json::Error> {
    let request: AcpRequest = serde_json::from_str(line)?;
    let method = request.method.clone();
    let response = handle_request(request);
    let status = if response.error.is_some() { "error" } else { "ok" };
    let metadata = metadata_for_method(&method, status);
    Ok((serde_json::to_string(&response)?, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_accepts_first_pass_methods() {
        assert!(validate_method("initialize").is_ok());
        assert!(validate_method("session/new").is_ok());
        assert!(validate_method("session/prompt").is_ok());
    }

    #[test]
    fn acp_rejects_unsupported_methods_explicitly() {
        let err = validate_method("terminal/create").expect_err("terminal management is unsupported");
        assert_eq!(err.kind, AcpAdapterErrorKind::UnsupportedMethod);
        assert_eq!(err.method, "terminal/create");
        assert!(err.message.contains("not supported"));
    }

    #[test]
    fn acp_metadata_is_normalized() {
        let metadata = metadata_for_method("session/prompt", "ok");
        assert_eq!(metadata["source"], "acp_ide_integration");
        assert_eq!(metadata["transport"], "stdio");
        assert_eq!(metadata["method"], "session/prompt");
        assert_eq!(metadata["status"], "ok");
    }

    #[test]
    fn acp_initialize_reports_limited_capabilities() {
        let response = handle_request(AcpRequest {
            id: serde_json::json!(1),
            method: "initialize".to_string(),
            params: Value::Null,
        });
        let result = response.result.expect("initialize returns result");
        assert_eq!(result["server"], "clankers");
        assert_eq!(result["capabilities"]["prompts"], true);
        assert_eq!(result["capabilities"]["terminals"], false);
        assert!(response.error.is_none());
    }

    #[test]
    fn acp_json_line_returns_structured_unsupported_error() {
        let line = r#"{"id":7,"method":"terminal/create","params":{}}"#;
        let response = handle_json_line(line).expect("valid json response");
        let value: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(value["id"], 7);
        assert_eq!(value["error"]["code"], -32004);
        assert_eq!(value["error"]["data"]["status"], "unsupported");
        assert_eq!(value["error"]["data"]["method"], "terminal/create");
    }

    #[test]
    fn acp_session_binding_receipt_uses_safe_metadata() {
        let receipt =
            bind_session(&serde_json::json!({"session_id":"known-session","model":"test-model","prompt":"secret"}))
                .expect("attach existing session");
        assert_eq!(receipt.session_id, "known-session");
        assert_eq!(receipt.mode, AcpSessionMode::Attach);
        assert_eq!(receipt.model.as_deref(), Some("test-model"));
        let serialized = serde_json::to_string(&receipt).expect("receipt json");
        assert!(!serialized.contains("secret"));
    }

    #[test]
    fn acp_prompt_receipt_hashes_prompt_without_raw_text() {
        let receipt =
            accept_prompt(&serde_json::json!({"session_id":"s1","prompt":"secret prompt"})).expect("prompt accepted");
        assert_eq!(receipt.session_id, "s1");
        assert_eq!(receipt.prompt_bytes, "secret prompt".len());
        assert_eq!(receipt.prompt_sha256.len(), 64);
        let serialized = serde_json::to_string(&receipt).expect("prompt receipt json");
        assert!(!serialized.contains("secret prompt"));
    }

    #[test]
    fn acp_prompt_requires_session_before_acceptance() {
        let response = handle_request(AcpRequest {
            id: serde_json::json!(9),
            method: "session/prompt".to_string(),
            params: serde_json::json!({"prompt":"hello"}),
        });
        let error = response.error.expect("missing session is an error");
        assert_eq!(error.code, -32010);
        assert!(!error.data.to_string().contains("hello"));
    }
}
