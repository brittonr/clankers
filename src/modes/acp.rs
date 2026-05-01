use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpAdapterErrorKind {
    UnsupportedMethod,
    InvalidRequest,
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
            message: format!("ACP method '{method}' is not supported by this first-pass adapter"),
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
        "source": "acp_ide_integration",
        "transport": "stdio",
        "method": method,
        "status": status,
    })
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AcpRequest {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
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
        };
        Self {
            id,
            result: None,
            error: Some(AcpErrorResponse {
                code,
                message: err.message,
                data: serde_json::json!({
                    "source": "acp_ide_integration",
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
            "capabilities": {
                "sessions": true,
                "prompts": true,
                "terminals": false,
                "diffs": false,
                "multiWorkspace": false,
            },
            "metadata": metadata_for_method("initialize", "ok"),
        }),
        "session/new" => serde_json::json!({
            "session": {
                "id": request.params.get("session_id").cloned().unwrap_or(Value::Null),
                "status": "accepted",
            },
            "metadata": metadata_for_method("session/new", "ok"),
        }),
        "session/prompt" => serde_json::json!({
            "accepted": true,
            "metadata": metadata_for_method("session/prompt", "accepted"),
        }),
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
    fn acp_json_line_returns_loggable_metadata_without_params() {
        let line = r#"{"id":8,"method":"session/prompt","params":{"prompt":"secret-ish text"}}"#;
        let (_response, metadata) = handle_json_line_with_metadata(line).expect("metadata response");
        assert_eq!(metadata["source"], "acp_ide_integration");
        assert_eq!(metadata["method"], "session/prompt");
        assert_eq!(metadata["status"], "ok");
        assert!(metadata.get("params").is_none());
        assert!(!metadata.to_string().contains("secret-ish"));
    }
}
