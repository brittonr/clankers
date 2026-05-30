# Ucan Basalt Daemon Auth Specification

## Purpose

Defines the `ucan-basalt-daemon-auth` capability.

## Requirements

### Requirement: Remote daemon auth uses public UCAN [r[ucan-basalt-daemon-auth.public-ucan]]

Remote daemon authentication MUST use canonical public UCAN credentials from OnixResearch `ucan` instead of the workspace-local `clanker-auth` token verifier by default.

#### Scenario: Public UCAN credential envelope is accepted [r[ucan-basalt-daemon-auth.public-ucan.credential-envelope]]
- GIVEN a remote peer presents a versioned Clankers public UCAN credential envelope containing a compact token, required proofs, audience/root metadata, and replay identifiers
- WHEN the daemon authenticates a remote create, attach, chat/RPC, or Matrix request
- THEN the daemon MUST decode the envelope with a version check
- AND it MUST verify the compact token and proof chain with the configured trusted roots, audience, time bounds, replay state, and revocation state
- AND it MUST expose only normalized public UCAN grants and redacted authority metadata to later authorization checks

#### Scenario: Legacy custom credentials are rejected by default [r[ucan-basalt-daemon-auth.public-ucan.reject-legacy]]
- GIVEN a remote peer presents the previous base64 `clanker-auth` credential format
- WHEN daemon auth is configured for normal remote access
- THEN the daemon MUST reject the credential before session creation or attach
- AND it MUST NOT silently fall back to `clanker-auth::TokenVerifier`
- AND any compatibility path MUST require an explicit migration/import action or operator-enabled compatibility mode

#### Scenario: Delegation chain is complete [r[ucan-basalt-daemon-auth.public-ucan.delegation-chain]]
- GIVEN a child or grandchild public UCAN credential is presented
- WHEN the daemon verifies the credential
- THEN every referenced proof needed to reach a trusted root MUST be supplied or resolvable through configured proof storage
- AND delegation MUST fail closed for missing proofs, wrong audience, expired or not-yet-valid tokens, revoked proof references, replayed credentials, or grants that widen beyond their parent authority

#### Scenario: UCAN source is reproducible [r[ucan-basalt-daemon-auth.public-ucan.dependency-source]]
- GIVEN Clankers builds the daemon auth crates
- WHEN Cargo and Nix resolve the public UCAN implementation
- THEN `clankers-ucan` MUST use the same remote-pinned OnixResearch `ucan` source family as Basalt
- AND default builds MUST NOT require a sibling `../../../ucan` checkout

### Requirement: Basalt policy gates remote authority [r[ucan-basalt-daemon-auth.basalt-policy]]

Every remote authority decision MUST combine verified public UCAN grants with Basalt policy enforcement for the same normalized Clankers resource and ability.

#### Scenario: Basalt is mandatory for admitted remote operations [r[ucan-basalt-daemon-auth.basalt-policy.mandatory]]
- GIVEN a public UCAN credential verifies successfully
- WHEN the remote peer requests session creation, session attach, prompt submission, tool execution, shell execution, file access, process control, model use, or another protected remote operation
- THEN the daemon MUST build a Basalt enforcement request for the operation's normalized resource and ability
- AND the operation MUST proceed only when both public UCAN authorization and Basalt policy allow the same request
- AND unknown contracts, resources outside policy, abilities outside policy, missing grants, and Basalt errors MUST deny fail-closed

### Requirement: Clankers auth vocabulary is stable and concrete [r[ucan-basalt-daemon-auth.vocabulary]]

Clankers MUST define a stable resource/ability vocabulary for daemon, session, tool, file, shell, process, and model operations.

#### Scenario: Operation matrix maps to concrete invocations [r[ucan-basalt-daemon-auth.vocabulary.operation-matrix]]
- GIVEN a protected daemon or tool operation is evaluated
- WHEN the auth layer constructs the public UCAN invocation and Basalt request
- THEN the resource MUST be a concrete normalized `clankers:` URI
- AND the ability MUST be a concrete Clankers ability such as `session/create`, `session/attach`, `tool/use`, `file/read`, `file/write`, `shell/execute`, `process/observe`, `process/start`, `process/mutate`, `process/stdin`, `process/logs`, or `model/use`
- AND wildcard, prefix, and caveat semantics MUST live in grants or caveats rather than in invocation requests

### Requirement: Daemon auth storage is versioned and revocable [r[ucan-basalt-daemon-auth.storage]]

Persistent auth storage MUST store versioned public UCAN records and revocation/replay state instead of opaque custom credential bytes.

#### Scenario: Stored records are versioned [r[ucan-basalt-daemon-auth.storage.versioned-records]]
- GIVEN the daemon stores or loads credentials for an iroh peer, Matrix user, or chat/RPC client
- WHEN it reads the auth database
- THEN each record MUST declare the credential envelope version and public UCAN schema
- AND malformed, unknown-version, or legacy custom records MUST be rejected or routed to explicit migration handling without granting remote authority

#### Scenario: Replay and revocation are enforced [r[ucan-basalt-daemon-auth.storage.replay-revocation]]
- GIVEN a credential proof reference or nonce has been revoked or already consumed under replay policy
- WHEN the credential is used for any remote admission
- THEN the daemon MUST deny the operation before tool/model/session effects execute
- AND the denial receipt MUST identify replay or revocation state without exposing raw token material

### Requirement: Remote entrypoints share one authority [r[ucan-basalt-daemon-auth.daemon-seams]]

QUIC control, QUIC attach, chat/RPC, Matrix, keyed-session recovery, and agent-process capability gates MUST use the same public UCAN + Basalt authority semantics.

#### Scenario: Remote entrypoints agree [r[ucan-basalt-daemon-auth.daemon-seams.remote-entrypoints]]
- GIVEN the same credential and requested operation are presented through QUIC create, QUIC attach, chat/RPC auth, Matrix stored credential lookup, or keyed-session recovery
- WHEN the operation maps to the same normalized resource and ability
- THEN each entrypoint MUST produce the same allow/deny decision and compatible redacted receipt metadata
- AND no entrypoint MAY decode or verify the legacy custom token path unless explicit migration compatibility is enabled

#### Scenario: Allow-all remains explicit [r[ucan-basalt-daemon-auth.daemon-seams.allow-all-boundary]]
- GIVEN the daemon is configured with an explicit local or test-only allow-all bypass
- WHEN a remote operation skips token checking
- THEN the bypass MUST be visible in a redacted receipt or diagnostic
- AND the bypass MUST NOT apply when normal remote auth is configured
- AND the bypass MUST NOT teach the capability gate to grant ambient authority to separately authenticated sessions

### Requirement: Tool execution is authorized at call time [r[ucan-basalt-daemon-auth.tool-gate]]

Remote session tool execution MUST authorize the exact requested operation at tool-call time using public UCAN and Basalt.

#### Scenario: Tool gate evaluates exact operation [r[ucan-basalt-daemon-auth.tool-gate.call-time]]
- GIVEN a remote-authenticated session attempts to call a tool
- WHEN the tool call is about to execute
- THEN the gate MUST construct concrete UCAN/Basalt requests for the tool name and any specific file path, shell command class, process action, model id, or session operation implied by the input
- AND the tool MUST execute only when all required requests are allowed
- AND human confirmation MUST remain an additional requirement for configured risky operations but MUST NOT override a UCAN/Basalt denial

### Requirement: Auth receipts are redacted [r[ucan-basalt-daemon-auth.receipts]]

Public UCAN and Basalt auth decisions MUST produce deterministic receipts that are safe for logs, daemon events, and tests.

#### Scenario: Receipts omit secrets and payload bodies [r[ucan-basalt-daemon-auth.receipts.redacted]]
- GIVEN any auth allow or deny decision is surfaced to logs, daemon events, tests, or persistent evidence
- WHEN the receipt is serialized
- THEN it MUST include schema version, policy id/hash, resource, ability, proof reference, audience/root identifiers, replay/revocation status, decision, and reason
- AND it MUST NOT include raw compact tokens, proof token bodies, signing keys, prompts, provider payloads, file contents, unredacted command strings, or raw tool input JSON

### Requirement: Migration is explicit and fail-closed [r[ucan-basalt-daemon-auth.migration]]

Legacy `clanker-auth` credentials MUST NOT remain ambient daemon authority after the switch.

#### Scenario: Legacy migration is operator-initiated [r[ucan-basalt-daemon-auth.migration.fail-closed]]
- GIVEN an operator needs to migrate an existing custom Clankers credential
- WHEN the operator invokes an explicit migration/import command or compatibility mode
- THEN the old credential MAY be verified with the legacy verifier only inside that bounded migration path
- AND the resulting stored credential MUST be a public UCAN envelope
- AND failed migration, absent compatibility mode, or direct remote presentation of the legacy credential MUST deny fail-closed

### Requirement: Verification is deterministic [r[ucan-basalt-daemon-auth.verification]]

The UCAN + Basalt daemon auth switch MUST be verified without live credentials, network services, private user state, or secret-bearing logs.

#### Scenario: Public UCAN fixtures cover auth edges [r[ucan-basalt-daemon-auth.verification.ucan-fixtures]]
- GIVEN deterministic public UCAN fixtures run in tests
- WHEN valid, malformed, expired, not-before, wrong-audience, missing-proof, replayed, revoked, and widening-delegation credentials are evaluated
- THEN each fixture MUST assert the expected allow or deny decision before daemon effects execute

#### Scenario: Basalt fixtures cover policy edges [r[ucan-basalt-daemon-auth.verification.basalt-fixtures]]
- GIVEN deterministic Basalt policy fixtures run in tests
- WHEN recognized and unrecognized resources, abilities, contracts, and UCAN grant sets are evaluated
- THEN policy allow/deny outcomes and receipt fields MUST be asserted without live policy services

#### Scenario: Daemon seams are covered [r[ucan-basalt-daemon-auth.verification.daemon-seams]]
- GIVEN deterministic daemon seam tests run
- WHEN QUIC create, QUIC attach, chat/RPC auth, Matrix stored credentials, and keyed session recovery are exercised
- THEN valid public UCAN credentials MUST authorize only policy-allowed operations
- AND missing, malformed, legacy, expired, revoked, wrong-audience, and policy-denied credentials MUST deny before session or tool effects execute

#### Scenario: Tool gate fixtures cover operation classes [r[ucan-basalt-daemon-auth.verification.tool-gate]]
- GIVEN deterministic tool-gate tests run
- WHEN read, write, edit, bash, process, model, and session operations are requested
- THEN each test MUST assert concrete invocation mapping, Basalt receipt contents, and fail-closed denial behavior

#### Scenario: Dependency boundary is enforced [r[ucan-basalt-daemon-auth.verification.dependency-boundary]]
- GIVEN dependency and source boundary checks run
- WHEN default daemon auth crates are inspected
- THEN they MUST fail if the default remote daemon verifier constructs `clanker-auth::TokenVerifier` credentials
- AND they MUST fail if `clankers-ucan` requires a local sibling `../../../ucan` path in default builds

#### Scenario: Closeout validation runs [r[ucan-basalt-daemon-auth.verification.closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused Rust tests, `cargo check --tests` for touched crates, Cairn validation/gates, diff checks, spec sync, and archive MUST pass
