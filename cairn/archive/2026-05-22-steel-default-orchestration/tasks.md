# Tasks: Steel Default Orchestration

## Policy and profile seam

- [x] [serial] I1: Add a Nickel-authored orchestration profile/policy that declares enabled/default state, exact planning seams, script identities and hashes, runtime budgets, fallback mode, allowed host actions, receipt redaction, rollout stage, and audit metadata [r[steel-default-orchestration.policy-selected-default]]
- [x] [parallel] I2: Add a policy export/check rail and positive/negative fixtures for disabled profiles, unsupported seams, missing script hashes, overbroad host actions, missing fallback policy, and unsafe receipt redaction [r[steel-default-orchestration.policy-selected-default.disabled]] [r[steel-default-orchestration.policy-selected-default.named-seam]]

## Rust planner and typed plan seams

- [x] [serial] I3: Add Rust-owned planner DTOs and interface for `TurnPlanningInput`, `OrchestrationPlan`, `OrchestrationDecision`, `OrchestrationPlanReceipt`, and Rust-native fallback status without I/O in the pure core [r[steel-default-orchestration.rust-adapter-seam.interface]] [r[steel-default-orchestration.typed-plan-output]]
- [x] [serial] I4: Implement a Steel planner adapter that calls the existing Steel runtime wrapper and converts script output into typed plans; controller/agent/daemon/TUI/provider callers must depend on the planner interface, not interpreter APIs [r[steel-default-orchestration.rust-adapter-seam.wrapper-only]]
- [x] [parallel] I5: Add an architecture checker proving CLI, daemon, TUI, attach, controller, provider, and tool-host shells do not import or instantiate Steel interpreter internals directly and only call the wrapper/planner seam [r[steel-default-orchestration.rust-adapter-seam]]

## Authorization, fallback, and receipts

- [x] [serial] I6: Route every effectful Steel plan item through existing Rust authorization seams: dynamic-runtime envelopes, session/disabled-tool checks, Nickel policy, UCAN-style authority, provider/router ownership, and mutation preflight/apply/rollback for mutation actions [r[steel-default-orchestration.rust-authorized-effects.allowed-envelope]]
- [x] [parallel] I7: Add denial fixtures for unknown host action, disabled action, over-budget profile, missing UCAN/session capability, direct provider/credential/daemon/TUI/native-tool attempts, and hot-reload attempts that add authority [r[steel-default-orchestration.rust-authorized-effects.denied-envelope]]
- [x] [serial] I8: Implement explicit fallback receipts for script load/eval/parse failure and fallback-disabled blocking, without loosening Steel profiles or leaking raw script/provider/credential material [r[steel-default-orchestration.fallback-and-receipts.script-failure]] [r[steel-default-orchestration.fallback-and-receipts.fallback-disabled]]

## Rollout evidence and first default seam

- [x] [serial] I9: Select the first low-risk default seam, such as tool-candidate ordering or host-action proposal, and wire Steel in comparison mode with Rust-native planner output preserved as fallback/oracle [r[steel-default-orchestration.rollout-evidence]]
- [x] [parallel] I10: Add deterministic comparison receipts that record Steel plan hash, Rust-native decision class, authorized/denied effect summaries, fallback status, profile/script/policy hashes, and repeated-run stability [r[steel-default-orchestration.rollout-evidence.comparison-receipt]]
- [x] [serial] I11: Promote the selected seam to default only through a reviewed policy/profile update after comparison evidence is passing; additional seams must require separate reviewed profile entries and fixtures [r[steel-default-orchestration.rollout-evidence.reviewed-expansion]]

## Documentation and gates

- [x] [parallel] D1: Document operator workflow, policy review checklist, profile toggles, fallback/kill-switch behavior, receipt review, and no-sandbox/no-authority wording [r[steel-default-orchestration.fallback-and-receipts]]
- [x] [serial] G1: Run focused Rust checks for the planner DTOs/adapters and existing Steel/dynamic-runtime rails [r[steel-default-orchestration.typed-plan-output]]
- [x] [serial] G2: Run the orchestration policy/checker rails [r[steel-default-orchestration.policy-selected-default]]
- [x] [serial] G3: Run `nix run .#cairn -- validate --root .` [r[steel-default-orchestration.rollout-evidence]]
- [x] [serial] G4: Run Cairn proposal/design/tasks gates for `steel-default-orchestration` and inspect receipt validity/verdict [r[steel-default-orchestration.rollout-evidence.reviewed-expansion]]
- [x] [serial] G5: Run `git diff --check` before commit [r[steel-default-orchestration.fallback-and-receipts]]
