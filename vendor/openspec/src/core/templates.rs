//! Built-in schemas and default artifact templates

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

/// Default proposal.md template
pub const PROPOSAL_TEMPLATE: &str = r"# {{change_name}}

## Intent

What problem does this change solve? Why now?

## Scope

### In Scope

-

### Out of Scope

-

## Approach

High-level approach to implementing this change.
";

pub const SPEC_TEMPLATE: &str = r"# {{change_name}} — Spec

## Purpose

Describe the behavioral contract this spec defines.

## Requirements

### Requirement Name

The system MUST ...

GIVEN precondition
WHEN action
THEN expected outcome
";

pub const DESIGN_TEMPLATE: &str = r"# {{change_name}} — Design

## Decisions

### Decision 1

**Choice:** ...
**Rationale:** ...
**Alternatives considered:** ...

## Architecture

Describe the technical architecture.

## Data Flow

Describe data flow for complex interactions.
";

pub const TASKS_TEMPLATE: &str = r"# {{change_name}} — Tasks

> **Legend:** `[ ]` not started · `[~]` in progress ⏱ · `[x]` done ✅ `<duration>`

## Phase 1

- [ ] Task 1
- [ ] Task 2

## Phase 2

- [ ] Task 3
- [ ] Task 4
";

/// Expand template variables
pub fn expand_template(template: &str, change_name: &str, context: &str, rules: &[String]) -> String {
    let mut result = template.replace("{{change_name}}", change_name);
    result = result.replace("{{context}}", context);
    if !rules.is_empty() {
        let rules_text = rules.iter().map(|r| format!("- {}", r)).collect::<Vec<_>>().join("\n");
        result = result.replace("{{rules}}", &rules_text);
    } else {
        result = result.replace("{{rules}}", "");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_template_basic() {
        let template = "# {{change_name}}\nContext: {{context}}";
        let result = expand_template(template, "my-change", "test context", &[]);
        assert!(result.contains("my-change"));
        assert!(result.contains("test context"));
    }

    #[test]
    fn test_expand_template_with_rules() {
        let template = "Rules:\n{{rules}}";
        let rules = vec!["rule 1".to_string(), "rule 2".to_string()];
        let result = expand_template(template, "change", "context", &rules);
        assert!(result.contains("- rule 1"));
        assert!(result.contains("- rule 2"));
    }

    #[test]
    fn test_expand_template_no_rules() {
        let template = "Rules:\n{{rules}}";
        let result = expand_template(template, "change", "context", &[]);
        assert!(result.contains("Rules:\n"));
        assert!(!result.contains("{{rules}}"));
    }
}
