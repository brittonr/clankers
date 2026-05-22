# Tasks: Steel Agent Turn Wiring

## Planning and policy selection

- [ ] [serial] I1: Add or identify the Rust-owned real agent-turn planning port and policy selection point for `steel.host.plan_turn`, preserving Rust-native planning as disabled/comparison fallback [r[steel-agent-turn-wiring.turn-planning-port]] [r[steel-agent-turn-wiring.policy-selected-mode]]
- [ ] [parallel] I2: Add policy/profile fixture coverage for disabled, comparison, and explicit default mode selection without allowing scripts to self-select default status, host functions, budgets, or fallback policy [r[steel-agent-turn-wiring.policy-selected-mode.comparison]] [r[steel-agent-turn-wiring.policy-selected-mode.default]]

## Agent-turn adapter wiring

- [ ] [serial] I3: Wire the real agent-turn shell to call a small Rust adapter that delegates to `clankers-runtime::steel_orchestration` and never imports Steel interpreter internals into agent/controller/daemon/TUI/provider/tool-host shells [r[steel-agent-turn-wiring.turn-planning-port.enabled]]
- [ ] [parallel] I4: Convert real turn context into bounded, redacted, hashable `TurnPlanningInput` material and reject malformed/over-budget/unredactable Steel output before any host effect [r[steel-agent-turn-wiring.typed-turn-plan]] [r[steel-agent-turn-wiring.typed-turn-plan.malformed]]
- [ ] [serial] I5: Support comparison mode receipts where Steel planning runs but Rust-native planning remains the execution oracle [r[steel-agent-turn-wiring.policy-selected-mode.comparison]] [r[steel-agent-turn-wiring.dogfood-evidence.real-path]]

## Authorization and fallback

- [ ] [serial] I6: Route every effectful Steel plan item through existing Rust dynamic-runtime/session/UCAN/disabled-tool/provider/tool/mutation authorization seams before execution [r[steel-agent-turn-wiring.rust-authorized-effects.allowed]]
- [ ] [parallel] I7: Add denial fixtures for unknown, disabled, unauthorized, over-budget, provider, credential, daemon, TUI, native-tool, filesystem, process, git, network, and mutation attempts [r[steel-agent-turn-wiring.rust-authorized-effects.denied]]
- [ ] [serial] I8: Implement explicit fallback/blocked receipts for disabled profile, script load/eval/parse failure, malformed output, denied authorization, and fallback-disabled policy [r[steel-agent-turn-wiring.fallback-receipts.allowed]] [r[steel-agent-turn-wiring.fallback-receipts.blocked]]

## Evidence, docs, and gates

- [ ] [parallel] D1: Update architecture/runtime docs and checker rails to describe the real agent-turn wiring seam, comparison/default modes, no-ambient-authority boundary, and fallback/receipt review path [r[steel-agent-turn-wiring.dogfood-evidence]]
- [ ] [serial] G1: Add fixture-backed Rust tests proving enabled real-path invocation, disabled Rust-native path, comparison/default selection, fallback/blocking, denied-effect behavior, and repeated stable redacted receipts [r[steel-agent-turn-wiring.dogfood-evidence.real-path]] [r[steel-agent-turn-wiring.dogfood-evidence.stable-redacted]]
- [ ] [serial] G2: Run focused runtime/agent/controller tests for the wired seam plus existing Steel orchestration/runtime/dynamic-runtime rails [r[steel-agent-turn-wiring.typed-turn-plan.accepted]]
- [ ] [serial] G3: Run Cairn validate and proposal/design/tasks gates for `steel-agent-turn-wiring` and inspect validity/verdict [r[steel-agent-turn-wiring.dogfood-evidence]]
- [ ] [serial] G4: Run `git diff --check` before commit [r[steel-agent-turn-wiring.fallback-receipts]]
