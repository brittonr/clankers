## Phase 1: Agent ports and service bundle

- [ ] [serial] I1: Define an agent runtime service bundle that covers model execution, tool registry, storage, prompt/context, hooks, skills, cost, and cancellation without exposing concrete provider/db/TUI types to turn policy. [covers=r[agent-runtime-shell.service-bundle.explicit-ports]]
- [ ] [serial] I2: Migrate turn execution helpers to read from the service bundle and engine-host adapters instead of directly reaching provider, database, prompt, skill, hook, or TUI DTO implementations. [covers=r[agent-runtime-shell.turn-policy.port-owned]]
- [ ] [parallel] I3: Keep `Agent::new` and desktop construction as compatibility shells that assemble the service bundle at the app edge. [covers=r[agent-runtime-shell.compatibility.desktop-shell]]
- [ ] [serial] I4: Update architecture rails to report every remaining concrete `clankers-agent` dependency with owner, adapter module, and removal/convergence condition. [covers=r[agent-runtime-shell.dependency-budget.owner-receipts]]

## Phase 2: Verification

- [ ] [parallel] V1: Add fake-service agent turn tests proving a turn can run without live provider/router/auth, database/session store, prompt bundles, skills directories, or TUI DTO construction. [covers=r[agent-runtime-shell.verification.fake-service-turn]]
- [ ] [serial] V2: Run agent turn fixture parity, controller parity where affected, dependency ownership rail, Cairn validate/gates, and `git diff --check`. [covers=r[agent-runtime-shell.verification.closeout]]
