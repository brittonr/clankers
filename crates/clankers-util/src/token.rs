//! Token estimation (character ÷ 4)

/// Estimate the number of tokens in a text string.
///
/// Uses a simple heuristic: character count divided by 4.
/// This is a rough approximation commonly used for English text.
///
/// # Examples
///
/// ```
/// use clankers_util::token::estimate_tokens;
///
/// let text = "Hello, world!";
/// let tokens = estimate_tokens(text);
/// assert_eq!(tokens, 3); // 13 chars / 4 = 3
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Estimate the number of tokens in a list of messages.
///
/// Serializes the messages to JSON and applies the token estimation heuristic.
/// This is useful for estimating the token count of conversation history.
///
/// # Examples
///
/// ```
/// use clankers_util::token::estimate_tokens_for_messages;
/// use serde_json::json;
///
/// let messages = vec![
///     json!({"role": "user", "content": "Hello"}),
///     json!({"role": "assistant", "content": "Hi there!"}),
/// ];
/// let tokens = estimate_tokens_for_messages(&messages);
/// assert!(tokens > 0);
/// ```
pub fn estimate_tokens_for_messages(messages: &[serde_json::Value]) -> usize {
    let serialized = serde_json::to_string(messages).unwrap_or_default();
    estimate_tokens(&serialized)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("test"), 1); // 4 chars / 4 = 1
        assert_eq!(estimate_tokens("hello world"), 2); // 11 chars / 4 = 2
    }

    #[test]
    fn test_estimate_tokens_for_messages() {
        let messages = vec![
            json!({"role": "user", "content": "Hello"}),
            json!({"role": "assistant", "content": "Hi"}),
        ];
        let tokens = estimate_tokens_for_messages(&messages);
        assert!(tokens > 0);
    }
}
