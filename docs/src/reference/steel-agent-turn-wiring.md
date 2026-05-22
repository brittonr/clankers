# Steel Agent Turn Wiring

The real agent turn shell can now call the reviewed `steel.host.plan_turn` planning seam before submitting a provider request. This is a wiring seam, not an authority transfer: Steel Scheme produces typed plan data and receipts; Rust still owns provider calls, tool execution, session state, mutation, fallback, and all host effects.

## Runtime path

1. `crates/clankers-agent::turn::run_turn_loop` receives an optional `AgentTurnSteelPlanningConfig` from the Rust-owned shell.
2. `turn/steel_planning.rs` converts bounded turn context into `TurnPlanningInput` using hashes, counts, model name, session id, and sorted tool names instead of raw prompts or tool bodies.
3. The adapter delegates to `clankers-runtime::plan_turn_with_steel_or_fallback` for the existing `steel.host.plan_turn` seam.
4. The runtime evaluates the constrained Steel wrapper host call, parses typed plan payloads, and routes selected action envelopes through Rust authorization receipts.
5. The agent shell emits a redacted `steel.host.plan_turn` receipt summary. If policy selects block-on-failure, the provider request is not sent.

## Modes

- **Disabled:** Rust-native turn planning remains selected; no Steel receipt hash is produced.
- **Comparison:** Steel planning runs and records authorization evidence, but Rust-native planning remains the execution oracle.
- **Default:** Steel can select the planning result only after Rust parses the typed plan and receives an authorized effect receipt.
- **Blocked:** malformed Steel output or denied policy with fallback disabled blocks before provider/tool effects.

## Boundaries

Steel receives no ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session, or mutation authority. It can only call registered host functions with the capabilities supplied by Rust. The current turn adapter registers `steel.host.plan_turn` only, keeps interpreter details out of agent/controller/daemon/TUI/provider shells, and emits deterministic redacted receipts for review.

## Evidence

Focused Rust tests cover comparison mode, default selection after Rust authorization, disabled fallback, malformed-plan fallback, fallback-disabled blocking, denied authorization, stable redacted receipts, and a real `run_turn_loop` invocation that emits the Steel planning receipt without leaking prompt text.
