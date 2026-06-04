## MODIFIED Requirements

### Requirement: Project job profiles [r[durable-process-jobs.project-profiles]]

The system MUST resolve supported named project job profiles through versioned, validated configuration and backend-neutral specs so common long-running tasks can be started reproducibly without coupling project config to backend implementations or ambient machine state.

#### Scenario: named profile resolves to backend-neutral start spec [r[durable-process-jobs.project-profiles.resolve]]

- GIVEN a project defines a named job profile such as `verify`, `nextest`, or `devServer`
- WHEN a caller starts that profile
- THEN Clankers SHOULD resolve it into the same backend-neutral `ProcessJobSpec` used by direct start requests
- THEN backend selection, resource policy, notification policy, cwd, environment policy, and capabilities MUST be validated before backend dispatch

#### Scenario: profile manifest discovery is deterministic [r[durable-process-jobs.project-profiles.discovery]]

- GIVEN global, workspace, and explicit profile manifests are all present
- WHEN Clankers resolves a named profile
- THEN it MUST apply documented precedence with explicit request input above workspace config and workspace config above global config
- THEN it MUST include the selected manifest path or source label, manifest schema version, profile name, and policy source in the resolved profile evidence
- THEN it MUST reject ambiguous duplicate profiles at the same precedence level with a typed validation error before backend dispatch

#### Scenario: profile resolution stays side-effect free [r[durable-process-jobs.project-profiles.side-effect-free]]

- GIVEN a profile manifest names a pueue or systemd backend
- WHEN Clankers validates or resolves the profile
- THEN it MUST NOT spawn a process, contact pueue/systemd, open live credentials, write redb, write logs, or emit user notifications during resolution
- THEN tests MUST prove resolution through fake stores/backends that fail if backend dispatch occurs

#### Scenario: invalid profile is rejected before execution [r[durable-process-jobs.project-profiles.invalid]]

- GIVEN a project job profile contains an unsupported backend, disallowed writable path, unsafe environment entry, malformed command/program shape, ambiguous manifest source, or resource value exceeding policy
- WHEN Clankers loads or starts the profile
- THEN it MUST reject the profile with typed validation errors and MUST NOT execute any command from it
- THEN the error receipt MUST include the profile name and safe policy reason without raw environment values, command secrets, headers, tokens, or credentials

### Requirement: Process job profile kit validates backend-neutral job manifests [r[durable-process-jobs.process-job-profile-kit]]

The system MUST define `process-job-profile-kit` as a composable Clankers brick with explicit ownership boundaries, deterministic fixtures, safe evidence, and guard rails that prevent profile drift from reaching backend dispatch.

#### Scenario: Brick boundary is explicit [r[durable-process-jobs.process-job-profile-kit.boundary]]

- GIVEN a product or contributor adopts the `process-job-profile-kit` brick
- WHEN the brick is documented, instantiated, or validated
- THEN the contract MUST name which behavior is reusable, which behavior stays product-owned, and which shell/runtime systems are out of scope
- THEN reusable profile parsing, policy validation, identity derivation, redaction, manifest discovery, and receipt projection MUST stay separate from product-owned daemon/session selection, backend spawning, persistence, and user notification delivery

#### Scenario: Profile resolution is pure and backend-neutral [r[durable-process-jobs.process-job-profile-kit.evidence]]

- GIVEN a project manifest defines a named process job profile
- WHEN the profile resolver accepts it
- THEN resolving a profile produces a backend-neutral start request without spawning a process, contacting pueue/systemd, writing storage, using TUI state, or reading ambient credentials
- THEN the resolved request MUST include explicit backend, command/program shape, cwd, owner, resource policy, notification policy, safe metadata, manifest schema version, and profile source evidence

#### Scenario: Profile policy fails closed before backend dispatch [r[durable-process-jobs.process-job-profile-kit.fail-closed]]

- GIVEN a process job profile names a disallowed backend, malformed command shape, secret-like environment key, resource limit above policy, disallowed cwd, disallowed writable path, or ambiguous manifest source
- WHEN profile validation runs
- THEN it MUST reject the profile before backend dispatch and MUST NOT execute any command from that profile
- THEN negative tests MUST cover each fail-closed class with safe typed error codes

#### Scenario: Profile receipts preserve safe identity [r[durable-process-jobs.process-job-profile-kit.receipts]]

- GIVEN a process/job is started from a named profile
- WHEN Clankers returns start, list, poll, log, wait, kill, notification, or GC receipts for that job
- THEN receipts MUST carry the stable Clankers job id, backend kind, profile name, manifest schema version, profile source label, policy source, and safe command preview where applicable
- THEN receipts MUST NOT include raw secret values, full unbounded commands, full environment maps, credentials, headers, tokens, or backend-only locators as public stable ids

#### Scenario: Brick drift is diagnosable [r[durable-process-jobs.process-job-profile-kit.drift]]

- GIVEN source code, docs, fixtures, policy, or generated inventories drift apart
- WHEN the brick validation rail runs
- THEN it MUST fail with a diagnostic that names the stale artifact and the expected owner of the update
- THEN intentional contract changes MUST require updating tests, docs, and receipt or fixture evidence together
- THEN `scripts/check-process-job-profile-kit.rs` MUST remain part of the embedded SDK acceptance rail while the profile kit is advertised as a reusable brick
