## Context

`scripts/test-harness.sh` already emits JSON, Markdown, and JUnit receipts and supports `CLANKERS_TEST_DRY_RUN=1`. Existing release-readiness docs treat those receipts as the source of truth, so the receipt contract should be tested by nextest rather than manually inspected.

## Goals / Non-Goals

**Goals:** Fast deterministic tests for dry-run mode selection and receipt consistency.

**Non-Goals:** Running full, live, VM, or flake-heavy gates; redesigning the harness profile model; adding new third-party parser dependencies.

## Decisions

### 1. Use an integration test around the shell harness

**Choice:** Add a Rust integration test that invokes the real `scripts/test-harness.sh` with an isolated `CLANKERS_TEST_RESULT_DIR` and dry-run enabled.

**Rationale:** This tests the operator-facing wrapper and receipt files directly while staying fast and credential-free.

**Alternative:** Extract the harness into Rust first. Rejected for this slice because a contract test provides immediate regression coverage with less churn.

### 2. Validate JUnit structurally without a new XML dependency

**Choice:** Assert the generated XML contains the expected testsuite/testcase/failure/skipped elements and escaped step names.

**Rationale:** Existing dependencies are sufficient for this receipt-contract slice.

## Risks / Trade-offs

**Shell wrapper remains source of step selection** → Mitigated by nextest-owned dry-run assertions that catch drift.
