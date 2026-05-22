# Steel Turn Planning Config Activation

Steel turn planning is now activated from reviewed configuration instead of test-only `TurnConfig` wiring. The default remains disabled: absent or disabled settings produce no Steel execution and no Steel planning receipt.

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

1. maps missing config to disabled/no Steel planning;
2. reads the profile and script relative to the current Clankers working directory unless paths are absolute;
3. verifies optional profile/script BLAKE3 hashes;
4. rejects empty or over-budget scripts before interpreter execution;
5. parses only the reviewed Nickel-exported profile schema;
6. requires the seam and allowed host action to be exactly `steel.host.plan_turn`;
7. requires the configured session capabilities and UCAN ability to satisfy the profile;
8. rejects disabled required host actions;
9. constrains receipts to `target/steel-turn-planning-config-activation/...` or another `target/` prefix.

## Authority boundaries

Steel Scheme receives no ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session, or mutation authority. It can request turn planning only through the typed Rust host seam. Rust still owns provider calls, tool execution, fallback/block decisions, receipts, verification, and all host effects. Nickel owns declarative profile/config; UCAN/session state supplies runtime authority.

## Rollout behavior

- **Disabled:** no Steel plan-turn config is built; Rust-native planning proceeds without a Steel receipt.
- **Comparison:** Steel runs and emits redacted planning evidence, but Rust-native execution remains selected.
- **Default:** Steel may select the planning result only after Rust parses typed output and receives authorized effect evidence.
- **Block:** malformed/denied Steel planning with block fallback stops before provider/tool effects.

## Receipts and redaction

The checker writes `target/steel-turn-planning-config-activation/receipt.json`. Receipts and docs intentionally contain hashes, paths, schema names, and policy/authority classes only: no raw prompts, provider payloads, credentials, compact UCAN proofs, script bodies, or tool bodies.
