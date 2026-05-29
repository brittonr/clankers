## Tasks

### Phase 1: substrate contract and policy

- [x] [serial] I1: Add `clankers_runtime::steel_tool_substrate` DTOs for catalog snapshots, invocation requests, plans, executor kinds, and receipts, with serde fixtures for malformed schema, executor-kind mismatch, input-hash mismatch, and redaction. [covers=r[steel-tool-plugin-substrate.substrate-contract.typed-plan],r[steel-tool-plugin-substrate.receipts.dispatch-fields],r[steel-tool-plugin-substrate.receipts.redaction]]
- [x] [serial] I2: Add the reviewed Steel host-function/profile surface for `steel.host.tool.list` and `steel.host.tool.call`, including bounded budgets, allowed executor kinds, fallback mode, and receipt destination policy. [covers=r[steel-tool-plugin-substrate.substrate-contract.no-ambient-authority],r[steel-tool-plugin-substrate.rollout.default-authorized-only]]
- [x] [serial] I3: Add settings and activation plumbing for `steelToolSubstrate` disabled/comparison/default/block modes, explicit kill switch, profile/script hash validation, and Rust-native fallback policy. [covers=r[steel-tool-plugin-substrate.rollout.comparison-oracle],r[steel-tool-plugin-substrate.rollout.default-authorized-only]]

### Phase 2: Rust built-in tooling

- [x] [serial] I4: Add a Rust-owned substrate dispatch port before direct built-in tool execution in the agent turn path, preserving call id, cancellation token, hook pipeline, capability gate, db/search services, progress events, accumulator, and output truncation. [covers=r[steel-tool-plugin-substrate.rust-builtins.semantic-parity],r[steel-tool-plugin-substrate.rust-builtins.denied-no-effect]]
- [x] [parallel] I5: Add deterministic built-in fixtures for one read-only tool and one mutating or progress-emitting tool that compare direct dispatch with Steel-mediated comparison/default/block modes. [covers=r[steel-tool-plugin-substrate.rust-builtins.semantic-parity],r[steel-tool-plugin-substrate.rollout.comparison-oracle]]

### Phase 3: WASM plugin substrate

- [x] [serial] I6: Route WASM/Extism plugin tool calls through the substrate adapter using executor kind `wasm_plugin`, active plugin inventory, manifest/function validation, disabled-tool checks, and existing host-call permission processing. [covers=r[steel-tool-plugin-substrate.wasm-plugins.policy-preserved],r[steel-tool-plugin-substrate.wasm-plugins.fail-closed]]
- [x] [parallel] I7: Add WASM plugin fixtures with the existing test plugin for success, inactive/missing plugin, wrong function, host-call permission denial, panic/error conversion, and receipt redaction. [covers=r[steel-tool-plugin-substrate.wasm-plugins.policy-preserved],r[steel-tool-plugin-substrate.wasm-plugins.fail-closed],r[steel-tool-plugin-substrate.receipts.redaction]]

### Phase 4: stdio plugin substrate

- [x] [serial] I8: Route stdio plugin tool calls through the substrate adapter using executor kind `stdio_plugin` while keeping `start_stdio_tool_call`, progress/result event polling, timeout, cancellation, cancel-grace, abandon, disconnect, and supervisor run-id handling Rust-owned. [covers=r[steel-tool-plugin-substrate.stdio-plugins.lifecycle-preserved]]
- [x] [parallel] I9: Add stdio plugin fixtures for success, progress, timeout, cancellation, disconnect, disable/restart race, restricted sandbox failure, and redacted stderr summaries. [covers=r[steel-tool-plugin-substrate.stdio-plugins.lifecycle-preserved],r[steel-tool-plugin-substrate.stdio-plugins.sandbox-fail-closed]]

### Phase 5: subagent and delegate substrate

- [x] [serial] I10: Route `subagent` and `delegate_task` calls through the substrate adapter using executor kind `subagent` while keeping ActorContext spawning, subprocess fallback, remote prompt RPC where configured, process monitoring, watchdogs, panel events, pane limits, cancellation, and worker metadata Rust-owned. [covers=r[steel-tool-plugin-substrate.subagents.lifecycle-preserved],r[steel-tool-plugin-substrate.subagents.denied-no-spawn]]
- [x] [parallel] I11: Add subagent/delegate fixtures for success, denied-before-spawn, cancellation/kill, subprocess fallback, daemon actor mode, panel-event propagation, watchdog status, and receipt redaction of child prompts/transcripts. [covers=r[steel-tool-plugin-substrate.subagents.lifecycle-preserved],r[steel-tool-plugin-substrate.subagents.denied-no-spawn],r[steel-tool-plugin-substrate.receipts.redaction]]

### Phase 6: catalog, receipts, and default rollout

- [x] [serial] I12: Build substrate catalog snapshots from the live built-in/plugin/subagent inventory, including disabled-tool/user-filter state, plugin reload/restart state, subagent/delegate availability, name-collision policy, source label, executor kind, and safe schema hashes. [covers=r[steel-tool-plugin-substrate.catalog-policy.live-inventory],r[steel-tool-plugin-substrate.catalog-policy.disabled-filtered]]
- [x] [parallel] I13: Add deterministic receipt helpers and golden fixtures that prove dispatch fields, fallback fields, child-agent status fields, output hashing, and forbidden raw material redaction for all executor kinds. [covers=r[steel-tool-plugin-substrate.receipts.dispatch-fields],r[steel-tool-plugin-substrate.receipts.redaction]]
- [x] [serial] I14: Switch default-mode tool/plugin/subagent dispatch to require authorized Steel-mediated plans for enabled executor kinds, retaining direct dispatch only for explicit disabled/comparison/fallback modes. [covers=r[steel-tool-plugin-substrate.rollout.default-authorized-only],r[steel-tool-plugin-substrate.verification.boundary-rail]]

### Phase 7: verification and documentation

- [x] [serial] V1: Add and run `scripts/check-steel-tool-plugin-substrate.rs` to verify DTO/schema fixtures, receipt redaction fixtures, source-boundary ownership, host-function allowlist, and no direct-dispatch bypass in default/block mode. [covers=r[steel-tool-plugin-substrate.substrate-contract.typed-plan],r[steel-tool-plugin-substrate.substrate-contract.no-ambient-authority],r[steel-tool-plugin-substrate.receipts.redaction],r[steel-tool-plugin-substrate.verification.boundary-rail]] [evidence=evidence/v1-checker.md]
- [x] [serial] V2: Run focused Rust built-in tests for direct-vs-Steel comparison parity, denied-no-effect behavior before `Tool::execute`, cancellation, hooks, accumulator/truncation, and capability denial. [covers=r[steel-tool-plugin-substrate.rust-builtins.semantic-parity],r[steel-tool-plugin-substrate.rust-builtins.denied-no-effect],r[steel-tool-plugin-substrate.rollout.comparison-oracle]] [evidence=evidence/v2-builtins.md]
- [x] [serial] V3: Run focused WASM plugin tests for success, inactive/missing plugin, wrong function, host-call permission denial, panic/error conversion, and receipt redaction. [covers=r[steel-tool-plugin-substrate.wasm-plugins.policy-preserved],r[steel-tool-plugin-substrate.wasm-plugins.fail-closed]] [evidence=evidence/v3-wasm.md]
- [x] [serial] V4: Run focused stdio plugin tests for success, progress, timeout, cancellation, disconnect, disable/restart race, restricted sandbox failure, and receipt redaction. [covers=r[steel-tool-plugin-substrate.stdio-plugins.lifecycle-preserved],r[steel-tool-plugin-substrate.stdio-plugins.sandbox-fail-closed]] [evidence=evidence/v4-stdio.md]
- [x] [serial] V5: Run focused subagent/delegate tests for success, denied-before-spawn, cancellation/kill, subprocess fallback, daemon actor mode, panel-event propagation, watchdog status, and receipt redaction. [covers=r[steel-tool-plugin-substrate.subagents.lifecycle-preserved],r[steel-tool-plugin-substrate.subagents.denied-no-spawn]] [evidence=evidence/v5-subagents.md]
- [x] [serial] V6: Add and run a deterministic dogfood rail that exercises one read-only built-in, one mutating/progress built-in, one WASM plugin, one stdio plugin, and one subagent/delegate call through default-mode Steel substrate dispatch with receipts under `target/steel-tool-plugin-substrate/`. [covers=r[steel-tool-plugin-substrate.verification.runtime-dogfood],r[steel-tool-plugin-substrate.catalog-policy.live-inventory],r[steel-tool-plugin-substrate.rollout.default-authorized-only]] [evidence=evidence/v6-dogfood.md]
- [x] [serial] V7: Run Cairn gates for `steel-tool-plugin-substrate`, `nix run .#cairn -- validate --root .`, docs build, focused cargo checks/tests touched by the substrate, and `git diff --check`. [covers=r[steel-tool-plugin-substrate.verification.boundary-rail],r[steel-tool-plugin-substrate.verification.runtime-dogfood]] [evidence=evidence/v7-final.md]
