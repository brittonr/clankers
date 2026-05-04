## Context

The current self-evolution implementation is intentionally conservative. `run` writes an isolated `candidate.txt` and `receipt.json`; `approve` validates a recommended run and writes `approval.json`, but the approval receipt records `applied=false`. The canonical `self-evolution-control` spec allows a candidate to be applied only after explicit human approval and documented install/merge behavior.

This change defines the next boundary: an explicit apply command that consumes those receipts and applies only a local file candidate when all gates pass.

## Goals / Non-Goals

**Goals:**

- Preserve the no-auto-promotion invariant: application is a separate explicit action.
- Validate the full receipt chain before touching a target.
- Keep first-pass application local, file-scoped, deterministic, and rollback-friendly.
- Produce a durable `application.json` receipt with enough evidence to audit or reverse the change.
- Make dry-run preflight and live apply share the same validation path.

**Non-Goals:**

- No branch merge, multi-file patch application, conflict resolution, or remote deployment in this slice.
- No in-process bypass of approval/session-control receipts.
- No automatic application from `run` or `approve`.
- No destructive apply without target hash guard and backup.

## Decisions

### 1. Application is a third explicit step

**Choice:** Add a separate application action after `run` and `approve`.

**Rationale:** Approval is not mutation. Keeping application separate gives humans a review point and keeps receipts unambiguous.

**Rejected alternative:** Make `approve` optionally apply the candidate. That couples consent recording to mutation and makes dry-run/non-dry-run behavior harder to audit.

**Implementation:** Add an action such as `self-evolution apply --receipt <receipt.json> --approval <approval.json> --mode replace-file --verify-command <cmd> [--dry-run] [--json]`.

### 2. Receipt chain validation is the authority boundary

**Choice:** Validate run receipt, approval receipt, target path, candidate path, run id, approval status, and pre-apply target hash before writing.

**Rationale:** The application command should not trust paths or approver labels provided only on the command line.

**Implementation:** Parse `SelfEvolutionRunReceipt` and `SelfEvolutionApprovalReceipt`; require matching run id, target, candidate, and `approval.approved=true`; require `approval.applied=false`; reject non-recommended or failed-eval run receipts; require the current target hash to match the run receipt baseline hash unless the command explicitly documents a later rebase path.

### 3. Local file replacement first

**Choice:** First implementation applies one candidate file over one target file by copying bytes after validation.

**Rationale:** The current candidate model materializes `candidate.txt`, so local file replacement is the smallest useful live path. More complex patch/worktree merge modes need their own OpenSpec slice.

**Implementation:** Support `replace-file` only. Reject directories, missing candidates, missing target parent directories, symlink-unsafe targets if detected, and unknown modes with structured errors.

### 4. Backup and verification receipts are mandatory

**Choice:** A live apply must create a run-scoped backup before writing and must record verification outcome after writing.

**Rationale:** Candidate application is the first live mutation path; rollback evidence must be present by construction.

**Implementation:** Write backup bytes to a sibling/run-scoped backup path such as `<run-dir>/backup/<target-file-name>.<sha>.bak`, then copy candidate to target, compute post-apply hash, run or record the verification command, and write `application.json` next to the approval receipt. If verification fails, keep the target changed but mark `status=applied_verification_failed` with explicit rollback instructions, unless a later task adds automatic rollback.

### 5. Dry-run is preflight, not a second policy path

**Choice:** `--dry-run` runs all validation and returns/writes a preflight receipt without backup or target mutation.

**Rationale:** Users need to inspect apply decisions before mutation, and tests need deterministic no-mutation behavior.

**Implementation:** Dry-run receipts use the same validators and include planned backup path, planned target hash transition, and verification command, with `applied=false`.

## Risks / Trade-offs

**Stale targets** → Mitigate with exact baseline hash guard and actionable stale-target errors.

**Rollback ambiguity** → Mitigate with backup path/hash and explicit restore command/instructions in `application.json`.

**Overbroad mutation** → Mitigate with first-pass `replace-file` only, target/candidate validation, and no directory or patch modes.

**Approval replay** → Mitigate with run id, target, candidate, approver, and `approval.applied=false` checks; application receipt records the approval receipt path and can be extended later with one-time-use ledgers.

## Validation Plan

- Unit-test apply validators for matching receipts, stale target hash, missing candidate, non-recommended run, failed-eval run, approval mismatch, and already-applied approval status.
- Unit-test dry-run preflight leaves target bytes unchanged and writes/returns `applied=false` receipt.
- Unit-test live replace-file creates backup, updates target bytes, records pre/post hashes, and writes `application.json`.
- Unit-test verification failure marks the application receipt as failed without hiding that target bytes changed.
- Add CLI parse/help tests for the apply action.
- Run focused self-evolution tests, MCP tests if session-control receipts are touched, binary check, OpenSpec validation, and `git diff --check`.
