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
}
