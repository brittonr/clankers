## Phase 1: Adapter ownership

- [ ] [serial] I1: Classify root crate dependencies as CLI parsing, desktop service construction, mode dispatch, or temporary compatibility, and move reusable policy out of root modules where found. [covers=r[root-controller-runtime-adapters.root-shell.composition-only]]
- [ ] [serial] I2: Define controller runtime/session service interfaces so command lifecycle and event projection can be tested without concrete provider, database, config, protocol, or TUI construction. [covers=r[root-controller-runtime-adapters.controller-shell.service-interfaces]]
- [ ] [parallel] I3: Migrate one controller prompt/control path to fake runtime/session services while preserving daemon/local/remote attach behavior. [covers=r[root-controller-runtime-adapters.controller-shell.fake-service-path]]
- [ ] [serial] I4: Update architecture rails to emit root/controller dependency budget receipts with owner, adapter module, and convergence condition. [covers=r[root-controller-runtime-adapters.dependency-budget.owner-receipts]]

## Phase 2: Verification

- [ ] [parallel] V1: Add controller fake-service fixtures for prompt submission, cancellation, thinking/disabled-tools control, session identity, and semantic event projection without sockets or TUI state. [covers=r[root-controller-runtime-adapters.verification.controller-fixtures]]
- [ ] [serial] V2: Run daemon/attach parity fixtures, FCIS boundary rail, dependency ownership rail, Cairn validate/gates, and `git diff --check`. [covers=r[root-controller-runtime-adapters.verification.closeout]]
