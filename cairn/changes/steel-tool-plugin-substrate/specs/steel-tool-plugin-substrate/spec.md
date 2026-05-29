## ADDED Requirements

### Requirement: Steel substrate contract is typed and Rust-owned [r[steel-tool-plugin-substrate.substrate-contract]]

Clankers MUST expose Steel-mediated tool, plugin, and subagent orchestration only through a Rust-owned substrate contract that accepts bounded redacted catalog/call metadata and returns typed invocation plans. Steel MUST NOT execute Rust, WASM, stdio, subagent, provider, filesystem, process, network, daemon, TUI, or plugin-manager effects directly.

#### Scenario: typed plan is the only executable Steel output [r[steel-tool-plugin-substrate.substrate-contract.typed-plan]]
- GIVEN a model requests a tool, plugin, subagent, or delegate call
- WHEN the Steel substrate is enabled for that call
- THEN Steel MUST return a supported versioned invocation plan schema
- AND Rust MUST reject free-form textual output, unknown schema versions, executor-kind mismatches, input-hash mismatches, and malformed plans before any host effect

#### Scenario: Steel receives no ambient authority [r[steel-tool-plugin-substrate.substrate-contract.no-ambient-authority]]
- GIVEN the Steel script evaluates `steel.host.tool.list` or `steel.host.tool.call`
- WHEN host-function data crosses the interpreter boundary
- THEN the data MUST be bounded and redacted
- AND it MUST NOT include raw prompts, subagent task bodies/transcripts, credentials, provider payloads, raw tool bodies, raw plugin stdout/stderr, process handles, filesystem handles, network handles, daemon handles, TUI handles, process-registry handles, session-controller handles, or plugin-manager handles

### Requirement: Rust built-in tools route through the substrate [r[steel-tool-plugin-substrate.rust-builtins]]

When the substrate is enabled for Rust built-in tools, Clankers MUST ask the Steel substrate to plan the invocation before executing the built-in tool. Rust MUST execute the tool only through an authorized substrate adapter or through an explicit disabled/comparison/fallback path.

#### Scenario: built-in execution preserves current semantics [r[steel-tool-plugin-substrate.rust-builtins.semantic-parity]]
- GIVEN a Rust built-in tool call is authorized by the Steel substrate
- WHEN Rust executes the call through the substrate adapter
- THEN existing capability checks, disabled-tool checks, cancellation, hook pipeline, progress events, database/search service availability, result accumulation, and output truncation MUST behave the same as the direct dispatch path
- AND the receipt MUST record executor kind `rust_builtin`

#### Scenario: denied built-in call performs no direct fallback effect [r[steel-tool-plugin-substrate.rust-builtins.denied-no-effect]]
- GIVEN Steel returns a plan for an unknown, disabled, unauthorized, over-budget, or mismatched built-in tool call
- WHEN Rust validates the plan
- THEN Rust MUST deny or block the call before `Tool::execute`
- AND it MUST NOT run the direct built-in dispatch path unless policy explicitly selected comparison or fallback mode

### Requirement: WASM plugin calls route through the substrate [r[steel-tool-plugin-substrate.wasm-plugins]]

When the substrate is enabled for WASM/Extism plugins, Clankers MUST plan plugin tool calls through the Steel substrate and execute them through Rust-owned plugin adapters that preserve manifest, permission, host-call, panic isolation, and result conversion behavior.

#### Scenario: WASM plugin adapter preserves plugin policy [r[steel-tool-plugin-substrate.wasm-plugins.policy-preserved]]
- GIVEN an active WASM plugin exposes a tool
- WHEN the Steel substrate authorizes the call with executor kind `wasm_plugin`
- THEN Rust MUST verify plugin name, tool name, function name, manifest state, active-plugin filtering, input hash, disabled-tool policy, and permissions before invoking the plugin
- AND host calls emitted by the plugin MUST still cross the existing Rust plugin host-call permission checks

#### Scenario: inactive or malformed WASM plugin call fails closed [r[steel-tool-plugin-substrate.wasm-plugins.fail-closed]]
- GIVEN the plan names a missing, inactive, disabled, unloaded, wrong-kind, or malformed WASM plugin call
- WHEN Rust validates the plan
- THEN the call MUST fail closed with a stable receipt status
- AND no WASM function may be invoked

### Requirement: Stdio plugin calls route through the substrate [r[steel-tool-plugin-substrate.stdio-plugins]]

When the substrate is enabled for stdio plugins, Clankers MUST plan stdio plugin tool calls through the Steel substrate and execute them through the existing Rust stdio supervisor/tool-call lifecycle.

#### Scenario: stdio lifecycle remains Rust-owned [r[steel-tool-plugin-substrate.stdio-plugins.lifecycle-preserved]]
- GIVEN a stdio plugin tool call is authorized by the Steel substrate
- WHEN Rust executes the call
- THEN `start_stdio_tool_call`, result-event polling, progress emission, timeout, cancellation, cancel-grace, abandon, disconnect handling, and supervisor restart/disable race protections MUST remain Rust-owned
- AND the receipt MUST record executor kind `stdio_plugin`

#### Scenario: stdio sandbox and launch policy still fail closed [r[steel-tool-plugin-substrate.stdio-plugins.sandbox-fail-closed]]
- GIVEN stdio launch policy, environment allowlist, Landlock, seccomp, writable-root resolution, or plugin state-dir setup rejects the plugin
- WHEN a Steel-mediated stdio plan is validated or executed
- THEN Rust MUST surface a stable failure receipt
- AND Steel MUST NOT receive child process handles, unrestricted environment data, or raw stderr beyond redacted receipt summaries

### Requirement: Subagent and delegate calls route through the substrate [r[steel-tool-plugin-substrate.subagents]]

When the substrate is enabled for subagent and delegate orchestration tools, Clankers MUST plan `subagent` and `delegate_task` calls through the Steel substrate and execute them through Rust-owned child-agent adapters that preserve actor/subprocess spawning, remote prompt RPC where configured, panel events, watchdogs, cancellation, and process monitoring.

#### Scenario: child-agent lifecycle remains Rust-owned [r[steel-tool-plugin-substrate.subagents.lifecycle-preserved]]
- GIVEN a subagent or delegate call is authorized by the Steel substrate
- WHEN Rust executes the call
- THEN in-process actor spawning, subprocess fallback, process monitoring, watchdogs, panel events, pane limits, kill/cancel requests, and session/controller construction MUST remain Rust-owned
- AND the receipt MUST record executor kind `subagent`

#### Scenario: denied child-agent call spawns nothing [r[steel-tool-plugin-substrate.subagents.denied-no-spawn]]
- GIVEN the plan names an unknown, disabled, unauthorized, over-budget, mismatched, or policy-denied subagent/delegate call
- WHEN Rust validates the plan
- THEN no actor, subprocess, remote prompt, worker, pane, or child session may be spawned
- AND the receipt MUST omit raw child prompts and transcripts while recording a stable denial reason

### Requirement: Catalog and policy snapshots are bounded and live [r[steel-tool-plugin-substrate.catalog-policy]]

The substrate MUST build catalog snapshots from the same live tool/plugin/subagent inventory used by agent turns, including built-in tools, WASM plugins, stdio plugins, subagent/delegate orchestration tools, disabled-tool policy, user tool filters, and active plugin state.

#### Scenario: catalog matches available dispatch surface [r[steel-tool-plugin-substrate.catalog-policy.live-inventory]]
- GIVEN tools, plugins, subagent executors, or delegate executors are enabled, disabled, reloaded, or restarted
- WHEN the next Steel substrate catalog snapshot is built
- THEN it MUST reflect the same callable inventory and collision rules as the current direct dispatch surface
- AND it MUST include safe executor-kind/source metadata without raw implementation bodies

#### Scenario: disabled and filtered tools stay unavailable [r[steel-tool-plugin-substrate.catalog-policy.disabled-filtered]]
- GIVEN a tool is disabled by settings, user filter, capability policy, plugin state, or built-in/plugin name collision policy
- WHEN Steel lists or plans calls
- THEN the tool MUST appear unavailable or be omitted according to policy
- AND a direct tool/plugin/subagent call MUST NOT bypass that decision in default/block mode

### Requirement: Receipts are deterministic and redacted [r[steel-tool-plugin-substrate.receipts]]

Every Steel-mediated tool, plugin, subagent, or delegate invocation MUST emit deterministic receipt material that is sufficient to audit policy, executor kind, fallback, child-agent status, and result status without leaking sensitive inputs or outputs.

#### Scenario: receipt fields prove dispatch decision [r[steel-tool-plugin-substrate.receipts.dispatch-fields]]
- GIVEN a Steel-mediated tool, plugin, subagent, or delegate call completes, falls back, is denied, or is blocked
- WHEN the receipt is written or emitted
- THEN it MUST include schema, call id, tool name, source label, executor kind, profile/policy identity, request hash, input hash, output hash when available, authorization status, fallback status, redaction class, child-agent status when applicable, and safe error class

#### Scenario: receipts redact sensitive material [r[steel-tool-plugin-substrate.receipts.redaction]]
- GIVEN a call includes prompt-adjacent text, subagent task bodies/transcripts, credentials, provider payloads, raw plugin output, raw stdout/stderr, raw script source, large tool output, or uncontrolled absolute paths
- WHEN the substrate produces receipt material
- THEN the receipt MUST omit or hash that material according to redaction policy
- AND deterministic tests MUST prove the forbidden material does not appear in receipts

### Requirement: Rollout and fallback are explicit [r[steel-tool-plugin-substrate.rollout]]

Steel-mediated dispatch MUST roll out through explicit disabled, comparison, default, and block modes. Direct dispatch MUST remain available only as an explicit operator kill switch, comparison oracle, or policy-authorized fallback until default-mode parity is proven.

#### Scenario: comparison mode does not change execution oracle [r[steel-tool-plugin-substrate.rollout.comparison-oracle]]
- GIVEN the substrate runs in comparison mode
- WHEN a tool, plugin, subagent, or delegate call is requested
- THEN Clankers MUST evaluate the Steel plan and emit receipts
- AND it MUST execute the current Rust direct dispatch path as the oracle

#### Scenario: default mode executes only authorized Steel-mediated plans [r[steel-tool-plugin-substrate.rollout.default-authorized-only]]
- GIVEN the substrate runs in default mode
- WHEN a tool, plugin, subagent, or delegate call is requested
- THEN Clankers MUST execute only an authorized Steel-mediated plan
- AND fallback to direct dispatch MUST occur only when policy explicitly allows fallback and the receipt records `fallback_used`

### Requirement: Verification covers all executor kinds [r[steel-tool-plugin-substrate.verification]]

The implementation MUST include deterministic fixture, boundary, and runtime dogfood evidence covering Rust built-in, WASM plugin, stdio plugin, and subagent/delegate executor kinds, including positive calls, denials, fallback/block behavior, cancellation, redaction, and catalog drift.

#### Scenario: source-boundary rail prevents bypass [r[steel-tool-plugin-substrate.verification.boundary-rail]]
- GIVEN the substrate is in default/block mode
- WHEN source-boundary checks inspect agent turn execution, plugin dispatch, daemon/TUI/provider shells, and runtime modules
- THEN validation MUST fail if a caller bypasses the Steel substrate adapter for enabled executor kinds or imports Steel interpreter internals outside the runtime wrapper

#### Scenario: runtime dogfood proves representative calls [r[steel-tool-plugin-substrate.verification.runtime-dogfood]]
- GIVEN deterministic local fixtures for one read-only built-in, one mutating or progress-emitting built-in, one WASM plugin tool, one stdio plugin tool, and one subagent/delegate call
- WHEN the dogfood rail runs in default mode
- THEN each call MUST complete or fail as expected through the Steel substrate
- AND receipts MUST prove executor kind, policy status, redaction, and no direct-dispatch bypass
