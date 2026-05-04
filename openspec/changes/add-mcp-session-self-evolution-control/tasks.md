## Phase 0: OpenSpec foundation

- [x] Author proposal, design, task plan, and delta specs for MCP session-control plus self-evolution control-plane scope.
- [x] Validate the OpenSpec change and commit the spec package.

## Phase 1: MCP session-control substrate

- [x] Add local stdio MCP bridge command surface, for example `clankers mcp serve`, with explicit local-only transport and actionable unsupported errors. [covers=mcp-session-control-plane.bridge]
- [x] Add an MCP session-action model and mapping layer that converts supported MCP operations into ordinary `SessionCommand` variants. [covers=mcp-session-control-plane.command-parity]
- [x] Expose initial MCP tools for prompt submission, abort/interrupt, thinking level, disabled tools/capabilities, confirmation response, compaction, and status/history reads. [covers=mcp-session-control-plane.tool-surface]
- [x] Add structured mutation receipts backed by accepted command submission and correlated daemon event/state evidence when available. [covers=mcp-session-control-plane.receipts]
- [x] Ensure MCP cannot access TUI internals, private controller calls, raw PTY/input injection, or privileged tool/session mutations outside the daemon/session protocol. [covers=mcp-session-control-plane.no-bypass]

## Phase 2: Parity and safety verification

- [ ] Add command-equivalence tests for MCP vs TUI/attach paths for prompt, abort, thinking level, disabled tools/capabilities, confirmation approval/denial, and compaction. [covers=mcp-session-control-plane.parity-tests]
- [x] Add fake-daemon or temp-socket integration tests for MCP bridge request/response behavior, daemon event streaming, history/status observation, and error propagation. [covers=mcp-session-control-plane.bridge]
- [ ] Add negative tests for unsupported methods, missing sessions, capability-ceiling violations, confirmation bypass attempts, and unsafe history/metadata leakage. [covers=mcp-session-control-plane.no-bypass]
- [x] Document MCP session-control setup, supported operations, safety model, and receipts in README/docs. [covers=mcp-session-control-plane.documentation]

## Phase 3: Self-evolution outer loop

- [ ] Add a disabled-by-default self-evolution run model that records target artifact, baseline command/eval, candidate output/worktree path, metrics, receipts, and recommendation. [covers=self-evolution-control.run-model]
- [ ] Implement a dry-run/fake-executor path that drives clankers through the MCP session-control bridge without requiring live provider credentials. [covers=self-evolution-control.mcp-orchestration]
- [ ] Enforce isolated candidate writes and reject live in-place mutation of installed skills, prompts, tools, or code during a run. [covers=self-evolution-control.isolation]
- [ ] Require explicit human approval before promotion/install/merge of any self-evolved candidate and record the approval as a normal confirmation/session event. [covers=self-evolution-control.promotion-gate]
- [ ] Add tests for baseline-vs-candidate scoring, failed eval handling, unchanged-candidate/noise detection, human approval gating, and receipt generation. [covers=self-evolution-control.verification]
- [ ] Document the self-evolution workflow, safety constraints, expected receipts, and suggested first local targets. [covers=self-evolution-control.documentation]

## Phase 4: Final verification

- [ ] Run targeted MCP/self-evolution tests, protocol checks, docs checks, OpenSpec validation, and `git diff --check`.
- [ ] Sync canonical specs and archive the change after all implementation tasks are complete.
