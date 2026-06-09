//! Token estimation helpers for provider-neutral message budgeting.

/// Estimate the number of tokens in a text string.
///
/// Uses a simple heuristic: UTF-8 byte count divided by 4. This is a rough
/// approximation commonly used for English text.
const TOKEN_ESTIMATE_BYTES_PER_TOKEN: u64 = 4;
const _: () = assert!(usize::BITS <= u64::BITS);

pub fn estimate_tokens(text: &str) -> u64 {
    let byte_count = text.len() as u64;
    byte_count / TOKEN_ESTIMATE_BYTES_PER_TOKEN
}

/// Estimate the number of tokens in a list of JSON-serializable message values.
///
/// Serializes the messages to JSON and applies the token estimation heuristic.
pub fn estimate_tokens_for_messages(messages: &[serde_json::Value]) -> u64 {
    let serialized = serde_json::to_string(messages).unwrap_or_default();
    estimate_tokens(&serialized)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn estimate_tokens_uses_character_divisor_heuristic() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("test"), 1);
        assert_eq!(estimate_tokens("hello world"), 2);
    }

    #[test]
    fn estimate_tokens_for_messages_serializes_json_messages() {
        let messages = vec![
            json!({"role": "user", "content": "Hello"}),
            json!({"role": "assistant", "content": "Hi"}),
        ];
        let tokens = estimate_tokens_for_messages(&messages);
        assert!(tokens > 0);
    }
}
