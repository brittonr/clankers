# steel-self-mutation-policy Specification

## MODIFIED Requirements

### Requirement: Steel host functions are typed mutation requests [r[steel-self-mutation-policy.host-functions]]
Steel scripts MAY request mutation only through typed Clankers host functions. Those functions MUST call Rust enforcement code and MUST NOT expose raw filesystem, process, network, git, credential, provider, daemon, TUI, or native-tool authority to Steel.

#### Scenario: orchestration patch is a typed proposal [r[steel-self-mutation-policy.host-functions.orchestration-patch-proposal]]
- GIVEN a repo-local Steel evolution pack wants to modify its own orchestration files
- WHEN Steel emits a `clankers.steel.orchestration-patch.v1` request
- THEN Rust MUST parse the typed request, validate target paths, expected before hashes, patch hash, intent, gate list, and activation policy before any write
- AND free-form Steel output MUST NOT become a patch or executable authority

#### Scenario: authority-kernel changes are checkpointed [r[steel-self-mutation-policy.host-functions.authority-kernel-checkpoint]]
- GIVEN an orchestration patch requests new host calls, wider budgets, new UCAN abilities, broader path roots, credential/provider/network access, direct git commit or push, disabled required gates, or Rust source capability changes
- WHEN Rust validates the mutation request
- THEN Clankers MUST deny automatic application and require a human/oracle checkpoint or ordinary coding-agent workflow
- AND Steel MUST NOT self-approve the authority increase by editing its own policy files

### Requirement: Mutation preflight and receipts are deterministic [r[steel-self-mutation-policy.receipts-and-preflight]]
Before live writes, Clankers MUST record deterministic preflight evidence and after any attempted write it MUST emit a receipt that supports audit, verification, and rollback.

#### Scenario: orchestration mutation records old and new pack identity [r[steel-self-mutation-policy.receipts-and-preflight.orchestration-pack-receipt]]
- GIVEN Rust accepts or rejects a Steel orchestration-pack mutation proposal
- WHEN the mutation receipt is written
- THEN it MUST include safe target metadata, old pack hash, proposed new pack hash when available, patch hash, policy hash, script hash, selected gate names, gate result hashes, activation decision, rollback reference, and issue code when denied
- AND it MUST NOT include raw prompts, credentials, compact UCAN tokens, provider payloads, secret paths, unbounded patch bodies, or raw private transcript material

#### Scenario: isolated apply protects the working tree [r[steel-self-mutation-policy.receipts-and-preflight.isolated-apply]]
- GIVEN a Steel orchestration patch passes preflight
- WHEN Rust applies the candidate mutation
- THEN it MUST apply first in an isolated worktree or staging area with expected before-hash checks
- AND the repository working tree MUST NOT be modified unless validation succeeds and policy allows explicit promotion

### Requirement: Verification and rollback gate success [r[steel-self-mutation-policy.verification-and-rollback]]
A live mutation MUST NOT be reported as successful or promoted to commit/application success unless required verification passes, and rollback MUST guard against clobbering operator edits.

#### Scenario: next-turn activation follows successful gates [r[steel-self-mutation-policy.verification-and-rollback.next-turn-activation]]
- GIVEN an orchestration-pack mutation passes isolated apply and all policy-selected gates
- WHEN Clankers promotes the mutated pack
- THEN the new pack MUST activate only on an explicit reload or later turn after receipt recording
- AND in-flight Steel evaluation MUST continue using the pre-mutation pack hash

#### Scenario: rollback rejects stale pack state [r[steel-self-mutation-policy.verification-and-rollback.orchestration-rollback]]
- GIVEN an orchestration-pack mutation receipt recorded pre-apply, post-apply, and backup hashes
- WHEN rollback is requested
- THEN Rust MUST verify the current pack hash still matches the recorded post-apply hash and the backup hash matches the recorded pre-apply hash before restoring files
- AND rollback MUST fail closed before writing if an operator or another agent changed the pack after mutation

### Requirement: Fixtures prove allowed and denied behavior [r[steel-self-mutation-policy.verification-fixtures]]
Implementation MUST include deterministic positive and negative fixtures for Nickel policy validation, UCAN authority checks, host-function enforcement, receipt redaction, verification gating, and rollback guards.

#### Scenario: orchestration mutation fixtures cover safe and denied cases [r[steel-self-mutation-policy.verification-fixtures.orchestration-pack]]
- GIVEN fixtures for a valid script/gate update, path escape, stale before hash, authority widening, required gate removal, failed validation, malformed patch schema, and stale rollback target
- WHEN focused verification runs
- THEN the valid fixture MUST promote only after gates pass
- AND every negative fixture MUST fail before forbidden writes, authority widening, or unsafe rollback occurs
