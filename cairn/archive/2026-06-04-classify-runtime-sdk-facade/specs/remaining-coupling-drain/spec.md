## ADDED Requirements

### Requirement: Runtime facade classification is explicit [r[remaining-coupling-drain.runtime-facade-classification]]

`clankers-runtime` MUST be classified as yellow-only, a documented green-subset facade, or a split set of green/yellow owners before new runtime APIs are advertised as embedded SDK contracts.

#### Scenario: runtime exports have green yellow red owners [r[remaining-coupling-drain.runtime-facade-classification.owner-map]]
- GIVEN runtime facade public exports are reviewed
- WHEN classification validation runs
- THEN each exported runtime API group MUST be mapped to green reusable SDK, yellow app-edge integration, or red desktop-only ownership
- AND SDK docs and lego policy MUST agree with that classification

#### Scenario: classification gates promotion [r[remaining-coupling-drain.runtime-facade-classification.promotion-gate]]
- GIVEN a runtime API depends on provider/auth/plugin/process/prompt filesystem/session storage/desktop state
- WHEN it is considered for embedded SDK promotion
- THEN it MUST either move behind a green neutral owner or remain yellow app-edge with explicit host injection requirements

### Requirement: Runtime public API rail is real inventory [r[remaining-coupling-drain.runtime-public-api-rail]]

Runtime facade boundary checks MUST inventory actual public exports and dependencies rather than relying on a small hardcoded denied-name list.

#### Scenario: runtime API inventory catches leakage [r[remaining-coupling-drain.runtime-public-api-rail.leakage]]
- GIVEN `clankers-runtime` exposes public types, functions, traits, modules, or reexports
- WHEN the runtime public API rail runs
- THEN forbidden daemon, TUI, provider-native, desktop storage, process backend, global path, or hidden service lookup items MUST fail with owner diagnostics

#### Scenario: runtime API labels stay deterministic [r[remaining-coupling-drain.runtime-public-api-rail.deterministic]]
- GIVEN runtime classification changes
- WHEN receipt generation runs
- THEN public API labels, dependency summaries, and source hashes MUST be deterministic and included in reviewable evidence

### Requirement: Runtime defaults fail closed without ambient services [r[remaining-coupling-drain.runtime-fail-closed-defaults]]

Runtime facade services that require provider, auth, plugin, process, prompt filesystem, skill, session, or storage behavior MUST fail closed unless a host explicitly injects the required service.

#### Scenario: missing runtime services do not discover desktop state [r[remaining-coupling-drain.runtime-fail-closed-defaults.no-ambient]]
- GIVEN an embedded host creates runtime defaults without service injection
- WHEN provider, auth, plugin, process, prompt filesystem, skill, session, or storage behavior is requested
- THEN runtime MUST return a typed unavailable/unsupported error
- AND it MUST NOT probe global/project config, auth files, daemon sockets, plugin directories, or desktop session stores
