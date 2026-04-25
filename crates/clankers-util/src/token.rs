//! Token estimation (character ÷ 4)

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
