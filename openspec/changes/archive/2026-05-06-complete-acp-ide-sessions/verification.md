# Verification: complete-acp-ide-sessions

## Inventory

Touched seams for the ACP foreground stdio session slice:

- `src/modes/acp.rs` — typed ACP request/error/session/prompt/capability models, safe receipts, structured unsupported/missing-session errors, and unit coverage.
- `tests/acp_ide_integration.rs` — foreground JSON-line integration coverage for initialize, prompt receipt, and unsupported editor surfaces.
- `README.md` — operator-facing ACP support/non-goal documentation.
- `docs/src/getting-started/quickstart.md` — quickstart ACP capability notes.
- `docs/src/reference/daemon.md` — daemon/ACP editor session notes.
- `docs/src/reference/request-lifecycle.md` — request lifecycle notes for ACP prompt receipts and unsupported surfaces.

## Drain Verification Matrix

| Rail | Command | Status | Scope rationale |
| --- | --- | --- | --- |
| Format | `cargo fmt` | PASS | Normalized Rust formatting before focused verification. |
| Unit | `CARGO_TARGET_DIR=target cargo test --lib acp -- --nocapture` | PASS — 11 passed | Covers ACP method validation, capabilities, safe metadata, prompt/session receipts, and CLI ACP parsing. |
| Integration | `CARGO_TARGET_DIR=target cargo test --test acp_ide_integration -- --nocapture` | PASS — 3 passed | Exercises foreground JSON-line initialize, prompt, and unsupported editor request paths. |
| Compile | `CARGO_TARGET_DIR=target cargo check --tests` | PASS | Builds affected crate and test targets after ACP model/API changes. |
| OpenSpec | `openspec validate complete-acp-ide-sessions --strict` | PASS | Validates active delta before archive. |
| Whitespace | `git diff --check` | PASS | Ensures no whitespace/EOF issues before archive. |
