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

use tokio::sync::mpsc;

use crate::provider::Provider;
use crate::provider::message::AgentMessage;
use crate::util::token::estimate_tokens;

/// Compaction result
#[derive(Debug)]
pub struct CompactionResult {
    /// Compacted messages (fewer than input)
    pub messages: Vec<AgentMessage>,
    /// Number of messages removed
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

/// Check if auto-compaction should trigger
pub fn should_auto_compact(messages: &[AgentMessage], max_context_tokens: usize, config: &AutoCompactConfig) -> bool {
    if !config.enabled {
        return false;
    }
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
        let _ = writeln!(convo_text, "[{}]: {}", role, text);
    }

    // Request a summary from the LLM
    let summary_prompt = format!(
        "Summarize the following conversation excerpt concisely. \
         Preserve key decisions, file paths mentioned, code changes made, \
         and any important context. Be brief but thorough.\n\n{}",
        convo_text
    );

    let summary_request = crate::provider::CompletionRequest {
        model: model.to_string(),
        messages: vec![AgentMessage::User(crate::provider::message::UserMessage {
            id: crate::provider::message::MessageId::generate(),
            content: vec![crate::provider::message::Content::Text { text: summary_prompt }],
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
    };

    let (tx, mut rx) = mpsc::channel(64);
    let provider_result = provider.complete(summary_request, tx).await;

    // Collect the summary from the stream
    let mut summary_text = String::new();
    if provider_result.is_ok() {
        while let Some(event) = rx.recv().await {
            if let crate::provider::streaming::StreamEvent::ContentBlockDelta {
                delta: crate::provider::streaming::ContentDelta::TextDelta { text },
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

    result.push(AgentMessage::User(crate::provider::message::UserMessage {
        id: crate::provider::message::MessageId::generate(),
        content: vec![crate::provider::message::Content::Text {
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
                    crate::provider::message::Content::Text { text } => Some(text.as_str()),
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
                    crate::provider::message::Content::Text { text } => Some(text.as_str()),
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
                    crate::provider::message::Content::Text { text } => Some(text.as_str()),
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

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
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
    result.push(AgentMessage::User(crate::provider::message::UserMessage {
        id: crate::provider::message::MessageId::generate(),
        content: vec![crate::provider::message::Content::Text {
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

    use super::*;
    use crate::provider::message::*;

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
