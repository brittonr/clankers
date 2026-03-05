//! Turn loop: prompt -> LLM -> tool calls -> repeat

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent::events::AgentEvent;
use crate::error::Error;
use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::ThinkingConfig;
use crate::provider::Usage;
use crate::provider::message::*;
use crate::provider::streaming::*;
use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult as ToolExecResult;
use crate::tools::progress::ToolResultAccumulator;

/// Configuration for a turn loop run
pub struct TurnConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub max_turns: u32,
}

/// Result of collecting a streamed response
struct CollectedResponse {
    content: Vec<Content>,
    model: String,
    usage: Usage,
    stop_reason: StopReason,
}

fn parse_stop_reason(s: &str) -> StopReason {
    match s {
        "end_turn" | "stop" => StopReason::Stop,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        _ => StopReason::Stop,
    }
}

/// Builder for accumulating streaming content blocks
#[derive(Clone)]
struct ContentBlockBuilder {
    content: Content,
    /// For ToolUse blocks, accumulate the raw JSON string
    raw_json: Option<String>,
}

impl ContentBlockBuilder {
    fn new(content: Content) -> Self {
        Self {
            content,
            raw_json: None,
        }
    }

    fn apply_delta(&mut self, delta: &ContentDelta) {
        match (&mut self.content, delta) {
            (Content::Text { text }, ContentDelta::TextDelta { text: delta_text }) => {
                text.push_str(delta_text);
            }
            (
                Content::Thinking { thinking },
                ContentDelta::ThinkingDelta {
                    thinking: delta_thinking,
                },
            ) => {
                thinking.push_str(delta_thinking);
            }
            (Content::ToolUse { .. }, ContentDelta::InputJsonDelta { partial_json }) => {
                self.raw_json.get_or_insert_with(String::new).push_str(partial_json);
            }
            _ => {}
        }
    }

    fn finalize(mut self) -> Content {
        // Parse accumulated JSON for ToolUse
        if let Content::ToolUse { ref mut input, .. } = self.content {
            if let Some(json_str) = self.raw_json
                && !json_str.is_empty()
                && let Ok(parsed) = serde_json::from_str::<Value>(&json_str)
                && parsed.is_object()
            {
                *input = parsed;
            } else if !input.is_object() {
                // Anthropic requires input to be a dict — ensure it's always an object
                *input = Value::Object(serde_json::Map::new());
            }
        }
        self.content
    }
}

/// Run the agent turn loop.
///
/// 1. Build CompletionRequest from messages + config
/// 2. Stream response from provider
/// 3. Collect response, extract tool calls
/// 4. If tool_use: execute tools in parallel, append results, loop
/// 5. If stop/max_tokens: return
pub async fn run_turn_loop(
    provider: &dyn Provider,
    tools: &HashMap<String, Arc<dyn Tool>>,
    messages: &mut Vec<AgentMessage>,
    config: &TurnConfig,
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    let tool_defs: Vec<ToolDefinition> = tools.values().map(|t| t.definition().clone()).collect();
    let mut cumulative_usage = Usage::default();

    for turn_index in 0..config.max_turns {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }

        let _ = event_tx.send(AgentEvent::TurnStart { index: turn_index });

        // Build completion request
        let request = CompletionRequest {
            model: config.model.clone(),
            messages: messages.clone(),
            system_prompt: Some(config.system_prompt.clone()),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            tools: tool_defs.clone(),
            thinking: config.thinking.clone(),
        };

        // Create channel for streaming
        let (stream_tx, mut stream_rx) = mpsc::channel(256);

        // Run provider.complete() and collection concurrently,
        // but also watch for cancellation so we can abort mid-stream.
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
        let collected = collected?;

        // Accumulate usage
        let turn_usage = collected.usage;
        cumulative_usage.input_tokens += turn_usage.input_tokens;
        cumulative_usage.output_tokens += turn_usage.output_tokens;
        cumulative_usage.cache_creation_input_tokens += turn_usage.cache_creation_input_tokens;
        cumulative_usage.cache_read_input_tokens += turn_usage.cache_read_input_tokens;

        // Emit usage update
        let _ = event_tx.send(AgentEvent::UsageUpdate {
            turn_usage: turn_usage.clone(),
            cumulative_usage: cumulative_usage.clone(),
        });

        // Build assistant message
        let assistant_msg = AssistantMessage {
            id: MessageId::generate(),
            content: collected.content.clone(),
            model: collected.model,
            usage: turn_usage,
            stop_reason: collected.stop_reason.clone(),
            timestamp: Utc::now(),
        };

        // Append to messages
        messages.push(AgentMessage::Assistant(assistant_msg.clone()));

        // Extract tool calls
        let tool_calls: Vec<_> = collected
            .content
            .iter()
            .filter_map(|c| {
                if let Content::ToolUse { id, name, input } = c {
                    Some((id.clone(), name.clone(), input.clone()))
                } else {
                    None
                }
            })
            .collect();

        // If no tool calls or stop reason isn't ToolUse, we're done
        if tool_calls.is_empty() || collected.stop_reason != StopReason::ToolUse {
            let _ = event_tx.send(AgentEvent::TurnEnd {
                index: turn_index,
                message: assistant_msg,
                tool_results: vec![],
            });
            break;
        }

        // Execute tools in parallel
        let tool_result_messages = execute_tools_parallel(tools, &tool_calls, event_tx, cancel.clone()).await;

        // Append tool results to messages
        for msg in &tool_result_messages {
            messages.push(AgentMessage::ToolResult(msg.clone()));
        }

        // Emit TurnEnd
        let _ = event_tx.send(AgentEvent::TurnEnd {
            index: turn_index,
            message: assistant_msg,
            tool_results: tool_result_messages,
        });

        // Continue to next turn
    }

    Ok(())
}

/// Collect streaming events into a complete response
async fn collect_stream_events(
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
                    stop_reason = parse_stop_reason(&reason);
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
async fn execute_tools_parallel(
    tools: &HashMap<String, Arc<dyn Tool>>,
    tool_calls: &[(String, String, Value)],
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> Vec<ToolResultMessage> {
    use futures::future::BoxFuture;
    use futures::future::FutureExt;

    let mut futures: Vec<BoxFuture<'static, ToolResultMessage>> = Vec::new();

    for (call_id, tool_name, input) in tool_calls {
        // Emit ToolCall event
        let _ = event_tx.send(AgentEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        });

        // Get the tool
        let tool = match tools.get(tool_name) {
            Some(t) => t.clone(),
            None => {
                // Tool not found - create error result immediately
                let result = ToolExecResult::error(format!("Tool '{}' not found", tool_name));

                let _ = event_tx.send(AgentEvent::ToolExecutionEnd {
                    call_id: call_id.clone(),
                    result: result.clone(),
                    is_error: true,
                });

                let tool_result_msg = ToolResultMessage {
                    id: MessageId::generate(),
                    call_id: call_id.clone(),
                    tool_name: tool_name.clone(),
                    content: tool_result_content_to_message_content(&result.content),
                    is_error: true,
                    details: result.details,
                    timestamp: Utc::now(),
                };

                futures.push(async move { tool_result_msg }.boxed());
                continue;
            }
        };

        // Spawn execution
        let call_id = call_id.clone();
        let tool_name = tool_name.clone();
        let input = input.clone();
        let event_tx = event_tx.clone();
        let cancel = cancel.clone();

        let fut = async move {
            // ── Sandbox: check all path-like parameters against the deny-list ──
            if let Some(reason) = check_tool_paths(&input) {
                let result = ToolExecResult::error(format!("🔒 {}", reason));

                let _ = event_tx.send(AgentEvent::ToolExecutionEnd {
                    call_id: call_id.clone(),
                    result: result.clone(),
                    is_error: true,
                });

                return ToolResultMessage {
                    id: MessageId::generate(),
                    call_id,
                    tool_name,
                    content: tool_result_content_to_message_content(&result.content),
                    is_error: true,
                    details: result.details,
                    timestamp: Utc::now(),
                };
            }

            let _ = event_tx.send(AgentEvent::ToolExecutionStart {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
            });

            // Subscribe to event bus BEFORE tool execution to capture all chunks
            let mut chunk_rx = event_tx.subscribe();
            let accumulator = Arc::new(parking_lot::Mutex::new(ToolResultAccumulator::new()));
            let acc_clone = accumulator.clone();
            let call_id_for_collector = call_id.clone();

            // Spawn collector task that feeds ToolResultChunk events into accumulator
            let collector = tokio::spawn(async move {
                loop {
                    match chunk_rx.recv().await {
                        Ok(AgentEvent::ToolResultChunk { call_id: cid, chunk })
                            if cid == call_id_for_collector =>
                        {
                            acc_clone.lock().push(chunk);
                        }
                        Ok(_) => {} // ignore other events
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
            });

            // Execute tool
            let ctx = ToolContext::new(call_id.clone(), cancel, Some(event_tx.clone()));
            let direct_result = tool.execute(&ctx, input).await;

            // Stop collector and decide which result to use
            collector.abort();
            let _ = collector.await;

            let result = {
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
            };

            let _ = event_tx.send(AgentEvent::ToolExecutionEnd {
                call_id: call_id.clone(),
                result: result.clone(),
                is_error: result.is_error,
            });

            ToolResultMessage {
                id: MessageId::generate(),
                call_id,
                tool_name,
                content: tool_result_content_to_message_content(&result.content),
                is_error: result.is_error,
                details: result.details,
                timestamp: Utc::now(),
            }
        }
        .boxed();

        futures.push(fut);
    }

    // Wait for all tools to complete
    futures::future::join_all(futures).await
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

/// Convert ToolResultContent to Content
fn tool_result_content_to_message_content(tool_content: &[crate::tools::ToolResultContent]) -> Vec<Content> {
    tool_content
        .iter()
        .map(|tc| match tc {
            crate::tools::ToolResultContent::Text { text } => Content::Text { text: text.clone() },
            crate::tools::ToolResultContent::Image { media_type, data } => Content::Image {
                source: ImageSource::Base64 {
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;
    use crate::tools::progress::ResultChunk;
    use crate::tools::ToolDefinition;

    // -----------------------------------------------------------------------
    // parse_stop_reason
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stop_reason_end_turn() {
        assert_eq!(parse_stop_reason("end_turn"), StopReason::Stop);
    }

    #[test]
    fn test_parse_stop_reason_stop() {
        assert_eq!(parse_stop_reason("stop"), StopReason::Stop);
    }

    #[test]
    fn test_parse_stop_reason_tool_use() {
        assert_eq!(parse_stop_reason("tool_use"), StopReason::ToolUse);
    }

    #[test]
    fn test_parse_stop_reason_max_tokens() {
        assert_eq!(parse_stop_reason("max_tokens"), StopReason::MaxTokens);
    }

    #[test]
    fn test_parse_stop_reason_unknown_defaults_to_stop() {
        assert_eq!(parse_stop_reason("something_else"), StopReason::Stop);
        assert_eq!(parse_stop_reason(""), StopReason::Stop);
    }

    // -----------------------------------------------------------------------
    // ContentBlockBuilder
    // -----------------------------------------------------------------------

    #[test]
    fn test_content_block_builder_text_delta() {
        let mut builder = ContentBlockBuilder::new(Content::Text { text: String::new() });
        builder.apply_delta(&ContentDelta::TextDelta {
            text: "Hello".to_string(),
        });
        builder.apply_delta(&ContentDelta::TextDelta {
            text: " world".to_string(),
        });

        match builder.finalize() {
            Content::Text { text } => assert_eq!(text, "Hello world"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_thinking_delta() {
        let mut builder = ContentBlockBuilder::new(Content::Thinking {
            thinking: String::new(),
        });
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: "Let me think...".to_string(),
        });
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: " more thoughts".to_string(),
        });

        match builder.finalize() {
            Content::Thinking { thinking } => assert_eq!(thinking, "Let me think... more thoughts"),
            other => panic!("Expected Thinking, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_tool_use_json_delta() {
        let mut builder = ContentBlockBuilder::new(Content::ToolUse {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            input: json!({}),
        });
        builder.apply_delta(&ContentDelta::InputJsonDelta {
            partial_json: r#"{"com"#.to_string(),
        });
        builder.apply_delta(&ContentDelta::InputJsonDelta {
            partial_json: r#"mand": "ls"}"#.to_string(),
        });

        match builder.finalize() {
            Content::ToolUse { input, name, .. } => {
                assert_eq!(name, "bash");
                assert_eq!(input, json!({"command": "ls"}));
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_tool_use_empty_json() {
        let builder = ContentBlockBuilder::new(Content::ToolUse {
            id: "call_2".to_string(),
            name: "test".to_string(),
            input: json!(null), // Non-object input should become {}
        });

        match builder.finalize() {
            Content::ToolUse { input, .. } => {
                assert!(input.is_object(), "Expected object, got {:?}", input);
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_tool_use_invalid_json_fallback() {
        let mut builder = ContentBlockBuilder::new(Content::ToolUse {
            id: "call_3".to_string(),
            name: "test".to_string(),
            input: json!({}),
        });
        // Incomplete JSON
        builder.apply_delta(&ContentDelta::InputJsonDelta {
            partial_json: r#"{"key": "#.to_string(),
        });

        match builder.finalize() {
            Content::ToolUse { input, .. } => {
                // Should keep original {} since parse failed
                assert!(input.is_object());
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_mismatched_delta_ignored() {
        let mut builder = ContentBlockBuilder::new(Content::Text {
            text: "hello".to_string(),
        });
        // Applying a thinking delta to a text block should be ignored
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: "thinking".to_string(),
        });

        match builder.finalize() {
            Content::Text { text } => assert_eq!(text, "hello"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // tool_result_content_to_message_content
    // -----------------------------------------------------------------------

    #[test]
    fn test_tool_result_text_conversion() {
        use crate::tools::ToolResultContent;
        let content = vec![ToolResultContent::Text {
            text: "output".to_string(),
        }];
        let result = tool_result_content_to_message_content(&content);
        assert_eq!(result.len(), 1);
        match &result[0] {
            Content::Text { text } => assert_eq!(text, "output"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_tool_result_image_conversion() {
        use crate::tools::ToolResultContent;
        let content = vec![ToolResultContent::Image {
            media_type: "image/png".to_string(),
            data: "base64data".to_string(),
        }];
        let result = tool_result_content_to_message_content(&content);
        assert_eq!(result.len(), 1);
        match &result[0] {
            Content::Image {
                source: ImageSource::Base64 { media_type, data },
            } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "base64data");
            }
            other => panic!("Expected Image, got {:?}", other),
        }
    }

    #[test]
    fn test_tool_result_mixed_content() {
        use crate::tools::ToolResultContent;
        let content = vec![
            ToolResultContent::Text {
                text: "text".to_string(),
            },
            ToolResultContent::Image {
                media_type: "image/jpeg".to_string(),
                data: "jpg_data".to_string(),
            },
        ];
        let result = tool_result_content_to_message_content(&content);
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0], Content::Text { .. }));
        assert!(matches!(&result[1], Content::Image { .. }));
    }

    #[test]
    fn test_tool_result_empty_content() {
        let result = tool_result_content_to_message_content(&[]);
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // Phase 4: Accumulator integration in execute_tools_parallel
    // -----------------------------------------------------------------------

    /// A tool that emits result chunks during execution
    struct ChunkEmittingTool {
        def: ToolDefinition,
    }

    impl ChunkEmittingTool {
        fn new() -> Self {
            Self {
                def: ToolDefinition {
                    name: "chunk_tool".to_string(),
                    description: "Emits result chunks".to_string(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            }
        }
    }

    #[async_trait]
    impl Tool for ChunkEmittingTool {
        fn definition(&self) -> &ToolDefinition {
            &self.def
        }

        async fn execute(&self, ctx: &ToolContext, _params: Value) -> ToolExecResult {
            // Emit several chunks
            ctx.emit_result_chunk(ResultChunk::text("line 1\nline 2"));
            ctx.emit_result_chunk(ResultChunk::text("line 3\nline 4"));
            ctx.emit_result_chunk(ResultChunk::text("line 5"));

            // Yield to let collector process events
            tokio::task::yield_now().await;

            // Return a direct result (should be ignored in favor of accumulated)
            ToolExecResult::text("direct result (should be overridden)")
        }
    }

    /// A tool that returns a direct result without emitting chunks
    struct DirectResultTool {
        def: ToolDefinition,
    }

    impl DirectResultTool {
        fn new() -> Self {
            Self {
                def: ToolDefinition {
                    name: "direct_tool".to_string(),
                    description: "Returns direct result".to_string(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            }
        }
    }

    #[async_trait]
    impl Tool for DirectResultTool {
        fn definition(&self) -> &ToolDefinition {
            &self.def
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolExecResult {
            ToolExecResult::text("direct output")
        }
    }

    #[tokio::test]
    async fn accumulator_collects_chunks_from_tool() {
        let tool: Arc<dyn Tool> = Arc::new(ChunkEmittingTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("chunk_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![(
            "call-1".to_string(),
            "chunk_tool".to_string(),
            json!({}),
        )];

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel).await;

        assert_eq!(results.len(), 1);
        let msg = &results[0];
        assert!(!msg.is_error);

        // Should contain accumulated text, not "direct result"
        let text = match &msg.content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert!(text.contains("line 1"), "expected accumulated text, got: {}", text);
        assert!(text.contains("line 5"), "expected accumulated text, got: {}", text);
        assert!(!text.contains("direct result"), "should use accumulated, not direct");

        // Should have details with accumulator metadata
        let details = msg.details.as_ref().expect("expected details");
        assert_eq!(details["chunks"], 3);
        assert!(details["total_lines"].as_u64().unwrap() >= 5);
        assert!(!details["truncated"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn direct_result_used_when_no_chunks() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![(
            "call-2".to_string(),
            "direct_tool".to_string(),
            json!({}),
        )];

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel).await;

        assert_eq!(results.len(), 1);
        let msg = &results[0];
        assert!(!msg.is_error);

        // Should contain the direct result text
        let text = match &msg.content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert_eq!(text, "direct output");

        // No details (direct result has no accumulator metadata)
        assert!(msg.details.is_none());
    }
}
