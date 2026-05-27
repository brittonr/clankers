# Design: Harden review metrics regression rail

## Context

The accepted `cairn-review-gates` spec and `scripts/check-cairn-review-gates.rs` already cover several omission classes: deterministic request/stream/retry/redaction/receipt/discovery tasks, repeated Codex subcontracts, design-stage omissions, spec-stage omissions, and oracle checkpoint evidence. Current review metrics still surface high counts for the same family of failures, so the next rail should explicitly bind metrics summaries to fixtures and diagnostics.

## Goals / Non-Goals

**Goals:**

- Convert repeated, sanitized metrics categories into fixture-backed diagnostics.
- Make the evidence path reviewable without exposing raw prompts, credentials, provider payloads, or hidden transcript data.
- Keep the local checker and operator docs synchronized so future task/design/spec artifacts carry exact obligations.

**Non-goals:**

- Replacing Cairn/Cairn generic gate internals.
- Running live Codex/provider probes.
- Re-reviewing or rewriting historical archived changes.

## Decisions

### 1. Metrics snapshot is evidence, fixtures are enforcement

**Choice:** Store the repeated finding counts and representative sanitized examples under `evidence/metrics-snapshot-2026-05-23.md`, then enforce only through deterministic fixture directories.

**Rationale:** Metrics explain priority, but fixtures are what prevent regressions. Keeping enforcement in `scripts/fixtures/cairn-review-gates/**` avoids hidden dependency on local metrics logs.

**Implementation:** Future implementation tasks add one or more negative fixtures for the next unsupported repeated category and matching positive fixtures that prove the exact explicit task/design/spec wording passes.

### 2. Keep rules project-local first

**Choice:** Extend `scripts/check-cairn-review-gates.rs` before changing upstream Cairn/Cairn core.

**Rationale:** The existing checker sees proposal, design, spec, tasks, fixtures, docs, and flake wiring. Generic lifecycle validators may not receive all of that context.

### 3. Treat human/oracle metrics as evidence-contract work

**Choice:** Repeated `human` findings require explicit `H#` tasks and `Artifact-Type: oracle-checkpoint` evidence rather than prose-only closeout.

**Rationale:** Human findings usually mean the reviewer lacked a complete artifact or judgment point. A deterministic rail can require durable evidence metadata even when it cannot decide the human question.

## Risks / Trade-offs

- **Overfitting to historical examples** → Require category-level fixtures and actionable diagnostics, not one exact archived path.
- **False confidence from metrics alone** → Use metrics only to choose fixture categories; the checker remains deterministic and fixture-backed.
- **Secret leakage** → Evidence snapshots include counts/classes/sanitized file examples only and explicitly exclude raw prompts, provider payloads, account IDs, tokens, and credentials.

## Verification Plan

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-cairn-review-gates.rs`
- `mdbook build docs`
- `nix run .#cairn -- gate proposal harden-review-metrics-regression-rail --root .`
- `nix run .#cairn -- gate design harden-review-metrics-regression-rail --root .`
- `nix run .#cairn -- gate tasks harden-review-metrics-regression-rail --root .`
- `nix run .#cairn -- validate --root .`
- `git diff --check`
