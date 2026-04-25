## Phase 0: Readiness

- [ ] R1 Validate this change before implementation with `openspec validate decouple-llm-contract-surface --strict` and, if using gates, proposal/design/tasks gates before marking implementation tasks done. [covers=embeddable-agent-engine.minimal-contract-dependencies,embeddable-agent-engine.engine-native-submission,embeddable-agent-engine.contract-boundary-rails]

## Phase 1: Canonical contract ownership

- [ ] I1 Move generic `Usage`, `ToolDefinition`, `ThinkingConfig`, stream metadata/deltas, and adjacent plain-data contracts into `clanker-message` without adding router/provider/runtime dependencies to that crate. [covers=embeddable-agent-engine.message-without-router,embeddable-agent-engine.router-provider-reexports]
- [ ] I2 Update `clanker-router` to import or re-export the canonical `clanker-message` contract types and remove the reverse `clanker-message` → `clanker-router` dependency. [covers=embeddable-agent-engine.message-without-router,embeddable-agent-engine.router-provider-reexports]
- [ ] I3 Update `clankers-provider` to import or re-export the canonical `clanker-message` contract types while preserving provider request JSON shape and existing provider/router adapter behavior. [covers=embeddable-agent-engine.router-provider-reexports]

## Phase 2: Engine surface cleanup

- [ ] I4 Remove direct `clanker-router` and `clankers-provider` dependencies from `clankers-engine`, using only `clanker-message`, `clankers-core`, and local engine-native data where needed. [covers=embeddable-agent-engine.minimal-contract-dependencies,embeddable-agent-engine.engine-cargo-tree-clean]
- [ ] I5 Change `EnginePromptSubmission` to accept `Vec<EngineMessage>` and move `AgentMessage` filtering/conversion into the Clankers agent adapter. [covers=embeddable-agent-engine.engine-native-submission,embeddable-agent-engine.no-agent-message-filtering,embeddable-agent-engine.adapter-transcript-conversion]

## Phase 3: Rails and verification

- [ ] V1 Add positive and negative adapter tests for transcript conversion: user/assistant/tool messages are preserved, shell-only history entries are excluded, and provider request construction still receives native provider messages. [covers=embeddable-agent-engine.adapter-transcript-conversion,embeddable-agent-engine.no-agent-message-filtering] [evidence=openspec/changes/decouple-llm-contract-surface/evidence/validation-plan.md]
- [ ] V2 Add cargo-tree dependency rails proving `clankers-engine` excludes provider/router/runtime crates and `clanker-message` excludes router/runtime crates. [covers=embeddable-agent-engine.engine-cargo-tree-clean,embeddable-agent-engine.message-without-router,embeddable-agent-engine.cargo-tree-rail] [evidence=openspec/changes/decouple-llm-contract-surface/evidence/validation-plan.md]
- [ ] V3 Add source-inventory rails for forbidden public-surface imports and run focused engine, message, provider/router compatibility, and Clankers agent adapter tests. [covers=embeddable-agent-engine.source-surface-rail,embeddable-agent-engine.contract-boundary-rails,embeddable-agent-engine.router-provider-reexports] [evidence=openspec/changes/decouple-llm-contract-surface/evidence/validation-plan.md]
