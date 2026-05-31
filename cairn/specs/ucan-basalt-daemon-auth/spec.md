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

### Requirement: Public file-tool invocations use concrete paths [r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths]]

Public UCAN + Basalt authorization MUST build file read/write invocation requests only from explicit, non-empty tool input paths.

#### Scenario: Omitted file path fails closed [r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths.omitted]]
- GIVEN a public UCAN-gated read-only or write file tool call omits `path`
- WHEN Clankers maps the call into UCAN/Basalt admission requests
- THEN mapping MUST fail closed before the tool executes
- AND Clankers MUST NOT fabricate an ambient current-directory, project-root, wildcard, or default file resource

#### Scenario: Blank file path fails closed [r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths.blank]]
- GIVEN a public UCAN-gated read-only or write file tool call provides an empty or whitespace-only `path`
- WHEN Clankers maps the call into UCAN/Basalt admission requests
- THEN mapping MUST fail closed before Basalt admission or tool execution
- AND the denial MUST use safe metadata without raw file contents, prompts, credentials, or token material

### Requirement: Public file-tool paths are canonical authorization resources [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths]]

Public UCAN + Basalt authorization MUST normalize file-tool paths into deterministic `clankers:file:...` resources before building `file/read` or `file/write` requests.

#### Scenario: Public auth carries session file root [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.root-context]]
- GIVEN a remote public UCAN-authenticated session is created, attached, or recovered
- WHEN Clankers constructs the public tool authorization context
- THEN it MUST include the session/project file root used to resolve relative file-tool paths
- AND absence of a required root for relative path resolution MUST deny before Basalt admission or tool execution

#### Scenario: Relative paths resolve under the session root [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.relative-resolution]]
- GIVEN a public UCAN-gated file tool call includes a relative path such as `src/lib.rs`
- AND the public authorization context has file root `/workspace/project`
- WHEN Clankers maps the call into admission requests
- THEN the file request resource MUST be `clankers:file:/workspace/project/src/lib.rs`
- AND the request MUST use the matching `file/read` or `file/write` ability

#### Scenario: Parent traversal escapes deny [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.traversal-denial]]
- GIVEN a public UCAN-gated file tool call includes `..` traversal that would escape the session file root
- WHEN Clankers maps the call into admission requests
- THEN mapping MUST fail closed before Basalt admission or tool execution
- AND Clankers MUST NOT rewrite the traversal into an ambient, project-parent, wildcard, or default resource

#### Scenario: Absolute paths keep explicit resource semantics [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.absolute-paths]]
- GIVEN a public UCAN-gated file tool call includes an absolute path
- WHEN Clankers maps the call into admission requests
- THEN the absolute path MUST remain the basis for the concrete `clankers:file:...` resource
- AND UCAN/Basalt grants and policy MUST decide whether that absolute resource is allowed

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

### Requirement: Public tool gate keeps concrete file authorization [r[ucan-basalt-daemon-auth.tool-gate.concrete-file-paths]]

The public UCAN + Basalt tool gate MUST require the generic `tool/use` authorization and the concrete file read/write authorization for file-oriented tools.

#### Scenario: Concrete file path builds file request [r[ucan-basalt-daemon-auth.tool-gate.concrete-file-paths.present]]
- GIVEN a public UCAN-gated file tool call includes a non-empty `path`
- WHEN Clankers maps the call into admission requests
- THEN the request set MUST include `tool/use` for the tool
- AND it MUST include the matching concrete `file/read` or `file/write` request for the supplied path

### Requirement: Public tool gate uses canonical file paths [r[ucan-basalt-daemon-auth.tool-gate.canonical-file-tool-paths]]

The public UCAN + Basalt tool gate MUST use canonical file authorization resources for every file read/write request it adds to a tool invocation.

#### Scenario: Canonical path request is evaluated at call time [r[ucan-basalt-daemon-auth.tool-gate.canonical-file-tool-paths.call-time]]
- GIVEN a public UCAN-authenticated session attempts a read-only or write file tool call
- WHEN the tool call is authorized
- THEN Clankers MUST authorize generic `tool/use` and the canonical `file/read` or `file/write` request at call time
- AND the tool MUST execute only when all required canonical requests are allowed

### Requirement: Local legacy capability settings remain unchanged [r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged]]

This hardening MUST NOT change the legacy `settings.defaultCapabilities` tool-name filter used for local/non-public-UCAN sessions.

#### Scenario: Legacy local gate keeps tool-name behavior [r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged.tool-only]]
- GIVEN a local legacy `UcanCapabilityGate` is built from `settings.defaultCapabilities`
- WHEN a tool-name-only capability authorizes a file-oriented tool without a path
- THEN this change MUST NOT add a new concrete-path requirement to that legacy local gate
- AND public UCAN + Basalt sessions MUST still use the stricter concrete-path requirement

#### Scenario: Legacy local gate keeps canonical-file-path behavior unchanged [r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged.canonical-file-paths]]
- GIVEN a local legacy `UcanCapabilityGate` is built from `settings.defaultCapabilities`
- WHEN it authorizes a file-oriented tool call
- THEN this change MUST NOT add session-root canonicalization requirements to that legacy local gate
- AND public UCAN + Basalt sessions MUST still use canonical file-path request construction

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

#### Scenario: Focused tests cover file path hardening [r[ucan-basalt-daemon-auth.verification.concrete-file-path-tests]]
- GIVEN focused tests run for the capability gate
- WHEN public UCAN-gated file tool calls omit, blank, or provide concrete paths
- THEN tests MUST prove omitted and blank paths deny before tool execution
- AND concrete paths still construct and authorize the expected file request

#### Scenario: Checker writes redacted receipt [r[ucan-basalt-daemon-auth.verification.concrete-file-path-checker]]
- GIVEN implementation, tests, specs, and tasks are present
- WHEN the deterministic checker runs
- THEN it MUST write a receipt under `target/ucan-concrete-file-tool-paths/`
- AND the receipt MUST hash source artifacts without embedding raw compact UCAN tokens, signing keys, prompts, provider payloads, file contents, or tool input bodies

#### Scenario: Concrete file path closeout validation runs [r[ucan-basalt-daemon-auth.verification.concrete-file-path-closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused Rust tests, checker receipt, Cairn validation/gates, sync/archive inspection, and diff checks MUST pass

#### Scenario: Focused tests cover canonical file paths [r[ucan-basalt-daemon-auth.verification.canonical-file-path-tests]]
- GIVEN focused capability-gate tests run
- WHEN public UCAN-gated file tools use relative, traversal, and absolute paths
- THEN tests MUST prove relative paths resolve under the session root
- AND traversal escapes deny before Basalt admission or tool execution
- AND absolute paths keep explicit resource semantics
- AND the legacy local gate remains unchanged

#### Scenario: Checker writes redacted canonicalization receipt [r[ucan-basalt-daemon-auth.verification.canonical-file-path-checker]]
- GIVEN implementation, tests, specs, and tasks are present
- WHEN the deterministic checker runs
- THEN it MUST write a receipt under `target/ucan-canonical-file-tool-paths/`
- AND the receipt MUST hash source artifacts without embedding raw compact UCAN tokens, signing keys, prompts, provider payloads, file contents, or tool input bodies

#### Scenario: Canonical file path closeout validation runs [r[ucan-basalt-daemon-auth.verification.canonical-file-path-closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused Rust tests, checker receipt, Cairn validation/gates, sync/archive inspection, and diff checks MUST pass

#### Scenario: Dependency boundary is enforced [r[ucan-basalt-daemon-auth.verification.dependency-boundary]]
- GIVEN dependency and source boundary checks run
- WHEN default daemon auth crates are inspected
- THEN they MUST fail if the default remote daemon verifier constructs `clanker-auth::TokenVerifier` credentials
- AND they MUST fail if `clankers-ucan` requires a local sibling `../../../ucan` path in default builds

#### Scenario: Closeout validation runs [r[ucan-basalt-daemon-auth.verification.closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused Rust tests, `cargo check --tests` for touched crates, Cairn validation/gates, diff checks, spec sync, and archive MUST pass
