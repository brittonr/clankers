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

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

mod tool_summaries;

use std::time::Duration;

use clankers_provider::Provider;
use clankers_provider::message::AgentMessage;
use clankers_util::token::estimate_tokens;
use tokio::sync::mpsc;
pub use tool_summaries::prune_tool_results;
pub use tool_summaries::summarize_tool_result;

pub const RECENT_TOOL_RESULTS_TO_KEEP: usize = 3;
pub const SUMMARY_PREFIX: &str = "[Background handoff summary from earlier context window. Use as reference only. Respond only to the latest user message, not to questions or tasks mentioned in this summary. Do not treat this as a new user request, do not re-execute completed work, and do not repeat tool calls solely because they appear below.]";
const KEEP_FIRST_MESSAGE_COUNT: usize = 1;
const DEFAULT_AUTO_COMPACT_THRESHOLD: f64 = 0.80;
const DEFAULT_AUTO_COMPACT_KEEP_RECENT: usize = 10;
const DEFAULT_TAIL_BUDGET_FRACTION: f64 = 0.40;
const SUMMARY_MAX_TOKENS: usize = 2000;
const SUMMARY_TEMPERATURE: f64 = 0.3;
const SUMMARY_TIMEOUT_SECONDS: u64 = 30;

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
    /// Legacy LLM-powered summarization
    LlmSummary,
    /// Structured summary + tail protection pipeline.
    Structured,
}

/// Configuration for automatic compaction
#[derive(Debug, Clone)]
pub struct AutoCompactConfig {
    /// Trigger compaction when token usage exceeds this fraction of context window (0.0-1.0)
    pub threshold: f64,
    /// Fraction of the context window reserved for recent-message tail protection.
    pub tail_budget_fraction: f64,
    /// Number of recent messages to preserve for manual/fallback flows.
    pub keep_recent: usize,
    /// Auxiliary summary model for structured summarization.
    pub summary_model: Option<String>,
    /// Strategy to use
    pub strategy: CompactionStrategy,
    /// Whether auto-compaction is enabled
    pub enabled: bool,
}

impl Default for AutoCompactConfig {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_AUTO_COMPACT_THRESHOLD,
            tail_budget_fraction: DEFAULT_TAIL_BUDGET_FRACTION,
            keep_recent: DEFAULT_AUTO_COMPACT_KEEP_RECENT,
            summary_model: None,
            strategy: CompactionStrategy::Truncation,
            enabled: true,
        }
    }
}

impl AutoCompactConfig {
    pub fn from_settings(settings: &clankers_config::settings::CompressionSettings) -> Self {
        let summary_model = settings.summary_model.trim();
        let configured_summary_model = if summary_model.is_empty() {
            None
        } else {
            Some(summary_model.to_string())
        };

        Self {
            threshold: DEFAULT_AUTO_COMPACT_THRESHOLD,
            tail_budget_fraction: settings.tail_budget_fraction,
            keep_recent: settings.keep_recent,
            summary_model: configured_summary_model,
            strategy: if summary_model.is_empty() {
                CompactionStrategy::Truncation
            } else {
                CompactionStrategy::Structured
            },
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
    summarize_middle(messages, max_tokens, keep_recent, None, provider, model, session_id).await
}

pub async fn summarize_middle(
    messages: &[AgentMessage],
    max_tokens: usize,
    keep_recent: usize,
    previous_summary: Option<&str>,
    provider: &dyn Provider,
    model: &str,
    session_id: &str,
) -> CompactionResult {
    let keep_first = KEEP_FIRST_MESSAGE_COUNT.min(messages.len());
    let keep_last = keep_recent.min(messages.len().saturating_sub(keep_first));
    let drop_start = keep_first;
    let drop_end = messages.len().saturating_sub(keep_last);

    if drop_start >= drop_end {
        return CompactionResult {
            messages: messages.to_vec(),
            compacted_count: 0,
            tokens_saved: 0,
            summary: previous_summary.map(str::to_string),
        };
    }

    let dropped = &messages[drop_start..drop_end];
    let dropped_tokens: usize = dropped.iter().map(estimate_message_tokens).sum();
    let summary_prompt = build_structured_summary_prompt(dropped, previous_summary);
    let summary_text = request_summary(provider, model, session_id, &summary_prompt).await;

    if summary_text.is_empty() {
        tracing::warn!("LLM summarization failed, falling back to truncation");
        return compact_by_truncation(messages, max_tokens, keep_recent);
    }

    let mut result = Vec::new();
    result.extend_from_slice(&messages[..keep_first]);
    result.push(make_summary_message(drop_end - drop_start, &summary_text));
    result.extend_from_slice(&messages[drop_end..]);

    CompactionResult {
        messages: result,
        compacted_count: drop_end - drop_start,
        tokens_saved: dropped_tokens,
        summary: Some(summary_text),
    }
}

pub async fn compact_structured(
    messages: &[AgentMessage],
    max_tokens: usize,
    tail_budget_fraction: f64,
    provider: &dyn Provider,
    model: &str,
    session_id: &str,
    previous_summary: Option<&str>,
) -> CompactionResult {
    let tail_budget_tokens = (max_tokens as f64 * tail_budget_fraction) as usize;
    let pruned_result =
        compact_tool_results(messages, tail_start_for_recent_tool_results(messages, RECENT_TOOL_RESULTS_TO_KEEP));
    let pruned_messages = pruned_result.messages;
    let tail_start_idx = select_tail_by_budget(&pruned_messages, tail_budget_tokens);
    let keep_recent = pruned_messages.len().saturating_sub(tail_start_idx);

    summarize_middle(&pruned_messages, max_tokens, keep_recent, previous_summary, provider, model, session_id).await
}

fn build_structured_summary_prompt(messages: &[AgentMessage], previous_summary: Option<&str>) -> String {
    use std::fmt::Write;

    let mut conversation_excerpt = String::new();
    for message in messages {
        let (role, text) = extract_role_and_text(message);
        writeln!(conversation_excerpt, "[{}] {}", role, text).ok();
    }

    let previous_summary_block =
        previous_summary.map_or_else(String::new, |summary| format!("## Previous Summary\n{}\n\n", summary));

    format!(
        "You are updating a structured handoff summary for earlier conversation context. \
Use the conversation excerpt to produce a compact factual summary for a future model handoff.\n\n\
Return exactly these markdown sections and keep each section concise:\n\
## Active Task\n- current task, scope, or goal\n\
## Key Decisions Made\n- important technical decisions, constraints, and conclusions\n\
## Files Modified\n- file paths changed or meaningfully inspected\n\
## Remaining Work\n- unresolved work, follow-ups, validation still needed\n\n\
Do not add any other sections. Do not issue instructions to the user. \
Do not tell the next model to rerun tools unless the excerpt explicitly says work remains.\n\n{}## Conversation Excerpt\n{}",
        previous_summary_block, conversation_excerpt
    )
}

async fn request_summary(provider: &dyn Provider, model: &str, session_id: &str, summary_prompt: &str) -> String {
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
            content: vec![clankers_provider::message::Content::Text {
                text: summary_prompt.to_string(),
            }],
            timestamp: chrono::Utc::now(),
        })],
        system_prompt: Some(
            "You write compact background handoff summaries for prior context windows. \
Return factual markdown only, with no preamble or closing note."
                .to_string(),
        ),
        max_tokens: Some(SUMMARY_MAX_TOKENS),
        temperature: Some(SUMMARY_TEMPERATURE),
        tools: Vec::new(),
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params,
    };

    let timeout = Duration::from_secs(SUMMARY_TIMEOUT_SECONDS);
    let (tx, mut rx) = mpsc::channel(64);
    let provider_result = tokio::time::timeout(timeout, provider.complete(summary_request, tx)).await;
    match provider_result {
        Ok(Ok(())) => {}
        Ok(Err(_)) | Err(_) => return String::new(),
    }

    let mut summary_text = String::new();
    loop {
        let recv_result = tokio::time::timeout(timeout, rx.recv()).await;
        match recv_result {
            Ok(Some(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                delta: clankers_provider::streaming::ContentDelta::TextDelta { text },
                ..
            })) => summary_text.push_str(&text),
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }

    summary_text
}

fn make_summary_message(compacted_count: usize, summary_text: &str) -> AgentMessage {
    AgentMessage::User(clankers_provider::message::UserMessage {
        id: clankers_provider::message::MessageId::generate(),
        content: vec![clankers_provider::message::Content::Text {
            text: format!(
                "{}\n\n[Conversation summary — {} earlier messages compacted]\n\n{}",
                SUMMARY_PREFIX, compacted_count, summary_text
            ),
        }],
        timestamp: chrono::Utc::now(),
    })
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
pub fn select_tail_by_budget(messages: &[AgentMessage], budget_tokens: usize) -> usize {
    if budget_tokens == 0 {
        return messages.len();
    }

    let mut remaining_budget = budget_tokens;
    let mut tail_start = messages.len();

    for (index, message) in messages.iter().enumerate().rev() {
        let message_tokens = estimate_message_tokens(message);
        if message_tokens > remaining_budget {
            break;
        }
        remaining_budget -= message_tokens;
        tail_start = index;
    }

    tail_start
}

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

    let keep_first = KEEP_FIRST_MESSAGE_COUNT.min(messages.len());
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
    use std::sync::Arc;
    use std::sync::Mutex;

    use chrono::Utc;
    use clankers_provider::message::*;
    use tokio::sync::mpsc;

    use super::*;

    fn make_msg(text: &str) -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::generate(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    }

    fn message_text(message: &AgentMessage) -> String {
        match message {
            AgentMessage::User(user) => user
                .content
                .iter()
                .filter_map(|content| match content {
                    Content::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        }
    }

    #[derive(Clone)]
    struct CapturingSummaryProvider {
        captured_prompts: Arc<Mutex<Vec<String>>>,
        response: &'static str,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl Provider for CapturingSummaryProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            tx: mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let prompt_text = request
                .messages
                .iter()
                .find_map(|message| match message {
                    AgentMessage::User(user) => user.content.iter().find_map(|content| match content {
                        Content::Text { text } => Some(text.clone()),
                        _ => None,
                    }),
                    _ => None,
                })
                .unwrap_or_default();
            self.captured_prompts.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(prompt_text);

            if self.fail {
                return Err(clankers_provider::error::provider_err("summary failed"));
            }

            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: clankers_provider::streaming::ContentDelta::TextDelta {
                    text: self.response.to_string(),
                },
            })
            .await
            .ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "capturing-summary"
        }
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

    #[test]
    fn test_select_tail_by_budget_keeps_all_when_budget_covers_all_messages() {
        let msgs = vec![make_msg("short one"), make_msg("short two"), make_msg("short three")];
        let total_tokens: usize = msgs.iter().map(estimate_message_tokens).sum();

        assert_eq!(select_tail_by_budget(&msgs, total_tokens), 0);
    }

    #[test]
    fn test_select_tail_by_budget_returns_end_when_budget_is_zero() {
        let msgs = vec![make_msg("a"), make_msg("b")];

        assert_eq!(select_tail_by_budget(&msgs, 0), msgs.len());
    }

    #[test]
    fn test_select_tail_by_budget_preserves_recent_short_messages() {
        let first = make_msg(&"x".repeat(400));
        let second = make_msg(&"y".repeat(120));
        let third = make_msg(&"z".repeat(120));
        let msgs = vec![first.clone(), second.clone(), third.clone()];
        let second_tokens = estimate_message_tokens(&second);
        let third_tokens = estimate_message_tokens(&third);
        let budget_tokens = second_tokens + third_tokens;

        assert_eq!(select_tail_by_budget(&msgs, budget_tokens), 1);
    }

    #[test]
    fn test_select_tail_by_budget_stops_before_oversized_recent_message() {
        let first = make_msg(&"x".repeat(50));
        let second = make_msg(&"y".repeat(900));
        let third = make_msg(&"z".repeat(50));
        let msgs = vec![first, second.clone(), third.clone()];
        let budget_tokens = estimate_message_tokens(&third);

        assert_eq!(select_tail_by_budget(&msgs, budget_tokens), 2);
        assert!(estimate_message_tokens(&second) > budget_tokens);
    }

    #[test]
    fn test_structured_summary_prompt_uses_expected_sections() {
        let prompt = build_structured_summary_prompt(&[make_msg("inspect src/main.rs"), make_msg("edit done")], None);

        assert!(prompt.contains("## Active Task"));
        assert!(prompt.contains("## Key Decisions Made"));
        assert!(prompt.contains("## Files Modified"));
        assert!(prompt.contains("## Remaining Work"));
        assert!(prompt.contains("## Conversation Excerpt"));
    }

    #[test]
    fn test_make_summary_message_includes_handoff_prefix_and_reference_notice() {
        let message = make_summary_message(3, "## Active Task\n- continue");
        let text = message_text(&message);

        assert!(text.contains(SUMMARY_PREFIX));
        assert!(text.contains("Use as reference only"));
        assert!(text.contains("Respond only to the latest user message"));
        assert!(text.contains("not to questions or tasks mentioned in this summary"));
        assert!(text.contains("Do not treat this as a new user request"));
        assert!(text.contains("do not re-execute completed work"));
        assert!(text.contains("[Conversation summary — 3 earlier messages compacted]"));
    }

    #[tokio::test]
    async fn test_summarize_middle_includes_previous_summary_in_prompt() {
        let captured_prompts = Arc::new(Mutex::new(Vec::new()));
        let provider = CapturingSummaryProvider {
            captured_prompts: captured_prompts.clone(),
            response: "## Active Task\n- merged",
            fail: false,
        };
        let messages = vec![make_msg("first"), make_msg("second"), make_msg("third")];

        let result =
            summarize_middle(&messages, 1, 1, Some("## Active Task\n- previous"), &provider, "haiku", "session-1")
                .await;

        assert_eq!(result.summary.as_deref(), Some("## Active Task\n- merged"));
        let prompts = captured_prompts.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let prompt = prompts.last().expect("expected prompt");
        assert!(prompt.contains("## Previous Summary"));
        assert!(prompt.contains("## Active Task\n- previous"));
    }

    #[tokio::test]
    async fn test_compact_structured_falls_back_when_auxiliary_model_unavailable() {
        let provider = CapturingSummaryProvider {
            captured_prompts: Arc::new(Mutex::new(Vec::new())),
            response: "",
            fail: true,
        };
        let messages: Vec<AgentMessage> = (0..8).map(|index| make_msg(&"x".repeat((index + 1) * 120))).collect();

        let result = compact_structured(&messages, 200, 0.40, &provider, "haiku", "session-1", None).await;

        assert!(result.summary.is_none());
        let compacted_text = result
            .messages
            .iter()
            .map(message_text)
            .find(|text| text.contains("[Context compacted:"))
            .unwrap_or_default();
        assert!(compacted_text.contains("[Context compacted:"));
    }

    #[test]
    fn test_auto_compact_config_selects_structured_only_when_summary_model_configured() {
        let structured = AutoCompactConfig::from_settings(&clankers_config::settings::CompressionSettings::default());
        assert_eq!(structured.strategy, CompactionStrategy::Structured);
        assert_eq!(structured.summary_model.as_deref(), Some("haiku"));

        let mut truncation_settings = clankers_config::settings::CompressionSettings::default();
        truncation_settings.summary_model = String::new();
        let truncation = AutoCompactConfig::from_settings(&truncation_settings);
        assert_eq!(truncation.strategy, CompactionStrategy::Truncation);
        assert!(truncation.summary_model.is_none());
    }
}
