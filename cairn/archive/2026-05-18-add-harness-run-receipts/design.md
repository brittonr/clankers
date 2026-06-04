## Context

The harness currently writes directly to top-level receipt files and log directories. The previous slice added dry-run contract coverage, which gives us a safe regression surface for changing receipt layout while preserving top-level compatibility files.

## Goals / Non-Goals

**Goals:** Run-scoped primary receipt directories, compatibility copies after completion, and tests that prove run identity is reflected in JSON and summaries.

**Non-Goals:** Solving concurrent writers with locking; changing expensive gate contents; removing stable top-level receipt paths.

## Decisions

### 1. Use run-scoped primary artifacts plus compatibility copies

**Choice:** Store primary artifacts under `$CLANKERS_TEST_RESULT_DIR/runs/<run-id>/` and copy `summary.md`, `results.json`, and `junit.xml` back to `$CLANKERS_TEST_RESULT_DIR/` after report generation.

**Rationale:** Existing docs and tools can keep reading stable paths once a run finishes, while operators can use `run_dir` for immutable evidence.

**Alternative:** Replace stable paths with symlinks. Rejected for portability and because copies are simpler to consume in CI artifacts.

### 2. Allow deterministic run IDs in tests

**Choice:** Support `CLANKERS_TEST_RUN_ID` for tests and keep generated IDs for normal runs.

**Rationale:** Deterministic IDs make contract tests simple and avoid sleeping or parsing timestamps.

## Risks / Trade-offs

**Compatibility files can still be overwritten by a later completed run** → Mitigated by embedding `run_id`/`run_dir` and preserving immutable per-run directories.
