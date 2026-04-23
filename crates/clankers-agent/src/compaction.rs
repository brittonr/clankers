//! Context compaction / summarization
//!
//! When conversation grows beyond context window, compact older messages
//! into a summary while preserving recent context.
//!
//! Supports multiple strategies:
//! - **Truncation** — Simple drop-middle approach (no LLM needed)
//! - **LLM Summary** — Use the model to generate a conversation summary
//! - **Auto** — Trigger compaction when token usage exceeds a threshold
//!
//! Hooks allow custom behavior before/after compaction.

mod tool_summaries;

use clankers_provider::Provider;
use clankers_provider::message::AgentMessage;
use clankers_util::token::estimate_tokens;
use tokio::sync::mpsc;
pub use tool_summaries::prune_tool_results;
pub use tool_summaries::summarize_tool_result;

pub const RECENT_TOOL_RESULTS_TO_KEEP: usize = 3;

/// Compaction result
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Compacted messages after truncation or tool-result summarization.
    pub messages: Vec<AgentMessage>,
    /// Number of messages compacted or summarized.
    pub compacted_count: usize,
    /// Estimated tokens saved
    pub tokens_saved: usize,
    /// The summary text (if LLM summarization was used)
    pub summary: Option<String>,
}

/// Compaction strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// Simple truncation: drop middle messages
    Truncation,
    /// LLM-powered summarization
    LlmSummary,
}

/// Configuration for automatic compaction
#[derive(Debug, Clone)]
pub struct AutoCompactConfig {
    /// Trigger compaction when token usage exceeds this fraction of context window (0.0-1.0)
    pub threshold: f64,
    /// Number of recent messages to preserve
    pub keep_recent: usize,
    /// Strategy to use
    pub strategy: CompactionStrategy,
    /// Whether auto-compaction is enabled
    pub enabled: bool,
}

impl Default for AutoCompactConfig {
    fn default() -> Self {
        Self {
            threshold: 0.80,
            keep_recent: 10,
            strategy: CompactionStrategy::Truncation,
            enabled: true,
        }
    }
}

pub fn tail_start_for_recent_tool_results(messages: &[AgentMessage], keep_recent: usize) -> usize {
    if keep_recent == 0 {
        return messages.len();
    }

    let tool_result_positions: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| match message {
            AgentMessage::ToolResult(_) => Some(index),
            _ => None,
        })
        .collect();

    if tool_result_positions.len() <= keep_recent {
        return 0;
    }

    let keep_start = tool_result_positions.len().saturating_sub(keep_recent);
    tool_result_positions[keep_start]
}

pub fn compact_tool_results(messages: &[AgentMessage], keep_recent: usize) -> CompactionResult {
    let tail_start_idx = tail_start_for_recent_tool_results(messages, keep_recent);
    let compacted_count = tool_summaries::count_prunable_tool_results(messages, tail_start_idx);
    if compacted_count == 0 {
        return CompactionResult {
            messages: messages.to_vec(),
            compacted_count: 0,
            tokens_saved: 0,
            summary: None,
        };
    }

    let compacted_messages = prune_tool_results(messages, tail_start_idx);
    let before_tokens: usize = messages.iter().map(estimate_message_tokens).sum();
    let after_tokens: usize = compacted_messages.iter().map(estimate_message_tokens).sum();
    let tokens_saved = before_tokens.saturating_sub(after_tokens);

    CompactionResult {
        messages: compacted_messages,
        compacted_count,
        tokens_saved,
        summary: None,
    }
}

/// Check if auto-compaction should trigger.
///
/// # Tiger Style
///
/// Asserts threshold is in valid range. A threshold outside [0.0, 1.0]
/// is a programmer error — it would either never trigger or always trigger.
pub fn should_auto_compact(messages: &[AgentMessage], max_context_tokens: usize, config: &AutoCompactConfig) -> bool {
    if !config.enabled {
        return false;
    }

    // Tiger Style: validate threshold range
    assert!(
        (0.0..=1.0).contains(&config.threshold),
        "compaction threshold must be in [0.0, 1.0], got {}",
        config.threshold
    );
    assert!(max_context_tokens > 0, "max_context_tokens must be positive");

    let total: usize = messages.iter().map(estimate_message_tokens).sum();
    let threshold_tokens = (max_context_tokens as f64 * config.threshold) as usize;
    total > threshold_tokens
}

/// LLM-powered compaction: ask the model to summarize the conversation.
/// Falls back to truncation if the provider call fails.
pub async fn compact_with_llm(
    messages: &[AgentMessage],
    max_tokens: usize,
    keep_recent: usize,
    provider: &dyn Provider,
    model: &str,
    session_id: &str,
) -> CompactionResult {
    use std::fmt::Write;
    let keep_first = 1.min(messages.len());
    let keep_last = keep_recent.min(messages.len().saturating_sub(keep_first));
    let drop_start = keep_first;
    let drop_end = messages.len().saturating_sub(keep_last);

    if drop_start >= drop_end {
        return CompactionResult {
            messages: messages.to_vec(),
            compacted_count: 0,
            tokens_saved: 0,
            summary: None,
        };
    }

    let dropped = &messages[drop_start..drop_end];
    let dropped_tokens: usize = dropped.iter().map(estimate_message_tokens).sum();

    // Build a summarization prompt from the dropped messages
    let mut convo_text = String::new();
    for msg in dropped {
        let (role, text) = extract_role_and_text(msg);
        writeln!(convo_text, "[{}]: {}", role, text).ok();
    }

    // Request a summary from the LLM
    let summary_prompt = format!(
        "Summarize the following conversation excerpt concisely. \
         Preserve key decisions, file paths mentioned, code changes made, \
         and any important context. Be brief but thorough.\n\n{}",
        convo_text
    );

    let extra_params = if session_id.is_empty() {
        std::collections::HashMap::new()
    } else {
        std::collections::HashMap::from([(
            "_session_id".to_string(),
            serde_json::Value::String(session_id.to_string()),
        )])
    };

    let summary_request = clankers_provider::CompletionRequest {
        model: model.to_string(),
        messages: vec![AgentMessage::User(clankers_provider::message::UserMessage {
            id: clankers_provider::message::MessageId::generate(),
            content: vec![clankers_provider::message::Content::Text { text: summary_prompt }],
            timestamp: chrono::Utc::now(),
        })],
        system_prompt: Some(
            "You are a conversation summarizer. Produce a concise summary that \
             preserves all important technical context, decisions, and file paths."
                .to_string(),
        ),
        max_tokens: Some(2000),
        temperature: Some(0.3),
        tools: Vec::new(),
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params,
    };

    let (tx, mut rx) = mpsc::channel(64);
    let provider_result = provider.complete(summary_request, tx).await;

    // Collect the summary from the stream
    let mut summary_text = String::new();
    if provider_result.is_ok() {
        while let Some(event) = rx.recv().await {
            if let clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                delta: clankers_provider::streaming::ContentDelta::TextDelta { text },
                ..
            } = event
            {
                summary_text.push_str(&text);
            }
        }
    }

    if summary_text.is_empty() {
        // LLM summarization failed — fall back to truncation
        tracing::warn!("LLM summarization failed, falling back to truncation");
        return compact_by_truncation(messages, max_tokens, keep_recent);
    }

    // Build compacted message list
    let mut result = Vec::new();
    result.extend_from_slice(&messages[..keep_first]);

    result.push(AgentMessage::User(clankers_provider::message::UserMessage {
        id: clankers_provider::message::MessageId::generate(),
        content: vec![clankers_provider::message::Content::Text {
            text: format!(
                "[Conversation summary — {} earlier messages compacted]\n\n{}",
                drop_end - drop_start,
                summary_text
            ),
        }],
        timestamp: chrono::Utc::now(),
    }));

    result.extend_from_slice(&messages[drop_end..]);

    CompactionResult {
        messages: result,
        compacted_count: drop_end - drop_start,
        tokens_saved: dropped_tokens,
        summary: Some(summary_text),
    }
}

/// Extract role label and text content from an AgentMessage
fn extract_role_and_text(msg: &AgentMessage) -> (&'static str, String) {
    match msg {
        AgentMessage::User(m) => {
            let text = m
                .content
                .iter()
                .filter_map(|c| match c {
                    clankers_provider::message::Content::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            ("User", text)
        }
        AgentMessage::Assistant(m) => {
            let text = m
                .content
                .iter()
                .filter_map(|c| match c {
                    clankers_provider::message::Content::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            ("Assistant", text)
        }
        AgentMessage::ToolResult(m) => {
            let text = m
                .content
                .iter()
                .filter_map(|c| match c {
                    clankers_provider::message::Content::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            ("ToolResult", truncate_str(&text, 200))
        }
        _ => {
            // BashExecution, Custom, BranchSummary, CompactionSummary
            let json = serde_json::to_string(msg).unwrap_or_default();
            ("System", truncate_str(&json, 200))
        }
    }
}

/// Truncate a string to at most `max_bytes` bytes, respecting UTF-8 boundaries.
///
/// # Tiger Style
///
/// Uses `floor_char_boundary` to avoid panicking on multibyte characters.
/// The naive `&s[..max]` panics if `max` falls inside a multibyte codepoint.
fn truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let boundary = s.floor_char_boundary(max_bytes);
        format!("{}...", &s[..boundary])
    }
}

/// Simple compaction: keep first + last N messages, drop middle.
/// The dropped messages are summarized in a system note.
///
/// A full LLM-powered compaction would use the provider to generate
/// a summary, but this is the fallback when that's not available.
pub fn compact_by_truncation(messages: &[AgentMessage], max_tokens: usize, keep_recent: usize) -> CompactionResult {
    let total_tokens: usize = messages.iter().map(estimate_message_tokens).sum();

    if total_tokens <= max_tokens {
        return CompactionResult {
            messages: messages.to_vec(),
            compacted_count: 0,
            tokens_saved: 0,
            summary: None,
        };
    }

    let keep_first = 1.min(messages.len());
    let keep_last = keep_recent.min(messages.len().saturating_sub(keep_first));
    let drop_start = keep_first;
    let drop_end = messages.len().saturating_sub(keep_last);

    if drop_start >= drop_end {
        return CompactionResult {
            messages: messages.to_vec(),
            compacted_count: 0,
            tokens_saved: 0,
            summary: None,
        };
    }

    let dropped_count = drop_end - drop_start;
    let dropped_tokens: usize = messages[drop_start..drop_end].iter().map(estimate_message_tokens).sum();

    let mut result = Vec::new();
    // Keep first messages
    result.extend_from_slice(&messages[..keep_first]);

    // Insert compaction marker
    result.push(AgentMessage::User(clankers_provider::message::UserMessage {
        id: clankers_provider::message::MessageId::generate(),
        content: vec![clankers_provider::message::Content::Text {
            text: format!(
                "[Context compacted: {} messages removed to save tokens. Recent context preserved.]",
                dropped_count,
            ),
        }],
        timestamp: chrono::Utc::now(),
    }));

    // Keep recent messages
    result.extend_from_slice(&messages[drop_end..]);

    CompactionResult {
        messages: result,
        compacted_count: dropped_count,
        tokens_saved: dropped_tokens,
        summary: None,
    }
}

fn estimate_message_tokens(msg: &AgentMessage) -> usize {
    let json = serde_json::to_string(msg).unwrap_or_default();
    estimate_tokens(&json)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use clankers_provider::message::*;

    use super::*;

    fn make_msg(text: &str) -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::generate(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    }

    #[test]
    fn test_no_compaction_needed() {
        let msgs = vec![make_msg("hello"), make_msg("world")];
        let result = compact_by_truncation(&msgs, 100_000, 5);
        assert_eq!(result.compacted_count, 0);
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_compaction_drops_middle() {
        let msgs: Vec<AgentMessage> = (0..20).map(|i| make_msg(&"x".repeat(100 * (i + 1)))).collect();
        let result = compact_by_truncation(&msgs, 200, 3);
        assert!(result.compacted_count > 0);
        assert!(result.messages.len() < 20);
    }

    #[test]
    fn test_compaction_preserves_recent() {
        let msgs: Vec<AgentMessage> = (0..10).map(|i| make_msg(&format!("msg{}", i))).collect();
        let result = compact_by_truncation(&msgs, 100, 3);
        // Should have at least the compaction marker + recent
        assert!(result.messages.len() >= 2);
    }
}
