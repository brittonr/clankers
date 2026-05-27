# Steel Turn Planning Config Activation

Steel turn planning is activated from reviewed configuration instead of test-only `TurnConfig` wiring. The default Steel planner is the bundled reviewed `steel.host.plan_turn` profile/script. Explicitly disabled settings keep Rust-native planning and produce no Steel planning receipt.

## Settings surface

`Settings::steel_turn_planning` is the typed activation surface. A valid enabled config supplies:

- `profilePath`: path to a Nickel-exported Steel orchestration profile JSON.
- `scriptPath`: path to the reviewed Steel Scheme script for `steel.host.plan_turn`.
- optional `scriptBlake3` / `profileBlake3`: expected BLAKE3 hashes for fail-closed freshness checks.
- optional `rolloutStage`: `disabled`, `comparison`, or `default`.
- optional `fallbackMode`: `rust_native` or `block`.
- optional `planningSeam`: must remain `steel.host.plan_turn`.
- `sessionCapabilities` and `grantedUcanAbilities`: runtime authority actually present for the session/script context.
- `disabledActions`: user/session-disabled host actions.
- optional `receiptPrefix`: must stay under `target/`.
- optional `maxInputBytes` and `maxSourceBytes`: Rust-owned budget checks before Steel execution.

## Activation path

Both normal turns and orchestrated phase turns call the same Rust helper, `steel_turn_planning_config_from_settings(...)`, before constructing `TurnConfig`. The helper:

1. maps missing config to the bundled reviewed Steel profile/script;
2. maps explicit `enabled = false` to disabled/no Steel planning;
3. reads explicit profile/script paths relative to the current Clankers working directory unless paths are absolute;
4. computes or verifies profile/script BLAKE3 hashes;
5. rejects empty or over-budget scripts before interpreter execution;
6. parses only the reviewed Nickel-exported profile schema;
7. requires the seam and allowed host action to be exactly `steel.host.plan_turn`;
8. requires the configured session capabilities and UCAN ability to satisfy the profile;
9. rejects disabled required host actions;
10. constrains receipts to `target/steel-turn-planning-config-activation/...` or another `target/` prefix.

## Authority boundaries

Steel Scheme receives no ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session, or mutation authority. It can request turn planning only through the typed Rust host seam. Rust still owns provider calls, tool execution, fallback/block decisions, receipts, verification, and all host effects. Nickel owns declarative profile/config; UCAN/session state supplies runtime authority.

## Rollout behavior

- **Default:** missing settings use the bundled reviewed `steel.host.plan_turn` profile/script and emit redacted Steel planning receipts.
- **Disabled:** explicit `steelTurnPlanning.enabled = false` builds no Steel plan-turn config; Rust-native planning proceeds without a Steel receipt.
- **Comparison:** Steel runs and emits redacted planning evidence, but Rust-native execution remains selected.
- **Default:** Steel may select the planning result only after Rust parses typed output and receives authorized effect evidence.
- **Block:** malformed/denied Steel planning with block fallback stops before provider/tool effects.

## Receipts and redaction

The checker writes `target/steel-turn-planning-config-activation/receipt.json`. Receipts and docs intentionally contain hashes, paths, schema names, and policy/authority classes only: no raw prompts, provider payloads, credentials, compact UCAN proofs, script bodies, or tool bodies.
