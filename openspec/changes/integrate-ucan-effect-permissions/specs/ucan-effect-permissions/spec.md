## ADDED Requirements

### Requirement: UCAN-backed effect admission

Protected Clankers effect handlers MUST verify a UCAN invocation decision before performing filesystem, shell, network, secret, browser, scheduler, remote execution, provider, artifact-store mutation, or delivery side effects.
r[ucan-effect-permissions.handler-admission]

#### Scenario: matching UCAN grant allows handler execution
r[ucan-effect-permissions.handler-admission.allow]

- GIVEN an effect request with a normalized ability, resource URI, caveat context, invoker DID, audience DID, and proof references
- AND the configured UCAN verifier returns an allowed invocation decision for those facts
- WHEN the effect dispatcher evaluates admission
- THEN the dispatcher invokes the effect handler
- THEN the effect receipt records safe authorization metadata and the handler result hash

#### Scenario: denied UCAN blocks side effects
r[ucan-effect-permissions.handler-admission.deny]

- GIVEN an effect request whose UCAN proof chain is missing, expired, revoked, wrong-audience, insufficient, or rejected by caveat policy
- WHEN the effect dispatcher evaluates admission
- THEN it returns a structured authorization denial before contacting the effect handler
- THEN filesystem, process, network, browser, provider, secret, scheduler, delivery, and remote resources are not touched

### Requirement: Stable Clankers effect capability vocabulary

The system MUST map effect ability classes to stable UCAN ability identifiers and normalized resource URIs before authorization.
r[ucan-effect-permissions.effect-vocabulary]

#### Scenario: known effect maps to stable ability and resource
r[ucan-effect-permissions.effect-vocabulary.known-effect]

- GIVEN a file-read, file-write, shell, network, secret, browser, scheduler, remote-exec, artifact-read, or artifact-write effect request
- WHEN Clankers builds UCAN invocation facts
- THEN the request maps to a documented `clankers/...` ability identifier
- THEN the resource URI is normalized by fixture-covered rules before verification

#### Scenario: unknown effect fails closed
r[ucan-effect-permissions.effect-vocabulary.unknown-effect]

- GIVEN an effect request whose class has no UCAN mapping
- WHEN admission is evaluated
- THEN the request is denied before handler execution
- THEN the denial identifies the unmapped effect class without exposing secrets

### Requirement: Clankers caveat policy hooks are deterministic and fail closed

The UCAN integration MUST evaluate Clankers-owned caveat payloads through explicit deterministic policy hooks and MUST deny authorization for unknown, malformed, or unsatisfied caveats.
r[ucan-effect-permissions.caveat-policy]

#### Scenario: path and command caveats narrow authority
r[ucan-effect-permissions.caveat-policy.path-command]

- GIVEN a token grants file or shell authority with path-prefix, command-allowlist, timeout, and max-bytes caveats
- WHEN an effect request is checked against that token
- THEN authorization succeeds only when the request facts satisfy every caveat
- THEN any wider path, unlisted command, excessive timeout, or excessive byte request is denied

#### Scenario: unknown caveat denies authorization
r[ucan-effect-permissions.caveat-policy.unknown-denies]

- GIVEN a matching UCAN capability contains a caveat unknown to Clankers
- WHEN invocation authorization evaluates the matched capability
- THEN authorization is denied with an unknown-caveat reason

### Requirement: Delegated execution contexts receive attenuated UCAN authority

Subagents, remote daemon peers, scheduled jobs, replay handlers, and plugin/MCP tool contexts MUST receive only delegated authority that is no broader than the parent session authority.
r[ucan-effect-permissions.delegation]

#### Scenario: subagent cannot widen parent authority
r[ucan-effect-permissions.delegation.no-widening]

- GIVEN a parent session has UCAN authority for a bounded set of effects, resources, caveats, and expiry
- WHEN Clankers creates a delegated subagent or remote execution grant
- THEN the child grant cannot add abilities, widen resources, drop parent caveats, extend expiry, or bypass replay/revocation requirements

#### Scenario: delegated denial preserves parent session
r[ucan-effect-permissions.delegation.child-denied]

- GIVEN a child context requests an effect outside its delegated authority
- WHEN admission evaluates the child request
- THEN only the child effect is denied
- THEN the parent session authority is not mutated or broadened

### Requirement: UCAN replay and revocation hooks gate protected effects

The system MUST integrate UCAN replay admission and revocation hooks for protected effects whose grants require freshness, nonce, proof-chain, or revocation checks.
r[ucan-effect-permissions.replay-revocation]

#### Scenario: duplicate replay is denied
r[ucan-effect-permissions.replay-revocation.duplicate]

- GIVEN a protected effect grant requires replay admission for a request nonce or invocation identifier
- WHEN the UCAN replay policy reports a duplicate, stale, malformed, denied, backend, or unknown outcome
- THEN the effect request is denied before handler execution

#### Scenario: revoked proof denies request
r[ucan-effect-permissions.replay-revocation.revoked]

- GIVEN a token or proof-chain entry is reported revoked by the configured UCAN revocation checker
- WHEN the effect request is authorized
- THEN authorization fails closed before handler execution

### Requirement: UCAN authorization receipts are content-addressed and redacted

Effect receipts, content-addressed artifact envelopes, replay records, and review outputs MUST record safe UCAN authorization metadata without persisting raw compact tokens, signing secrets, headers, environment values, or unredacted provider payloads.
r[ucan-effect-permissions.authorization-receipts]

#### Scenario: allowed receipt records proof identity safely
r[ucan-effect-permissions.authorization-receipts.allowed]

- GIVEN an effect request is allowed by UCAN verification
- WHEN Clankers records the effect receipt
- THEN the receipt includes effect ability, resource URI, authorization status, issuer/audience identifiers, caveat classes or IDs, proof-chain hash/reference, replay/revocation status where applicable, and handler result hash
- THEN the receipt excludes raw compact tokens and secret material

#### Scenario: denied receipt redacts sensitive details
r[ucan-effect-permissions.authorization-receipts.denied-redacted]

- GIVEN an effect request is denied because of a missing proof, caveat violation, revocation, replay failure, malformed token, or backend error
- WHEN Clankers records the denial
- THEN the receipt records a structured denial class and safe proof metadata
- THEN raw tokens, headers, environment values, and secret-bearing caveat payloads are not printed or persisted in queryable ledger facts

### Requirement: Typed ledger records authorization decisions

The typed durable session ledger MUST persist safe structured facts for UCAN authorization decisions so reviews and queries can distinguish allowed, denied, simulated, replayed, and unavailable effect outcomes.
r[ucan-effect-permissions.ledger-facts]

#### Scenario: ledger query finds authorization denial by class
r[ucan-effect-permissions.ledger-facts.query-denial]

- GIVEN a session contains denied effect requests
- WHEN a caller queries the typed ledger by authorization status, effect ability, resource class, or denial class
- THEN Clankers returns matching redacted records with stable IDs and artifact/proof references
- THEN it does not expose raw token or secret payloads

### Requirement: Clankers consumes sibling UCAN public APIs through an adapter seam

The Clankers integration MUST depend on public APIs from the sibling `../ucan/` library through a narrow adapter and MUST NOT reimplement UCAN token parsing, proof traversal, attenuation, caveat preservation, replay admission, revocation, or invocation authorization semantics locally.
r[ucan-effect-permissions.ucan-adapter]

#### Scenario: adapter calls public UCAN invocation workflow
r[ucan-effect-permissions.ucan-adapter.public-api]

- GIVEN a normalized Clankers effect request and configured authority/proof context
- WHEN the UCAN authorization adapter verifies the request
- THEN it calls public `ucan` library issuance/verification/invocation/replay/revocation APIs as applicable
- THEN Clankers maps the resulting allowed or denied decision into its own effect-admission result

#### Scenario: dependency source is reproducible before release
r[ucan-effect-permissions.ucan-adapter.reproducible-source]

- GIVEN Clankers release or CI packaging includes the UCAN integration
- WHEN dependency resolution runs without an ambient sibling checkout
- THEN the UCAN source is resolved from a pinned or vendored reproducible source
- OR the release check fails with an explicit unsupported-sibling-checkout error before claiming distributable support
