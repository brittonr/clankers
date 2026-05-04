## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `polish-mcp-tool-runtime`.
- [x] Validate the OpenSpec package with `openspec validate polish-mcp-tool-runtime --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `integrations-mcp` code/docs seams and record the exact files to touch.
- [x] Add typed policy/config/request/receipt models with unit tests.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules.
- [x] Run `cargo check --tests` for affected crates.
- [x] Run `git diff --check`.
- [x] Sync the delta spec into the canonical `integrations-mcp` spec and archive the change after implementation tasks complete.
