## ADDED Requirements

### Requirement: Current hotspot roadmap is traceable [r[coupling-hotspot-remediation.current-hotspot-roadmap]]

Clankers MUST track the current coupling-hotspot remediation work as explicit, traceable architecture requirements before implementation changes begin.

#### Scenario: all identified hotspots have requirements [r[coupling-hotspot-remediation.current-hotspot-roadmap.all-hotspots-covered]]
- GIVEN the architecture review identifies config/TUI, agent/concrete-system, tool-catalog, controller/protocol, daemon/session-builder, slash-command, provider/router, runtime/process-job, and root-reexport coupling
- WHEN the remediation Cairn package is reviewed
- THEN each hotspot MUST have a named requirement, implementation task, and verification task
- AND future implementation changes MUST preserve or refine these IDs instead of introducing untracked architecture cleanup

### Requirement: Config stays independent from TUI rendering [r[coupling-hotspot-remediation.config-tui-boundary]]

`clankers-config` MUST own configuration schemas, defaults, validation, and path resolution without depending on TUI rendering crates or terminal color/keymap constructors.

#### Scenario: theme config projects at the display edge [r[coupling-hotspot-remediation.config-tui-boundary.theme-projection]]
- GIVEN a theme is loaded from user configuration
- WHEN code needs a `clankers-tui` theme or `ratatui` color
- THEN the projection MUST occur in a TUI-owned adapter or product-shell edge
- AND `clankers-config` MUST expose only data-only theme selections or schema values

#### Scenario: keybinding config is data-only [r[coupling-hotspot-remediation.config-tui-boundary.keybinding-data]]
- GIVEN keybindings are loaded from settings
- WHEN they are converted into runtime action registries or TUI keymaps
- THEN config code MUST NOT import TUI rendering modules
- AND headless config tests MUST run without constructing terminal UI types

### Requirement: Agent depends on turn ports instead of concrete systems [r[coupling-hotspot-remediation.agent-port-boundary]]

`clankers-agent` MUST express turn orchestration through explicit ports or narrow adapters for model calls, tool execution, prompt assembly, storage, hooks, skills, usage/cost tracking, and runtime services.

#### Scenario: fake ports can run a turn fixture [r[coupling-hotspot-remediation.agent-port-boundary.fake-turn]]
- GIVEN a focused turn-loop test uses deterministic fake ports
- WHEN the agent executes a user prompt, model response, tool call, and completion
- THEN the test MUST NOT construct concrete provider/router/auth/db/TUI state
- AND the externally observed agent events MUST remain compatible with the pre-migration fixture

#### Scenario: TUI and DB concerns do not drive turn decisions [r[coupling-hotspot-remediation.agent-port-boundary.no-ui-db-policy]]
- GIVEN turn orchestration chooses model calls, tool calls, retries, compaction, or continuation
- WHEN the decision is made
- THEN the decision MUST be based on neutral turn state and port responses
- AND concrete TUI progress types or database storage DTOs MUST NOT be required for the decision

### Requirement: Tool catalog construction has named owners [r[coupling-hotspot-remediation.tool-catalog-boundary]]

Built-in, optional, plugin, daemon-only, and extension/runtime tools MUST be registered through capability-specific catalog or factory owners rather than one monolithic constructor function.

#### Scenario: tool family ownership is inspectable [r[coupling-hotspot-remediation.tool-catalog-boundary.family-owners]]
- GIVEN the tool inventory rail runs
- WHEN it inspects registered tools
- THEN each tool MUST report a family owner such as core, orchestration, daemon-session, matrix, plugin, or extension-runtime
- AND diagnostics MUST identify the owning factory for new or misplaced tools

#### Scenario: mode-specific wiring stays at mode edges [r[coupling-hotspot-remediation.tool-catalog-boundary.mode-edges]]
- GIVEN standalone, daemon, attach, or headless mode builds a tool set
- WHEN mode-specific channels or handles are needed
- THEN the mode edge MUST pass those dependencies to the relevant factory
- AND unrelated tool families MUST NOT receive or depend on that mode-specific state

### Requirement: Controller separates domain policy from protocol DTOs [r[coupling-hotspot-remediation.controller-protocol-boundary]]

`clankers-controller` MUST handle session domain policy using neutral commands, outcomes, and events before projecting to daemon wire types such as `SessionCommand` and `DaemonEvent`.

#### Scenario: domain command handling is transport-free [r[coupling-hotspot-remediation.controller-protocol-boundary.domain-input]]
- GIVEN a prompt, abort, thinking-level, disabled-tool, compaction, loop, or auto-test request arrives
- WHEN controller domain handling is tested
- THEN the test MUST NOT require a Unix socket, QUIC stream, TUI `App`, or raw daemon frame
- AND protocol-specific DTO construction MUST happen through a projection adapter

#### Scenario: protocol projection is centralized [r[coupling-hotspot-remediation.controller-protocol-boundary.protocol-projection]]
- GIVEN controller domain outcomes need to reach a daemon or attach client
- WHEN they are converted to wire events
- THEN the conversion MUST happen through named protocol projection modules
- AND domain policy modules MUST NOT directly construct behavior-specific daemon events inline

### Requirement: Daemon control socket uses a session-builder seam [r[coupling-hotspot-remediation.daemon-session-builder-boundary]]

Daemon control socket code MUST frame control requests and responses while delegating session construction, resume resolution, actor spawn inputs, registry payloads, and tool rebuild setup to a session-builder seam.

#### Scenario: create/resume can be tested without a socket [r[coupling-hotspot-remediation.daemon-session-builder-boundary.socketless-builder]]
- GIVEN create-session or resume-session inputs
- WHEN the session builder is tested
- THEN it MUST produce session id, model/system prompt selection, seed messages, actor inputs, initial commands, and registry payloads without opening a Unix socket
- AND socket code MUST only send the projected control response after builder success or failure

#### Scenario: keyed sessions share the same builder [r[coupling-hotspot-remediation.daemon-session-builder-boundary.keyed-builder]]
- GIVEN Matrix, chat, remote, or local daemon paths need a keyed session
- WHEN a session is recovered or created
- THEN the same session-builder seam MUST own revive-in-place behavior and actor spawn inputs
- AND transport-specific handlers MUST NOT fork separate recovery logic

### Requirement: Slash commands produce declarative effects [r[coupling-hotspot-remediation.slash-effect-boundary]]

Slash command handlers MUST parse command input and return declarative effects instead of directly mutating TUI app state, plugin managers, DB/session managers, and agent command channels.

#### Scenario: effect interpreter is shared across modes [r[coupling-hotspot-remediation.slash-effect-boundary.shared-interpreter]]
- GIVEN a slash command is available in standalone, local attach, or remote attach mode
- WHEN the command is dispatched
- THEN the handler MUST return effects that a shared interpreter can apply for that mode
- AND attach parity behavior MUST NOT require reimplementing command-specific policy in transport loops

#### Scenario: plugin and UI effects are explicit [r[coupling-hotspot-remediation.slash-effect-boundary.explicit-effects]]
- GIVEN a slash command interacts with plugins, session history, local UI, or daemon state
- WHEN the handler succeeds or fails
- THEN the returned effect MUST identify the target subsystem and user-visible message
- AND fail-closed/no-op behavior MUST be deterministic and fixture-tested

### Requirement: Provider and router concerns have one owner [r[coupling-hotspot-remediation.provider-router-boundary]]

Provider-native request shaping, auth/account probing, routing/fallback/cooldown, retry/refresh behavior, and stream normalization MUST each have one explicit implementation owner.

#### Scenario: request bodies are fixture-owned [r[coupling-hotspot-remediation.provider-router-boundary.literal-request-fixtures]]
- GIVEN a provider-specific request is built
- WHEN tests assert the request body
- THEN expected JSON MUST come from literal fixtures or independently authored values
- AND tests MUST NOT build expected bodies by calling the same implementation under test

#### Scenario: adapters do not duplicate routing policy [r[coupling-hotspot-remediation.provider-router-boundary.no-adapter-routing]]
- GIVEN compatibility adapters bridge Clankers DTOs to `clanker-router`
- WHEN routing, fallback, cooldown, provider availability, or explicit-prefix fail-closed behavior is needed
- THEN the adapter MUST delegate to the routing owner
- AND it MUST NOT implement an independent fallback/cooldown path for the same provider family

### Requirement: Runtime process-job contracts split from adapters [r[coupling-hotspot-remediation.runtime-process-boundary]]

Process-job contracts MUST be split so runtime DTOs/policies, backend adapters, persistence mapping, notification decisions, retention/GC, and the agent-visible `process` tool projection are independently owned and tested.

#### Scenario: process tool is a thin JSON projection [r[coupling-hotspot-remediation.runtime-process-boundary.thin-tool-projection]]
- GIVEN the model calls the `process` tool
- WHEN tool JSON is parsed
- THEN the tool adapter MUST produce typed process-job requests, call a process-job service, and project typed receipts
- AND it MUST NOT directly construct storage DTOs, backend process state, notification policy, or retention/GC decisions

#### Scenario: storage/backend conversion is isolated [r[coupling-hotspot-remediation.runtime-process-boundary.storage-backend-adapters]]
- GIVEN process-job state must be persisted or reconciled with a backend
- WHEN storage or backend DTOs are constructed
- THEN the conversion MUST live in storage/backend adapter modules
- AND runtime contract tests MUST be able to run without spawning native child processes

### Requirement: Root compatibility re-exports are removed after migration [r[coupling-hotspot-remediation.root-reexport-boundary]]

Root-crate compatibility modules and broad `pub use` aliases that mask extracted-crate ownership MUST be treated as temporary migration shims and removed once in-repo call sites import owning crates directly.

#### Scenario: owning-crate imports replace root wrappers [r[coupling-hotspot-remediation.root-reexport-boundary.direct-imports]]
- GIVEN code uses agent, config, provider, plugin, session, util, message, DB, TUI, or model-selection APIs
- WHEN the API already has an owning workspace crate
- THEN in-repo code MUST import that owning crate directly
- AND new code MUST NOT route through root compatibility wrappers

#### Scenario: removal rail prevents reintroduction [r[coupling-hotspot-remediation.root-reexport-boundary.no-reintroduction]]
- GIVEN compatibility wrappers are deleted or marked unused
- WHEN the architecture rail runs
- THEN it MUST fail if a broad root re-export wrapper is reintroduced without an explicit migration task
- AND the diagnostic MUST name the owning crate that should be imported instead
