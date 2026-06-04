## Context

`crates/clankers-runtime/src/lib.rs` is a high-churn or safety-critical module whose current size makes review, parity testing, and future bug isolation harder than necessary.

## Goals / Non-Goals

**Goals:** Split responsibilities into named modules, keep pure policy/validation logic testable, preserve external behavior, and add parity or negative tests for the moved boundaries.

**Non-Goals:** Redesign product behavior, change user-facing defaults, add new provider/tool capabilities, or remove existing compatibility paths as part of the decomposition.

## Decisions

### 1. Decompose by responsibility, not by arbitrary line count

**Choice:** Extract modules around stable responsibilities and public seams rather than only trying to shrink the file mechanically.

**Rationale:** Clankers benefits when functional-core logic and imperative shells can be reviewed independently. Mechanical splits without ownership boundaries would make future drains harder.

**Alternative:** Leave the file in place and add comments. Rejected because the issue is review/test blast radius, not only readability.

**Implementation:** Keep a small root module that re-exports or orchestrates the new modules. Move tests with the behavior they cover and add compatibility tests when public imports are affected.

### 2. Preserve behavior before cleanup

**Choice:** Capture or identify existing parity/negative tests before moving code, then run the smallest relevant gate after each extraction.

**Rationale:** These seams touch runtime, session, provider, or safety behavior where silent drift is riskier than duplicated code during the transition.

**Alternative:** Rewrite the subsystem in one pass. Rejected as too broad and hard to verify.

## Risks / Trade-offs

**Import churn** → Mitigate with root re-exports and small compatibility tests.

**Test gaps around side effects** → Mitigate with caller-path tests that exercise commands/session/provider behavior, not only helper units.

**Overbroad refactor** → Keep behavior changes out of scope; if a bug is found, land a focused fix with a regression test.

## Validation Plan

- Strictly validate the OpenSpec change.
- Run targeted tests for the decomposed seam after each slice.
- Run `cargo check --tests` for crates whose public imports or callers changed.
- Run `git diff --check` before commit/archive.
