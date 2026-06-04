## Why

The completed MCP/session-control self-evolution slice can generate isolated candidates, record run receipts, and capture human approval, but approval currently stops at `applied=false`. The next safe step is a human-gated application path that can copy or merge an approved candidate into the active target only after validating the original run receipt, approval receipt, target identity, and post-apply verification plan.

This change keeps the no-auto-promotion boundary intact: self-evolution may recommend and approval may be recorded, but application remains an explicit command with receipt-backed preflight, backup/rollback evidence, and user-attributed approval.

## What Changes

- **Application model**: Add a first-class candidate application request/receipt that links a run receipt, approval receipt, candidate artifact, target artifact, mode, backup, verification command, and outcome.
- **Human gate enforcement**: Require the prior approval receipt to match the run, target, candidate, session, and approver before any target write is attempted.
- **Safe apply modes**: Start with local file replacement/copy from an isolated candidate, with dry-run preflight and exact target-hash checks before live application.
- **Audit and rollback evidence**: Record pre-apply target hash, backup path/hash, post-apply hash, verification result, and rollback instructions in `application.json`.

## Capabilities

### New Capabilities

- `self-evolution-candidate-application`: Approved self-evolution candidates can be applied through an explicit, auditable command without bypassing the session-control approval boundary.

### Modified Capabilities

- `self-evolution-control`: The promotion-gate requirement gains a concrete application path while preserving explicit human approval and no automatic promotion.

## In Scope

- `clankers self-evolution apply` or equivalent CLI surface.
- Receipt validation across `receipt.json`, `approval.json`, candidate path, target path, run id, and approval status.
- Dry-run preflight that reports what would be applied without touching the target.
- Local file target replacement/copy with target hash guard, run-scoped backup, and `application.json` receipt.
- Deterministic tests for happy path, dry-run no-mutation, stale target hash rejection, approval mismatch rejection, non-recommended candidate rejection, failed verification handling, and rollback metadata.
- README/docs updates for review/apply/rollback workflow.

## Out of Scope

- Automatic apply immediately after recommendation or approval.
- Applying candidates without prior approval receipt.
- Network/remote target application.
- Multi-file git patch merge, branch merge, or conflict resolution beyond explicit local file copy/replacement in the first implementation slice.
- Deleting backups automatically or hiding rollback instructions.

## Impact

- **Files likely affected**: `src/self_evolution.rs`, `src/commands/self_evolution.rs`, `src/cli.rs`, README/docs, self-evolution tests, and OpenSpec specs.
- **APIs**: New apply options/receipt structs and validation helpers; possible new CLI enum variant.
- **Dependencies**: No new dependencies expected; reuse existing hashing, JSON, filesystem, and timestamp crates.
- **Testing**: Focused self-evolution unit tests, CLI parse/help tests, docs checks if present, `cargo check -p clankers --bins`, OpenSpec validation, and `git diff --check`.
