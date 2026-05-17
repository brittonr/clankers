## Context

The user-visible failure is: the Clankers TUI accepts/handles the first prompt, but a later prompt in the same session does not stream or respond. Existing architecture splits ownership across prompt assembly, core/controller lifecycle, shell dispatch, and engine-owned model/tool turn execution. The fix should preserve that ownership rather than papering over the symptom in the TUI renderer.

Relevant existing seams:

- `src/modes/event_loop_runner/mod.rs` dispatches controller follow-ups and completes prompt/follow-up lifecycle when `TaskResult::PromptDone` arrives.
- `src/modes/agent_task.rs` forwards `AgentCommand::Prompt` and `AgentCommand::SetModel` into the agent runtime.
- `crates/clankers-controller` owns pending prompt/follow-up lifecycle and completion acknowledgement helpers.
- `crates/clankers-engine` owns model/tool turn state after a prompt is accepted.

## Goals / Non-Goals

**Goals:**

- Reproduce the two-prompt failure with a deterministic regression before or alongside the fix.
- Ensure ordinary second prompts and controller-dispatched follow-ups both stream and complete.
- Keep dispatch acceptance, streaming turn progress, and terminal completion as distinct correlated events.
- Add negative coverage for rejected follow-up dispatch so the UI/session does not remain stuck busy.

**Non-Goals:**

- Do not change provider auth, model selection, or OpenAI/OpenAI-Codex transport contracts.
- Do not move prompt assembly into the engine.
- Do not add new user-facing commands or configuration.

## Decisions

### Decision 1: Test the real repeated-prompt path, not only pure reducers

**Choice:** Add at least one runtime/session-path regression that submits two prompts through the same shell path used by TUI or daemon attach, using a fake provider/router adapter so no live credentials are required.

**Rationale:** Pure lifecycle tests can prove state transitions but miss the symptom where the UI/session accepts input yet no streaming output is delivered after the first turn.

**Implementation:** Prefer an existing harness seam such as event-loop runner/controller tests, fake `RouterCompatAdapter`, or a daemon/session socket test. The assertion must observe both prompts producing streamed assistant content and prompt completion in order.

### Decision 2: Keep dispatch acknowledgement separate from completion

**Choice:** Treat `ack_follow_up_dispatch(...)` as only accepted/rejected enqueue evidence. Only `TaskResult::PromptDone`, engine terminal outcome, cancellation, or explicit failure should complete the pending prompt/follow-up.

**Rationale:** The likely failure class is stale lifecycle state: a prompt is accepted or acknowledged, but the controller/shell believes work is already done or still busy from prior work, so later streaming is dropped or never requested.

**Implementation:** Audit `start_embedded_prompt_with_follow_up`, `pending_dispatched_follow_up_id`, `complete_dispatched_follow_up`, `finish_embedded_prompt`, and ordinary prompt start/completion callers for correlation loss or premature clearing.

### Decision 3: Repair at the lifecycle/correlation seam before renderer changes

**Choice:** Fix prompt/follow-up lifecycle state and model request correlation before touching TUI rendering.

**Rationale:** If model deltas are not emitted or are attributed to a stale prompt, renderer-only changes can hide the bug without making daemon/attach/Matrix paths correct.

**Implementation:** Verify the second prompt has fresh model request correlation and that streaming deltas are routed to the active turn/session. Only update TUI rendering if evidence shows events exist but are not displayed.

## Risks / Trade-offs

**Flaky live-provider reproduction** → Use fake provider/router streams for deterministic regression; keep live OpenAI-Codex smoke optional.

**Overfitting to one prompt source** → Cover both ordinary user prompt after first completion and controller-dispatched follow-up completion.

**State cleanup regressions** → Include negative rejected-dispatch coverage and cancellation/failure completion assertions so the session can recover.

## Validation Plan

- `cargo fmt --check`
- Focused lifecycle/controller regression for repeated prompts.
- Focused shell/runtime regression that submits two prompts in one session and asserts both stream and complete.
- Existing engine/controller prompt lifecycle tests touched by the repair.
- `CARGO_TARGET_DIR=target cargo check --tests -p clankers-core -p clankers-controller -p clankers-engine -p clankers`
- `openspec validate fix-follow-up-prompt-streaming --strict --json`
- `git diff --check`
