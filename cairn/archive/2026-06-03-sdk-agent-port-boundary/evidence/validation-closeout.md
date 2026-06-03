# Agent port closeout validation evidence

Evidence-ID: sdk-agent-port-boundary-closeout
Artifact-Type: command-output-summary
Task-ID: V3
Covers: sdk-agent-port-boundary.verification,sdk-agent-port-boundary.verification.parity,sdk-agent-port-boundary.verification.boundary-rail
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent fake_runtime_service_bundle_turn_runs_without_desktop_systems
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
./scripts/check-agent-turn-port-boundary.rs
./scripts/check-lego-architecture-boundaries.rs
./scripts/check-embedded-sdk-deps.rs
nix run .#cairn -- gate proposal sdk-agent-port-boundary --root .
nix run .#cairn -- gate design sdk-agent-port-boundary --root .
nix run .#cairn -- gate tasks sdk-agent-port-boundary --root .
nix run .#cairn -- validate --root .
git diff --check
```

## Relevant output

```text
fake_runtime_service_bundle_turn_runs_without_desktop_systems
PASS clankers-agent turn::tests::fake_runtime_service_bundle_turn_runs_without_desktop_systems
Summary: 1 test run: 1 passed, 193 skipped

standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
PASS clankers-agent turn::tests::standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
Summary: 1 test run: 1 passed, 193 skipped

concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
PASS clankers-agent turn::ports::tests::concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
Summary: 1 test run: 1 passed, 193 skipped

./scripts/check-agent-turn-port-boundary.rs
ok: agent turn port boundary rail passed

./scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json

./scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 180 packages and excludes forbidden runtime crates

nix run .#cairn -- gate proposal sdk-agent-port-boundary --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- gate design sdk-agent-port-boundary --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- gate tasks sdk-agent-port-boundary --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- validate --root .
"valid": true

git diff --check
exit 0
```

## Coverage notes

The closeout bundle combines the focused agent parity tests, the concrete dependency budget test, agent-port and lego architecture rails, embedded SDK dependency slice, Cairn proposal/design/tasks gates, Cairn validation, and whitespace check for the completed `sdk-agent-port-boundary` change.
