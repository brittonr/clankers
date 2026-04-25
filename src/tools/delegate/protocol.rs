//! Remote worker communication protocol via iroh RPC

use std::sync::Arc;

use clanker_tui_types::SubagentEvent;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::tools::ToolResult;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

/// Lazily-initialized, shared iroh endpoint. Created once on first remote
/// delegation, then reused for all subsequent calls across the process lifetime.
pub type SharedEndpoint = Arc<tokio::sync::OnceCell<iroh::Endpoint>>;

/// Maximum number of retry attempts for transient connection failures.
const MAX_RETRIES: u32 = 3;
/// Initial backoff delay between retries.
const RETRY_BACKOFF_MS: u64 = 500;
/// Length of short node ID display (first N chars of node ID)
const NODE_ID_DISPLAY_LEN: usize = 12;
/// Length of task preview for panel display
const TASK_PREVIEW_SHORT_LEN: usize = 60;

pub enum RemoteCallError {
    ConnectionError(String),
    ApplicationError(String),
    Cancelled,
}

/// Run a worker via iroh RPC to a remote peer.
///
/// Uses a shared endpoint (created lazily on first call) and retries transient
/// connection failures with exponential backoff. Streams text deltas to the
/// subagent panel in real-time via `send_rpc_streaming`.
pub async fn run_remote_worker(
    worker_name: &str,
    task: &str,
    node_id: &str,
    identity_path: &std::path::Path,
    shared_endpoint: SharedEndpoint,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
) -> ToolResult {
    use crate::modes::rpc::iroh;
    use crate::modes::rpc::protocol::Request;

    let sub_id = format!("worker:{}", worker_name);
    let task_preview: String = task.chars().take(TASK_PREVIEW_SHORT_LEN).collect();
    let short_node = &node_id[..NODE_ID_DISPLAY_LEN.min(node_id.len())];

    emit_started_event(panel_tx, &sub_id, worker_name, short_node, &task_preview);

    let remote = match parse_peer_node_id(node_id, short_node, panel_tx, &sub_id) {
        Ok(pk) => pk,
        Err(result) => return result,
    };

    // Get or create the shared endpoint (once per process lifetime)
    let identity_path_owned = identity_path.to_path_buf();
    let endpoint = match shared_endpoint
        .get_or_try_init(|| async {
            let identity = iroh::Identity::load_or_generate(&identity_path_owned);
            iroh::start_endpoint(&identity).await
        })
        .await
    {
        Ok(ep) => ep,
        Err(e) => {
            let msg = format!("Failed to start iroh endpoint: {}", e);
            if let Some(tx) = panel_tx {
                tx.send(SubagentEvent::Error {
                    id: sub_id,
                    message: msg.clone(),
                })
                .ok();
            }
            return ToolResult::error(msg);
        }
    };

    let request = Request::new("prompt", serde_json::json!({ "text": task }));

    retry_remote_call(worker_name, short_node, endpoint, remote, &request, &sub_id, panel_tx, signal).await
}

/// Emit a "Started" event to the subagent panel
fn emit_started_event(
    panel_tx: Option<&PanelTx>,
    sub_id: &str,
    worker_name: &str,
    short_node: &str,
    task_preview: &str,
) {
    if let Some(tx) = panel_tx {
        tx.send(SubagentEvent::Started {
            id: sub_id.to_string(),
            name: format!("{} → {}", worker_name, short_node),
            task: task_preview.to_string(),
            pid: None,
        })
        .ok();
    }
}

/// Parse the peer node ID. Returns error ToolResult on failure.
fn parse_peer_node_id(
    node_id: &str,
    short_node: &str,
    panel_tx: Option<&PanelTx>,
    sub_id: &str,
) -> Result<::iroh::PublicKey, ToolResult> {
    match node_id.parse() {
        Ok(pk) => Ok(pk),
        Err(e) => {
            let msg = format!("Invalid peer node ID '{}': {}", short_node, e);
            if let Some(tx) = panel_tx {
                tx.send(SubagentEvent::Error {
                    id: sub_id.to_string(),
                    message: msg.clone(),
                })
                .ok();
            }
            Err(ToolResult::error(msg))
        }
    }
}

/// Retry the remote RPC call with exponential backoff
async fn retry_remote_call(
    worker_name: &str,
    short_node: &str,
    endpoint: &iroh::Endpoint,
    remote: ::iroh::PublicKey,
    request: &crate::modes::rpc::protocol::Request,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
) -> ToolResult {
    let mut last_err = String::new();

    for attempt in 0..=MAX_RETRIES {
        // Exponential backoff before retries
        if attempt > 0
            && let Err(result) = wait_for_retry(attempt, sub_id, panel_tx, &signal).await
        {
            return result;
        }

        // Attempt the RPC call
        match try_remote_call(endpoint, remote, request, sub_id, panel_tx, &signal).await {
            Ok(text) => {
                if let Some(tx) = panel_tx {
                    tx.send(SubagentEvent::Done { id: sub_id.to_string() }).ok();
                }
                return ToolResult::text(format!("[remote:{}] {}", short_node, text));
            }
            Err(RemoteCallError::ApplicationError(msg)) => {
                if let Some(tx) = panel_tx {
                    tx.send(SubagentEvent::Error {
                        id: sub_id.to_string(),
                        message: msg.clone(),
                    })
                    .ok();
                }
                return ToolResult::error(msg);
            }
            Err(RemoteCallError::Cancelled) => {
                return ToolResult::error(format!("Remote worker '{}' cancelled", worker_name));
            }
            Err(RemoteCallError::ConnectionError(e)) => {
                last_err = e;
                tracing::warn!(
                    "Remote worker '{}' attempt {}/{} failed: {}",
                    worker_name,
                    attempt + 1,
                    MAX_RETRIES + 1,
                    last_err
                );
            }
        }
    }

    // All retries exhausted
    let msg = format!("Failed to reach peer '{}' after {} attempts: {}", short_node, MAX_RETRIES + 1, last_err);
    if let Some(tx) = panel_tx {
        tx.send(SubagentEvent::Error {
            id: sub_id.to_string(),
            message: msg.clone(),
        })
        .ok();
    }
    ToolResult::error(msg)
}

/// Try a single remote RPC call with streaming notifications
async fn try_remote_call(
    endpoint: &iroh::Endpoint,
    remote: ::iroh::PublicKey,
    request: &crate::modes::rpc::protocol::Request,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: &CancellationToken,
) -> Result<String, RemoteCallError> {
    use crate::modes::rpc::iroh;

    let sub_id_clone = sub_id.to_string();
    let panel_tx_clone = panel_tx.cloned();

    let result = tokio::select! {
        res = iroh::send_rpc_streaming(endpoint, remote, request, move |notification| {
            handle_streaming_notification(notification, &sub_id_clone, panel_tx_clone.as_ref());
        }) => res.map(|(_, response)| response),
        () = signal.cancelled() => {
            if let Some(tx) = panel_tx {
                tx.send(SubagentEvent::Error {
                    id: sub_id.to_string(),
                    message: "Cancelled".into()
                }).ok();
            }
            return Err(RemoteCallError::Cancelled);
        }
    };

    match result {
        Ok(response) => {
            if let Some(result) = response.ok {
                let text = result.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(text.to_string())
            } else if let Some(err) = response.error {
                Err(RemoteCallError::ApplicationError(format!("Remote peer error: {}", err)))
            } else {
                Err(RemoteCallError::ApplicationError("Empty response from remote peer".to_string()))
            }
        }
        Err(e) => Err(RemoteCallError::ConnectionError(e.to_string())),
    }
}

/// Handle streaming RPC notifications (text deltas, tool calls, etc.)
fn handle_streaming_notification(notification: &Value, sub_id: &str, panel_tx: Option<&PanelTx>) {
    let method = notification.get("method").and_then(|v| v.as_str());

    match method {
        Some("agent.text_delta") => {
            if let Some(text) = notification.get("params").and_then(|p| p.get("text")).and_then(|v| v.as_str())
                && let Some(tx) = panel_tx
            {
                for line in text.split('\n') {
                    if !line.is_empty() {
                        tx.send(SubagentEvent::Output {
                            id: sub_id.to_string(),
                            line: line.to_string(),
                        })
                        .ok();
                    }
                }
            }
        }
        Some("agent.tool_call") => {
            if let Some(tx) = panel_tx {
                let tool =
                    notification.get("params").and_then(|p| p.get("tool_name")).and_then(|v| v.as_str()).unwrap_or("?");
                tx.send(SubagentEvent::Output {
                    id: sub_id.to_string(),
                    line: format!("[tool: {}]", tool),
                })
                .ok();
            }
        }
        Some("agent.tool_result") => {
            if let Some(tx) = panel_tx {
                let is_error = notification
                    .get("params")
                    .and_then(|p| p.get("is_error"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if is_error {
                    tx.send(SubagentEvent::Output {
                        id: sub_id.to_string(),
                        line: "[tool error]".to_string(),
                    })
                    .ok();
                }
            }
        }
        _ => {}
    }
}

/// Wait for retry with exponential backoff, checking for cancellation
async fn wait_for_retry(
    attempt: u32,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: &CancellationToken,
) -> Result<(), ToolResult> {
    let backoff = std::time::Duration::from_millis(RETRY_BACKOFF_MS * 2u64.pow(attempt - 1));
    if let Some(tx) = panel_tx {
        tx.send(SubagentEvent::Output {
            id: sub_id.to_string(),
            line: format!("Retry {}/{} after {:?}...", attempt, MAX_RETRIES, backoff),
        })
        .ok();
    }
    tokio::select! {
        () = tokio::time::sleep(backoff) => Ok(()),
        () = signal.cancelled() => {
            if let Some(tx) = panel_tx {
                tx.send(SubagentEvent::Error {
                    id: sub_id.to_string(),
                    message: "Cancelled".into()
                }).ok();
            }
            Err(ToolResult::error("Cancelled during retry backoff".to_string()))
        }
    }
}
