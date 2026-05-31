# Proposal: Turn Lifecycle Hook Contracts

## Why

Clankers already exposes hook concepts for prompts and turns, but the semantics are incomplete:

- `HookPoint::PrePrompt` / `PostPrompt` and `HookPayload::prompt(...)` exist, but the prompt path does not currently fire them.
- `HookPoint::TurnStart` / `TurnEnd` exist and are fired asynchronously from `AgentEvent::TurnStart` / `TurnEnd` through controller event processing, but they are lifecycle notifications, not blocking pre/post turn gates.
- `PreTool` / `PostTool` are the only fully wired pre/post hook pair today; `PreTool` can deny/modify, and `PostTool` fires asynchronously after execution.

This leaves no clear contract for users who want to run logic before an agent turn starts or after it completes, and it makes plugins/scripts guess whether `turn-start` means “about to run and can stop it” or “a model transcript turn already began.”

## What Changes

Define and implement first-class prompt and agent-turn hook contracts:

- Wire `PrePrompt` and `PostPrompt` around raw user prompt handling.
- Add or clarify a blocking pre-agent-turn hook that runs after context/model setup but before the first model request/tool loop for a prompt.
- Add or clarify a post-agent-turn hook that observes the final prompt result, usage, error status, and redacted summary after the turn completes.
- Preserve existing `TurnStart` / `TurnEnd` lifecycle notifications for compatibility, and document their non-blocking model-turn/event semantics.
- Add script/plugin/runtime tests proving ordering, denial/modify behavior, payload redaction, daemon/standalone parity, and no double firing.

## Impact

- **Files**: `crates/clankers-hooks/src/{point,payload,dispatcher,script}.rs`, `crates/clankers-agent/src/lib.rs`, `crates/clankers-agent/src/turn/**`, `crates/clankers-controller/src/event_processing.rs`, `crates/clankers-plugin/src/hooks.rs`, hook docs/help under `src/slash_commands/handlers/hooks.rs` or generated docs.
- **Testing**: hook pipeline unit tests, agent prompt-path tests, daemon/controller parity tests, plugin hook mapping tests, script hook fixtures, and focused docs/source contract tests.
