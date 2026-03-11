//! Turn loop: prompt -> LLM -> tool calls -> repeat

mod execution;
mod model_switch;
mod usage;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use execution::execute_tools_parallel;
use execution::execute_turn;
use model_switch::check_model_switch;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use usage::update_usage_tracking;

use crate::events::AgentEvent;
use crate::error::AgentError;
use crate::error::Result;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::Provider;
use clankers_provider::ThinkingConfig;
use clankers_provider::Usage;
use clankers_provider::message::*;
use clankers_provider::streaming::*;
use crate::tool::Tool;
use crate::tool::ModelSwitchSlot;

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
pub(crate) struct CollectedResponse {
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

    pub(crate) fn finalize(mut self) -> Content {
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
    cost_tracker: Option<&Arc<CostTracker>>,
    model_switch_slot: Option<&ModelSwitchSlot>,
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    session_id: &str,
) -> Result<()> {
    let tool_defs: Vec<_> = tools.values().map(|t| t.definition().clone()).collect();
    let mut cumulative_usage = Usage::default();
    let mut active_model = config.model.clone();

    for turn_index in 0..config.max_turns {
        // Check for model switch and cancellation
        check_model_switch(&mut active_model, model_switch_slot, event_tx)?;
        if cancel.is_cancelled() {
            return Err(AgentError::Cancelled);
        }

        let _ = event_tx.send(AgentEvent::TurnStart { index: turn_index });

        // Execute turn and get response
        let collected = execute_turn(provider, messages, config, &active_model, &tool_defs, event_tx, &cancel).await?;

        // Update usage tracking
        update_usage_tracking(&mut cumulative_usage, &collected.usage, &active_model, cost_tracker, event_tx);

        // Build and append assistant message
        let assistant_msg = build_assistant_message(&collected);
        messages.push(AgentMessage::Assistant(assistant_msg.clone()));

        // Extract tool calls
        let tool_calls = extract_tool_calls(&collected.content);

        // If no tool calls, we're done
        if tool_calls.is_empty() || collected.stop_reason != StopReason::ToolUse {
            let _ = event_tx.send(AgentEvent::TurnEnd {
                index: turn_index,
                message: assistant_msg,
                tool_results: vec![],
            });
            break;
        }

        // Execute tools and append results
        let tool_result_messages = execute_tools_parallel(
            tools, &tool_calls, event_tx, cancel.clone(),
            hook_pipeline.clone(), session_id,
        ).await;
        for msg in &tool_result_messages {
            messages.push(AgentMessage::ToolResult(msg.clone()));
        }

        let _ = event_tx.send(AgentEvent::TurnEnd {
            index: turn_index,
            message: assistant_msg,
            tool_results: tool_result_messages,
        });
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

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "").await;

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

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "").await;

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
