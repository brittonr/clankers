## ADDED Requirements

### Requirement: Daemon actor loop consumes assembled runtime [r[daemon-session-assembly-split.actor-loop]]

Daemon session actor loops MUST receive assembled runtime/controller inputs and MUST NOT construct unrelated tool, hook, plugin, capability, persistence, or child-session policies inline.

#### Scenario: Actor loop owns multiplexing only [r[daemon-session-assembly-split.actor-loop.multiplexing-only]]
- GIVEN `agent_process.rs` actor loop code is inspected
- WHEN the loop polls session commands, actor signals, confirmations, plugin UI events, and controller events
- THEN it MAY route commands, drain/broadcast events, and handle shutdown
- AND it MUST NOT construct hook pipelines, capability gates, tool catalogs, plugin managers, session stores, or child-session factories inline

### Requirement: Session assembly is socketless and explicit [r[daemon-session-assembly-split.assembly]]

Daemon session assembly MUST be performed by named builder/adapters that can be tested without binding Unix sockets or requiring a live actor registry.

#### Scenario: Assembly bundle contains runtime inputs [r[daemon-session-assembly-split.assembly.bundle]]
- GIVEN a daemon session is created, resumed, recovered by key, or spawned as an ephemeral child
- WHEN assembly completes
- THEN the resulting bundle MUST include the prepared controller/runtime inputs, capability ceiling, hook pipeline decision, tool rebuilder/projection handles, and event channels needed by the actor loop
- AND the bundle MUST be constructible in a socketless fixture

#### Scenario: Capability and hook policy are builder-owned [r[daemon-session-assembly-split.assembly.hooks-capabilities]]
- GIVEN session settings, UCAN/public auth, default capabilities, and plugin manager inputs are supplied
- WHEN assembly decides hooks and capabilities
- THEN merge, ceiling, hook registration, and plugin hook attachment policy MUST live in builder/adapters with focused tests
- AND actor loop code MUST consume the result rather than rebuilding the policy

### Requirement: Tool/plugin projections are edge-owned [r[daemon-session-assembly-split.tools-plugins]]

Daemon tool rebuilding and plugin summary/tool-list projection MUST live in named projection modules or assembly helpers, while the actor loop only triggers refresh/drain operations.

#### Scenario: Live plugin refresh remains behaviorally identical [r[daemon-session-assembly-split.tools-plugins.live-refresh]]
- GIVEN stdio plugin registrations or restarts happen while a daemon session is running
- WHEN the actor tick refreshes tool inventory or drains plugin UI frames
- THEN the daemon MUST emit the same `ToolList`, plugin status, notification, widget, and system-message events as before the split
- AND construction of the projection policy MUST be outside the actor loop

### Requirement: Daemon assembly verification is deterministic [r[daemon-session-assembly-split.verification]]

Verification MUST combine socketless assembly fixtures, actor parity tests, session recovery tests, and architecture rails.

#### Scenario: Socketless fixtures cover spawn decisions [r[daemon-session-assembly-split.verification.socketless-builder]]
- GIVEN create, resume, keyed-session, and ephemeral child-session inputs
- WHEN builder fixtures run
- THEN they MUST verify construction decisions without opening Unix sockets or requiring a running actor registry

#### Scenario: Closeout preserves daemon parity [r[daemon-session-assembly-split.verification.closeout]]
- GIVEN assembly responsibilities have moved out of the actor loop
- WHEN closeout validation runs
- THEN focused daemon/session tests, attach/keyed recovery parity tests, architecture rails, Cairn gates/validate, and diff checks MUST pass or include explicit checked evidence for environmental limitations
