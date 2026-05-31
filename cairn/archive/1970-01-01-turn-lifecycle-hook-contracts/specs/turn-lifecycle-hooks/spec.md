## ADDED Requirements

### Requirement: Prompt hook semantics are explicit and wired [r[turn-lifecycle-hooks.prompt-hooks]]

Clankers MUST fire prompt hooks around user prompt handling with deterministic ordering and explicit mutation/denial semantics.

#### Scenario: pre-prompt can deny or modify raw input [r[turn-lifecycle-hooks.prompt-hooks.pre-prompt-gate]]
- GIVEN a user prompt enters standalone, daemon, or embedded prompt execution
- WHEN a handler subscribes to `PrePrompt`
- THEN the hook MUST fire before the user message is appended to conversation history
- THEN a `Deny` verdict MUST stop prompt execution with a safe user-visible error and no model request
- THEN a `Modify` verdict MUST update the prompt payload only through documented prompt fields before history append

#### Scenario: post-prompt observes final prompt outcome [r[turn-lifecycle-hooks.prompt-hooks.post-prompt-observe]]
- GIVEN a user prompt completes, is denied, is cancelled, or fails
- WHEN a handler subscribes to `PostPrompt`
- THEN the hook MUST receive the final prompt status, safe error if any, and prompt correlation metadata
- THEN the hook MUST NOT rewrite the already recorded prompt outcome

### Requirement: Agent-turn hooks are distinct from model-turn notifications [r[turn-lifecycle-hooks.agent-turn-hooks]]

Clankers MUST provide first-class pre/post agent-turn hooks for a whole prompt execution, while preserving existing `TurnStart` / `TurnEnd` lifecycle notifications as non-blocking transcript/model-turn events.

#### Scenario: pre-turn runs before first model request [r[turn-lifecycle-hooks.agent-turn-hooks.pre-turn-before-model]]
- GIVEN context, compaction, model selection, and turn config are ready for a prompt
- WHEN a handler subscribes to the pre-agent-turn hook
- THEN the hook MUST fire synchronously before the first model request or tool loop submission
- THEN a `Deny` verdict MUST prevent model and tool execution and produce a safe prompt error
- THEN existing `TurnStart` notifications MUST NOT be used as the blocking pre-turn gate

#### Scenario: post-turn observes complete agent turn [r[turn-lifecycle-hooks.agent-turn-hooks.post-turn-after-outcome]]
- GIVEN an agent turn finishes successfully, fails, or is cancelled
- WHEN a handler subscribes to the post-agent-turn hook
- THEN the hook MUST receive the final turn status, model, usage if available, safe error if any, tool-call count, and turn correlation metadata
- THEN the hook MUST fire exactly once for the prompt-level turn and MUST NOT fire once per model retry or transcript subturn

#### Scenario: lifecycle notifications remain compatible [r[turn-lifecycle-hooks.agent-turn-hooks.lifecycle-compat]]
- GIVEN existing plugins or scripts subscribe to `turn-start` or `turn-end`
- WHEN the model/tool loop emits `AgentEvent::TurnStart` or `AgentEvent::TurnEnd`
- THEN those subscribers MUST continue receiving non-blocking lifecycle notifications
- THEN documentation MUST distinguish these notifications from blocking pre/post agent-turn hooks

### Requirement: Hook payloads are typed, correlated, and safe [r[turn-lifecycle-hooks.payload-contract]]

Prompt and turn hooks MUST use typed payloads with stable correlation identifiers and safe-by-default content exposure.

#### Scenario: turn payload is safe by default [r[turn-lifecycle-hooks.payload-contract.safe-turn-payload]]
- GIVEN a pre/post agent-turn hook fires
- WHEN the hook payload is serialized for scripts or plugins
- THEN it MUST include `session_id`, turn/prompt correlation ID, model, prompt digest or bounded preview, message count, and status fields where applicable
- THEN it MUST NOT include full system prompt, full conversation history, credentials, or unrestricted tool output unless an explicit opt-in policy exists

#### Scenario: payload correlation spans prompt and turn hooks [r[turn-lifecycle-hooks.payload-contract.correlation]]
- GIVEN `PrePrompt`, `PreTurn`, `PostTurn`, and `PostPrompt` fire for the same user prompt
- WHEN their payloads are inspected
- THEN they MUST share a stable correlation identifier so audit plugins can join the lifecycle without relying on timestamps or raw text

### Requirement: Hook dispatch ownership is single and mode-parity safe [r[turn-lifecycle-hooks.dispatch-ownership]]

Prompt and agent-turn hook dispatch MUST have one owner per hook phase and MUST fire consistently across standalone, daemon/controller-owned, and embedded runtime paths.

#### Scenario: pre hooks are not synthesized from post-facto events [r[turn-lifecycle-hooks.dispatch-ownership.pre-owner]]
- GIVEN a blocking pre hook is required
- WHEN implementation chooses a dispatch seam
- THEN it MUST run at the agent/runtime prompt boundary before work starts
- THEN controller event processing MUST NOT synthesize that pre hook from `AgentEvent::TurnStart` or other already-emitted events

#### Scenario: mode parity fires hooks once [r[turn-lifecycle-hooks.dispatch-ownership.mode-parity]]
- GIVEN the same prompt executes in standalone and daemon/controller-owned modes
- WHEN hook handlers record fired points
- THEN each enabled prompt/agent-turn hook MUST fire once in the same relative order
- THEN existing tool hooks MUST keep their current behavior and ordering within the model/tool loop

### Requirement: Hook configuration and documentation name lifecycle semantics [r[turn-lifecycle-hooks.docs-config]]

User-facing hook help, generated docs, and script/plugin mapping MUST document the difference between prompt hooks, agent-turn hooks, model-turn lifecycle notifications, and tool hooks.

#### Scenario: hook listing distinguishes blocking and notification hooks [r[turn-lifecycle-hooks.docs-config.blocking-vs-notification]]
- GIVEN a user views hook help or generated hook docs
- WHEN prompt/turn hook points are listed
- THEN blocking pre hooks MUST be labeled as deny/modify-capable or deny-only
- THEN post hooks and `turn-start`/`turn-end` notifications MUST be labeled as observational/non-blocking

#### Scenario: plugin mapping covers new hook points [r[turn-lifecycle-hooks.docs-config.plugin-mapping]]
- GIVEN plugin hooks subscribe to prompt or turn lifecycle events
- WHEN `clankers-plugin` maps `HookPoint` values to plugin event names
- THEN every new hook point MUST have an explicit mapping or an explicit unsupported reason

### Requirement: Turn hook validation proves no model/tool side effects on denial [r[turn-lifecycle-hooks.validation]]

Implementation MUST include deterministic tests that prove hook ordering, denial behavior, payload redaction, and no unwanted model/tool execution.

#### Scenario: denial prevents model and tool execution [r[turn-lifecycle-hooks.validation.pre-turn-deny]]
- GIVEN a pre-agent-turn hook returns `Deny`
- WHEN a prompt is submitted
- THEN no provider/model request MUST be recorded
- THEN no tool call MUST execute
- THEN the user-visible completion MUST report a safe hook denial error

#### Scenario: ordering is deterministic [r[turn-lifecycle-hooks.validation.ordering]]
- GIVEN handlers subscribe to prompt, agent-turn, lifecycle notification, and tool hooks
- WHEN a prompt produces model output and a tool call
- THEN tests MUST prove the relative order of `PrePrompt`, pre-agent-turn, existing `TurnStart`, tool hooks, existing `TurnEnd`, post-agent-turn, and `PostPrompt`

#### Scenario: payload redaction is tested [r[turn-lifecycle-hooks.validation.redaction]]
- GIVEN a prompt or tool output contains secret-like text
- WHEN post-turn payloads are captured by a script or plugin test
- THEN full secret text MUST be absent from safe-by-default turn payload fields
- THEN correlation fields and bounded previews/digests MUST still be present
