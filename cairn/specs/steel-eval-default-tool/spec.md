# Steel Eval Default Tool Specification

## Purpose

Defines the `steel-eval-default-tool` capability.

## Requirements

### Requirement: Default Steel eval publication [r[steel-eval-default-tool.default-publication]]

Clankers MUST publish the built-in `steel_eval` tool under ordinary default settings when the safe default profile is used.

#### Scenario: Empty settings default publishes Steel eval
- GIVEN Clankers loads settings with no `steelEval` override
- WHEN Clankers builds the built-in tool list from those settings
- THEN `steel_eval` MUST be present in the available tool names
- AND the selected profile MUST be the reviewed pure default profile

### Requirement: No default host-authority escalation [r[steel-eval-default-tool.no-authority-escalation]]

Default publication MUST NOT grant Steel ambient host authority, host functions, session capabilities, or mutation rights.

#### Scenario: Default profile remains pure
- GIVEN Clankers uses default Steel eval settings
- WHEN an agent invokes `steel_eval`
- THEN the runtime MUST evaluate through the existing Steel runtime wrapper
- AND host calls MUST remain unavailable unless explicitly configured by a reviewed non-default profile
- AND the result MUST remain a deterministic redacted receipt

### Requirement: Explicit Steel eval opt-out [r[steel-eval-default-tool.explicit-opt-out]]

Clankers MUST allow users to omit default Steel eval publication with explicit settings.

#### Scenario: Explicit disabled setting omits tool
- GIVEN settings set `steelEval.enabled` to false
- WHEN Clankers builds the built-in tool list
- THEN `steel_eval` MUST be omitted from available tool names

### Requirement: Disabled-tool parity remains authoritative [r[steel-eval-default-tool.disabled-parity]]

Default publication MUST remain subordinate to the normal disabled-tool policy.

#### Scenario: Disabled tool filter removes default Steel eval
- GIVEN default settings publish `steel_eval`
- AND the user disables `steel_eval` through the normal disabled-tool mechanism
- WHEN Clankers computes allowed tools
- THEN `steel_eval` MUST be removed through the same disabled-tool path as other built-in tools

### Requirement: Focused verification evidence [r[steel-eval-default-tool.verification]]

The implementation MUST include focused tests for default publication, explicit opt-out, disabled-tool filtering, and default safe authority shape.

#### Scenario: Focused tests cover default Steel eval behavior
- GIVEN the implementation changes Steel eval defaults
- WHEN the focused Rust checks run
- THEN tests MUST prove default publication, explicit opt-out, and disabled-tool parity
- AND Cairn validation/gates MUST pass for this change
