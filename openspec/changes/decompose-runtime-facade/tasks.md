## Phase 1: Baseline and Extraction

- [ ] [serial] Inventory public types in clankers-runtime/src/lib.rs and group them into session, builder, services, tools, confirmation, prompt, and events modules.
- [ ] [depends:baseline] Move type groups into modules while preserving root re-exports and docs.
- [ ] [depends:baseline] Add or update API compatibility tests proving old root imports still compile.
- [ ] [depends:baseline] Run focused runtime/tool-host embedding tests and default-safe negative tests.
- [ ] [serial] Run cargo fmt, cargo nextest -p clankers-runtime, cargo check --tests for runtime/embedding dependents, openspec validate, and git diff --check.
