//! Context building, token estimation, window management

use clankers_config::settings::Settings;
use clankers_provider::message::AgentMessage;

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

/// Estimate tokens for a single message
fn estimate_message_tokens(message: &AgentMessage) -> usize {
    let json = serde_json::to_string(message).unwrap_or_default();
    clankers_util::token::estimate_tokens(&json)
}

/// Build full context for an LLM request
pub fn build_context(messages: &[AgentMessage], system_prompt: &str, max_input_tokens: usize) -> AgentContext {
    let system_tokens = clankers_util::token::estimate_tokens(system_prompt);
    let truncated = truncate_messages(messages, max_input_tokens, system_tokens);
    let msg_tokens: usize = truncated.iter().map(estimate_message_tokens).sum();

    AgentContext {
        system_prompt: system_prompt.to_string(),
        messages: truncated,
        estimated_tokens: system_tokens + msg_tokens,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use clankers_provider::message::*;

    fn make_user_msg(text: &str) -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::generate(),
            content: vec![Content::Text { text: text.to_string() }],
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
        let ctx = build_context(&msgs, "system prompt", 100_000);
        assert_eq!(ctx.system_prompt, "system prompt");
        assert!(!ctx.messages.is_empty());
        assert!(ctx.estimated_tokens > 0);
    }
}
