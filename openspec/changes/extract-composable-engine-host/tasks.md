## Phase 0: Readiness

- [ ] R1 Confirm `decouple-llm-contract-surface` or equivalent engine/message boundary cleanup is complete before migrating the host runner. [covers=embeddable-agent-engine.composable-host-contract,embeddable-agent-engine.host-extraction-rails]
- [ ] R2 Validate this change before implementation with `openspec validate extract-composable-engine-host --strict` and, if using gates, proposal/design/tasks gates before marking implementation tasks done. [covers=embeddable-agent-engine.composable-host-contract,embeddable-agent-engine.reusable-tool-host,embeddable-agent-engine.reusable-stream-accumulator,embeddable-agent-engine.host-extraction-rails]

## Phase 1: Host contracts

- [ ] I1 Add the engine-host layer with model execution, tool execution, retry sleep, event sink, cancellation, and usage-observation traits using engine-native request/response/effect/input data. [covers=embeddable-agent-engine.composable-host-contract,embeddable-agent-engine.host-runner-traits]
- [ ] I2 Move reusable async engine-effect interpretation from `clankers-agent::turn` into the host runner while keeping provider I/O and actual sleeping behind host traits. [covers=embeddable-agent-engine.host-runner-traits,embeddable-agent-engine.no-duplicated-runner-policy]

## Phase 2: Tool and stream components

- [ ] I3 Extract tool catalog/executor/result accumulation behavior into a reusable tool-host surface with explicit success, error, missing-tool, capability-denied, cancellation, and truncation outcomes. [covers=embeddable-agent-engine.reusable-tool-host,embeddable-agent-engine.tool-host-catalog]
- [ ] I4 Adapt built-in, WASM plugin, and stdio plugin tools through the reusable tool-host executor seam without moving plugin runtime supervision into the engine. [covers=embeddable-agent-engine.plugin-tool-adapter,embeddable-agent-engine.tool-host-catalog]
- [ ] I5 Move stream accumulation into a deterministic reusable module and document handling for malformed JSON, non-object JSON, missing starts, late deltas, duplicate indexes, and provider errors. [covers=embeddable-agent-engine.reusable-stream-accumulator,embeddable-agent-engine.stream-folding-positive,embeddable-agent-engine.stream-folding-negative]

## Phase 3: Clankers adapter migration

- [ ] I6 Update `clankers-agent` to assemble the reusable host runner with Clankers provider, tool, hook, DB, capability, usage, model-switch, event-bus, and cancellation adapters. [covers=embeddable-agent-engine.agent-default-assembly,embeddable-agent-engine.host-adapter-parity]
- [ ] I7 Keep system-prompt assembly, session persistence decisions, daemon protocol conversion, and TUI rendering outside the engine-host and tool-host crates. [covers=embeddable-agent-engine.agent-default-assembly,embeddable-agent-engine.no-duplicated-runner-policy]

## Phase 4: Verification

- [ ] V1 Add positive and negative unit tests for stream accumulation, including text, thinking, tool JSON, usage deltas, provider errors, malformed JSON, non-object JSON, missing starts, late deltas, and duplicate indexes. [covers=embeddable-agent-engine.stream-folding-positive,embeddable-agent-engine.stream-folding-negative] [evidence=openspec/changes/extract-composable-engine-host/evidence/validation-plan.md]
- [ ] V2 Add host-runner tests with fake model/tool/sleep/event adapters proving effect interpretation, retry scheduling, cancellation, terminal behavior, tool success, tool failure, and missing-tool behavior. [covers=embeddable-agent-engine.host-runner-traits,embeddable-agent-engine.tool-host-catalog,embeddable-agent-engine.composable-host-contract] [evidence=openspec/changes/extract-composable-engine-host/evidence/validation-plan.md]
- [ ] V3 Add source rails rejecting duplicated runner policy in `clankers-agent::turn` while allowing adapter-only event, hook, usage, model-switch, and provider request conversion code. [covers=embeddable-agent-engine.no-duplicated-runner-policy,embeddable-agent-engine.host-extraction-rails] [evidence=openspec/changes/extract-composable-engine-host/evidence/validation-plan.md]
- [ ] V4 Add runtime parity tests proving existing Clankers flows preserve streaming deltas, tool events, tool failures, retry behavior, cancellation, usage updates, hook dispatch, and model switching through the reusable host runner. [covers=embeddable-agent-engine.agent-default-assembly,embeddable-agent-engine.host-adapter-parity] [evidence=openspec/changes/extract-composable-engine-host/evidence/validation-plan.md]
