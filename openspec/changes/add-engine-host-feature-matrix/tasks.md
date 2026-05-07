## Phase 1: Specification foundation

- [x] [serial] Define the engine/host feature-matrix testing scope.

## Phase 2: Matrix fixture and runner

- [ ] [serial] Define matrix axes for model mode, stop reason, tool behavior, retry behavior, cancellation timing, usage observation, stream validity, and request budget.
- [ ] [depends:matrix-axes] Add engine/host matrix fixtures with pairwise coverage and critical triples.
- [ ] [depends:matrix-axes] Add a deterministic matrix runner that reports case IDs, axis values, and assertion failures.

## Phase 3: Verification rails

- [ ] [depends:matrix-runner] Add a freshness checker that fails when an axis value or critical interaction has no executed case.
- [ ] [depends:matrix-runner] Wire the matrix into `scripts/check-embedded-agent-sdk.sh` or a called sub-check.
- [ ] [serial] Run focused engine, engine-host, and embedded SDK acceptance checks; archive, commit, and push.
