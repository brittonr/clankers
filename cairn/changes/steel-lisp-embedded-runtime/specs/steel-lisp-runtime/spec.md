# Steel Lisp Runtime Specification

## Purpose

Defines the `steel-lisp-runtime` capability for embedding Steel Lisp/Scheme in Clankers as a constrained local scripting runtime.

## Requirements

### Requirement: Runtime wrapper owns Steel evaluation [r[steel-lisp-runtime.wrapper-owned-evaluation]]

Clankers MUST embed Steel through a focused Rust runtime wrapper that owns request DTOs, response DTOs, runtime profile selection, and receipt construction. Product shells MUST NOT call Steel interpreter APIs directly.

#### Scenario: CLI evaluates through the wrapper [r[steel-lisp-runtime.wrapper-owned-evaluation.cli-wrapper]]
- GIVEN a user evaluates a Steel expression through the Clankers CLI
- **WHEN** the expression is executed
- **THEN** the CLI MUST call the Clankers Steel runtime wrapper
- **AND** the result MUST be returned as structured output plus a receipt

#### Scenario: shell code does not import interpreter internals [r[steel-lisp-runtime.wrapper-owned-evaluation.no-shell-interpreter-leak]]
- GIVEN root CLI, daemon, TUI, attach, or provider modules need Steel support
- **WHEN** those modules invoke Steel behavior
- **THEN** they MUST use Clankers-owned runtime DTOs or adapter functions
- **AND** they MUST NOT construct the Steel interpreter directly

### Requirement: Host effects are explicit and capability gated [r[steel-lisp-runtime.capability-gated-host-effects]]

Steel scripts MUST have no ambient access to filesystem, process, network, credentials, providers, daemon sessions, TUI state, or native tool execution. Any host-visible effect MUST be registered as an explicit host function and checked against the session capability set, disabled-tool policy, and the runtime evaluation profile before execution.

#### Scenario: approved host function executes [r[steel-lisp-runtime.capability-gated-host-effects.approved-host-function]]
- GIVEN a Steel script calls a host function registered for the current evaluation
- **WHEN** the session capabilities and disabled-tool policy allow that function
- **THEN** the host function MAY execute through the typed Clankers tool or effect seam
- **AND** the receipt MUST record the approved host function name and safe outcome class

#### Scenario: denied host function fails closed [r[steel-lisp-runtime.capability-gated-host-effects.denied-host-function]]
- GIVEN a Steel script calls an unknown, disabled, or unauthorized host function
- **WHEN** the runtime evaluates the call
- **THEN** evaluation MUST fail with a typed denial outcome
- **AND** no fallback filesystem, process, network, credential, provider, daemon, TUI, or native tool authority may be used

### Requirement: Operator and agent surfaces are explicit [r[steel-lisp-runtime.explicit-surfaces]]

Clankers MUST expose Steel support through explicit reviewed surfaces. The first implementation MUST provide status and deterministic CLI eval/run behavior; an agent-visible Steel tool MAY be added only when it reuses the same runtime wrapper, capability checks, resource limits, and receipt redaction policy.

#### Scenario: status reports runtime availability [r[steel-lisp-runtime.explicit-surfaces.status]]
- GIVEN an operator runs `clankers steel status`
- **WHEN** the Steel dependency and configured runtime profile are available
- **THEN** Clankers MUST report the runtime availability, version/profile metadata, and whether agent-tool exposure is enabled
- **AND** the status output MUST NOT require executing user-provided Steel code

#### Scenario: optional agent tool shares the same runtime [r[steel-lisp-runtime.explicit-surfaces.agent-tool-shares-runtime]]
- GIVEN the `steel_eval` agent tool is enabled
- **WHEN** the LLM invokes it in standalone, daemon, or attach contexts
- **THEN** the tool MUST use the same runtime wrapper and policy checks as CLI evaluation
- **AND** daemon `ToolList` and disabled-tool rebuild behavior MUST remain consistent with other built-in or plugin tools

### Requirement: Resource limits and redaction are deterministic [r[steel-lisp-runtime.deterministic-limits-and-redaction]]

Steel evaluation MUST enforce a named runtime profile with bounded source size, output size, host-call count, and execution budget. Receipts MUST use stable issue codes and redact source snippets, paths, credentials, provider payloads, raw host diagnostics, and oversized output according to Clankers policy.

#### Scenario: output limit truncates safely [r[steel-lisp-runtime.deterministic-limits-and-redaction.output-limit]]
- GIVEN a Steel script produces output larger than the configured profile allows
- **WHEN** evaluation completes or is stopped by the output limit
- **THEN** the user-facing output MUST be bounded
- **AND** the receipt MUST report a stable truncation or limit issue code without embedding the full oversized output

#### Scenario: execution budget failure is typed [r[steel-lisp-runtime.deterministic-limits-and-redaction.execution-budget]]
- GIVEN a Steel script exceeds its step, fuel, host-call, or wall-clock execution budget
- **WHEN** the runtime stops evaluation
- **THEN** Clankers MUST return a typed resource-limit failure
- **AND** it MUST NOT retry with a less restrictive profile automatically

### Requirement: Verification proves allowed and denied behavior [r[steel-lisp-runtime.verification-contracts]]

The implementation MUST include deterministic tests or checked fixtures for successful Lisp evaluation, approved host-function execution, denied host functions, resource-limit behavior, receipt redaction, CLI status/eval/run output, and daemon/tool parity when an agent-visible tool is enabled.

#### Scenario: positive runtime fixture is deterministic [r[steel-lisp-runtime.verification-contracts.positive-fixture]]
- GIVEN the Steel runtime test fixture evaluates a pure expression and an approved fake host function
- **WHEN** the fixture is run twice
- **THEN** stable receipt fields MUST match
- **AND** no live credentials, provider calls, sockets, daemon state, TUI state, or filesystem authority are required

#### Scenario: negative fixture blocks ambient authority [r[steel-lisp-runtime.verification-contracts.negative-authority-fixture]]
- GIVEN the Steel runtime test fixture attempts an unknown host function or ambient effect
- **WHEN** evaluation runs under the default profile
- **THEN** the fixture MUST fail closed with a stable denial issue code
- **AND** verification MUST prove the forbidden effect did not execute
