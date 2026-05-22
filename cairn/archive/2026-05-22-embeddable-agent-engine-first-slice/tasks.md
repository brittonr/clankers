## Tasks

- [x] [serial] I1: Define the embeddable engine facade and neutral DTO surface, including request, event, outcome, and receipt shapes that do not require root/TUI/daemon types [r[embeddable-agent-engine.neutral-engine-facade]] [r[embeddable-agent-engine.no-inward-display-or-protocol-leaks]]
- [x] [serial] I2: Define host-provided ports for model completion, tool execution, persistence/history, hooks, prompts, and cost accounting, delegating concrete I/O to shell adapters [r[embeddable-agent-engine.host-supplied-effect-ports]]
- [x] [parallel] I3: Add a deterministic fixture host that runs one minimal turn through fake model/tool ports without credentials, sockets, TUI state, or daemon protocol state [r[embeddable-agent-engine.fixture-host-proves-embeddability]]
- [x] [parallel] I4: Add architecture rails that fail when reusable engine modules import root-shell, TUI, daemon protocol, Matrix, or concrete provider/auth/router construction types directly [r[embeddable-agent-engine.no-inward-display-or-protocol-leaks]] [r[embeddable-agent-engine.engine-architecture-rails]]
- [x] [serial] I5: Migrate one existing Clankers shell path to call the engine facade through an adapter while preserving standalone/controller turn behavior [r[embeddable-agent-engine.shell-dogfoods-engine-api]]
- [x] [serial] V1: Run Cairn validate plus proposal/design/tasks gates for this package [r[embeddable-agent-engine.engine-architecture-rails]]
- [x] [serial] V2: Run the fixture host positive turn and one negative no-leak rail or compile/check fixture [r[embeddable-agent-engine.fixture-host-proves-embeddability]] [r[embeddable-agent-engine.no-inward-display-or-protocol-leaks]]
- [x] [serial] V3: Run focused parity checks for the migrated shell path and `git diff --check` before commit [r[embeddable-agent-engine.shell-dogfoods-engine-api]]
