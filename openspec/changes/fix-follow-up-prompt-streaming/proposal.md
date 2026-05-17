## Why

Clankers can accept the first interactive/TUI prompt but then fail to stream or produce a response for the next prompt in the same session. That breaks the core coding-agent loop: users cannot iterate after an initial answer, and provider routing/auth fixes are hard to validate because the UI appears alive while accepted work is no longer reaching the streaming turn path.

## What Changes

- **Prompt lifecycle repair**: Ensure every accepted user prompt after the first, including controller-dispatched follow-ups and ordinary consecutive TUI prompts, enters the same model/tool streaming turn path as the initial prompt.
- **Completion correlation**: Keep dispatch acknowledgement separate from turn completion so shell/controller state is not cleared or marked complete before streaming/model work finishes.
- **Regression evidence**: Add deterministic tests that fail on the current symptom: prompt one streams/completes, prompt two is accepted and streams/completes in the same session.

## Capabilities

### Modified Capabilities

- `embeddable-agent-engine`: Clarifies that accepted follow-up/subsequent prompts must produce correlated model requests, streaming events, and terminal completion.
- `prompt-assembly`: Clarifies that reused prompt assembly must not suppress or skip subsequent prompts in an already-running session.

## Impact

- **Files likely affected**: `src/modes/event_loop_runner/mod.rs`, `src/modes/agent_task.rs`, `crates/clankers-controller/src/*`, `crates/clankers-core/src/*`, and focused TUI/daemon regression tests.
- **APIs**: No user-facing CLI/API changes expected; this is behavioral correctness.
- **Dependencies**: No new dependencies expected.
- **Testing**: Add focused lifecycle/controller tests plus one runtime TUI/session-path regression that submits two prompts and asserts both produce streamed output and completion.
