## Phase 1: Controller runtime boundary

- [ ] [serial] I1: Inventory `SessionController` concrete fields and classify them as command state, runtime adapter, persistence service, hook service, or projection edge. r[sdk-controller-runtime-boundary.inventory] [covers=sdk-controller-runtime-boundary.inventory]
- [ ] [serial] I2: Move one production prompt/control path to an injected `ControllerRuntimeAdapter` instead of direct `Agent` ownership/mutation. r[sdk-controller-runtime-boundary.runtime-adapter.production-injection] [covers=sdk-controller-runtime-boundary.runtime-adapter.production-injection]
- [ ] [parallel] I3: Narrow persistence/search access behind a controller/session service adapter or owner receipt. r[sdk-controller-runtime-boundary.persistence.service-owned] [covers=sdk-controller-runtime-boundary.persistence.service-owned]
- [ ] [parallel] I4: Keep daemon/TUI/protocol projection in conversion modules and update rails for any moved constructors. r[sdk-controller-runtime-boundary.projection.centralized] [covers=sdk-controller-runtime-boundary.projection.centralized]

## Phase 2: Verification

- [ ] [serial] V1: Add fake-runtime fixtures for prompt, cancel, thinking, disabled tools, resume identity, and semantic event projection without sockets or providers. r[sdk-controller-runtime-boundary.verification.fake-runtime] [covers=sdk-controller-runtime-boundary.verification.fake-runtime]
- [ ] [serial] V2: Add agent-backed parity tests showing desktop daemon behavior is unchanged for the migrated path. r[sdk-controller-runtime-boundary.verification.agent-parity] [covers=sdk-controller-runtime-boundary.verification.agent-parity]
- [ ] [serial] V3: Run controller focused tests, FCIS shell boundaries, lego architecture rail, Cairn gates/validate, and relevant daemon attach parity tests. r[sdk-controller-runtime-boundary.verification] [covers=sdk-controller-runtime-boundary.verification]
