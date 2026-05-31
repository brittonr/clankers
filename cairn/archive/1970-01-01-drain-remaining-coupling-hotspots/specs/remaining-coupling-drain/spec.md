## ADDED Requirements

### Requirement: Remaining hotspot inventory stays traceable [r[remaining-coupling-drain.hotspot-inventory]]

Clankers MUST track every remaining coupling hotspot identified by the post-decoupling architecture review with an explicit owner, target boundary, and validation path before closing the drain.

#### Scenario: all current hotspots are represented [r[remaining-coupling-drain.hotspot-inventory.all-current-hotspots]]
- GIVEN architecture review identifies root-shell, agent-concrete-dependency, process-job, controller-command, daemon-actor, display-dto, provider-router, and architecture-rail coupling
- WHEN the Cairn change is reviewed
- THEN every hotspot MUST have a requirement and at least one implementation or verification task
- THEN future drain work can be prioritized without relying on untracked prose

### Requirement: Root shell policy drains to owned bricks [r[remaining-coupling-drain.root-shell-policy]]

The root `clankers` crate MUST remain an application-edge shell: it may wire concrete services, but reusable domain policy, storage schemas, provider shaping, process-job policy, rendering semantics, and protocol conversion MUST live in named workspace crates or focused adapter modules with owner receipts.

#### Scenario: root dependency edges remain app-edge wiring [r[remaining-coupling-drain.root-shell-policy.app-edge-wiring]]
- GIVEN root code uses an internal workspace crate
- WHEN architecture rails inventory the dependency edge
- THEN the edge MUST have an owner receipt explaining why it is application-edge wiring
- THEN reusable behavior behind that edge MUST be tested at its owner rather than only through root modes

#### Scenario: root modules shrink policy ownership [r[remaining-coupling-drain.root-shell-policy.policy-drained]]
- GIVEN a large root tool or mode module owns reusable behavior
- WHEN the behavior can be expressed as a neutral service, DTO, or workspace brick
- THEN implementation MUST move that behavior to the owner and leave root code as parsing, wiring, or projection

### Requirement: Agent concrete dependencies shrink behind ports [r[remaining-coupling-drain.agent-concrete-dependencies]]

`clankers-agent` MUST keep turn policy behind model, tool, storage, prompt, hook, skill, cost, and cancellation ports, and MUST reduce direct concrete dependencies on provider/router/DB/config/procmon/TUI systems as those adapters move to application edges.

#### Scenario: turn policy uses neutral ports [r[remaining-coupling-drain.agent-concrete-dependencies.neutral-ports]]
- GIVEN a turn helper needs model execution, tool execution, storage, hooks, usage, skills, or cancellation
- WHEN source-boundary rails inspect reusable turn policy
- THEN concrete provider/router/auth, DB/search, TUI display, procmon, and project path lookup types MUST be absent unless the module is a named adapter

#### Scenario: dependency budget moves downward [r[remaining-coupling-drain.agent-concrete-dependencies.budget-decreases]]
- GIVEN `clankers-agent` still has concrete dependency receipts
- WHEN a drain slice touches one dependency family
- THEN the slice MUST either remove that dependency, narrow it to a dev/test/adapter-only edge, or update the owner receipt with a smaller convergence condition

### Requirement: Process-job policy splits from the root tool [r[remaining-coupling-drain.process-job-policy]]

The agent-visible `process` tool MUST stay a thin JSON-to-typed-request adapter over process-job services. Native process management, backend capability rules, durable storage mapping, redaction, notification policy, and retention/GC MUST be owned by runtime/process service modules or backend adapters.

#### Scenario: root process tool is a projection [r[remaining-coupling-drain.process-job-policy.root-projection]]
- GIVEN an agent calls the process tool
- WHEN source and fixture rails inspect the root tool
- THEN the root module MUST parse JSON, call a typed service, and project typed receipts
- THEN backend/native/storage policy MUST NOT be implemented inline in the agent-visible projection

#### Scenario: process service owns backend policy [r[remaining-coupling-drain.process-job-policy.backend-owner]]
- GIVEN a process job uses native, pueue, systemd, or future backends
- WHEN policy differs by backend
- THEN capability, storage, retention, notification, and redaction behavior MUST be isolated behind typed backend/service interfaces with focused tests

### Requirement: Controller command seams split by responsibility [r[remaining-coupling-drain.controller-command-seams]]

`clankers-controller` MUST keep command input translation, authorization, core reducer effect interpretation, runtime dispatch, persistence, continuation policy, and protocol/event projection in separately testable modules.

#### Scenario: command dispatch does not own every layer [r[remaining-coupling-drain.controller-command-seams.single-purpose]]
- GIVEN a session command is handled
- WHEN source-boundary rails inspect controller command code
- THEN no single function or module SHOULD own wire parsing, authorization, core input construction, runtime mutation, persistence, and daemon/TUI event projection for the same behavior

#### Scenario: projection stays centralized [r[remaining-coupling-drain.controller-command-seams.projection-owner]]
- GIVEN controller behavior emits user-visible or transport-visible output
- WHEN that output is converted to protocol/TUI events
- THEN conversion MUST go through the explicit projection owner rather than reconstructing protocol DTOs in command policy paths

### Requirement: Daemon actor construction separates assembly from loop policy [r[remaining-coupling-drain.daemon-actor-assembly]]

Daemon session startup MUST split session runtime assembly from actor-loop multiplexing. Tool construction, capability gates, persistence, hooks, plugin UI startup, controller config, and child-session fallback policy MUST be prepared by focused builders/adapters before the actor loop runs.

#### Scenario: actor loop receives assembled runtime [r[remaining-coupling-drain.daemon-actor-assembly.loop-inputs]]
- GIVEN a daemon session actor starts
- WHEN the actor loop begins polling commands, signals, confirmations, plugin events, and controller events
- THEN it MUST receive already-assembled runtime/controller inputs
- THEN it MUST NOT construct unrelated tools, persistence, hooks, capability gates, or plugin host policy inline

#### Scenario: session assembly is socketless-testable [r[remaining-coupling-drain.daemon-actor-assembly.socketless-tests]]
- GIVEN create, resume, keyed-session, and child-session inputs
- WHEN assembly policy is tested
- THEN tests MUST not bind Unix sockets or require a running actor registry to verify construction decisions

### Requirement: Display and protocol DTOs drain inward [r[remaining-coupling-drain.display-protocol-dto-leakage]]

Display/protocol DTO crates MUST stay at projection edges. Agent, runtime, and reusable controller logic MUST prefer neutral message, runtime, core, or service DTOs over TUI/protocol constructors for decisions.

#### Scenario: display DTOs stay at display adapters [r[remaining-coupling-drain.display-protocol-dto-leakage.display-edge]]
- GIVEN reusable logic emits progress, messages, usage, tool results, or session state
- WHEN that logic is compiled without TUI rendering
- THEN it MUST not need display-only constructors or display-state enums except through an explicit projection adapter

#### Scenario: protocol DTOs stay at transport adapters [r[remaining-coupling-drain.display-protocol-dto-leakage.protocol-edge]]
- GIVEN daemon, attach, Matrix, ACP, MCP, or RPC transports observe session behavior
- WHEN frames are produced
- THEN they MUST be projected from neutral domain events/receipts at transport adapters
- THEN transport DTOs MUST NOT become canonical domain state in reusable modules

### Requirement: Provider/router compatibility converges to one owner per concern [r[remaining-coupling-drain.provider-router-convergence]]

Provider-native request shaping, model/account discovery, auth refresh/probing, routing/fallback/cooldown, retry behavior, and stream normalization MUST each have one owner. Compatibility layers MUST translate DTOs only and MUST NOT duplicate policy.

#### Scenario: compatibility adapters are thin [r[remaining-coupling-drain.provider-router-convergence.thin-adapters]]
- GIVEN a `clankers-provider` adapter calls `clanker-router`
- WHEN source and fixture rails inspect the adapter
- THEN it MUST only translate DTOs, stream events, and errors
- THEN routing, fallback, cooldown, auth probe, and provider-native body-shaping policy MUST remain owned by the router/provider backend modules

#### Scenario: duplicate provider abstractions are tracked [r[remaining-coupling-drain.provider-router-convergence.duplicate-abstractions]]
- GIVEN two provider request/event traits remain in the workspace
- WHEN a new backend or request field is added
- THEN constructor-count and projection parity rails MUST prove the adapters stay in sync or the duplicate abstraction must be collapsed

### Requirement: Architecture rails become typed or behavioral [r[remaining-coupling-drain.architecture-rail-hardening]]

Architecture boundary verification MUST replace brittle string-presence anchors with typed Cargo metadata, Rust AST/module inventories, deterministic behavior fixtures, or generated ownership manifests whenever practical.

#### Scenario: brittle source anchors are drained [r[remaining-coupling-drain.architecture-rail-hardening.source-anchors]]
- GIVEN a rail currently asserts exact source snippets
- WHEN the rail is touched for a drain slice
- THEN it MUST either become a typed/behavioral check or document why exact-string matching remains necessary
- THEN refactors that preserve ownership should not fail only because code moved to a new owner file

#### Scenario: rail diagnostics identify owners [r[remaining-coupling-drain.architecture-rail-hardening.owner-diagnostics]]
- GIVEN a boundary rail fails
- WHEN a developer reads the diagnostic
- THEN the diagnostic MUST name the source, target owner, and expected replacement path rather than requiring manual grep archaeology

### Requirement: Drain closeout preserves behavior and traceability [r[remaining-coupling-drain.closeout-validation]]

Every drain slice MUST preserve existing user-visible behavior and update traceability, evidence, and architecture rails before closeout.

#### Scenario: focused and broad validation pass [r[remaining-coupling-drain.closeout-validation.validation-pass]]
- GIVEN a drain slice is complete
- WHEN validation runs
- THEN focused tests for the moved seam, Cairn gates/validate, architecture rails, `./scripts/verify.sh`, and full nextest partitions MUST pass or have explicit checked evidence for any environmental limitation

#### Scenario: evidence is durable [r[remaining-coupling-drain.closeout-validation.durable-evidence]]
- GIVEN a verification task is checked
- WHEN a reviewer opens the evidence path
- THEN it MUST contain the exact command, result, and relevant pass/fail summary needed to verify the claim without relying on transient terminal scrollback
