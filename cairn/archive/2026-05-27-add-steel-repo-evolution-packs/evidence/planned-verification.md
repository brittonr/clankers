# Planned verification evidence

Artifact-Type: verification-plan
Task-ID: V1, V2
Covers: steel-repo-evolution-packs.verification.fixtures, steel-repo-evolution-packs.verification.docs
Status: PLANNED

## Planned checks

- Focused Rust tests for absent, valid, malformed, hash-mismatched, path-escaped, unknown-host-call, and over-budget repo-local Steel packs.
- A deterministic checker script that writes redacted receipts under `target/steel-repo-evolution-packs/`.
- Docs build after adding operator guide material.
- Cairn proposal/design/tasks gates, `cairn validate`, and whitespace diff checks.

## Completion rule

Do not mark V1 or V2 complete until command output replaces this planning note with PASS evidence.
