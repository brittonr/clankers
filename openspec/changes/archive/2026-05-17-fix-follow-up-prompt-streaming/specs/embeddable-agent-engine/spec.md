## MODIFIED Requirements

### Requirement: The engine API MUST expose explicit host-driven execution contracts

The engine MUST define explicit host-facing contracts for model execution requests, tool execution requests, host feedback, and semantic engine events after an adapter has accepted any core-owned prompt lifecycle transition. Host adapters MUST submit every accepted prompt lifecycle transition exactly once into the model/tool turn path, including the first prompt, ordinary subsequent user prompts, queued prompts, loop follow-ups, and controller-dispatched follow-ups. Dispatch acknowledgement MUST NOT be treated as prompt completion.
r[embeddable-agent-engine.host-driven-contracts]

#### Scenario: subsequent accepted user prompt streams in the same session
r[embeddable-agent-engine.host-driven-contracts.subsequent-user-prompt-streams]

- GIVEN a Clankers TUI or daemon-backed session has already accepted, streamed, and completed an initial user prompt
- WHEN the user submits a second ordinary prompt in the same session
- THEN the shell/controller adapter MUST accept the prompt lifecycle transition once
- THEN the accepted prompt MUST reach the model/tool turn path and emit user-visible streaming assistant output
- THEN the session MUST emit a terminal completion outcome for the second prompt
- THEN stale busy, pending prompt, follow-up, or loop state from the first prompt MUST NOT block dispatch or hide streaming output

#### Scenario: dispatched follow-up waits for real prompt completion
r[embeddable-agent-engine.host-driven-contracts.follow-up-completion-correlation]

- GIVEN the controller dispatches a follow-up prompt after post-prompt planning
- WHEN the shell acknowledges that the follow-up was accepted for execution
- THEN the acknowledgement MUST only record dispatch acceptance or rejection
- THEN the controller MUST keep a correlated pending follow-up until the agent/model/tool turn reports success, failure, or cancellation
- THEN successful dispatch alone MUST NOT clear busy state, pending prompt state, loop state, or follow-up completion state

#### Scenario: failed follow-up dispatch is visible and recoverable
r[embeddable-agent-engine.host-driven-contracts.follow-up-dispatch-failure-visible]

- GIVEN the shell cannot enqueue an accepted follow-up prompt because the agent command channel is closed or the lifecycle start is rejected
- WHEN the controller receives the dispatch acknowledgement
- THEN the follow-up MUST be marked rejected with a safe user-visible reason
- THEN the session MUST not remain permanently busy
- THEN a later ordinary user prompt MUST still be accepted and streamed normally

### Requirement: Turn orchestration MUST be engine-owned reusable policy

The reusable engine boundary MUST own model/tool turn orchestration after prompt lifecycle acceptance, while core-owned lifecycle and follow-up dispatch policy remains outside the engine. Repeated accepted prompts in a session MUST create fresh correlated model work instead of reusing, dropping, or completing against the prior prompt's request identifiers.
r[embeddable-agent-engine.turn-orchestration-owned-after-acceptance]

#### Scenario: repeated accepted prompts allocate fresh model request correlation
r[embeddable-agent-engine.turn-orchestration-owned-after-acceptance.repeated-prompt-correlation]

- GIVEN one prompt has completed with model request and terminal outcome correlation identifiers
- WHEN a later prompt is accepted in the same session
- THEN the new turn MUST allocate fresh model request correlation for its first model request
- THEN provider streaming deltas MUST be attributed to the later prompt's active turn
- THEN completion of the prior prompt MUST NOT satisfy or cancel the later prompt
