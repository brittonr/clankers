//! Turn loop: prompt -> LLM -> tool calls -> repeat

mod execution;
mod model_switch;
mod usage;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::Provider;
use clankers_provider::ThinkingConfig;
use clankers_provider::Usage;
use clankers_provider::message::*;
use clankers_provider::streaming::*;
use execution::execute_tools_parallel;
use execution::execute_turn;
use model_switch::check_model_switch;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use usage::update_usage_tracking;

use crate::error::AgentError;
use crate::error::Result;
use crate::events::AgentEvent;
use crate::tool::ModelSwitchSlot;
use crate::tool::Tool;

/// Configuration for a turn loop run
pub struct TurnConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub max_turns: u32,
    /// Output truncation config for tool results
    pub output_truncation: clanker_loop::OutputTruncationConfig,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
}

/// Result of collecting a streamed response
pub(crate) struct CollectedResponse {
    content: Vec<Content>,
    model: String,
    usage: Usage,
    stop_reason: StopReason,
}

fn tool_definitions_from_controller_inventory(
    controller_tools: &HashMap<String, Arc<dyn Tool>>,
) -> Vec<crate::tool::ToolDefinition> {
    controller_tools.values().map(|tool| tool.definition().clone()).collect()
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
pub(crate) struct ContentBlockBuilder {
    content: Content,
    /// For ToolUse blocks, accumulate the raw JSON string
    raw_json: Option<String>,
}

impl ContentBlockBuilder {
    pub(crate) fn new(content: Content) -> Self {
        Self {
            content,
            raw_json: None,
        }
    }

    pub(crate) fn apply_delta(&mut self, delta: &ContentDelta) {
        match (&mut self.content, delta) {
            (Content::Text { text }, ContentDelta::TextDelta { text: delta_text }) => {
                text.push_str(delta_text);
            }
            (
                Content::Thinking { thinking, .. },
                ContentDelta::ThinkingDelta {
                    thinking: delta_thinking,
                },
            ) => {
                thinking.push_str(delta_thinking);
            }
            (Content::Thinking { signature, .. }, ContentDelta::SignatureDelta { signature: sig_delta }) => {
                signature.push_str(sig_delta);
            }
            (Content::ToolUse { .. }, ContentDelta::InputJsonDelta { partial_json }) => {
                self.raw_json.get_or_insert_with(String::new).push_str(partial_json);
            }
            _ => {}
        }
    }

    pub(crate) fn finalize(mut self) -> Content {
        // Parse accumulated JSON for ToolUse
        if let Content::ToolUse { ref mut input, ref name, .. } = self.content {
            match self.raw_json {
                Some(ref json_str) if !json_str.is_empty() => {
                    match serde_json::from_str::<Value>(json_str) {
                        Ok(parsed) if parsed.is_object() => {
                            *input = parsed;
                        }
                        Ok(parsed) => {
                            // Valid JSON but not an object — wrap it so the tool
                            // still sees something rather than empty {}.
                            tracing::warn!(
                                tool = name,
                                json_type = parsed.as_str().map(|_| "string")
                                    .or(parsed.as_array().map(|_| "array"))
                                    .unwrap_or("other"),
                                "tool input JSON is not an object, wrapping in {{\"_raw\": ...}}",
                            );
                            let mut map = serde_json::Map::new();
                            map.insert("_raw".to_string(), parsed);
                            *input = Value::Object(map);
                        }
                        Err(e) => {
                            tracing::warn!(
                                tool = name,
                                json_len = json_str.len(),
                                error = %e,
                                "failed to parse accumulated tool input JSON",
                            );
                            // Keep the initial empty {} — tool will see missing params
                        }
                    }
                }
                Some(_) => {
                    // raw_json was set but empty (initial empty arguments chunk)
                    tracing::debug!(tool = name, "tool input JSON is empty string");
                }
                None => {
                    // No InputJsonDelta events received at all
                    tracing::debug!(tool = name, "no InputJsonDelta events for tool_use block");
                }
            }
            // Ensure input is always an object
            if !input.is_object() {
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
#[allow(clippy::too_many_arguments)]
pub async fn run_turn_loop(
    provider: &dyn Provider,
    controller_tools: &HashMap<String, Arc<dyn Tool>>,
    messages: &mut Vec<AgentMessage>,
    config: &TurnConfig,
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
    cost_tracker: Option<&Arc<CostTracker>>,
    model_switch_slot: Option<&ModelSwitchSlot>,
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    session_id: &str,
    db: Option<clankers_db::Db>,
    capability_gate: Option<&Arc<dyn crate::tool::CapabilityGate>>,
    user_tool_filter: Option<&Vec<String>>,
) -> Result<()> {
    let tool_defs = tool_definitions_from_controller_inventory(controller_tools);
    let mut cumulative_usage = Usage::default();
    let mut active_model = config.model.clone();

    for turn_index in 0..config.max_turns {
        // Check for model switch and cancellation
        check_model_switch(&mut active_model, model_switch_slot, event_tx)?;
        if cancel.is_cancelled() {
            return Err(AgentError::Cancelled);
        }

        event_tx.send(AgentEvent::TurnStart { index: turn_index }).ok();

        // Execute turn with retry on transient failures.
        // Up to 2 retries with exponential backoff (1s, 4s).
        let collected = {
            const MAX_TURN_RETRIES: u32 = 2;
            let mut last_err = None;
            let mut collected_ok = None;
            for attempt in 0..=MAX_TURN_RETRIES {
                match execute_turn(provider, messages, config, &active_model, &tool_defs, event_tx, &cancel, session_id).await {
                    Ok(c) => {
                        collected_ok = Some(c);
                        break;
                    }
                    Err(e) if e.is_retryable() && attempt < MAX_TURN_RETRIES => {
                        let backoff = std::time::Duration::from_secs(1 << (attempt * 2));
                        tracing::warn!(
                            attempt = attempt + 1,
                            max_retries = MAX_TURN_RETRIES,
                            error = %e,
                            ?backoff,
                            "Retryable turn error, backing off before retry",
                        );
                        tokio::select! {
                            () = cancel.cancelled() => return Err(AgentError::Cancelled),
                            () = tokio::time::sleep(backoff) => {}
                        }
                        last_err = Some(e);
                    }
                    Err(e) => return Err(e),
                }
            }
            match collected_ok {
                Some(c) => c,
                None => return Err(last_err.expect("retry loop must set last_err")),
            }
        };

        // Update usage tracking
        update_usage_tracking(&mut cumulative_usage, &collected.usage, &active_model, cost_tracker, event_tx);

        // Build and append assistant message
        let assistant_msg = build_assistant_message(&collected);
        messages.push(AgentMessage::Assistant(assistant_msg.clone()));

        // Extract tool calls
        let tool_calls = extract_tool_calls(&collected.content);

        tracing::debug!(
            turn = turn_index,
            stop_reason = ?collected.stop_reason,
            tool_calls = tool_calls.len(),
            content_blocks = collected.content.len(),
            "turn collected",
        );
        for (call_id, name, input) in &tool_calls {
            let input_keys: Vec<&str> = input.as_object()
                .map(|m| m.keys().map(|k| k.as_str()).collect())
                .unwrap_or_default();
            tracing::debug!(
                call_id,
                tool = name,
                input_keys = ?input_keys,
                input_empty = input.as_object().is_none_or(|m| m.is_empty()),
                "extracted tool call",
            );
        }

        // If no tool calls, we're done
        if tool_calls.is_empty() || collected.stop_reason != StopReason::ToolUse {
            if !tool_calls.is_empty() && collected.stop_reason != StopReason::ToolUse {
                tracing::warn!(
                    turn = turn_index,
                    stop_reason = ?collected.stop_reason,
                    tool_calls = tool_calls.len(),
                    "tool calls present but stop_reason is not ToolUse — tools will NOT execute",
                );
            }
            event_tx.send(AgentEvent::TurnEnd {
                index: turn_index,
                message: assistant_msg,
                tool_results: vec![],
            }).ok();
            break;
        }

        // Execute tools and append results (with truncation)
        let tool_result_messages = execute_tools_parallel(
            controller_tools,
            &tool_calls,
            event_tx,
            cancel.clone(),
            hook_pipeline.clone(),
            session_id,
            db.clone(),
            capability_gate.cloned(),
            user_tool_filter.cloned(),
        )
        .await;
        let tool_result_messages = apply_output_truncation(tool_result_messages, &config.output_truncation);
        for msg in &tool_result_messages {
            messages.push(AgentMessage::ToolResult(msg.clone()));
        }

        event_tx.send(AgentEvent::TurnEnd {
            index: turn_index,
            message: assistant_msg,
            tool_results: tool_result_messages,
        }).ok();
    }

    Ok(())
}

/// Build assistant message from collected response
fn build_assistant_message(collected: &CollectedResponse) -> AssistantMessage {
    AssistantMessage {
        id: MessageId::generate(),
        content: collected.content.clone(),
        model: collected.model.clone(),
        usage: collected.usage.clone(),
        stop_reason: collected.stop_reason.clone(),
        timestamp: Utc::now(),
    }
}

/// Extract tool calls from content blocks
fn extract_tool_calls(content: &[Content]) -> Vec<(String, String, Value)> {
    content
        .iter()
        .filter_map(|c| {
            if let Content::ToolUse { id, name, input } = c {
                Some((id.clone(), name.clone(), input.clone()))
            } else {
                None
            }
        })
        .collect()
}

/// Apply output truncation to tool result messages.
///
/// For each tool result, extracts text content, runs it through the truncation
/// layer, and rebuilds the message with truncated text and a temp file path
/// if truncation was applied.
fn apply_output_truncation(
    messages: Vec<ToolResultMessage>,
    config: &clanker_loop::OutputTruncationConfig,
) -> Vec<ToolResultMessage> {
    if !config.enabled {
        return messages;
    }

    messages
        .into_iter()
        .map(|mut msg| {
            // Extract text content blocks, truncate, and rebuild
            let mut truncated_content = Vec::new();
            let mut was_any_truncated = false;

            for block in &msg.content {
                match block {
                    Content::Text { text } => {
                        let result = clanker_loop::truncate_tool_output(text, config);
                        if result.truncated {
                            was_any_truncated = true;
                            tracing::info!(
                                tool = msg.tool_name,
                                original_lines = result.original_lines,
                                original_bytes = result.original_bytes,
                                "Tool output truncated"
                            );
                        }
                        truncated_content.push(Content::Text { text: result.content });
                    }
                    other => truncated_content.push(other.clone()),
                }
            }

            if was_any_truncated {
                msg.content = truncated_content;
            }
            msg
        })
        .collect()
}

/// Convert ToolResultContent to Content
pub(crate) fn tool_result_content_to_message_content(tool_content: &[crate::tool::ToolResultContent]) -> Vec<Content> {
    tool_content
        .iter()
        .map(|tc| match tc {
            crate::tool::ToolResultContent::Text { text } => Content::Text { text: text.clone() },
            crate::tool::ToolResultContent::Image { media_type, data } => Content::Image {
                source: ImageSource::Base64 {
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
            },
        })
        .collect()
}

#[cfg(test)]
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_panic, no_unwrap, reason = "test code — panics are assertions"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;
    use crate::tool::ToolContext;
    use crate::tool::ToolDefinition;
    use crate::tool::ToolResult as ToolExecResult;
    use crate::tool::progress::ResultChunk;

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
            signature: String::new(),
        });
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: "Let me think...".to_string(),
        });
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: " more thoughts".to_string(),
        });

        match builder.finalize() {
            Content::Thinking { thinking, .. } => assert_eq!(thinking, "Let me think... more thoughts"),
            other => panic!("Expected Thinking, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_signature_delta() {
        let mut builder = ContentBlockBuilder::new(Content::Thinking {
            thinking: "some thought".to_string(),
            signature: String::new(),
        });
        builder.apply_delta(&ContentDelta::SignatureDelta {
            signature: "sig_part1".to_string(),
        });
        builder.apply_delta(&ContentDelta::SignatureDelta {
            signature: "_part2".to_string(),
        });

        match builder.finalize() {
            Content::Thinking { thinking, signature } => {
                assert_eq!(thinking, "some thought");
                assert_eq!(signature, "sig_part1_part2");
            }
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
        use crate::tool::ToolResultContent;
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
        use crate::tool::ToolResultContent;
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
        use crate::tool::ToolResultContent;
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

        let tool_calls = vec![("call-1".to_string(), "chunk_tool".to_string(), json!({}))];

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, None).await;

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
        assert!(details["total_lines"].as_u64().expect("total_lines should be u64") >= 5);
        assert!(!details["truncated"].as_bool().expect("truncated should be bool"));
    }

    #[tokio::test]
    async fn direct_result_used_when_no_chunks() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-2".to_string(), "direct_tool".to_string(), json!({}))];

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, None).await;

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

    #[tokio::test]
    async fn user_tool_filter_blocks_unlisted_tools() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        // Filter only allows "read" — direct_tool should be blocked
        let filter = Some(vec!["read".to_string()]);
        let results = execute_tools_parallel(
            &tools, &tool_calls, &event_tx, cancel, None, "", None, None, filter,
        )
        .await;

        assert_eq!(results.len(), 1);
        assert!(results[0].is_error);
        let text = match &results[0].content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert!(text.contains("🔒"), "expected locked error, got: {text}");
    }

    #[tokio::test]
    async fn user_tool_filter_allows_listed_tools() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        // Filter allows direct_tool
        let filter = Some(vec!["direct_tool,read".to_string()]);
        let results = execute_tools_parallel(
            &tools, &tool_calls, &event_tx, cancel, None, "", None, None, filter,
        )
        .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
    }

    #[tokio::test]
    async fn user_tool_filter_none_allows_all() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        // No filter — full access
        let results = execute_tools_parallel(
            &tools, &tool_calls, &event_tx, cancel, None, "", None, None, None,
        )
        .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
    }

    #[tokio::test]
    async fn user_tool_filter_applies_latest_allowlist_per_call() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        let blocked_results = execute_tools_parallel(
            &tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            Some(vec!["read".to_string()]),
        )
        .await;
        assert!(blocked_results[0].is_error);

        let allowed_results = execute_tools_parallel(
            &tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            Some(vec!["direct_tool".to_string()]),
        )
        .await;
        assert!(!allowed_results[0].is_error);
    }

    #[tokio::test]
    async fn controller_filtered_tool_inventory_replaces_available_tools_without_turn_local_state() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut full_tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        full_tools.insert("direct_tool".to_string(), Arc::clone(&tool));
        let filtered_tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let (event_tx, _rx) = broadcast::channel(256);
        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        let allowed_results = execute_tools_parallel(
            &full_tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            None,
        )
        .await;
        assert!(!allowed_results[0].is_error);

        let filtered_results = execute_tools_parallel(
            &filtered_tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            None,
        )
        .await;
        assert!(filtered_results[0].is_error);
        let text = match &filtered_results[0].content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert_eq!(text, "Tool 'direct_tool' not found");
    }

    // -----------------------------------------------------------------------
    // Turn-level retry tests
    // -----------------------------------------------------------------------

    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;

    /// Provider that fails N times with a retryable error, then succeeds.
    struct RetryableFailProvider {
        failures_remaining: AtomicUsize,
        status: u16,
    }

    impl RetryableFailProvider {
        fn new(fail_count: usize, status: u16) -> Self {
            Self {
                failures_remaining: AtomicUsize::new(fail_count),
                status,
            }
        }
    }

    #[async_trait]
    impl clankers_provider::Provider for RetryableFailProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let remaining = self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            if remaining > 0 {
                return Err(clankers_provider::error::provider_err_with_status(
                    self.status,
                    format!("HTTP error {}", self.status),
                ));
            }
            // Succeed: send minimal valid response
            tx.send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "msg-1".into(),
                    model: "test-model".into(),
                    role: "assistant".into(),
                },
            }).await.ok();
            tx.send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: Content::Text { text: String::new() },
            }).await.ok();
            tx.send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta { text: "OK".into() },
            }).await.ok();
            tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage { input_tokens: 10, output_tokens: 2, cache_creation_input_tokens: 0, cache_read_input_tokens: 0 },
            }).await.ok();
            tx.send(StreamEvent::MessageStop).await.ok();
            Ok(())
        }
        fn models(&self) -> &[clankers_provider::Model] { &[] }
        fn name(&self) -> &str { "test" }
    }

    fn make_turn_config() -> TurnConfig {
        TurnConfig {
            model: "test-model".into(),
            system_prompt: "You are a test assistant.".into(),
            max_tokens: Some(100),
            temperature: None,
            thinking: None,
            max_turns: 1,
            output_truncation: clanker_loop::OutputTruncationConfig::default(),
            no_cache: true,
            cache_ttl: None,
        }
    }

    fn make_user_message() -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg"),
            content: vec![Content::Text { text: "hello".into() }],
            timestamp: Utc::now(),
        })
    }

    #[tokio::test]
    async fn turn_request_includes_session_id_extra_param() {
        use std::sync::Mutex;

        struct CapturingProvider {
            captured: Mutex<Option<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for CapturingProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                *self.captured.lock().expect("capture lock poisoned") = Some(request);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "OK".into() },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 2,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                }).await.ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "capturing"
            }
        }

        let provider = CapturingProvider {
            captured: Mutex::new(None),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            cancel,
            None,
            None,
            None,
            "session-123",
            None,
            None,
            None,
        )
        .await
        .expect("turn should succeed");

        let captured = provider
            .captured
            .lock()
            .expect("capture lock poisoned")
            .take()
            .expect("request should be captured");
        assert_eq!(captured.extra_params.get("_session_id"), Some(&json!("session-123")));
    }

    #[tokio::test]
    async fn turn_request_reuses_session_id_across_later_turns() {
        use std::sync::Mutex;

        struct SequenceCapturingProvider {
            captured: Mutex<Vec<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for SequenceCapturingProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.captured
                    .lock()
                    .expect("capture lock poisoned")
                    .push(request);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "OK".into() },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 2,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                }).await.ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "sequence-capturing"
            }
        }

        let provider = SequenceCapturingProvider {
            captured: Mutex::new(Vec::new()),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);

        run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-stable",
            None,
            None,
            None,
        )
        .await
        .expect("first turn should succeed");

        messages.push(AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg-2"),
            content: vec![Content::Text { text: "hello again".into() }],
            timestamp: Utc::now(),
        }));

        run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-stable",
            None,
            None,
            None,
        )
        .await
        .expect("second turn should succeed");

        let captured = provider.captured.lock().expect("capture lock poisoned");
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].extra_params.get("_session_id"), Some(&json!("session-stable")));
        assert_eq!(captured[1].extra_params.get("_session_id"), Some(&json!("session-stable")));
    }

    #[tokio::test]
    async fn turn_request_reuses_session_id_after_resume() {
        use std::sync::Mutex;

        struct ResumeCapturingProvider {
            captured: Mutex<Vec<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for ResumeCapturingProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.captured
                    .lock()
                    .expect("capture lock poisoned")
                    .push(request);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "OK".into() },
                }).await.ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 2,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                }).await.ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "resume-capturing"
            }
        }

        let provider = ResumeCapturingProvider {
            captured: Mutex::new(Vec::new()),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let mut before_resume_messages = vec![make_user_message()];

        run_turn_loop(
            &provider,
            &tools,
            &mut before_resume_messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-resumed",
            None,
            None,
            None,
        )
        .await
        .expect("turn before resume should succeed");

        let mut resumed_messages = vec![AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg-3"),
            content: vec![Content::Text { text: "after resume".into() }],
            timestamp: Utc::now(),
        })];

        run_turn_loop(
            &provider,
            &tools,
            &mut resumed_messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-resumed",
            None,
            None,
            None,
        )
        .await
        .expect("turn after resume should succeed");

        let captured = provider.captured.lock().expect("capture lock poisoned");
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].extra_params.get("_session_id"), Some(&json!("session-resumed")));
        assert_eq!(captured[1].extra_params.get("_session_id"), Some(&json!("session-resumed")));
    }

    #[tokio::test]
    async fn turn_retry_recovers_on_second_attempt() {
        // Fails once with 502, then succeeds
        let provider = RetryableFailProvider::new(1, 502);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let result = run_turn_loop(
            &provider, &tools, &mut messages, &config, &event_tx, cancel,
            None, None, None, "test-session", None, None, None,
        ).await;

        assert!(result.is_ok(), "expected success after retry, got: {:?}", result);
        // Should have appended an assistant message
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn turn_retry_non_retryable_error_skips_retry() {
        // Fails with 400 (non-retryable) — should fail immediately
        let provider = RetryableFailProvider::new(1, 400);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let result = run_turn_loop(
            &provider, &tools, &mut messages, &config, &event_tx, cancel,
            None, None, None, "test-session", None, None, None,
        ).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_retryable(), "400 should not be retryable");
        // Messages unchanged — failed turn didn't append
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn turn_retry_cancellation_during_backoff() {
        // Fails with 502 (retryable), cancel during backoff
        let provider = RetryableFailProvider::new(3, 502); // always fails
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        // Cancel shortly after first failure's backoff starts
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            cancel_clone.cancel();
        });

        let result = run_turn_loop(
            &provider, &tools, &mut messages, &config, &event_tx, cancel,
            None, None, None, "test-session", None, None, None,
        ).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AgentError::Cancelled));
    }
}
