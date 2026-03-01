//! Line-delimited JSON over stdin/stdout.
//!
//! Reads `Request` objects from stdin (one per line), executes them,
//! and writes `Response` objects to stdout. Streaming methods (prompt)
//! emit intermediate notification frames before the final response.

use std::io::BufRead;
use std::io::Write;
use std::io::{self};
use std::sync::Arc;

use serde_json::json;

use super::protocol::Request;
use super::protocol::Response;
use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::provider::Provider;
use crate::provider::streaming::ContentDelta;

/// Context needed to build agents for prompt execution.
pub struct RpcContext {
    pub provider: Arc<dyn Provider>,
    pub tools: Vec<Arc<dyn crate::tools::Tool>>,
    pub settings: crate::config::settings::Settings,
    pub model: String,
    pub system_prompt: String,
}

/// Run the stdio server.
pub async fn run_stdio_rpc(ctx: RpcContext) -> crate::error::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| crate::error::Error::Io { source: e })?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::error(format!("Parse error: {}", e));
                writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap_or_default())
                    .map_err(|e| crate::error::Error::Io { source: e })?;
                continue;
            }
        };

        let response = handle_request(&request, &ctx).await;

        writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_default())
            .map_err(|e| crate::error::Error::Io { source: e })?;
        stdout.flush().map_err(|e| crate::error::Error::Io { source: e })?;
    }

    Ok(())
}

async fn handle_request(request: &Request, ctx: &RpcContext) -> Response {
    match request.method.as_str() {
        "ping" => Response::success(json!("pong")),

        "version" => Response::success(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "name": "clankers"
        })),

        "prompt" => handle_prompt(request, ctx).await,

        _ => Response::error(format!("Method not found: {}", request.method)),
    }
}

/// Handle the `prompt` method.
///
/// Expects params: `{ "text": "..." }` and optionally
/// `{ "model": "...", "system_prompt": "..." }`.
///
/// Streams notification lines to stdout during execution:
/// - `{ "type": "text_delta", "text": "..." }`
/// - `{ "type": "tool_call", ... }`
/// - `{ "type": "tool_result", ... }`
///
/// Returns the final collected text as the response.
async fn handle_prompt(request: &Request, ctx: &RpcContext) -> Response {
    let text = match request.params.get("text").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            return Response::error("Missing required param: \"text\"");
        }
    };

    let model = request
        .params
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| ctx.model.clone());

    let system_prompt = request
        .params
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| ctx.system_prompt.clone());

    let mut agent =
        Agent::new(Arc::clone(&ctx.provider), ctx.tools.clone(), ctx.settings.clone(), model, system_prompt);

    let mut rx = agent.subscribe();

    let notification_handle = tokio::spawn(async move {
        let mut collected_text = String::new();
        let stdout = io::stdout();

        while let Ok(event) = rx.recv().await {
            let frame = match event {
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::TextDelta { ref text },
                    ..
                } => {
                    collected_text.push_str(text);
                    Some(json!({ "type": "text_delta", "text": text }))
                }
                AgentEvent::ToolCall {
                    ref tool_name,
                    ref call_id,
                    ref input,
                } => Some(json!({
                    "type": "tool_call",
                    "tool_name": tool_name,
                    "call_id": call_id,
                    "input": input,
                })),
                AgentEvent::ToolExecutionEnd {
                    ref call_id,
                    ref result,
                    is_error,
                } => Some(json!({
                    "type": "tool_result",
                    "call_id": call_id,
                    "content": format!("{:?}", result),
                    "is_error": is_error,
                })),
                AgentEvent::AgentEnd { .. } => break,
                _ => None,
            };

            if let Some(frame) = frame {
                let mut out = stdout.lock();
                let _ = writeln!(out, "{}", serde_json::to_string(&frame).unwrap_or_default());
                let _ = out.flush();
            }
        }

        collected_text
    });

    if let Err(e) = agent.prompt(&text).await {
        return Response::error(format!("Agent error: {}", e));
    }

    let collected_text = match notification_handle.await {
        Ok(text) => text,
        Err(e) => {
            return Response::error(format!("Internal error: {}", e));
        }
    };

    Response::success(json!({
        "text": collected_text,
        "status": "complete",
    }))
}
