## Context

The current acceptance command passes with `CDPATH=` but fails when the user's shell exports `CDPATH`, because `cd` prints a path on stdout inside command substitution. The agent turn module also emits dead-code warnings during `cargo test -p clankers-agent --lib turn::tests::`, and `.drain-state.md` still contains stale pending-commit language.

## Goals / Non-Goals

**Goals:** make acceptance self-contained, keep test output warning-clean for the touched package, and make drain-state accurately report idle status.

**Non-Goals:** change engine semantics, remove adapter rails, or archive any implementation changes.

## Decisions

### 1. Sanitize script-local environment

**Choice:** set `CDPATH=` or otherwise suppress `cd` stdout inside `scripts/check-embedded-agent-sdk.sh` before computing `SCRIPT_DIR` and `REPO_ROOT`.

**Rationale:** the script should not require every caller to remember environment cleanup.

**Alternative:** document `CDPATH=` in the guide only. Rejected because the acceptance command is advertised as copy-paste runnable.

### 2. Prefer code deletion or cfg-scoped helpers over broad allows

**Choice:** remove unused helper functions if obsolete, or move them behind the tests/features that need them. Use narrow `#[allow(dead_code)]` only when a helper is deliberately retained for a near-term seam and the reason is local.

**Rationale:** decoupling rails should remain signal-rich; blanket suppressions hide regressions.

### 3. Treat drain-state as operator bookkeeping

**Choice:** update `.drain-state.md` to an idle/clean state after spec queue reviews.

**Rationale:** stale drain state can mislead future autonomous drains even though `openspec list` is clean.
