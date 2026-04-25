//! Wire protocol types and frame I/O helpers

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::error::Error;
use crate::error::Result;

/// JSON-RPC-like request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC-like response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// RPC error payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

/// Streaming notification (no id, not a final response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            id: Some(1),
            method: method.into(),
            params,
        }
    }
}

impl Response {
    pub fn success(id: Option<u64>, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<u64>, code: i32, message: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }

    /// True if this is an error response.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

// ── Frame I/O ───────────────────────────────────────────────────────────

/// Write a length-prefixed frame to a QUIC send stream.
pub async fn write_frame(send: &mut iroh::endpoint::SendStream, data: &[u8]) -> Result<()> {
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len).await.map_err(io_err)?;
    send.write_all(data).await.map_err(io_err)?;
    Ok(())
}

/// Read a length-prefixed frame from a QUIC recv stream.
pub async fn read_frame(recv: &mut iroh::endpoint::RecvStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.map_err(io_err)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 10_000_000 {
        return Err(Error::Streaming {
            message: "Frame too large (>10MB)".to_string(),
        });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(io_err)?;
    Ok(data)
}

fn io_err(e: impl std::fmt::Display) -> Error {
    Error::Streaming {
        message: format!("RPC I/O error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_request_serialization_roundtrip() {
        let req = Request::new("complete", json!({"model": "gpt-4o"}));
        assert_eq!(req.id, Some(1));
        assert_eq!(req.method, "complete");

        let json = serde_json::to_string(&req).unwrap();
        let decoded: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.method, "complete");
        assert_eq!(decoded.params["model"], "gpt-4o");
    }

    #[test]
    fn test_response_success() {
        let resp = Response::success(Some(1), json!({"status": "ok"}));
        assert!(!resp.is_error());
        assert_eq!(resp.id, Some(1));
        assert_eq!(resp.result.unwrap()["status"], "ok");
    }

    #[test]
    fn test_response_error() {
        let resp = Response::error(Some(1), -32601, "method not found");
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "method not found");
        assert!(resp.result.is_none());
    }

    #[test]
    fn test_response_serialization_skips_none() {
        let resp = Response::success(Some(1), json!("ok"));
        let json = serde_json::to_string(&resp).unwrap();
        // "error" field should be absent (skip_serializing_if)
        assert!(!json.contains("\"error\""));

        let err_resp = Response::error(Some(1), -1, "fail");
        let json = serde_json::to_string(&err_resp).unwrap();
        // "result" field should be absent
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_notification_serialization() {
        let notif = Notification {
            method: "stream.event".into(),
            params: json!({"type": "MessageStop"}),
        };
        let json = serde_json::to_string(&notif).unwrap();
        let decoded: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.method, "stream.event");
    }

    #[test]
    fn test_request_default_params() {
        // Deserializing without "params" should give default (null)
        let json = r#"{"id":1,"method":"status"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "status");
        assert!(req.params.is_null());
    }

    #[test]
    fn test_notification_vs_response_discrimination() {
        // Notification has "method" but no "id"
        let notif = Notification {
            method: "stream.event".into(),
            params: json!({"type": "TextDelta", "text": "hello"}),
        };
        let json = serde_json::to_value(&notif).unwrap();
        assert!(json.get("method").is_some());
        assert!(json.get("id").is_none());

        // Response has "id"
        let resp = Response::success(Some(1), json!("done"));
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("id").is_some());
    }
}
