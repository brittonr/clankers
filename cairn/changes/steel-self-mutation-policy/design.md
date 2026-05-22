# Design: Steel Self-Mutation Policy

## Overview

Live self-modification is modeled as a separate capability from normal Steel evaluation and conservative self-evolution. A Steel script can produce an intention and patch, then call a typed host function such as `request_mutation`. The Rust host validates that request against exported Nickel policy and a UCAN authorization proof before any bytes are written.

```text
Steel script
  -> request_mutation(target, verb, patch, intent)
  -> Rust host loads exported Nickel mutation policy
  -> Rust host validates UCAN ability/resource/expiry/delegation/revocation
  -> Rust host preflights git/checkpoint/target hash/approval state
  -> Rust host applies patch through typed mutation adapter
  -> Rust host runs required verification
  -> Rust host emits receipt + rollback evidence
```

Steel never receives raw filesystem, shell, git, network, provider, credential, or daemon authority. All mutation happens through Rust-owned host functions.

## Nickel policy responsibilities

Nickel is the declarative policy authoring boundary. It should define:

- mutation target classes: `skill`, `prompt`, `tool_description`, `repo_code`, and future additive classes;
- allowed path roots and deny patterns per class;
- allowed host-function verbs per class, such as `propose_patch`, `apply_patch_to_allowed_target`, `write_skill_candidate`, `commit_candidate`, and `rollback_mutation`;
- approval tier required per class and verb;
- preflight requirements, including clean git state, checkpoint creation, target hash match, dirty-WIP preservation, and active-session visibility;
- required post-write verification commands or named check profiles;
- Steel runtime profile and budget required for mutation-capable scripts;
- receipt redaction rules and safe metadata fields;
- rollback requirements and stale-target rejection behavior.

Rust should consume exported typed policy data or generated fixtures. Runtime SDK/engine crates should not embed Nickel evaluation directly unless a later change explicitly proves why live Nickel evaluation is needed at that boundary.

## UCAN authority responsibilities

UCAN is the runtime authorization proof. The mutation adapter checks a UCAN-derived authorization against the Nickel policy decision before mutation.

Ability strings should be stable and narrow, for example:

- `clankers/steel/mutation.propose`;
- `clankers/steel/mutation.apply`;
- `clankers/steel/mutation.commit`;
- `clankers/steel/mutation.rollback`.

Resource strings should bind the class and normalized target scope, for example:

- `skill:agentkit-port/hermes-agent`;
- `prompt:system/clankers-agent`;
- `repo:/crates/clankers-agent/**`.

The host must check expiry, audience/session binding where available, delegation limits, revocation/deny status, and exact ability/resource compatibility. Receipts record only safe UCAN metadata: ability, normalized resource, issuer/audience fingerprints if available, expiry bucket/status, and denial class. Receipts must not include compact UCAN tokens, private keys, bearer credentials, or raw proofs.

## Host-function contract

The first mutation-capable Steel host functions should be typed and narrow:

- `propose_mutation(target, patch, intent)` validates target class/path and returns a receipt-only candidate without writing.
- `apply_mutation(target, patch, intent, approval_ref)` applies a checked patch only after Nickel policy, UCAN authority, approval, preflight, and checkpoint checks pass.
- `commit_mutation(receipt_ref, message)` may commit only when Nickel policy and UCAN allow it and required verification passed.
- `rollback_mutation(receipt_ref)` restores guarded backup bytes only if current target hash matches the recorded post-apply hash.

All host functions share the same Steel runtime wrapper, resource budgets, disabled-tool parity, receipt redaction, and capability checks as non-mutating Steel evaluation.

## Preflight and verification

Before live writes, the Rust host must:

- normalize and classify the target path or named artifact;
- reject path traversal, symlink escapes, untracked target ambiguity, and class/path mismatches;
- preserve or reject dirty WIP according to policy;
- record target pre-hash and checkpoint/backup metadata;
- verify approval tier and UCAN authority;
- run mutation in a way that can produce deterministic before/after evidence.

After writes, the host runs policy-selected verification. Code mutation should require at least a focused compile/test/lint profile. Skill or prompt mutation should require syntax/frontmatter/schema validation and a deterministic smoke or review fixture where practical. Failed verification must be visible and rollbackable rather than reported as success.

## Relationship to self-evolution

This does not weaken `self-evolution-control.isolation`. The default self-evolution path remains isolated candidate generation followed by explicit approval/application. Live mutation is a separate opt-in path that requires explicit Steel mutation profile, Nickel policy allowance, UCAN authority, and receipt-backed rollback.

## Risks and mitigations

- **Policy/authority drift:** keep Nickel policy hashes and UCAN-safe authorization metadata in receipts; test both allowed and denied paths.
- **Overbroad UCAN grants:** use narrow abilities/resources and deny wildcard resources unless Nickel policy explicitly allows a bounded class.
- **Steel escape hatch:** deny all raw filesystem/process/network/provider/credential access and prove with negative fixtures.
- **Path escape or symlink attacks:** normalize targets in Rust and reject unsafe paths before patch application.
- **Verification laundering:** failed checks must produce failure receipts and block commit/promotion by default.
- **Rollback clobbers operator edits:** rollback must verify current post-apply hash before restoring backup bytes.
