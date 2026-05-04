## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `expand-context-references`.
- [x] Validate the OpenSpec package with `openspec validate expand-context-references --strict` and record any follow-up findings.

## Phase 2: Implementation

- [ ] Inventory current `context-references` code/docs seams and record the exact files to touch.
- [ ] Add typed policy/config/request/receipt models with unit tests.
- [ ] Implement the first runtime/adapter slice behind deterministic fake tests.
- [ ] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy.
- [ ] Update README and relevant docs for supported behavior, non-goals, and safety policy.

## Phase 3: Verification and Closeout

- [ ] Run targeted package/integration checks for the touched modules.
- [ ] Run `cargo check --tests` for affected crates.
- [ ] Run `git diff --check`.
- [ ] Sync the delta spec into the canonical `context-references` spec and archive the change after implementation tasks complete.
