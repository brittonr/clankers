## Phase 1: Baseline and Extraction

- [x] [serial] Inventory public types in `clankers-runtime/src/lib.rs` and group them into session, builder, services, tools, confirmation, prompt, and events modules. ✅ events group extracted first (completed: 2026-05-12T22:23:14Z)
- [x] [depends:baseline] Extract event/metadata/status types into `src/events.rs` while preserving root re-exports and docs. ✅ `events` module added with root compatibility re-exports (completed: 2026-05-12T22:23:14Z)
- [x] [depends:baseline] Add API compatibility tests proving old root event imports still compile. ✅ `crates/clankers-runtime/tests/api_compat.rs` covers root/module event paths (completed: 2026-05-12T22:23:14Z)
- [x] [depends:baseline] Run focused runtime/tool-host embedding tests and default-safe negative tests. ✅ `cargo nextest run -p clankers-runtime --no-fail-fast` passed 27/27; `cargo check --tests -p clankers-runtime -p clankers` passed in dev shell (completed: 2026-05-12T22:23:14Z)
- [ ] [depends:baseline] Move remaining type groups into modules while preserving root re-exports and docs.
- [ ] [serial] Run final cargo fmt, cargo nextest -p clankers-runtime, cargo check --tests for runtime/embedding dependents, openspec validate, and git diff --check after all module groups are extracted.
