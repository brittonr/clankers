# Current-HEAD Release Evidence Index 2026-05-27

This page indexes the fresh current-HEAD full harness receipt for internal Clankers readiness review after native process restart support landed. It records local generated receipt paths for operator inspection; it does not publish raw logs, move readiness tags, or claim public unattended-production readiness.

## Payload binding

- Evaluated payload commit: `23fa07f2f0005d931d0bbe025ec33a447d4d86fb`
- Evaluated payload subject: `Track restarted native processes`
- Evaluated payload branch/upstream: `main` / `origin/main`
- Evaluated payload describe: `internal-readiness-2026-05-26-dogfood-full-10-g23fa07f2`
- Evaluated payload tracked dirty: `false`
- Evaluated payload ahead/behind at capture time: `3\t0`
- Evidence-recording commit: this checked-in page is committed after the harness run and is not itself the evaluated payload.

## Fresh full harness receipt

- Mode: `full`
- Run id: `20260527T050749Z-3639565`
- Started: `2026-05-27T05:07:49Z`
- Finished: `2026-05-27T05:20:23Z`
- Result: 8 passed / 0 failed / 0 skipped
- Results JSON: `target/test-harness/runs/20260527T050749Z-3639565/results.json`
- Summary: `target/test-harness/runs/20260527T050749Z-3639565/summary.md`
- Log directory: `target/test-harness/runs/20260527T050749Z-3639565/logs/`
- Latest aliases at capture time: `target/test-harness/results.json`, `target/test-harness/summary.md`, `target/test-harness/junit.xml`

Passed steps:

- `cargo fmt check` — `target/test-harness/runs/20260527T050749Z-3639565/logs/cargo_fmt_check.log`
- `cargo check tests` — `target/test-harness/runs/20260527T050749Z-3639565/logs/cargo_check_tests.log`
- `cargo nextest workspace` — `target/test-harness/runs/20260527T050749Z-3639565/logs/cargo_nextest_workspace.log`
- `cargo clippy` — `target/test-harness/runs/20260527T050749Z-3639565/logs/cargo_clippy.log`
- `repo verify` — `target/test-harness/runs/20260527T050749Z-3639565/logs/repo_verify.log`
- `tigerstyle` — `target/test-harness/runs/20260527T050749Z-3639565/logs/tigerstyle.log`
- `live readiness aspen2-qwen36` — `target/test-harness/runs/20260527T050749Z-3639565/logs/live_readiness_aspen2-qwen36.log`
- `dogfood bg-process-tui` — `target/test-harness/runs/20260527T050749Z-3639565/logs/dogfood_bg-process-tui.log`

## Generated local evidence index

The checked-in helper generated a local index under `target/release-evidence/current-head/`:

- Markdown index: `target/release-evidence/current-head/index.md`
- JSON index: `target/release-evidence/current-head/index.json`
- Full receipt selected there: `20260527T050749Z-3639565`
- Full receipt payload verification: `payload_commit_verified=true`
- Full results hash in generated index: `2b772bfacc94769d62f8ee08bdfc281a83463598b4b05c523e90fe4a08e1a5f1`
- Full summary hash in generated index: `cf5c9c68e5e7e16e4fca42d24c3378c62e9eb7a17678806b127be709cd4f452d`

The helper run itself was recorded as mode `evidence-index` with run id `20260527T052312Z-3939826`; that run verifies the index-generation rail only and is not a replacement for the full harness receipt above.

## Readiness tag boundary

- Existing readiness tag: `internal-readiness-2026-05-26-dogfood-full`
- Tag object: `cc04004bfd7f6e6a45e3164c2f7890d71f6ee985`
- Peeled tagged payload: `ccec74b659dc588934378aed34638b333304695f`
- Tag movement: not moved by this evidence-index slice.
- Boundary note: the fresh evaluated payload `23fa07f2f0005d931d0bbe025ec33a447d4d86fb` is ten commits after the existing readiness tag; moving or creating another readiness tag is intentionally deferred to a separate operator decision.

## Non-claims

- This page indexes local generated evidence and receipt paths; it does not commit raw `target/test-harness/**` logs.
- It does not claim missing historical harness profiles as fresh passes; the generated index still reports stale/missing non-full modes separately.
- It does not claim public unattended-production readiness.
- It does not move or replace `internal-readiness-2026-05-26-dogfood-full`.
