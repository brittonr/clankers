//! Built-in schemas and default artifact templates

/// Default proposal.md template
pub const PROPOSAL_TEMPLATE: &str = r#"# {{change_name}}

## Intent

What problem does this change solve? Why now?

## Scope

### In Scope

- 

### Out of Scope

- 

## Approach

High-level approach to implementing this change.
"#;

pub const SPEC_TEMPLATE: &str = r#"# {{change_name}} — Spec

## Purpose

Describe the behavioral contract this spec defines.

## Requirements

### Requirement Name

The system MUST ...

GIVEN precondition
WHEN action
THEN expected outcome
"#;

pub const DESIGN_TEMPLATE: &str = r#"# {{change_name}} — Design

## Decisions

### Decision 1

**Choice:** ...
**Rationale:** ...
**Alternatives considered:** ...

## Architecture

Describe the technical architecture.

## Data Flow

Describe data flow for complex interactions.
"#;

pub const TASKS_TEMPLATE: &str = r#"# {{change_name}} — Tasks

## Phase 1

- [ ] Task 1
- [ ] Task 2

## Phase 2

- [ ] Task 3
- [ ] Task 4
"#;

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
