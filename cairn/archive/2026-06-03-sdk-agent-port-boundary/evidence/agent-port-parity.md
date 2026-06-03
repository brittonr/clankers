# Agent port parity evidence

Evidence-ID: sdk-agent-port-boundary-parity
Artifact-Type: command-output-summary
Task-ID: V1
Covers: sdk-agent-port-boundary.verification.parity
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent fake_runtime_service_bundle_turn_runs_without_desktop_systems
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
```

## Relevant output

```text
fake_runtime_service_bundle_turn_runs_without_desktop_systems
PASS clankers-agent turn::tests::fake_runtime_service_bundle_turn_runs_without_desktop_systems
Summary: 1 test run: 1 passed, 193 skipped

standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
PASS clankers-agent turn::tests::standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
Summary: 1 test run: 1 passed, 193 skipped
```

## Coverage notes

The fake runtime service bundle exercises `AgentRuntimeServices` without live desktop provider, database, hook, or skill discovery. The standalone shell adapter parity test exercises the compatibility adapter path and verifies persisted assistant output and terminal success behavior remain intact for the migrated port seam.
