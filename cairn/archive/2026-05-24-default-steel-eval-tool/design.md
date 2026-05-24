# Design: Default Steel eval tool publication

## Context

The accepted `steel-eval-agent-tool` contract already requires a pure default profile with zero ambient host authority. The implementation currently has that safe profile, but gates publication behind `steelEval.enabled = true` in settings. This makes the tool opt-in even though the default profile is intentionally bounded and auditable.

## Approach

Make the config default publish the tool:

- `SteelEvalSettings::default().enabled = true`.
- Deserializing empty settings should produce the same default-enabled safe profile.
- Users can still explicitly set `steelEval.enabled = false` to omit the tool.
- `build_tiered_tools` continues to register the tool through the existing settings path, so daemon/standalone/attach parity and disabled-tool filtering remain unchanged.

## Safety Boundary

Default publication is not authority escalation:

- The default profile keeps `max_host_calls = 0`.
- The default profile has no session capabilities.
- The default profile has no host functions.
- Runtime execution still routes through `clankers_runtime::steel_runtime`.
- Tool calls still emit deterministic receipts instead of raw host output.

## Verification

Focused tests cover:

- Empty JSON settings default to `steel_eval.enabled == true`.
- Default settings publish `steel_eval`.
- Explicit `steelEval.enabled = false` omits `steel_eval`.
- Disabled-tool filtering still removes `steel_eval` even when default-published.
