## Phase 1: Baseline and Extraction

- [ ] [serial] Inventory attach.rs responsibilities and identify test seams for session resolution, local commands, event projection, and client loop.
- [ ] [depends:baseline] Move session resolution/socket connection code into an attach session module with existing and negative tests.
- [ ] [depends:baseline] Move local slash/semantic command handling into an attach command module shared by parity tests.
- [ ] [depends:baseline] Move daemon-event projection/client loop helpers into focused modules and preserve TUI snapshot/recovery tests.
- [ ] [serial] Run cargo fmt, attach/MCP parity nextest filters, cargo check --tests -p clankers, openspec validate, and git diff --check.
