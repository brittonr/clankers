## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `add-remote-external-memory-providers`.
- [x] Validate the OpenSpec package with `openspec validate add-remote-external-memory-providers --strict` and record any follow-up findings.

## Phase 2: Implementation

- [ ] Inventory current `external-memory-providers` code/docs seams and record the exact files to touch.
- [ ] Add typed policy/config/request/receipt models with unit tests.
- [ ] Implement the first runtime/adapter slice behind deterministic fake tests.
- [ ] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy.
- [ ] Update README and relevant docs for supported behavior, non-goals, and safety policy.

## Phase 3: Verification and Closeout

- [ ] Run targeted package/integration checks for the touched modules.
- [ ] Run `cargo check --tests` for affected crates.
- [ ] Run `git diff --check`.
- [ ] Sync the delta spec into the canonical `external-memory-providers` spec and archive the change after implementation tasks complete.
