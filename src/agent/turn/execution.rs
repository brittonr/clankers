//! Tool execution logic and turn execution flow

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::CollectedResponse;
use super::ContentBlockBuilder;
use super::TurnConfig;
use crate::agent::events::AgentEvent;
use crate::error::Error;
use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::Usage;
use crate::provider::message::*;
use crate::provider::streaming::*;
use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult as ToolExecResult;
use crate::tools::progress::ToolResultAccumulator;

/// Execute a single turn: build request, stream response, collect results
pub(super) async fn execute_turn(
    provider: &dyn Provider,
    messages: &[AgentMessage],
    config: &TurnConfig,
    active_model: &str,
    tool_defs: &[ToolDefinition],
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: &CancellationToken,
) -> Result<CollectedResponse> {
    let request = CompletionRequest {
        model: active_model.to_string(),
        messages: messages.to_vec(),
        system_prompt: Some(config.system_prompt.clone()),
        max_tokens: config.max_tokens,
        temperature: config.temperature,
        tools: tool_defs.to_vec(),
        thinking: config.thinking.clone(),
    };

    let (stream_tx, mut stream_rx) = mpsc::channel(256);
    let event_tx_clone = event_tx.clone();
    let complete_fut = provider.complete(request, stream_tx);
    let collect_fut = collect_stream_events(&mut stream_rx, &event_tx_clone);

    let (complete_result, collected) = tokio::select! {
        biased;
        () = cancel.cancelled() => {
            return Err(Error::Cancelled);
        }
        result = async { tokio::join!(complete_fut, collect_fut) } => result,
    };
    complete_result?;
    collected
}

/// Collect streaming events into a complete response
pub(super) async fn collect_stream_events(
    stream_rx: &mut mpsc::Receiver<StreamEvent>,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> Result<CollectedResponse> {
    let mut content_builders: Vec<ContentBlockBuilder> = Vec::new();
    let mut model = String::new();
    let mut usage = Usage::default();
    let mut stop_reason = StopReason::Stop;

    while let Some(event) = stream_rx.recv().await {
        match event {
            StreamEvent::MessageStart { message } => {
                model = message.model.clone();
            }
            StreamEvent::ContentBlockStart { index, content_block } => {
                // Ensure we have enough slots
                while content_builders.len() <= index {
                    content_builders.push(ContentBlockBuilder::new(Content::Text { text: String::new() }));
                }
                content_builders[index] = ContentBlockBuilder::new(content_block.clone());

                // Forward to TUI/consumers
                let _ = event_tx.send(AgentEvent::ContentBlockStart { index, content_block });
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                // Forward delta event with index
                let _ = event_tx.send(AgentEvent::MessageUpdate {
                    index,
                    delta: delta.clone(),
                });

                // Apply delta to content block builder
                if let Some(builder) = content_builders.get_mut(index) {
                    builder.apply_delta(&delta);
                }
            }
            StreamEvent::ContentBlockStop { index } => {
                // Forward to TUI/consumers
                let _ = event_tx.send(AgentEvent::ContentBlockStop { index });
            }
            StreamEvent::MessageDelta {
                stop_reason: sr,
                usage: u,
            } => {
                if let Some(reason) = sr {
                    stop_reason = super::parse_stop_reason(&reason);
                }
                // Update usage (keep higher values from message_delta)
                if u.output_tokens > 0 {
                    usage.output_tokens = u.output_tokens;
                }
                if u.input_tokens > 0 {
                    usage.input_tokens = u.input_tokens;
                }
                if u.cache_read_input_tokens > 0 {
                    usage.cache_read_input_tokens = u.cache_read_input_tokens;
                }
                if u.cache_creation_input_tokens > 0 {
                    usage.cache_creation_input_tokens = u.cache_creation_input_tokens;
                }
            }
            StreamEvent::MessageStop => {
                break;
            }
            StreamEvent::Error { error } => {
                return Err(Error::ProviderStreaming { message: error });
            }
        }
    }

    // Finalize all content blocks
    let content: Vec<Content> = content_builders.into_iter().map(|builder| builder.finalize()).collect();

    Ok(CollectedResponse {
        content,
        model,
        usage,
        stop_reason,
    })
}

/// Execute tools in parallel and return their results
pub(super) async fn execute_tools_parallel(
    tools: &HashMap<String, Arc<dyn Tool>>,
    tool_calls: &[(String, String, Value)],
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> Vec<ToolResultMessage> {
    use futures::future::BoxFuture;
    use futures::future::FutureExt;

    let futures: Vec<BoxFuture<'static, ToolResultMessage>> = tool_calls
        .iter()
        .map(|(call_id, tool_name, input)| {
            execute_single_tool(
                tools.get(tool_name).cloned(),
                call_id.clone(),
                tool_name.clone(),
                input.clone(),
                event_tx.clone(),
                cancel.clone(),
            )
            .boxed()
        })
        .collect();

    futures::future::join_all(futures).await
}

/// Execute a single tool and return its result message
async fn execute_single_tool(
    tool: Option<Arc<dyn Tool>>,
    call_id: String,
    tool_name: String,
    input: Value,
    event_tx: broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> ToolResultMessage {
    // Emit ToolCall event
    let _ = event_tx.send(AgentEvent::ToolCall {
        tool_name: tool_name.clone(),
        call_id: call_id.clone(),
        input: input.clone(),
    });

    // Check if tool exists
    let Some(tool) = tool else {
        let error_msg = format!("Tool '{}' not found", tool_name);
        return create_error_result(call_id, tool_name, error_msg, &event_tx);
    };

    // Check sandbox paths
    if let Some(reason) = check_tool_paths(&input) {
        return create_error_result(call_id, tool_name, format!("🔒 {}", reason), &event_tx);
    }

    let _ = event_tx.send(AgentEvent::ToolExecutionStart {
        call_id: call_id.clone(),
        tool_name: tool_name.clone(),
    });

    // Execute with accumulator
    let result = execute_tool_with_accumulator(tool, &call_id, input, &event_tx, cancel).await;

    let _ = event_tx.send(AgentEvent::ToolExecutionEnd {
        call_id: call_id.clone(),
        result: result.clone(),
        is_error: result.is_error,
    });

    ToolResultMessage {
        id: MessageId::generate(),
        call_id,
        tool_name,
        content: super::tool_result_content_to_message_content(&result.content),
        is_error: result.is_error,
        details: result.details,
        timestamp: Utc::now(),
    }
}

/// Execute tool with result accumulator for streaming output
async fn execute_tool_with_accumulator(
    tool: Arc<dyn Tool>,
    call_id: &str,
    input: Value,
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> ToolExecResult {
    // Subscribe to event bus BEFORE tool execution to capture all chunks
    let mut chunk_rx = event_tx.subscribe();
    let accumulator = Arc::new(parking_lot::Mutex::new(ToolResultAccumulator::new()));
    let acc_clone = accumulator.clone();
    let call_id_for_collector = call_id.to_string();

    // Spawn collector task that feeds ToolResultChunk events into accumulator
    let collector = tokio::spawn(async move {
        loop {
            match chunk_rx.recv().await {
                Ok(AgentEvent::ToolResultChunk { call_id: cid, chunk }) if cid == call_id_for_collector => {
                    acc_clone.lock().push(chunk);
                }
                Ok(_) => {} // ignore other events
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => {},
            }
        }
    });

    // Execute tool
    let ctx = ToolContext::new(call_id.to_string(), cancel, Some(event_tx.clone()));
    let direct_result = tool.execute(&ctx, input).await;

    // Stop collector and decide which result to use
    collector.abort();
    let _ = collector.await;

    let acc = std::mem::take(&mut *accumulator.lock());
    if acc.total_bytes() > 0 {
        // Chunks were collected — use accumulated (truncated) result
        let mut accumulated = acc.finalize();
        // Preserve error status from the direct result
        accumulated.is_error = direct_result.is_error;
        accumulated
    } else {
        // No chunks emitted — use tool's direct return (backward compat)
        direct_result
    }
}

/// Create an error result message
pub(super) fn create_error_result(
    call_id: String,
    tool_name: String,
    error_msg: String,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> ToolResultMessage {
    let result = ToolExecResult::error(error_msg);

    let _ = event_tx.send(AgentEvent::ToolExecutionEnd {
        call_id: call_id.clone(),
        result: result.clone(),
        is_error: true,
    });

    ToolResultMessage {
        id: MessageId::generate(),
        call_id,
        tool_name,
        content: super::tool_result_content_to_message_content(&result.content),
        is_error: true,
        details: result.details,
        timestamp: Utc::now(),
    }
}

/// Check all path-like parameters in a tool call against the sandbox path policy.
///
/// Extracts values from common parameter names (`path`, `file`, `directory`,
/// `cwd`, `command`) and checks each against the global deny-list.
/// For `command` parameters, extracts file paths from the shell command text.
///
/// Returns `Some(reason)` if any path is blocked, `None` if all are allowed.
fn check_tool_paths(input: &Value) -> Option<String> {
    use crate::tools::sandbox::check_path;

    // Direct path parameters used by read, write, edit, ls, find, grep, etc.
    for key in ["path", "file", "directory", "cwd"] {
        if let Some(reason) = input.get(key).and_then(|v| v.as_str()).and_then(check_path) {
            return Some(reason);
        }
    }

    // Shell commands: scan for paths that look sensitive.
    // We check each whitespace-delimited token that starts with / or ~.
    if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
        let expanded = cmd.replace('~', &dirs::home_dir().unwrap_or_default().to_string_lossy());
        for token in expanded.split_whitespace() {
            if (token.starts_with('/') || token.starts_with("~/"))
                && let Some(reason) = check_path(token)
            {
                return Some(reason);
            }
        }
    }

    None
}
