## Tasks

- [x] [serial] I1: Inspect existing project-local review-gate rail, fixtures, guidance, and accepted Cairn spec [r[cairn-review-gates.deterministic-verification-tasks.repeated-subcontract-omission]]
- [x] [serial] I2: Add subcontract-specific diagnostics for default/override request shape, active account persistence, entitlement probe retries, and tool-call delta stream boundaries [r[cairn-review-gates.deterministic-verification-tasks.repeated-subcontract-omission]]
- [x] [parallel] I3: Add negative and positive sanitized fixtures proving generic tasks fail and explicit fixture/helper/command tasks pass for those subcontracts [r[cairn-review-gates.deterministic-verification-tasks.subcontract-fixture-task]]
- [x] [parallel] I4: Update operator guidance so future task ledgers carry the exact subcontract names, not only broad request/SSE/retry labels [r[cairn-review-gates.deterministic-verification-tasks.subcontract-fixture-task]]
- [x] [serial] V1: Run `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-cairn-review-gates.rs` [r[cairn-review-gates.deterministic-verification-tasks.repeated-subcontract-omission]]
- [x] [serial] V2: Run `cargo fmt --check`, `mdbook build docs`, `nix run .#cairn -- gate proposal harden-task-omission-gate --root .`, `nix run .#cairn -- gate design harden-task-omission-gate --root .`, `nix run .#cairn -- gate tasks harden-task-omission-gate --root .`, `nix run .#cairn -- validate --root .`, and `git diff --check` [r[cairn-review-gates.deterministic-verification-tasks.subcontract-fixture-task]]
