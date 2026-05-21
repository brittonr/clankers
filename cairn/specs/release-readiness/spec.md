# release-readiness Specification

## Purpose
Define the release-readiness gates that must be owned by Rust tests and discoverable through `cargo nextest`, including credential-free E2E coverage plus explicit opt-in live, VM, and flake/CI checks. This specification prevents Bash harnesses from becoming the source of truth for release-readiness assertions.

## Requirements

### Requirement: Nextest release-readiness matrix [r[release-readiness.nextest-matrix]]
The repository MUST expose release-readiness coverage as Rust tests discoverable by `cargo nextest`. The coverage MUST include default credential-free readiness, opt-in live-provider readiness, opt-in NixOS VM readiness, and opt-in flake/CI readiness without relying on Bash scripts as the assertion engine.

#### Scenario: Default nextest run includes credential-free readiness [r[release-readiness.nextest-matrix.default-offline]]
- GIVEN a developer runs `cargo nextest run --workspace --no-fail-fast` without credentials or live hosts
- WHEN the readiness tests are discovered
- THEN credential-free readiness tests for fake/deterministic CLI flows are present and run
- AND live, VM, and flake-heavy tests either self-skip with explicit prerequisite messages or are profile-gated with documented nextest filters

#### Scenario: Readiness inventory fails closed [r[release-readiness.nextest-matrix.inventory]]
- GIVEN a required readiness surface exists in this specification
- WHEN the readiness inventory test runs
- THEN it fails if the surface is absent from Rust test inventory
- AND it fails if the only assertion path is a Bash script selector

### Requirement: Rust-owned E2E CLI coverage [r[release-readiness.rust-e2e]]
The repository MUST implement CLI E2E coverage as Rust integration tests that execute the compiled `clankers` binary with isolated temp homes/workdirs, bounded timeouts, deterministic fake-provider settings, and direct assertions on exit status, stdout/stderr, JSON output, file effects, and redaction.

#### Scenario: Fake provider print and tool flows run without real credentials [r[release-readiness.rust-e2e.fake-provider-tools]]
- GIVEN no real provider credential is available
- WHEN the Rust E2E tests run fake-provider print, read, write/edit, and JSON-output flows
- THEN the binary exits successfully for expected success cases
- AND the tests assert deterministic output or side effects without invoking OAuth, browser login, or network providers

#### Scenario: CLI failures are asserted structurally [r[release-readiness.rust-e2e.failure-structure]]
- GIVEN an E2E scenario expects an error or unsupported input
- WHEN the Rust E2E test invokes the binary
- THEN it asserts the nonzero status and bounded redacted stderr/stdout shape
- AND it does not pass solely because a substring appeared in an unstructured shell log

### Requirement: Opt-in VM readiness tests [r[release-readiness.vm-nextest]]
The repository MUST represent NixOS VM readiness checks as Rust integration tests runnable by `cargo nextest`. These tests MUST invoke the flake VM checks under explicit opt-in gating, assert command status and bounded redacted diagnostics, and cover `vm-smoke`, `vm-remote-daemon`, `vm-session-recovery`, `vm-plugin-runtime`, `vm-module-daemon`, `vm-module-router`, and `vm-module-integration`.

#### Scenario: VM readiness dispatches every required flake check [r[release-readiness.vm-nextest.all-checks]]
- GIVEN VM readiness is explicitly enabled on a host capable of running NixOS VM checks
- WHEN the VM readiness nextest filter runs
- THEN every required VM check is invoked by name through a bounded Rust command helper
- AND each command receipt is asserted as pass or reported as a failing test with redacted diagnostics

#### Scenario: VM readiness remains safe by default [r[release-readiness.vm-nextest.safe-default]]
- GIVEN VM readiness is not explicitly enabled
- WHEN the default workspace nextest run discovers the VM readiness tests
- THEN the tests do not boot VMs by default
- AND the inventory still proves the VM readiness tests exist and documents the opt-in filter/env required to run them

### Requirement: Opt-in flake/CI readiness tests [r[release-readiness.flake-nextest]]
The repository MUST represent flake/CI readiness as a Rust integration test runnable by `cargo nextest` under explicit opt-in gating. The test MUST invoke `nix flake check` with bounded redacted diagnostics and MUST distinguish an unset opt-in gate from a passed flake check.

#### Scenario: Flake readiness invokes CI-equivalent check [r[release-readiness.flake-nextest.invokes-check]]
- GIVEN flake readiness is explicitly enabled
- WHEN the flake readiness nextest filter runs
- THEN the test invokes `nix flake check` from the repository root
- AND it fails with bounded redacted diagnostics if the command fails

#### Scenario: Flake readiness is not overclaimed when gated off [r[release-readiness.flake-nextest.gated-off]]
- GIVEN flake readiness is not explicitly enabled
- WHEN the default workspace nextest run completes
- THEN the release-readiness report MUST NOT claim `nix flake check` passed unless the opt-in test actually ran
- AND docs MUST show the exact nextest filter/env needed to run it

### Requirement: Bash scripts are convenience wrappers only [r[release-readiness.no-bash-source-of-truth]]
Bash scripts MAY remain as developer convenience wrappers, but release-readiness assertions MUST live in Rust tests or Rust-owned helpers that are exercised by `cargo nextest`.

#### Scenario: Script wrapper delegates rather than asserts [r[release-readiness.no-bash-source-of-truth.wrapper-delegates]]
- GIVEN a legacy script such as `tests/e2e/run-tests.sh` remains in the repository
- WHEN it is used for compatibility
- THEN it delegates to documented nextest filters or is labeled non-canonical
- AND it does not remain the only implementation of any release-readiness assertion

### Requirement: Opt-in live readiness tests [r[release-readiness.live-nextest]]
The repository MUST represent live local-model readiness as Rust integration tests runnable by `cargo nextest` with explicit opt-in gates, short availability probes, bounded generation timeouts, and no implicit OAuth/browser login flows. For Clankers testing, dogfood, and release-readiness slices that require a live model, qwen on aspen2 MUST be the primary live test model path unless a task explicitly scopes a different provider.

#### Scenario: Qwen on aspen2 is the primary live testing model [r[release-readiness.live-nextest.qwen-aspen2-primary]]
- GIVEN a Clankers testing, dogfood, or release-readiness slice needs live model evidence
- WHEN an operator follows the release-readiness documentation or harness inventory
- THEN the documented primary live model path SHALL be qwen on aspen2 through the `aspen2-qwen36` harness selector
- AND OpenAI OAuth/Codex-backed checks SHALL NOT be substituted for this live testing path unless the task explicitly requests that provider

#### Scenario: Live readiness runs against configured local model [r[release-readiness.live-nextest.local-model]]
- GIVEN live readiness is explicitly enabled and a configured OpenAI-compatible local model endpoint is available
- WHEN the nextest live readiness filter runs
- THEN the test sends a bounded request through the routed provider path
- AND it asserts a deterministic completion or stream-shape contract

#### Scenario: Live readiness is explicit when unavailable [r[release-readiness.live-nextest.unavailable]]
- GIVEN live readiness is not explicitly enabled or the configured model endpoint is unavailable
- WHEN the live readiness test is discovered
- THEN it reports an explicit skip/prerequisite message
- AND it does not mark the endpoint as verified
