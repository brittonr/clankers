//! Capability enforcement for session tool access control.
//!
//! Each SessionController holds an `Option<Vec<String>>` of allowed tool
//! patterns. `None` means full access. `Some(patterns)` means only tools
//! matching a pattern are allowed.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use clankers_protocol::DaemonEvent;

/// Check whether a tool name is allowed by the capability set.
///
/// Returns `true` if the tool should be allowed.
pub fn is_tool_allowed(tool_name: &str, capabilities: &Option<Vec<String>>) -> bool {
    let Some(patterns) = capabilities else {
        return true; // None = full access
    };

    for pattern in patterns {
        if pattern == "*" {
            return true;
        }
        // Comma-separated list of tool names
        for name in pattern.split(',') {
            let name = name.trim();
            if name == tool_name || name == "*" {
                return true;
            }
        }
    }

    false
}

/// Clamp child capabilities to be a subset of parent capabilities.
///
/// - If parent is `None` (full access), child keeps its requested caps.
/// - If parent has restrictions, child capabilities are intersected with parent's.
/// - If child is `None` (full access request), it gets parent's restrictions.
pub fn clamp_capabilities(parent: &Option<Vec<String>>, child_requested: &Option<Vec<String>>) -> Option<Vec<String>> {
    match (parent, child_requested) {
        // Parent has full access — child gets what it asked for
        (None, child) => child.clone(),

        // Parent restricted, child wants full — child gets parent's restrictions
        (Some(parent_caps), None) => Some(parent_caps.clone()),

        // Both restricted — intersect
        (Some(parent_caps), Some(child_caps)) => {
            let parent_tools: std::collections::HashSet<&str> =
                parent_caps.iter().flat_map(|p| p.split(',').map(str::trim)).collect();

            let filtered: Vec<String> = child_caps
                .iter()
                .filter_map(|cap| {
                    let child_tools: Vec<&str> = cap.split(',').map(str::trim).collect();
                    let allowed: Vec<&str> = child_tools
                        .into_iter()
                        .filter(|t| *t == "*" && parent_tools.contains("*") || parent_tools.contains(t))
                        .collect();
                    if allowed.is_empty() {
                        None
                    } else {
                        Some(allowed.join(","))
                    }
                })
                .collect();

            Some(filtered)
        }
    }
}

/// Build a `ToolBlocked` event for a denied tool call.
pub fn tool_blocked_event(call_id: &str, tool_name: &str) -> DaemonEvent {
    DaemonEvent::ToolBlocked {
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
        reason: format!("Tool '{tool_name}' not allowed by session capabilities"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_capabilities_allows_all() {
        assert!(is_tool_allowed("bash", &None));
        assert!(is_tool_allowed("read", &None));
        assert!(is_tool_allowed("anything", &None));
    }

    #[test]
    fn test_wildcard_allows_all() {
        let caps = Some(vec!["*".to_string()]);
        assert!(is_tool_allowed("bash", &caps));
        assert!(is_tool_allowed("read", &caps));
    }

    #[test]
    fn test_specific_tools() {
        let caps = Some(vec!["read,grep,find,ls".to_string()]);
        assert!(is_tool_allowed("read", &caps));
        assert!(is_tool_allowed("grep", &caps));
        assert!(is_tool_allowed("find", &caps));
        assert!(is_tool_allowed("ls", &caps));
        assert!(!is_tool_allowed("bash", &caps));
        assert!(!is_tool_allowed("write", &caps));
    }

    #[test]
    fn test_multiple_capability_entries() {
        let caps = Some(vec!["read,grep".to_string(), "bash".to_string()]);
        assert!(is_tool_allowed("read", &caps));
        assert!(is_tool_allowed("grep", &caps));
        assert!(is_tool_allowed("bash", &caps));
        assert!(!is_tool_allowed("write", &caps));
    }

    #[test]
    fn test_clamp_parent_full_access() {
        let parent = None;
        let child = Some(vec!["read,grep".to_string()]);
        let result = clamp_capabilities(&parent, &child);
        assert_eq!(result, Some(vec!["read,grep".to_string()]));
    }

    #[test]
    fn test_clamp_child_full_access() {
        let parent = Some(vec!["read,grep,bash".to_string()]);
        let child = None;
        let result = clamp_capabilities(&parent, &child);
        assert_eq!(result, Some(vec!["read,grep,bash".to_string()]));
    }

    #[test]
    fn test_clamp_both_restricted() {
        let parent = Some(vec!["read,grep,bash".to_string()]);
        let child = Some(vec!["read,grep".to_string()]);
        let result = clamp_capabilities(&parent, &child);
        assert_eq!(result, Some(vec!["read,grep".to_string()]));
    }

    #[test]
    fn test_clamp_child_exceeds_parent() {
        let parent = Some(vec!["read,grep".to_string()]);
        let child = Some(vec!["read,grep,bash,write".to_string()]);
        let result = clamp_capabilities(&parent, &child);
        // bash and write should be stripped
        let result = result.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("read"));
        assert!(result[0].contains("grep"));
        assert!(!result[0].contains("bash"));
    }

    #[test]
    fn test_clamp_both_full() {
        let result = clamp_capabilities(&None, &None);
        assert!(result.is_none());
    }

    #[test]
    fn test_tool_blocked_event() {
        let event = tool_blocked_event("call-1", "bash");
        assert!(matches!(event, DaemonEvent::ToolBlocked { tool_name, .. } if tool_name == "bash"));
    }
}
