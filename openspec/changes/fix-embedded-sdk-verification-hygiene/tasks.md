## Phase 1: Specification foundation

- [x] [serial] Define reproducible embedded SDK verification hygiene scope.

## Phase 2: Script and warning cleanup

- [ ] [serial] Patch `scripts/check-embedded-agent-sdk.sh` so it computes paths correctly with `CDPATH` set.
- [ ] [serial] Remove or narrowly justify the `clankers-agent` turn dead-code warnings seen in focused tests.
- [ ] [serial] Reset `openspec/changes/.drain-state.md` to an accurate idle state after the active queue is drained.

## Phase 3: Verification and closeout

- [ ] [depends:script-cleanup] Capture a negative-before/positive-after `CDPATH` reproduction for the script path computation.
- [ ] [depends:script-cleanup] Run `CDPATH=/tmp scripts/check-embedded-agent-sdk.sh` and the normal embedded SDK acceptance command.
- [ ] [depends:warning-cleanup] Run `cargo test -p clankers-agent --lib turn::tests::` and verify no new warnings from the touched turn helpers.
- [ ] [serial] Validate OpenSpec, archive, commit, and push.
