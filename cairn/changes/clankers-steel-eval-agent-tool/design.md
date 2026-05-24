## Context

The accepted `steel-lisp-runtime` spec requires wrapper-owned Steel evaluation, explicit capability-gated host effects, deterministic limits/redaction, and an optional agent-visible `steel_eval` tool that shares the runtime. Prior Steel turn-planning work uses Steel behind Rust-owned planning ports, but it does not give agents a direct, reviewed eval tool for small bounded Scheme programs.

## Goals / Non-Goals

**Goals:**

- Add a built-in `steel_eval` tool surface that is disabled by default until reviewed settings/profile material enables it.
- Keep Rust as the authority owner: request validation, profile selection, disabled-tool checks, runtime wrapper invocation, host-function registration, output shaping, and receipt construction stay in Rust.
- Make daemon/attach/standalone tool discovery and disabled-tool behavior match other built-in tools.
- Provide deterministic fixtures for success, denial, limit, redaction, and parity behavior.

**Non-Goals:**

- No ambient filesystem/process/network/provider/credential/daemon/TUI authority.
- No claim of OS/process/VM sandbox isolation.
- No Steel-authored mutation of skills, prompts, code, sessions, or settings.
- No expansion of Steel default orchestration beyond already reviewed planning seams.

## Decisions

### Decision: Tool is a thin host shell over the existing runtime wrapper

**Choice:** `steel_eval` accepts a bounded source string plus optional named runtime profile and returns structured evaluation output with a deterministic receipt.

**Rationale:** The accepted Steel runtime already owns evaluation DTOs, budgets, host-function checks, and receipts. A separate interpreter path would duplicate policy and create an authority bypass.

**Rejected:** Calling Steel interpreter APIs directly from the generic tool dispatcher. That would violate the wrapper-only and dependency-isolation contracts.

### Decision: Default host authority is pure-eval only

**Choice:** The first tool profile exposes pure expression/script evaluation and zero host functions unless a reviewed profile explicitly registers named fake/deterministic host functions for tests.

**Rationale:** A direct agent tool is more exposed than orchestration-only Steel, so the default must be deny-by-default and useful without granting host effects.

**Rejected:** Reusing all dynamic-runtime action envelopes as Steel host functions in the first slice. That is broader than needed and belongs behind separate policy/UCAN review.

### Decision: Tool registration follows existing built-in tool policy

**Choice:** The tool appears in tool discovery only when settings/profile policy enables agent exposure. Disabled-tool state must hide or deny it consistently in standalone, daemon, local attach, and remote attach flows.

**Rationale:** Tool-list parity is user-visible; a daemon-only or standalone-only Steel tool would break attach expectations and could make disabled-tool receipts misleading.

**Rejected:** Always registering `steel_eval` and relying solely on runtime denial. That leaks an unavailable surface and complicates disabled-tool semantics.

### Decision: Receipts are safe summaries, not transcript dumps

**Choice:** Receipts include stable schema/version, tool name, profile id, source hash, bounded output hash/length, issue codes, redaction class, host-call summary, and runtime outcome. They must omit raw prompts, provider payloads, credentials, tokens, paths, scripts when policy says redact, and oversized output.

**Rationale:** The tool is for agent use; receipts need enough evidence for debugging and determinism without becoming another sensitive-data channel.

## Risks / Trade-offs

- **Interpreter availability drift** → `steel_eval` status/registration must fail closed when Steel is unavailable or the configured profile is invalid.
- **Tool-list parity regressions** → add focused tests around built-in registry, daemon `ToolList`, and disabled-tool rebuild behavior.
- **Receipt over-sharing** → negative tests must include secret-like input/output markers and assert safe redaction/hash-only fields.
- **Budget bypass** → request validation and runtime wrapper tests must prove source/output/host-call/execution limits come from the named profile and do not fall back to broader defaults.

## Verification Plan

- Validate the Cairn package and proposal/design/tasks gates.
- Add Rust tests for request validation and pure runtime delegation.
- Add negative Rust tests for disabled profile/tool, unknown host functions, malformed profile, over-limit source/output, and redaction.
- Add parity tests proving standalone and daemon/attach discovery use the same enabled/disabled tool state.
