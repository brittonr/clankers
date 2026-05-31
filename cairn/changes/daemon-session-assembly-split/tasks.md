## Phase 1: Daemon assembly split

- [ ] [serial] I1: Inventory construction responsibilities still owned by `src/modes/daemon/agent_process.rs`, including agent builder setup, hook pipeline, capability gates, tool rebuilder, plugin projections, spawn planning, and keyed/ephemeral session paths. r[daemon-session-assembly-split.actor-loop.multiplexing-only] [covers=daemon-session-assembly-split.actor-loop.multiplexing-only]
- [ ] [serial] I2: Define a daemon session runtime bundle or builder output that contains the prepared controller, event channels, tool rebuilder, hook pipeline/capability decisions, and plugin projection handles consumed by the actor loop. r[daemon-session-assembly-split.assembly.bundle] [covers=daemon-session-assembly-split.assembly.bundle]
- [ ] [serial] I3: Move hook pipeline and capability gate construction out of `agent_process.rs` into socketless assembly helpers. r[daemon-session-assembly-split.assembly.hooks-capabilities] [covers=daemon-session-assembly-split.assembly.hooks-capabilities]
- [ ] [serial] I4: Move tool rebuilder and plugin summary/tool-list projection construction into named daemon assembly/projection modules while preserving live stdio plugin refresh behavior. r[daemon-session-assembly-split.tools-plugins] [covers=daemon-session-assembly-split.tools-plugins]
- [ ] [serial] I5: Route create/resume/keyed/ephemeral session spawn paths through the socketless builder output and leave `agent_process.rs` as actor loop plumbing. r[daemon-session-assembly-split.assembly] [covers=daemon-session-assembly-split.assembly]

## Phase 2: Verification

- [ ] [serial] V1: Add socketless builder fixtures for create, resume, keyed-session recovery, and ephemeral child-session assembly decisions. r[daemon-session-assembly-split.verification.socketless-builder] [covers=daemon-session-assembly-split.verification.socketless-builder]
- [ ] [serial] V2: Add focused daemon actor parity tests for plugin summary, tool-list refresh, disabled-tool rebuild, and keyed-session recovery after the assembly split. r[daemon-session-assembly-split.tools-plugins.live-refresh] [covers=daemon-session-assembly-split.tools-plugins.live-refresh]
- [ ] [serial] V3: Run focused daemon/session tests, attach parity tests affected by session recovery, lego/FCIS architecture rails, Cairn gates/validate, and `git diff --check`. r[daemon-session-assembly-split.verification.closeout] [covers=daemon-session-assembly-split.verification.closeout]
