//! Plan mode — architecture-first workflow before edits
//!
//! When plan mode is active, the agent:
//! 1. Reads and analyzes without making changes
//! 2. Proposes an architecture/implementation plan
//! 3. Asks for approval before proceeding with edits
//!
//! Toggled via `/plan` slash command or `--plan` CLI flag.

// PlanState re-exported from clanker-tui-types (canonical definition).
pub use clanker_tui_types::PlanState;

/// System prompt suffix added when plan mode is active
pub const PLAN_MODE_PROMPT: &str = r"
## Plan Mode Active

You are currently in PLAN MODE. In this mode:

1. **DO NOT** make any file edits (no write, edit, or bash commands that modify files)
2. **DO** read files, grep, find, and analyze the codebase thoroughly
3. **DO** produce a detailed implementation plan with:
   - Architecture overview
   - List of files to create/modify
   - Step-by-step implementation order
   - Potential risks or concerns
   - Estimated scope (small/medium/large)

Format your plan as:

```
## Implementation Plan

### Overview
[Brief description of the approach]

### Files to Modify
- `path/to/file.rs` — [what changes]
- `path/to/new_file.rs` — [new file, purpose]

### Steps
1. [First step]
2. [Second step]
...

### Risks
- [Risk 1]
- [Risk 2]

### Scope: [small|medium|large]
```

After presenting the plan, ask the user if they want to proceed with implementation.
";

/// System prompt for execution phase (after plan approval)
pub const PLAN_EXECUTE_PROMPT: &str = r"
## Executing Approved Plan

The user has approved your implementation plan. Now execute it step by step:
1. Follow the plan you proposed
2. Create/modify files as outlined
3. Report progress after each step
4. If you encounter issues, explain them before deviating from the plan
";

/// Check if a tool is allowed in plan mode
pub fn is_tool_allowed_in_plan_mode(tool_name: &str) -> bool {
    // In plan mode, only read-only tools are allowed
    matches!(tool_name, "read" | "grep" | "find" | "ls" | "web" | "review" | "ask" | "todo") || tool_name == "bash" // bash is allowed but edits should be self-restricted by the prompt
}

/// Generate a plan summary from the agent's output
pub fn extract_plan_summary(text: &str) -> Option<String> {
    // Look for the "## Implementation Plan" section
    if let Some(start) = text.find("## Implementation Plan") {
        let plan_text = &text[start..];
        // Find the end of the plan section (next ## or end of text)
        let end = plan_text[3..].find("\n## ").map(|i| i + 3).unwrap_or(plan_text.len());
        Some(plan_text[..end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_state_transitions() {
        let state = PlanState::Inactive;
        assert!(!state.is_active());

        let state = PlanState::Planning;
        assert!(state.is_active());
        assert_eq!(state.label(), "planning");
    }

    #[test]
    fn test_tool_restrictions() {
        assert!(is_tool_allowed_in_plan_mode("read"));
        assert!(is_tool_allowed_in_plan_mode("grep"));
        assert!(is_tool_allowed_in_plan_mode("find"));
        assert!(is_tool_allowed_in_plan_mode("bash"));
        // write/edit are not in the allowed list but bash is borderline
    }

    #[test]
    fn test_extract_plan_summary() {
        let text =
            "Some intro text.\n\n## Implementation Plan\n\n### Overview\nDo the thing.\n\n### Steps\n1. Step one\n";
        let summary = extract_plan_summary(text);
        assert!(summary.is_some());
        assert!(summary.unwrap().contains("Overview"));
    }

    #[test]
    fn test_extract_plan_summary_none() {
        let text = "Just a regular response with no plan.";
        assert!(extract_plan_summary(text).is_none());
    }
}
