# Steel Turn Planning Config Activation Design

## Architecture

The activation path is intentionally narrow:

```text
settings / Nickel-exported profile
  -> clankers-config typed settings
  -> clankers-agent Rust loader/builder
  -> AgentTurnSteelPlanningConfig
  -> existing turn::run_turn_loop Steel planning hook
  -> clankers-runtime::steel_orchestration
  -> Rust-owned provider/tool/effect execution
```

Steel Scheme is not loaded directly by config code and does not gain ambient authority. Configuration selects a reviewed profile and script binding; Rust validates and constructs the typed adapter config.

## Data ownership

- Nickel owns declarative profile intent: enabled flag, rollout stage, fallback mode, seam name, script binding, budgets, receipt policy, required session capabilities, and UCAN ability name.
- Rust owns deserialization, path normalization, hash checks, file reads, plan payload construction, receipt emission, provider/tool execution, and errors.
- UCAN/session state owns runtime grants; configuration may name required abilities but does not itself mint authority.
- Steel owns only the trusted planning logic behind the reviewed host seam and returns typed plan data.

## Implementation notes

- Defaults remain disabled. Missing config must map to `None`.
- The loader should reject unknown seam names, invalid rollout/fallback values, empty hashes, missing scripts, hash mismatches, overly large scripts, or profiles requesting unsupported host actions.
- The script source should be passed as the already-supported host invocation form or loaded profile script as bounded source only after validation. Raw script body must not be persisted in receipts.
- The resulting adapter config must include only safe session capability/UCAN metadata and disabled-action policy needed by `clankers-runtime::steel_orchestration`.
- Normal turns and model-role/orchestrated phase turns should use the same activation helper to avoid drift.

## Verification

Focused tests should cover:

- no config remains disabled and emits no Steel-authored decision receipt;
- comparison mode config emits a Steel planning receipt but keeps Rust-native execution;
- default mode config selects Steel only after Rust/runtime authorization;
- invalid profile/script/hash/config fails closed to `None` or a stable configuration error before the turn starts;
- denied/malformed planning remains blocked or fallback-controlled by existing runtime policy;
- receipts remain deterministic and redacted.

A checker rail should inspect the settings surface, activation helper, turn call sites, docs, and accepted spec markers, and should write its receipt under `target/steel-turn-planning-config-activation/`.
