# Planned verification evidence

Artifact-Type: verification-plan
Task-ID: V1, V2
Covers: steel-self-mutation-policy.verification-fixtures.orchestration-pack, steel-self-mutation-policy.host-functions.authority-kernel-checkpoint
Status: PLANNED

## Planned checks

- Focused Rust tests for valid orchestration patch proposal, path escape, stale before hash, authority widening, required gate removal, failed validation, malformed patch schema, and stale rollback target.
- A deterministic checker script that writes redacted receipts under `target/steel-orchestration-pack-mutation/`.
- Docs build after adding operator guide material for dry-run, isolated apply, next-turn activation, and rollback.
- Cairn proposal/design/tasks gates, `cairn validate`, and whitespace diff checks.

## Completion rule

Do not mark V1 or V2 complete until command output replaces this planning note with PASS evidence.
