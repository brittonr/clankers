# Design: Steel orchestration-pack mutation

## Overview

Steel may evolve actual orchestration by proposing patches to the repo-local Steel pack. Rust remains the authority kernel.

Allowed first target root:

```text
.clankers/steel/
```

A later change may add other reviewed roots. This change should keep bundled policy and Rust source changes outside the automatic path unless a human checkpoint explicitly upgrades the request to a normal coding-agent workflow.

## Mutation flow

1. Steel reads safe receipts/context through the repo evolution host ABI.
2. Steel emits `clankers.steel.orchestration-patch.v1` with intent, target files, expected before hashes, patch body hash, gate list, and activation policy.
3. Rust validates path scope, before hashes, patch size, profile authority, UCAN/session data, and forbidden authority changes.
4. Rust applies the patch in an isolated worktree/staging directory.
5. Rust runs the policy-selected gates.
6. Rust writes a mutation receipt with old/new pack hashes, patch hash, gate result hashes, rollback data, and activation decision.
7. The changed pack may activate only on a later turn or explicit reload after validation succeeds.

## Authority kernel boundary

Steel may change orchestration scripts, gate selection, and repo-local policy that stays inside the existing Rust host ABI. Steel may not self-approve:

- new host calls
- wider budgets
- new UCAN abilities
- broader path roots
- credential/provider/network access
- direct git push or commit authority
- disabling required gates for its own mutation

Requests that widen authority become human/oracle checkpoints or ordinary Rust coding tasks.

## Metaprogramming boundary

Steel macros and DSL expansion are allowed for orchestration planning, but the expanded patch plan must be serialized, hash-bound, and validated by Rust. Dynamic `eval` or generated Steel code must not add host calls or bypass the typed plan schema.

## Rollback

Every accepted mutation must store enough safe data to restore the prior pack if current hashes still match the post-apply receipt. Rollback must fail closed if an operator or another agent modified the pack after activation.

## Rollout

Start with dry-run patch proposals and isolated validation. Commit/apply-to-working-tree behavior should remain explicit until receipts and review history prove the loop is stable.
