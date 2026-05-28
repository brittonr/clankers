## ADDED Requirements

### Requirement: Session ledger API is neutral [r[session-resume-brick.ledger-contract]]

The reusable session/resume brick MUST define neutral ledger DTOs and traits for session identity, prompt turns, assistant/tool content, summaries, usage, and safe receipts without requiring desktop storage or transport types.

#### Scenario: Ledger entries avoid shell DTOs [r[session-resume-brick.ledger-contract.neutral-entries]]
- GIVEN an embedding host persists or restores conversation state
- WHEN it uses the reusable session ledger API
- THEN entries MUST be plain neutral DTOs that can represent user, assistant, tool, summary, usage, and receipt metadata
- AND the API MUST NOT require `AgentMessage`, `DaemonEvent`, TUI conversation blocks, JSONL file paths, database row types, or root shell state

### Requirement: Replay restores engine history [r[session-resume-brick.replay-contract]]

The session brick MUST reconstruct stable engine history for follow-up prompts and fail closed on unsupported or malformed persisted content.

#### Scenario: Ledger replay yields engine messages [r[session-resume-brick.replay-contract.engine-history]]
- GIVEN a session ledger contains prior user, assistant, tool, and summary entries
- WHEN a follow-up prompt is submitted with resume enabled
- THEN replay MUST produce `EngineMessage` history in the persisted causal order
- AND unsupported shell-only entries MUST be rejected or summarized at the adapter edge rather than silently dropped by the reusable brick

### Requirement: Storage backends are host-owned [r[session-resume-brick.storage-adapters]]

In-memory, product, JSONL, database, or desktop session stores MUST be adapters behind the ledger contract rather than mandatory generic SDK dependencies.

#### Scenario: Host-owned stores plug in [r[session-resume-brick.storage-adapters.host-owned]]
- GIVEN a host supplies a session store implementation
- WHEN runtime resume loads or saves ledger entries
- THEN the runtime MUST use the supplied store
- AND absent stores or desktop defaults MUST NOT be discovered implicitly in embedded mode

### Requirement: Runtime resume fails closed when required [r[session-resume-brick.runtime-integration]]

Runtime session creation MUST distinguish stateless prompts from resume-required sessions.

#### Scenario: Missing session stops before side effects [r[session-resume-brick.runtime-integration.fail-closed-missing-session]]
- GIVEN a host requests resume for a specific session id
- WHEN the session store has no matching record or reports unsupported replay
- THEN the runtime MUST return a typed missing/unsupported-session error before model or tool execution

### Requirement: Session resume verification is deterministic [r[session-resume-brick.verification]]

Verification MUST prove restored context and fail-closed behavior across at least two store shapes.

#### Scenario: Two backend fixtures preserve context [r[session-resume-brick.verification.two-backend-resume]]
- GIVEN two different store implementations persist the same prior conversation
- WHEN a follow-up prompt runs
- THEN each backend MUST produce the same ordered model-request history after neutral projection

#### Scenario: Fail-closed fixtures prevent side effects [r[session-resume-brick.verification.fail-closed]]
- GIVEN missing-session or unsupported-store fixtures run
- WHEN resume is requested
- THEN no model or tool adapter call may occur

#### Scenario: Closeout validates session brick [r[session-resume-brick.verification.closeout]]
- GIVEN implementation is complete
- WHEN focused validation runs
- THEN session/runtime/controller tests, embedded session-store checks, Cairn validation/gates, and diff checks MUST pass
