Evidence-ID: architecture-ownership-rails
Task-ID: V3
Artifact-Type: command-log
Covers: remaining-coupling-drain.root-shell-policy, remaining-coupling-drain.agent-concrete-dependencies, remaining-coupling-drain.process-job-policy, remaining-coupling-drain.controller-command-seams, remaining-coupling-drain.daemon-actor-assembly, remaining-coupling-drain.display-protocol-dto-leakage, remaining-coupling-drain.provider-router-convergence
Status: complete

# Architecture Ownership Rails

Implemented the remaining drain slices:

- Root/process policy: moved native process admission decision/summary policy from the root `src/tools/process.rs` tool into `clankers-runtime::process_jobs` as `ProcessJobNativeAdmissionDecision` and `native_process_job_admission_decision(...)`.
- Agent concrete dependencies: removed direct `clanker-router` and `clanker-tui-types` normal dependencies from `clankers-agent`; tool definitions now come from `clanker-message`, provider models from `clankers-provider`, and tool progress uses neutral agent DTOs.
- Process-job policy: root process tool now calls the runtime-owned native admission policy and remains the native registry/service shell.
- Controller command seam: extracted protocol image to provider-content translation into `crates/clankers-controller/src/command_images.rs` with focused tests.
- Daemon actor assembly: moved UCAN/default capability merge policy into socketless `src/modes/daemon/session_builder.rs` with a socketless fixture.
- Display/protocol DTO leakage: replaced agent/root-tool use of `clanker-tui-types::ToolProgress` with neutral agent progress DTOs and projected to TUI types only in `src/event_translator.rs`.
- Provider/router convergence: removed the duplicate RPC-local router request builder wrapper and delegated directly to `router_request_bridge::build_router_request`.
- Architecture rail: updated `scripts/check-lego-architecture-boundaries.rs` to follow thinking-cycle policy ownership through `src/slash_commands/effects.rs`, and updated the dependency ownership baseline to show the reduced agent dependency budget.

## Focused validation

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo check -p clankers-agent -p clankers-controller -p clankers --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-agent structured_progress
```

Result: 2 tests run, 2 passed, 187 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-runtime native_admission_decision
```

Result: 1 test run, 1 passed, 173 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-controller protocol_images_project_to_provider_image_content
```

Result: 1 test run, 1 passed, 232 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers session_capability_merge
```

Result: 1 test run, 1 passed, 1530 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers test_translate_tool_progress_update_projects_neutral_agent_progress_to_tui_edge
```

Result: 1 test run, 1 passed, 1530 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-provider rpc_request
```

Result: 2 tests run, 2 passed, 175 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  ./scripts/check-lego-architecture-boundaries.rs
```

Result: exit status 0; wrote `target/lego-architecture/dependency-ownership-inventory.json`.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp rustfmt --check \
  crates/clankers-agent/src/tool.rs \
  src/tools/progress.rs \
  src/event_translator.rs \
  crates/clankers-controller/src/command_images.rs \
  crates/clankers-controller/src/command.rs \
  crates/clankers-runtime/src/process_jobs.rs \
  src/tools/process.rs \
  src/modes/daemon/agent_process.rs \
  src/modes/daemon/session_builder.rs \
  crates/clankers-provider/src/rpc_provider.rs
```

Result: exit status 0.
