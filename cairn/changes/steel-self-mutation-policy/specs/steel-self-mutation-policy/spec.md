# Steel Self-Mutation Policy Specification

## Purpose

Defines an explicit opt-in capability for Steel Lisp scripts to request live mutation of Clankers skills, prompts, tool descriptions, or code through Nickel policy and UCAN authority, without granting Steel ambient host access.

## Requirements

### Requirement: Live mutation is explicit and opt-in [r[steel-self-mutation-policy.explicit-opt-in]]

Clankers MUST treat Steel-requested live mutation as a separate capability from default Steel evaluation and isolated self-evolution candidate generation.

#### Scenario: default Steel cannot mutate live artifacts [r[steel-self-mutation-policy.explicit-opt-in.default-deny]]
- GIVEN a Steel script runs under the default runtime profile
- **WHEN** it attempts to change an installed skill, active prompt, tool description, or repository file
- **THEN** Clankers MUST deny the request before mutation
- **AND** the denial receipt MUST identify the missing live-mutation profile or capability without exposing secrets

#### Scenario: mutation-capable run is named and visible [r[steel-self-mutation-policy.explicit-opt-in.named-run]]
- GIVEN an operator enables a Steel live-mutation run
- **WHEN** Clankers constructs the runtime request
- **THEN** the request MUST name the mutation profile, target class, intended verb, approval reference, and receipt destination
- **AND** attached or session-control observers MUST be able to see that a mutation-capable run is active

### Requirement: Nickel owns declarative mutation policy [r[steel-self-mutation-policy.nickel-policy]]

Mutation target classes, path scopes, host-function verbs, approval tiers, preflight gates, verification gates, runtime profiles, redaction rules, and rollback requirements MUST be declared in Nickel-authored policy and consumed by Rust as exported typed data or generated fixtures.

#### Scenario: policy export validates target classes and verbs [r[steel-self-mutation-policy.nickel-policy.export-contract]]
- GIVEN the mutation policy is authored in Nickel
- **WHEN** the policy export/check rail runs
- **THEN** it MUST reject unknown target classes, malformed path scopes, unsupported mutation verbs, missing approval tiers, missing verification profiles, and missing rollback requirements
- **AND** it MUST emit stable exported data suitable for Rust tests and runtime policy loading

#### Scenario: Rust runtime consumes exported policy [r[steel-self-mutation-policy.nickel-policy.runtime-boundary]]
- GIVEN a mutation request is evaluated at runtime
- **WHEN** Rust needs policy data
- **THEN** Rust MUST use exported typed policy data or generated fixtures at the enforcement boundary
- **AND** generic SDK or engine crates MUST NOT depend on live Nickel evaluation to decide per-call mutation authority

### Requirement: UCAN authorizes runtime mutation verbs and resources [r[steel-self-mutation-policy.ucan-authority]]

Every live mutation host function MUST validate a UCAN-derived authorization for the requested ability and normalized resource before writing, committing, or rolling back.

#### Scenario: matching UCAN allows policy-approved mutation [r[steel-self-mutation-policy.ucan-authority.allowed]]
- GIVEN Nickel policy allows a mutation verb for a target
- **AND** the caller presents a non-expired UCAN authorization with matching ability, resource, audience/session binding where required, and delegation limits
- **WHEN** the host function validates the request
- **THEN** the mutation MAY proceed to preflight and verification
- **AND** the receipt MUST record only safe UCAN metadata such as ability, normalized resource, expiry status, and authorization outcome

#### Scenario: missing or expired UCAN fails closed [r[steel-self-mutation-policy.ucan-authority.denied]]
- GIVEN a Steel script requests live mutation without a matching UCAN authorization, with an expired authorization, with a revoked authorization, or with an authorization for a different resource
- **WHEN** the host function validates authority
- **THEN** Clankers MUST reject the request before mutation
- **AND** the receipt MUST NOT include compact UCAN tokens, private keys, bearer credentials, or raw proofs

### Requirement: Steel host functions are typed mutation requests [r[steel-self-mutation-policy.host-functions]]

Steel scripts MAY request mutation only through typed Clankers host functions. Those functions MUST call Rust enforcement code and MUST NOT expose raw filesystem, process, network, git, credential, provider, daemon, TUI, or native-tool authority to Steel.

#### Scenario: apply mutation goes through Rust authority [r[steel-self-mutation-policy.host-functions.apply-through-rust]]
- GIVEN a Steel script calls an allowed apply-mutation host function with a target, patch, intent, and approval reference
- **WHEN** the host function executes
- **THEN** Rust MUST validate Nickel policy, UCAN authority, target normalization, preflight checks, and required approval before applying bytes
- **AND** Steel MUST receive only a structured result or denial receipt

#### Scenario: raw ambient write is denied [r[steel-self-mutation-policy.host-functions.raw-write-denied]]
- GIVEN a Steel script attempts direct filesystem write, shell execution, git mutation, credential access, provider access, or daemon mutation outside a typed host function
- **WHEN** evaluation runs under any Steel mutation profile
- **THEN** Clankers MUST deny the ambient effect with a stable issue code
- **AND** no fallback path may satisfy the request with broader authority

### Requirement: Mutation preflight and receipts are deterministic [r[steel-self-mutation-policy.receipts-and-preflight]]

Before live writes, Clankers MUST record deterministic preflight evidence and after any attempted write it MUST emit a receipt that supports audit, verification, and rollback.

#### Scenario: preflight records target and checkpoint evidence [r[steel-self-mutation-policy.receipts-and-preflight.preflight]]
- GIVEN a live mutation request targets an allowed skill, prompt, tool description, or code path
- **WHEN** preflight runs
- **THEN** Clankers MUST record target class, normalized target identity, pre-mutation hash, dirty-WIP decision, checkpoint or backup plan, Nickel policy hash, UCAN authorization outcome, and approval state
- **AND** it MUST reject path traversal, symlink escape, class/path mismatch, stale target hash, or unsupported target ambiguity before writing

#### Scenario: receipt is safe and rollbackable [r[steel-self-mutation-policy.receipts-and-preflight.safe-receipt]]
- GIVEN a mutation host function succeeds, fails, or is denied
- **WHEN** the receipt is written
- **THEN** it MUST include stable status, issue codes, safe target metadata, policy hash, safe UCAN metadata, before/after hashes when applicable, verification outcome, backup or rollback reference, and redaction decisions
- **AND** it MUST NOT include secrets, raw UCAN proofs, compact tokens, credentials, oversized patch bodies, provider payloads, or uncontrolled absolute-path dumps

### Requirement: Verification and rollback gate success [r[steel-self-mutation-policy.verification-and-rollback]]

A live mutation MUST NOT be reported as successful or promoted to commit/application success unless required verification passes, and rollback MUST guard against clobbering operator edits.

#### Scenario: failed verification blocks success [r[steel-self-mutation-policy.verification-and-rollback.failed-verification]]
- GIVEN a live mutation writes candidate bytes but policy-selected verification fails
- **WHEN** Clankers finalizes the mutation receipt
- **THEN** the receipt MUST mark the mutation as verification-failed or blocked
- **AND** follow-on commit, promotion, or success reporting MUST be denied by default

#### Scenario: rollback verifies post-apply and backup hashes [r[steel-self-mutation-policy.verification-and-rollback.guarded-rollback]]
- GIVEN a mutation receipt recorded post-apply target hash and backup hash
- **WHEN** rollback is requested
- **THEN** Clankers MUST verify the current target hash still matches the recorded post-apply hash and the backup hash matches the recorded backup before restoring bytes
- **AND** it MUST reject rollback before writing if the target changed after mutation

### Requirement: Fixtures prove allowed and denied behavior [r[steel-self-mutation-policy.verification-fixtures]]

Implementation MUST include deterministic positive and negative fixtures for Nickel policy validation, UCAN authority checks, host-function enforcement, receipt redaction, verification gating, and rollback guards.

#### Scenario: positive fixture applies allowed mutation [r[steel-self-mutation-policy.verification-fixtures.positive]]
- GIVEN a fixture Nickel policy allows a bounded skill or prompt mutation
- **AND** a fixture UCAN authorization matches the requested ability and resource
- **WHEN** the Steel host function applies a small deterministic patch
- **THEN** verification MUST pass and repeated runs MUST produce stable receipt fields excluding intentionally variable display metadata

#### Scenario: negative fixtures fail closed [r[steel-self-mutation-policy.verification-fixtures.negative]]
- GIVEN fixtures for path escape, missing UCAN, expired UCAN, wrong-resource UCAN, unauthorized verb, raw ambient write, failed verification, and stale rollback target
- **WHEN** the fixtures run
- **THEN** each MUST fail before the forbidden effect or unsafe rollback occurs
- **AND** each denial receipt MUST use stable issue codes and safe redacted metadata
