## Phase 0: OpenSpec foundation

- [x] Author proposal, design, task plan, and delta spec for human-gated self-evolution candidate application.
- [x] Validate and commit the OpenSpec package.

## Phase 1: Application model and validation

- [ ] Add self-evolution application options and receipt models for run receipt path, approval receipt path, apply mode, target/candidate identity, backup path, verification command, dry-run state, and outcome. [covers=self-evolution-control.application-model]
- [ ] Implement receipt-chain validation that rejects mismatched run/approval receipts, non-recommended candidates, failed evaluations, missing candidates, stale target hashes, already-applied approval states, and unsupported apply modes before mutation. [covers=self-evolution-control.application-validation]
- [ ] Add deterministic unit tests for validation failures and safe dry-run preflight. [covers=self-evolution-control.application-validation]

## Phase 2: Local file application

- [ ] Implement `replace-file` dry-run and live apply paths with shared validation, run-scoped backup creation, target copy, pre/post hashes, and `application.json` writing. [covers=self-evolution-control.application-execution]
- [ ] Add CLI action such as `clankers self-evolution apply --receipt ... --approval ... --mode replace-file --verify-command ... [--dry-run] [--json]`. [covers=self-evolution-control.application-cli]
- [ ] Add tests for live replacement, backup/rollback metadata, no-mutation dry run, stale-target rejection, and verification failure receipt status. [covers=self-evolution-control.application-execution]

## Phase 3: Documentation and verification

- [ ] Document the run → approve → apply workflow, receipt review checklist, dry-run preflight, backup location, rollback instructions, and first-pass `replace-file` limitation. [covers=self-evolution-control.application-documentation]
- [ ] Run focused self-evolution tests, MCP tests if touched, `cargo check -p clankers --bins`, OpenSpec validation, and `git diff --check`.
- [ ] Sync canonical specs and archive this change after implementation is complete.
