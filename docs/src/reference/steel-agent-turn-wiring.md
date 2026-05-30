# Steel Agent Turn Wiring

The real agent turn shell calls the reviewed `steel.host.plan_turn` planning seam before provider/tool effects. In default rollout, an authorized Steel plan now selects the Steel execution adapter for the turn. This is still not an authority transfer: Steel Scheme produces typed plan data and receipts, while Rust-owned host functions execute provider calls, tool execution, session state, mutation, fallback, and all host effects.

## Runtime path

1. `crates/clankers-agent::turn::run_turn_loop` receives an optional `AgentTurnSteelPlanningConfig` from the Rust-owned shell.
2. `turn/steel_planning.rs` converts bounded turn context into `TurnPlanningInput` using hashes, counts, model name, session id, and sorted tool names instead of raw prompts or tool bodies.
3. The adapter delegates to `clankers-runtime::plan_turn_with_steel_or_fallback` for the existing `steel.host.plan_turn` seam.
4. The runtime evaluates the constrained Steel wrapper host call, parses typed plan payloads, and routes selected action envelopes through Rust authorization receipts.
5. The agent shell emits a redacted `steel.host.plan_turn` receipt summary including the selected executor. If policy selects block-on-failure, the provider request is not sent.
6. When the receipt selects `executor=SteelScheme`, `turn/steel_execution.rs` builds a redacted `SteelTurnExecutionInput` for `steel.host.execute_turn` from hashes, session target, host-runner label, session capabilities, and UCAN abilities.
7. Rust calls `clankers-runtime::authorize_steel_turn_execution` before the host runner. Missing `turn-execution` session capability, missing `clankers/steel/orchestrate.execute_turn` UCAN ability, disabled action policy, unsafe destinations, or unsupported host-function policy deny before any provider request.
8. After authorization and host-runner return, the adapter emits a daemon-visible redacted `steel.host.execute_turn` receipt with executor, session hash, model label, result class, host-runner label, execution authority status/reason, authority receipt hash, safe counts, and receipt hash.

## Modes

- **Disabled:** Rust-native turn planning remains selected; no Steel receipt hash is produced.
- **Comparison:** Steel planning runs and records authorization evidence, but Rust-native planning remains the execution oracle.
- **Default:** Steel can select the planning result and the Steel-selected execution adapter only after Rust parses the typed plan and receives an authorized effect receipt.
- **Blocked:** malformed Steel output or denied policy with fallback disabled blocks before provider/tool effects.

## Boundaries

Steel receives no ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session, or mutation authority. It can only call registered host functions with the capabilities supplied by Rust. The current profile names `steel.host.plan_turn` for typed planning and `steel.host.execute_turn` for the Steel-selected execution authority check; both remain Rust-owned host seams that keep interpreter details out of controller/daemon/TUI/provider shells and emit deterministic redacted planning and execution receipts for review.

## Evidence

Focused Rust tests cover comparison mode, default selection after Rust authorization, disabled fallback, malformed-plan fallback, fallback-disabled blocking, denied planning authorization, denied execution authority before provider calls, stable redacted receipts, and real `run_turn_loop` invocations that emit Steel planning receipts for both Rust-native comparison and Steel-selected default execution plus an execution receipt when the Steel-selected adapter runs, without leaking prompt text.
