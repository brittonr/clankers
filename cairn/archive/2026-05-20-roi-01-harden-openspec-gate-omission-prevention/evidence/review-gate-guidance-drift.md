# OpenSpec Review Gate Guidance Drift Evidence

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-openspec-review-gates.rs
```

Result: PASS

Guidance/wiring assertions now covered by the checker:

- `openspec/AGENTS.md` documents deterministic verification task obligations for request shape, stream boundaries, retry policy, security/redaction policy, receipt fields, and discovery visibility.
- `openspec/AGENTS.md` documents concrete fixture/helper/command task wording and names `scripts/check-openspec-review-gates.rs`.
- `docs/src/reference/openspec-review-gates.md` documents the durable fixture root `scripts/fixtures/openspec-review-gates`, checker command, diagnostic names, and oracle checkpoint metadata fields.
- `flake.nix` exposes `checks.<system>.openspec-review-gates` and runs `scripts/check-openspec-review-gates.rs` through a Rust-owned derivation.
- Fixtures were moved out of the active change into `scripts/fixtures/openspec-review-gates` so the rail remains runnable after the OpenSpec change is archived.
