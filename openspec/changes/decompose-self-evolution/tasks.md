## Phase 1: Baseline and Extraction

- [ ] [serial] Inventory self_evolution.rs responsibilities across run, approve, apply, rollback, receipt validation, candidate IO, hashes, and verification.
- [ ] [depends:baseline] Extract receipt models/validators and hash/path guards with negative tests.
- [ ] [depends:baseline] Extract run/approval/apply/rollback orchestration shells around pure validators.
- [ ] [depends:baseline] Preserve CLI dogfood and stale-target/rollback regression coverage.
- [ ] [serial] Run cargo fmt, self_evolution lib and CLI/integration tests, cargo check --tests -p clankers, openspec validate, and git diff --check.
