## Context

`review_metrics_promotions` for `/home/brittonr/git/clankers` reports high-count repeated OpenSpec findings, led by task omissions (`491` examples), deterministic-check omissions (`151` examples), and task/prompt traceability omissions (`132` examples). `openspec/AGENTS.md` already documents how repeated `human`/`oracle` findings should be handled, but the workflow still relies too much on reviewers noticing the same patterns late.

## Goals / Non-Goals

**Goals:**
- Turn repeated review-metrics classes into deterministic fixtures or template requirements.
- Make task-stage omissions fail before implementation work depends on incomplete ledgers.
- Preserve explicit `H#` oracle evidence for cases that need human judgment.

**Non-Goals:**
- Perfect semantic proof that every task satisfies every requirement.
- Retroactive repair of all archived changes.
- Runtime behavior changes outside OpenSpec tooling and guidance.

## Decisions

### Decision 1: Metrics-derived fixtures drive the first rail

**Choice:** Start from sanitized repeated review-metrics examples and encode representative false-negative cases as fixtures.

**Rationale:** The highest ROI is preventing already-observed failures. Metrics give concrete contracts: missing stream boundaries, missing retry counts, missing active-account auth checks, and absent deterministic entitlement-probe fixtures.

**Alternative:** Build a broad natural-language semantic checker first. Rejected because it is harder to verify and more likely to produce noisy findings.

**Implementation:** Add fixture files or inline fixture cases with minimal proposal/design/spec/tasks snippets and expected diagnostics. The implementation may be Rust-owned or an existing repo script, but diagnostics must be deterministic and safe to run locally.

### Decision 2: Task traceability is contract-oriented, not keyword-count oriented

**Choice:** The rail should look for explicit task coverage of contract categories that the artifacts themselves name: request shape, stream boundaries, retry policy, redaction/security policy, receipt fields, discovery visibility, and oracle checkpoints.

**Rationale:** Repeated findings are usually not "no task exists"; they are "the task is too vague to prove the required contract." The check must reward concrete fixtures/commands and reject generic testing phrases.

**Alternative:** Require one task per scenario mechanically. Rejected because some tasks legitimately cover multiple scenarios when they name a concrete shared fixture/evidence path.

### Decision 3: Oracle checkpoints stay explicit and checked in

**Choice:** Human-routed repeated findings require `H#` tasks plus `Artifact-Type: oracle-checkpoint` evidence under the change.

**Rationale:** This matches `openspec/AGENTS.md` and prevents prose-only closeout from erasing review context.

**Alternative:** Allow final summaries to serve as oracle evidence. Rejected because summaries are not durable, structured, or independently reviewable.

## Risks / Trade-offs

**False positives** → Keep the first implementation fixture-backed and limited to high-confidence repeated classes.

**Template drift** → Add a cheap drift check that keeps guidance, fixtures, and diagnostics aligned.

**Overhead for small changes** → Allow small changes to satisfy the rail with a concrete focused command/fixture rather than heavyweight evidence bundles.

## Validation Plan

- Add positive and negative fixtures for at least: vague deterministic-check task, missing stream/retry boundary task, concrete fixture-backed task, missing oracle checkpoint, and valid oracle checkpoint.
- Run the affected gate/check command and save/quote diagnostics in task closeout.
- Run `openspec validate roi-01-harden-openspec-gate-omission-prevention --strict --json`.
- Run `git diff --check`.
