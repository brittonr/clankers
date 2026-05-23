# Design: automated current-HEAD release evidence index

## Command shape

The developer-facing command is:

```bash
./scripts/test-harness.sh evidence-index
```

The harness delegates to a Rust cargo-script helper:

```bash
./scripts/check-current-head-release-evidence.rs --result-dir target/test-harness --out-dir target/release-evidence/current-head
```

The helper may expose `--allow-dirty` for development/testing, but the harness mode MUST NOT pass it. Default behavior fails closed on dirty tracked state.

## Functional core

The Rust helper owns deterministic parsing and validation:

- Discover repository metadata from `git`: branch, HEAD, upstream/ahead-behind summary, describe/tag distance, tracked dirty state, and active lifecycle directories.
- Discover harness receipt candidates from `<result-dir>/runs/*/results.json`.
- Parse receipt JSON into a small typed model for `mode`, `run_id`, timestamps, pass/fail/skip counts, and step log references.
- Accept only receipts with `failed == 0`, `passed > 0`, and all referenced logs/results/summary files present.
- Select the latest valid receipt per mode using `(finished_at, run_id)` deterministic ordering.
- Emit explicit `missing`/`unverified` entries for modes without current evidence instead of silently claiming coverage.

## Imperative shell

The script shell is limited to Git subprocess calls, filesystem traversal, and writing output artifacts. It writes:

- `index.json`: machine-readable schema `clankers.current_head_release_evidence_index.v1`.
- `index.md`: human-readable summary with payload HEAD, dirty/lifecycle status, selected receipts, and non-claims.

## Safety and non-claims

The rail is an index over available local receipts, not a replacement for running the harness modes. It MUST distinguish:

- selected passed local receipts;
- missing receipt classes;
- receipts whose payload commit cannot be proven because older harness receipts did not record HEAD;
- dirty checkout state.

## Acceptance checks

- Harness list/help documents `evidence-index`.
- Dry-run harness contract includes the delegated Rust script command.
- The script can be run with `--allow-dirty` in development to prove output wiring without overclaiming clean state.
- A clean post-commit smoke can run `./scripts/test-harness.sh evidence-index` and write target artifacts for the committed payload.
