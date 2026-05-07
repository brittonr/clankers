## Phase 1: Specification foundation

- [x] [serial] Define shell-adapter parity matrix testing scope.

## Phase 2: Shared fixtures and shell runners

- [ ] [serial] Define shell parity axes for entrypoint, prompt source, store mode, confirmation outcome, disabled-tool policy, tool result class, model result class, and event translation.
- [ ] [depends:shell-axes] Add shared transcript/model/tool fixtures usable by standalone agent, controller/daemon adapter seams, and bounded embedded/batch paths.
- [ ] [depends:shell-fixtures] Add matrix runners or focused tests for each supported shell entrypoint.

## Phase 3: Verification rails

- [ ] [depends:shell-matrix] Extend FCIS/source-boundary rails to require matrix evidence for adapter-only ownership.
- [ ] [depends:shell-matrix] Wire shell parity matrix checks into embedded SDK or full decoupling acceptance as a bounded step.
- [ ] [serial] Run focused shell parity, FCIS boundary, and embedded SDK checks; archive, commit, and push.
