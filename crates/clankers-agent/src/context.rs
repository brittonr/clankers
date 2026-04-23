//! Context building, token estimation, window management

use clankers_config::settings::Settings;
use clankers_message::message::*;

/// Built context ready for an LLM request
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Assembled system prompt
    pub system_prompt: String,
    /// Messages to send (possibly truncated for context window)
    pub messages: Vec<AgentMessage>,
    /// Estimated token count
    pub estimated_tokens: usize,
}

/// Build the system prompt from components.
///
/// Assembly order:
/// 1. Settings prefix (if any)
/// 2. Base system prompt (from agent definition or default)
/// 3. Available tools description
/// 4. Available agent definitions
/// 5. Additional context (skills, specs, context files)
/// 6. Settings suffix (if any)
pub fn build_system_prompt(
    base_prompt: &str,
    settings: &Settings,
    tool_names: &[String],
    agent_names: &[String],
    additional_context: &[String],
) -> String {
    let mut parts = Vec::new();

    if let Some(ref prefix) = settings.system_prompt_prefix {
        parts.push(prefix.clone());
    }

    parts.push(base_prompt.to_string());

    if !tool_names.is_empty() {
        parts.push(format!("\nAvailable tools: {}", tool_names.join(", ")));
    }

    if !agent_names.is_empty() {
        parts.push(format!("\nAvailable agents: {}", agent_names.join(", ")));
    }

    for ctx in additional_context {
        if !ctx.is_empty() {
            parts.push(ctx.clone());
        }
    }

    if let Some(ref suffix) = settings.system_prompt_suffix {
        parts.push(suffix.clone());
    }

    parts.join("\n\n")
}

/// Truncate messages to fit within a token budget.
///
/// Strategy: keep first message + as many recent messages as fit.
/// Drops middle messages first.
pub fn truncate_messages(
    messages: &[AgentMessage],
    max_tokens: usize,
    system_prompt_tokens: usize,
) -> Vec<AgentMessage> {
    let budget = max_tokens.saturating_sub(system_prompt_tokens);
    if budget == 0 {
        return vec![];
    }

    let message_tokens: Vec<usize> = messages.iter().map(estimate_message_tokens).collect();

    let total: usize = message_tokens.iter().sum();
    if total <= budget {
        return messages.to_vec();
    }

    let mut result = Vec::new();
    let mut remaining = budget;

    // Always keep first message if it fits
    if !messages.is_empty() && message_tokens[0] <= remaining {
        result.push(messages[0].clone());
        remaining -= message_tokens[0];
    }

    // Add messages from the end until budget exhausted
    let mut tail = Vec::new();
    for i in (1..messages.len()).rev() {
        if message_tokens[i] <= remaining {
            tail.push(messages[i].clone());
            remaining -= message_tokens[i];
        } else {
            break;
        }
    }

    tail.reverse();
    result.extend(tail);
    result
}

/// Compact old tool results to reduce token usage.
///
/// Replaces old ToolResult messages with short summaries while keeping
/// the last `keep_recent` tool results intact for context.
pub fn compact_stale_tool_results(messages: &[AgentMessage], keep_recent: usize) -> Vec<AgentMessage> {
    let tail_start_idx = crate::compaction::tail_start_for_recent_tool_results(messages, keep_recent);
    crate::compaction::prune_tool_results(messages, tail_start_idx)
}

/// Estimate tokens for a single message
fn estimate_message_tokens(message: &AgentMessage) -> usize {
    let json = serde_json::to_string(message).unwrap_or_default();
    clankers_util::token::estimate_tokens(&json)
}

/// Build full context for an LLM request
pub fn build_context(
    messages: &[AgentMessage],
    system_prompt: &str,
    max_input_tokens: usize,
    compact: bool,
) -> AgentContext {
    let system_tokens = clankers_util::token::estimate_tokens(system_prompt);
    let effective_messages = if compact {
        compact_stale_tool_results(messages, crate::compaction::RECENT_TOOL_RESULTS_TO_KEEP)
    } else {
        messages.to_vec()
    };
    let truncated = truncate_messages(&effective_messages, max_input_tokens, system_tokens);
    let msg_tokens: usize = truncated.iter().map(estimate_message_tokens).sum();
    let total_tokens = system_tokens + msg_tokens;

    // Auto-nudge when context exceeds 80% capacity
    let final_prompt = if max_input_tokens > 0 && total_tokens * 100 / max_input_tokens >= 80 {
        let pct = total_tokens * 100 / max_input_tokens;
        format!(
            "{}\n\n[Context is at {}% capacity. Consider using the compress tool to summarize older messages.]",
            system_prompt, pct
        )
    } else {
        system_prompt.to_string()
    };

    AgentContext {
        system_prompt: final_prompt,
        messages: truncated,
        estimated_tokens: total_tokens,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn make_user_msg(text: &str) -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::generate(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    }

    fn make_tool_result_msg(tool_name: &str, text: &str) -> AgentMessage {
        AgentMessage::ToolResult(ToolResultMessage {
            id: MessageId::generate(),
            call_id: format!("call_{}", generate_id()),
            tool_name: tool_name.to_string(),
            content: vec![Content::Text { text: text.to_string() }],
            is_error: false,
            details: None,
            timestamp: Utc::now(),
        })
    }

    fn make_tool_result_with_image(tool_name: &str) -> AgentMessage {
        AgentMessage::ToolResult(ToolResultMessage {
            id: MessageId::generate(),
            call_id: format!("call_{}", generate_id()),
            tool_name: tool_name.to_string(),
            content: vec![Content::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string(),
                }
            }],
            is_error: false,
            details: None,
            timestamp: Utc::now(),
        })
    }

    #[test]
    fn test_build_system_prompt_basic() {
        let settings = Settings::default();
        let result = build_system_prompt("base prompt", &settings, &[], &[], &[]);
        assert!(result.contains("base prompt"));
    }

    #[test]
    fn test_build_system_prompt_with_tools() {
        let settings = Settings::default();
        let tools = vec!["read".to_string(), "write".to_string()];
        let result = build_system_prompt("base", &settings, &tools, &[], &[]);
        assert!(result.contains("read"));
        assert!(result.contains("write"));
    }

    #[test]
    fn test_build_system_prompt_with_prefix_suffix() {
        let settings = Settings {
            system_prompt_prefix: Some("PREFIX".to_string()),
            system_prompt_suffix: Some("SUFFIX".to_string()),
            ..Default::default()
        };
        let result = build_system_prompt("base", &settings, &[], &[], &[]);
        assert!(result.starts_with("PREFIX"));
        assert!(result.ends_with("SUFFIX"));
    }

    #[test]
    fn test_truncate_messages_within_budget() {
        let msgs = vec![make_user_msg("hello"), make_user_msg("world")];
        let result = truncate_messages(&msgs, 100_000, 100);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_truncate_messages_drops_middle() {
        // Create messages that exceed budget
        let msgs: Vec<AgentMessage> =
            (0..20).map(|i| make_user_msg(&format!("message number {} with some content", i))).collect();
        let result = truncate_messages(&msgs, 500, 100);
        // Should keep first + some recent
        assert!(result.len() < msgs.len());
        assert!(!result.is_empty());
    }

    #[test]
    fn test_build_context() {
        let msgs = vec![make_user_msg("hello")];
        let ctx = build_context(&msgs, "system prompt", 100_000, true);
        assert_eq!(ctx.system_prompt, "system prompt");
        assert!(!ctx.messages.is_empty());
        assert!(ctx.estimated_tokens > 0);
    }

    #[test]
    fn test_compact_keeps_recent_tool_results() {
        let msgs = vec![
            make_user_msg("start"),
            make_tool_result_msg("read", "old content"),
            make_tool_result_msg("write", "another old result"),
            make_tool_result_msg("bash", "recent result 1"),
            make_tool_result_msg("edit", "recent result 2"),
            make_tool_result_msg("grep", "recent result 3"),
            make_user_msg("end"),
        ];

        let result = compact_stale_tool_results(&msgs, 3);
        assert_eq!(result.len(), 7);

        // Check that the last 3 tool results are kept intact
        if let AgentMessage::ToolResult(tool_result) = &result[3] {
            assert_eq!(tool_result.tool_name, "bash");
            if let Content::Text { text } = &tool_result.content[0] {
                assert_eq!(text, "recent result 1");
            }
        } else {
            panic!("Expected ToolResult");
        }

        // Check that earlier tool results are compacted
        if let AgentMessage::ToolResult(tool_result) = &result[1] {
            assert_eq!(tool_result.tool_name, "read");
            if let Content::Text { text } = &tool_result.content[0] {
                assert!(text.starts_with("[read]"));
                assert_ne!(text, "old content");
                assert!(text.contains("chars"));
            }
        } else {
            panic!("Expected ToolResult");
        }
    }

    #[test]
    fn test_compact_replaces_old_tool_results() {
        let old_content = "This is a multi-line\ntool result\nwith several lines";
        let msgs = vec![
            make_tool_result_msg("read", old_content),
            make_tool_result_msg("write", "recent content"),
        ];

        let result = compact_stale_tool_results(&msgs, 1);
        assert_eq!(result.len(), 2);

        // First (old) result should be compacted
        if let AgentMessage::ToolResult(tool_result) = &result[0] {
            if let Content::Text { text } = &tool_result.content[0] {
                assert!(text.starts_with("[read]"));
                assert_ne!(text, old_content);
                assert!(text.contains("chars"));
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected ToolResult");
        }

        // Second (recent) result should be intact
        if let AgentMessage::ToolResult(tool_result) = &result[1] {
            if let Content::Text { text } = &tool_result.content[0] {
                assert_eq!(text, "recent content");
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected ToolResult");
        }
    }

    #[test]
    fn test_compact_preserves_non_tool_messages() {
        let msgs = vec![
            make_user_msg("user message 1"),
            make_tool_result_msg("read", "old tool result"),
            make_user_msg("user message 2"),
            make_tool_result_msg("write", "recent tool result"),
        ];

        let result = compact_stale_tool_results(&msgs, 1);
        assert_eq!(result.len(), 4);

        // User messages should be unchanged
        if let AgentMessage::User(user_msg) = &result[0] {
            if let Content::Text { text } = &user_msg.content[0] {
                assert_eq!(text, "user message 1");
            }
        } else {
            panic!("Expected User message");
        }

        if let AgentMessage::User(user_msg) = &result[2] {
            if let Content::Text { text } = &user_msg.content[0] {
                assert_eq!(text, "user message 2");
            }
        } else {
            panic!("Expected User message");
        }
    }

    #[test]
    fn test_compact_with_no_tool_results() {
        let msgs = vec![make_user_msg("hello"), make_user_msg("world")];

        let result = compact_stale_tool_results(&msgs, 3);
        assert_eq!(result.len(), 2);

        // Messages should be unchanged
        for (i, msg) in result.iter().enumerate() {
            if let AgentMessage::User(user_msg) = msg {
                if let Content::Text { text } = &user_msg.content[0] {
                    match i {
                        0 => assert_eq!(text, "hello"),
                        1 => assert_eq!(text, "world"),
                        _ => panic!("Unexpected message"),
                    }
                }
            } else {
                panic!("Expected User message");
            }
        }
    }

    #[test]
    fn test_compact_with_image_content() {
        let msgs = vec![
            make_tool_result_with_image("screenshot"),
            make_tool_result_msg("read", "recent content"),
        ];

        let result = compact_stale_tool_results(&msgs, 1);
        assert_eq!(result.len(), 2);

        // Image result should be compacted
        if let AgentMessage::ToolResult(tool_result) = &result[0] {
            if let Content::Text { text } = &tool_result.content[0] {
                assert_eq!(text, "[screenshot] (image result)");
            } else {
                panic!("Expected text content after compaction");
            }
        } else {
            panic!("Expected ToolResult");
        }
    }

    #[test]
    fn test_nudge_at_high_capacity() {
        // Create a context that uses >80% of a small budget
        let msgs = vec![make_user_msg("hello world this is a test message")];
        let ctx = build_context(&msgs, "system prompt", 50, false);
        // With such a tiny budget, the nudge should trigger
        assert!(ctx.system_prompt.contains("capacity"));
    }

    #[test]
    fn test_no_nudge_at_low_capacity() {
        let msgs = vec![make_user_msg("hi")];
        let ctx = build_context(&msgs, "system", 1_000_000, false);
        assert!(!ctx.system_prompt.contains("capacity"));
    }
}
