## Phase 0: Readiness

- [ ] R1 Validate this change before implementation with `openspec validate separate-engine-core-composition --strict` and, if using gates, proposal/design/tasks gates before marking implementation tasks done. [covers=embeddable-agent-engine.core-engine-ownership,embeddable-agent-engine.no-dormant-core-state,embeddable-agent-engine.core-engine-boundary-rails]

## Phase 1: Ownership cleanup

- [ ] I1 Remove dormant `core_state: Option<CoreState>` from `EngineState` and update engine constructors, terminal helpers, reducer transitions, and tests without changing turn behavior. [covers=embeddable-agent-engine.no-dormant-core-state,embeddable-agent-engine.engine-state-active-data]
- [ ] I2 Document reducer ownership in crate-level docs or README text for `clankers-core` and `clankers-engine`, including lifecycle/control policy versus model/tool turn policy. [covers=embeddable-agent-engine.core-engine-ownership,embeddable-agent-engine.core-owned-policy,embeddable-agent-engine.engine-owned-policy]

## Phase 2: Explicit adapter composition

- [ ] I3 Add or clarify pure adapter helpers that sequence prompt lifecycle core effects, engine turn execution inputs/effects, and post-prompt follow-up evaluation without provider I/O, tool I/O, daemon protocol, or TUI rendering. [covers=embeddable-agent-engine.explicit-adapter-composition,embeddable-agent-engine.composition-tests]
- [ ] I4 Keep controller and agent shells as effect interpreters: core-owned lifecycle behavior flows through `clankers-core`, engine-owned turn behavior flows through `clankers-engine`, and shell code performs only translation/I/O. [covers=embeddable-agent-engine.core-owned-policy,embeddable-agent-engine.engine-owned-policy,embeddable-agent-engine.cross-reducer-source-rail]

## Phase 3: Rails and verification

- [ ] V1 Add positive composition tests for prompt start/completion, engine turn execution, terminal completion, queued prompt replay, loop follow-up, and auto-test follow-up sequencing through explicit adapters. [covers=embeddable-agent-engine.explicit-adapter-composition,embeddable-agent-engine.composition-tests] [evidence=openspec/changes/separate-engine-core-composition/evidence/validation-plan.md]
- [ ] V2 Add negative composition tests for out-of-order core completion, mismatched effect IDs, wrong-phase engine feedback, post-terminal engine feedback, and lifecycle/turn feedback sent to the wrong reducer. [covers=embeddable-agent-engine.composition-tests,embeddable-agent-engine.core-engine-boundary-rails] [evidence=openspec/changes/separate-engine-core-composition/evidence/validation-plan.md]
- [ ] V3 Extend FCIS/source rails to fail on dormant core state inside `EngineState`, core-owned lifecycle policy inside `clankers-engine`, and engine-owned turn policy duplicated in controller or agent shells outside approved adapter seams. [covers=embeddable-agent-engine.cross-reducer-source-rail,embeddable-agent-engine.core-engine-boundary-rails,embeddable-agent-engine.no-dormant-core-state] [evidence=openspec/changes/separate-engine-core-composition/evidence/validation-plan.md]
