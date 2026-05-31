Evidence-ID: attach-parity-rail-hardening
Task-ID: V2
Artifact-Type: command-log
Covers: remaining-coupling-drain.architecture-rail-hardening
Status: pass

# Attach Parity Rail Hardening

Implementation summary:

- Replaced most `tests/attach_parity_docs.rs` exact source-string anchors with a small `syn` source inventory over structs, functions, methods, paths, call paths, method calls, and use paths.
- Kept exact source-order checks only for the disabled-tools ordering contract and documented the `matches!` macro literal fallback where `syn` does not parse macro bodies as expressions.
- Added root dev-dependency `syn = { version = "2", features = ["full", "visit"] }` for this typed architecture rail.

Commands run from `/home/brittonr/git/clankers`:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run --test attach_parity_docs
status: 0
Summary: 4 tests run: 4 passed, 0 skipped

rustfmt --check tests/attach_parity_docs.rs
status: 0
```
