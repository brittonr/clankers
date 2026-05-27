## Phase 1: Scaffold and evidence

- [x] [serial] S1: Preserve a sanitized metrics snapshot for the repeated omission/incoherence classes driving this rail [r[cairn-review-gates.review-metrics-regression-rail.snapshot-selects-category]] [evidence=evidence/metrics-snapshot-2026-05-23.md]
- [x] [serial] S2: Define the metrics-regression delta spec, design, and verification plan before touching checker behavior [r[cairn-review-gates.review-metrics-regression-rail.secret-free-evidence]]

## Phase 2: Fixture-backed implementation

- [x] [serial] I1: Inspect `scripts/check-cairn-review-gates.rs`, existing fixtures, and `docs/src/reference/cairn-review-gates.md` to identify the highest-count unsupported metrics category [r[cairn-review-gates.review-metrics-regression-rail.snapshot-selects-category]]
- [x] [serial] I2: Add category-specific checker diagnostics plus at least one failing sanitized fixture and one passing sanitized fixture for the selected category [r[cairn-review-gates.review-metrics-regression-rail.fixture-backed-category]]
- [x] [parallel] I3: Update operator guidance so the new diagnostic tells authors the exact fixture/helper/command/evidence/oracle shape required [r[cairn-review-gates.review-metrics-regression-rail.guidance-and-wiring]]
- [x] [parallel] I4: Keep fixture/evidence content secret-free and limited to sanitized contract text, counts, classes, and safe examples [r[cairn-review-gates.review-metrics-regression-rail.secret-free-evidence]]

## Phase 3: Verification and archive

- [x] [serial] V1: Run `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-cairn-review-gates.rs` [r[cairn-review-gates.review-metrics-regression-rail.fixture-backed-category]]
- [x] [serial] V2: Run `mdbook build docs`, `nix run .#cairn -- gate proposal harden-review-metrics-regression-rail --root .`, `nix run .#cairn -- gate design harden-review-metrics-regression-rail --root .`, `nix run .#cairn -- gate tasks harden-review-metrics-regression-rail --root .`, `nix run .#cairn -- validate --root .`, and `git diff --check` [r[cairn-review-gates.review-metrics-regression-rail.guidance-and-wiring]]
