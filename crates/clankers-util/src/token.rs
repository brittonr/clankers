//! Token estimation compatibility reexports.
//!
//! Provider-neutral token budgeting lives in `clanker-message`; this module
//! keeps the historical `clankers_util::token` path available for callers that
//! have not moved to the neutral contract crate yet.

pub use clanker_message::estimate_tokens;
pub use clanker_message::estimate_tokens_for_messages;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("test"), 1);
        assert_eq!(estimate_tokens("hello world"), 2);
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
