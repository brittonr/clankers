## ADDED Requirements

### Requirement: Runtime sessions delegate to engine host [r[runtime-engine-host.engine-host-delegation]]

The host-facing runtime facade MUST execute accepted prompts through `clankers-engine` and `clankers-engine-host` rather than maintaining a parallel model-event shortcut.

#### Scenario: Submit prompt runs the engine-host runner [r[runtime-engine-host.engine-host-delegation.submit-prompt]]
- GIVEN a host creates a runtime session and submits a prompt
- WHEN prompt execution begins
- THEN the runtime MUST build an `EnginePromptSubmission` from host-owned prompt and replay state
- AND it MUST drive the turn through `clankers-engine-host::run_engine_turn`
- AND direct model-adapter event injection MUST NOT be the primary execution path

#### Scenario: Host adapters are explicit [r[runtime-engine-host.engine-host-delegation.host-adapters]]
- GIVEN model, tool, retry, event, cancellation, or usage behavior is required
- WHEN the runtime executes a turn
- THEN each behavior MUST be supplied through an explicit engine-host adapter or a documented safe default
- AND absent required adapters MUST fail closed without constructing desktop provider, plugin, daemon, database, or TUI fallbacks

### Requirement: Runtime emits projected semantic session events [r[runtime-engine-host.session-events]]

The runtime MUST expose host-facing `SessionEvent` values by projecting engine-host activity and terminal reports into semantic, safe events.

#### Scenario: Engine activity projects to session events [r[runtime-engine-host.session-events.engine-projection]]
- GIVEN a runtime turn produces assistant text, thinking, tool calls, tool results, usage, errors, or completion
- WHEN the host receives runtime events
- THEN events MUST appear in causal order as semantic `SessionEvent` variants
- AND event metadata MUST include safe session/prompt identifiers without daemon frames, TUI widgets, provider payloads, credentials, or raw hidden context

### Requirement: Runtime adapter parity is executable [r[runtime-engine-host.adapter-parity]]

At least one existing headless or batch shell path MUST use or match the runtime engine-host path with deterministic fake adapters.

#### Scenario: Batch/headless path uses runtime facade [r[runtime-engine-host.adapter-parity.batch-headless]]
- GIVEN a batch/headless fake-provider prompt fixture
- WHEN it runs through the runtime facade
- THEN the path MUST execute through engine-host adapters and not through daemon sockets or TUI state

#### Scenario: Agent and runtime fake paths agree [r[runtime-engine-host.adapter-parity.agent-runtime]]
- GIVEN the same fake model/tool prompt fixture is run through the agent turn adapter and runtime facade
- WHEN the resulting events and model requests are compared
- THEN session id, prompt acceptance, model request metadata, event ordering, and terminal completion semantics MUST match after edge-specific projection

### Requirement: Runtime engine-host verification is deterministic [r[runtime-engine-host.verification]]

Verification MUST cover the runtime engine-host path without live credentials, network, daemon startup, plugin subprocesses, or user-local Clankers state.

#### Scenario: Deterministic matrix covers core effects [r[runtime-engine-host.verification.deterministic-matrix]]
- GIVEN deterministic runtime tests are executed
- WHEN model success, tool continuation, retryable model failure, cancellation, usage observation, and missing adapter cases run
- THEN each case MUST assert engine-host report state, projected events, and fail-closed behavior

#### Scenario: Closeout runs embedded acceptance [r[runtime-engine-host.verification.closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused runtime tests, focused agent parity tests, the embedded SDK acceptance rail, Cairn validation/gates, and diff checks MUST pass
