## Phase 1: Baseline and Extraction

- [ ] [serial] Inventory current responsibilities in turn/mod.rs and record baseline behavior fixtures.
- [ ] [depends:baseline] Extract pure turn-state/policy helpers for phase transitions, retry/cancellation decisions, and tool-feedback routing.
- [ ] [depends:baseline] Extract provider streaming and completion request adaptation behind focused modules with parity tests.
- [ ] [depends:baseline] Extract transcript/session/usage side-effect shells and run targeted turn-loop plus agent tests.
- [ ] [serial] Run cargo fmt, targeted nextest filters for turn/agent, cargo check --tests for touched crates, openspec validate, and git diff --check.
