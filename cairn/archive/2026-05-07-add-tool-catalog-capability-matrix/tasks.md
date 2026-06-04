## Phase 1: Specification foundation

- [x] [serial] Define the tool catalog capability-matrix testing scope.

## Phase 2: Matrix fixture and instrumentation

- [x] [serial] Define catalog matrix axes for pack set, disabled filter, custom tool registration, collision policy, extension runtime availability, and side-effect class.
- [x] [depends:catalog-axes] Add fake extension runtime instrumentation for descriptor publication, execution attempts, and startup side effects.
- [x] [depends:catalog-axes] Add matrix cases covering read-only/default/dangerous/custom/extension combinations and explicit exclusions.

## Phase 3: Verification rails

- [x] [depends:catalog-matrix] Add a freshness checker for catalog axes and critical pack combinations.
- [x] [depends:catalog-matrix] Wire catalog matrix checks into embedded SDK acceptance.
- [x] [serial] Run focused catalog tests and embedded SDK acceptance; archive, commit, and push.
