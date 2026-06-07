## MODIFIED Requirements

### Requirement: Trait seam refactors are explicit [r[remaining-coupling-drain.trait-seam-refactors]]

Clankers MUST introduce new trait seams only for coupling hotspots with multiple concrete implementations, runtime-specific shell state, or deterministic test-double needs. Each trait seam MUST identify its behavior owner, adapter boundary, DTOs crossing the seam, and focused validation rail before it replaces enum/runtime-kind branching.

#### Scenario: candidate seams are inventoried before implementation [r[remaining-coupling-drain.trait-seam-refactors.inventory]]
- GIVEN architecture review identifies plugin runtime, OAuth flow, session transport, session format, and process-job shell-port candidates
- WHEN a trait seam slice is planned
- THEN the slice MUST inventory current branches, duplicate adapters, existing tests, and concrete implementations for every candidate it touches
- AND it MUST record whether each candidate will be traitified, deferred, or intentionally kept as an enum/function boundary

#### Scenario: boundaries are justified instead of blanket traitified [r[remaining-coupling-drain.trait-seam-refactors.justified-boundaries]]
- GIVEN a candidate is a passive DTO, a single-implementation helper, or a simple enum label
- WHEN trait-seam review runs
- THEN the candidate MUST NOT be converted to a trait only for style
- AND the design or evidence MUST explain the simpler owner boundary that remains

#### Scenario: plugin runtime state is runtime-owned [r[remaining-coupling-drain.trait-seam-refactors.plugin-runtime]]
- GIVEN Extism, stdio, and future plugin kinds have different runtime handles and lifecycle rules
- WHEN plugin lifecycle operations load, stop, reload, invoke tools, drain host events, or project live inventory
- THEN runtime-specific state MUST be owned behind a plugin-runtime trait or equivalent narrow port
- AND `PluginManager` MUST remain the registry/orchestration owner instead of accumulating scattered runtime-kind branches

#### Scenario: OAuth provider flows share one provider-flow port [r[remaining-coupling-drain.trait-seam-refactors.oauth-flow]]
- GIVEN Anthropic, OpenAI Codex, or another OAuth provider needs authorization URL construction, code exchange, refresh, and optional account identity extraction
- WHEN provider auth support is added or modified
- THEN provider-specific endpoint and token logic MUST live behind a provider-flow trait or equivalent provider-owned port
- AND provider-scoped credential storage and refresh invalidation MUST remain shared rather than duplicated per provider

#### Scenario: framed session transports share I/O policy [r[remaining-coupling-drain.trait-seam-refactors.session-transport]]
- GIVEN local Unix sockets and remote QUIC streams both carry framed daemon control or attach sessions
- WHEN handshake, reconnect, read, or write behavior is changed
- THEN transport-specific I/O MUST sit behind a framed-transport seam or equivalent adapter boundary
- AND wire DTO construction MUST stay in the existing transport conversion owners required by FCIS rails

#### Scenario: session storage formats are format-owned [r[remaining-coupling-drain.trait-seam-refactors.session-format]]
- GIVEN JSONL and Automerge session files both need read, append, summary, list, and migration behavior
- WHEN session storage call sites evolve
- THEN format-specific behavior MUST live behind a session-format/store trait or equivalent format owner
- AND callers MUST NOT grow ad hoc extension checks for behavior that belongs to the format owner

#### Scenario: process-job shell ports stay below backend policy [r[remaining-coupling-drain.trait-seam-refactors.process-job-shell-ports]]
- GIVEN native, pueue, systemd, and durable reconciliation paths need command execution or wall-clock time
- WHEN process-job shell behavior is tested or shared
- THEN command execution and clock access SHOULD move behind narrow shell-port traits
- AND backend capability, retention, notification, redaction, and durable-storage policy MUST remain owned by the existing typed process-job service/backend boundaries
