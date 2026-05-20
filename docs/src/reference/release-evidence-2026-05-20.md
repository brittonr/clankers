# Release Evidence 2026-05-20

This bundle indexes the durable evidence for the `internal-readiness-2026-05-20` checkpoint. It is an evidence bundle for internal/trusted dogfooding readiness, not a public unattended production-readiness claim.

## Checkpoint identity

- Readiness tag: `internal-readiness-2026-05-20`
- Tagged commit: `84788aa78183b609b211983cc9258d653d26eea6`
- Tagged commit subject: `docs: add full harness evidence to readiness note`
- Tagged commit date: `2026-05-20 12:43:42 -0400`
- Evidence bundle generated: `2026-05-20T20:03:10Z`
- Source readiness note: `docs/src/reference/internal-readiness-2026-05-20.md`

## Repository state at evidence capture

- Clankers branch state: `## main...origin/main`
- Clankers worktree state before bundle capture: clean
- Active OpenSpec changes: none
- Current readiness scope: internal/trusted dogfood and embedded SDK readiness only

## Deterministic harness evidence

- Full harness summary: `target/test-harness/summary.md`
- Full harness results: `target/test-harness/results.json`
- Full harness run: `20260520T161137Z-1226432`
- Full harness status: `6` passed, `0` failed, `0` skipped
- Harness steps that passed:
  - `cargo fmt --check`
  - `cargo check --tests`
  - `cargo nextest run --workspace --no-fail-fast`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `./scripts/verify.sh`
  - `./xtask/tigerstyle.sh`

## Embedded SDK receipt evidence

- Receipt path: `target/embedded-sdk-release/receipt.json`
- Receipt schema: `clankers.embedded_sdk.release_receipt.v1`
- Receipt commit: `84788aa78183b609b211983cc9258d653d26eea6`
- Receipt branch state: `## main...origin/main`
- Hashed artifacts: `50`
- Required Rust-owned rail: `scripts/check-embedded-agent-sdk.rs`
- Routine Nix check: `checks.<system>.embedded-sdk-release-receipt`

## Canonical OpenSpec evidence

The canonical specs below validated strictly during evidence capture:

- `embedded-composition-kits`: valid
- `durable-process-jobs`: valid
- `openspec-review-gates`: valid

Post-bundle follow-up normalized legacy spec formatting and made the repo-wide strict OpenSpec gate green:

- Command: `openspec validate --all --strict --json`
- Result: `83` items, `0` invalid
- Evidence file: `/tmp/clankers-openspec-all-green.json`

## External-product dogfood evidence

Latest Remora external-product dogfood against this tagged Clankers checkpoint:

- State directory: `/home/brittonr/remora-operator-state/tile-clankers-tagged-20260520T184154Z`
- Status receipt: `/home/brittonr/remora-operator-state/tile-clankers-tagged-20260520T184154Z/review-schedule-last-status.json`
- Repository under review: `brittonr/tile`
- Change request: `#2`
- Mode: report-only execution with `--execute --reasoning-execute`
- Health: `ok`
- Runs: `1`
- OK: `1`
- Failed: `0`
- Findings: `0`
- Reasoning provider: `openai-codex`
- Reasoning model: `openai-codex/gpt-5.3-codex`
- Required fallback: `qwen36-aspen2`
- Primary attempts: `1`
- Fallback attempts: `0`
- Accepted primary: `1`
- Accepted fallback: `0`

Operator receipts in the state directory were clean:

- `dirty_state`: `clean`
- `verification_status`: `success`
- `commit_status`: `success`
- `push_status`: `success`
- `missing_receipts`: `false`
- `unverified_delegated_work`: `false`
- `dirty_after_mutation`: `false`
- `budget_exceeded_mutation_attempt`: `false`

## Evidence interpretation

This checkpoint is suitable for internal/trusted dogfooding and embedded SDK readiness claims because it combines:

1. a clean tagged Clankers commit;
2. a fresh full deterministic harness pass;
3. a regenerated embedded SDK receipt for the tagged commit;
4. strict validation of the canonical readiness-related OpenSpec specs;
5. a successful external-product Remora dogfood run with native primary/fallback proof fields; and
6. clean operator receipts for the dogfood state.

It does not claim unattended public production readiness. Host-dependent live/VM/flake rails are optional additional evidence; the 2026-05-20 follow-up runs below passed after the release bundle was first written.

## Optional live/VM/flake readiness evidence

Additional host-dependent readiness rails were run after the deterministic bundle and are recorded separately because they depend on local services, Nix builders, and VM availability:

- Live local-model rail: `./scripts/test-harness.sh live aspen2-qwen36`
  - Run: `20260520T201602Z-2329773`
  - Result: `1` passed, `0` failed, `0` skipped
  - Log: `target/test-harness/runs/20260520T201602Z-2329773/logs/live_readiness_aspen2-qwen36.log`
- VM rail: `./scripts/test-harness.sh vm all`
  - Run: `20260520T201943Z-2415288`
  - Result: `1` passed, `0` failed, `0` skipped
  - Log: `target/test-harness/runs/20260520T201943Z-2415288/logs/vm_readiness_all.log`
- Flake CI rail: `./scripts/test-harness.sh ci`
  - Initial run `20260520T203921Z-2789216` exposed a real Nix source-filter issue in the `fmt` check: the sandboxed `cargo fmt --check` could not resolve the optional sibling `../../../ucan` manifest path.
  - `flake.nix` now runs the fmt check from a writable source copy with a minimal sibling `ucan` manifest so Cargo metadata can evaluate the workspace without requiring the out-of-tree checkout.
  - Rerun: `20260520T205229Z-3068420`
  - Result: `1` passed, `0` failed, `0` skipped
  - Log: `target/test-harness/runs/20260520T205229Z-3068420/logs/flake_readiness.log`
