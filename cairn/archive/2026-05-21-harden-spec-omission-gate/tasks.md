# Tasks

- [x] S1 [serial] r[cairn-review-gates.spec-stage-omission-prevention.spec-categories] Add spec-stage omission categories and diagnostics to the review-gate checker. Completed 2026-05-21T16:02:57Z; `scripts/check-cairn-review-gates.rs` now emits `missing-omitted-provider-default-spec`, `missing-malformed-account-claim-spec`, and `missing-provider-scoped-status-spec`.
- [x] S2 [serial] r[cairn-review-gates.spec-stage-omission-prevention.spec-fixtures] Add negative and positive sanitized fixtures for missing spec requirements. Completed 2026-05-21T16:02:57Z; added `negative-spec-specific-omissions` and `positive-spec-specific-coverage` fixtures.
- [x] S3 [serial] r[cairn-review-gates.spec-stage-omission-prevention.spec-docs] Document spec-stage diagnostics and operator guidance. Completed 2026-05-21T16:02:57Z; updated `docs/src/reference/cairn-review-gates.md`.
- [x] V1 [serial] r[cairn-review-gates.spec-stage-omission-prevention.spec-fixtures] Verify with `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-cairn-review-gates.rs`. Completed 2026-05-21T16:02:57Z; fixture runner passed.
- [x] V2 [serial] r[cairn-review-gates.spec-stage-omission-prevention.spec-docs] Verify with `cargo fmt --check`, `mdbook build docs`, Cairn gates, Cairn validate, and diff checks. Completed 2026-05-21T16:02:57Z; all focused checks passed before archive.
