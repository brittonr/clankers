# Design: Turn Lifecycle Hook Contracts

## Context

The hook crate already models lifecycle points, but today only tool and git hooks have a complete blocking/pre + async/post implementation. Prompt and turn hooks are either dormant (`PrePrompt` / `PostPrompt`) or post-facto notifications (`TurnStart` / `TurnEnd` fired from `AgentEvent`s by the controller). The design must avoid breaking existing plugin/script subscribers while giving users a true pre-turn gate.

## Decisions

### 1. Separate prompt hooks, agent-turn gates, and model-turn notifications

**Choice:** Treat these as three different concepts:

1. **Prompt hooks** operate on raw user input before/after prompt handling.
2. **Agent-turn hooks** operate around the whole prompt execution (context assembly, model/tool loop, final outcome).
3. **TurnStart/TurnEnd notifications** remain non-blocking lifecycle events for transcript/model-turn activity.

**Rationale:** Existing `TurnStart`/`TurnEnd` names are already used by plugins and metrics. Reinterpreting them as blocking hooks would be a compatibility break and would confuse model-request rounds with a whole prompt invocation.

### 2. Pre hooks are synchronous and fail closed

**Choice:** `PrePrompt` and the new pre-agent-turn hook run synchronously through `HookPipeline::fire(...)`. `Deny` stops the prompt/turn with an actionable error, and `Modify` is accepted only where the payload contract explicitly supports mutation.

**Rationale:** Users expect “pre” hooks to be enforcement points. Fire-and-forget pre hooks are unsafe because the model/tool loop could already have started before a denial arrives.

### 3. Post hooks observe but do not change outcomes

**Choice:** Post prompt/turn hooks fire asynchronously by default after the outcome is known. Their failures are logged or surfaced through diagnostics, but they do not rewrite already emitted model/tool results.

**Rationale:** Post hooks are for audit, metrics, notifications, and plugin updates. Making them blocking would couple prompt completion latency to unrelated hook backends.

### 4. Payloads are explicit and redacted by default

**Choice:** Add a typed turn payload (or extend `HookData`) with fields such as `turn_id`, `session_id`, `model`, `prompt_digest`, `prompt_preview`, `message_count`, `tool_call_count`, `status`, `error`, `usage`, and redacted summary text. Raw prompt/system prompt may remain prompt-hook data where already expected, but turn hooks should prefer digests/previews unless a future policy explicitly permits full content.

**Rationale:** Turn hooks are likely to be used for external notifications and audit sinks. Default payloads should be safe for logs and plugins while still allowing correlation.

### 5. The controller should not double-fire agent-owned gates

**Choice:** Agent-owned prompt/turn gates should fire at the prompt execution boundary. The controller may keep firing existing lifecycle notifications from `AgentEvent::TurnStart/TurnEnd`, but it must not also synthesize blocking pre/post agent-turn hooks from those events.

**Rationale:** The agent has the ordering context needed to stop work before the first model request. Controller event processing sees events after they are emitted and cannot safely implement a pre-turn gate.

## Proposed Ordering

For one user prompt invocation:

1. `PrePrompt` fires before user message append. It may deny or modify the prompt text/content.
2. The prompt is authorized, appended, and `AgentStart` is emitted.
3. Context, compaction, skill nudges, model selection, and turn config are prepared.
4. `PreTurn` (or an equivalent newly named pre-agent-turn hook point) fires synchronously before the first model request/engine submission. It may deny; any future modification must be explicit and tested.
5. Existing `TurnStart` / `TurnEnd` lifecycle notifications may fire during model/tool loop execution for transcript/model-turn events.
6. `PostTurn` (or equivalent) observes the final turn outcome after `run_turn_loop`/orchestration completes.
7. `PostPrompt` observes the final prompt outcome before the prompt call returns or before the controller emits final `PromptDone`, depending on the owning seam chosen during implementation.

## Risks / Trade-offs

- **Naming compatibility:** Adding `PreTurn`/`PostTurn` while retaining `TurnStart`/`TurnEnd` creates more hook points, but avoids breaking existing subscribers.
- **Latency:** Blocking pre hooks can slow prompt start. The implementation should make timeouts/configuration explicit.
- **Payload secrecy:** Prompt hooks currently allow raw prompt text. Turn hooks should default to safe previews/digests to avoid widening secret exposure.
- **Daemon vs standalone parity:** Standalone agents and daemon-owned `SessionController` paths wire hooks differently today. Tests must prove each mode fires the same hook sequence once.
