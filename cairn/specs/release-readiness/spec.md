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

### Requirement: Harness evidence-index command [r[automated-current-head-release-evidence-index.harness-command]]
The repository MUST expose a developer-facing `./scripts/test-harness.sh evidence-index` mode that delegates to a Rust-owned evidence-index helper.

#### Scenario: Harness lists the evidence-index mode [r[automated-current-head-release-evidence-index.harness-command.listed]]
- GIVEN a developer runs `./scripts/test-harness.sh list`
- WHEN the harness prints available modes
- THEN it includes `evidence-index`
- AND it describes the mode as an index over existing local harness receipts rather than a replacement for running readiness profiles

#### Scenario: Harness delegates to Rust helper [r[automated-current-head-release-evidence-index.harness-command.delegates]]
- GIVEN a developer runs `./scripts/test-harness.sh evidence-index`
- WHEN the mode executes
- THEN the harness invokes the Rust-owned current-head release evidence helper
- AND the normal harness receipt records the delegated command and result

### Requirement: Current-head index generator [r[automated-current-head-release-evidence-index.generator]]
The evidence helper MUST gather current Git state, lifecycle state, and local harness receipts into a deterministic evidence index.

#### Scenario: Generator records repository state [r[automated-current-head-release-evidence-index.generator.repo-state]]
- GIVEN the helper runs in a Clankers checkout
- WHEN it writes an index
- THEN the index records branch, HEAD, upstream status, tag/describe distance, dirty status, and active Cairn/Cairn change directories

#### Scenario: Generator selects latest valid receipt per mode [r[automated-current-head-release-evidence-index.generator.receipt-selection]]
- GIVEN multiple harness run receipts exist under the result directory
- WHEN the helper evaluates receipts
- THEN it selects at most one latest passed receipt per mode using deterministic finished-time/run-id ordering
- AND it reports missing or invalid receipt classes without treating them as readiness passes

### Requirement: Fail-closed safety [r[automated-current-head-release-evidence-index.fail-closed]]
The helper MUST fail closed by default on unsafe or ambiguous promotion inputs.

#### Scenario: Dirty tracked worktree blocks default index [r[automated-current-head-release-evidence-index.fail-closed.dirty]]
- GIVEN the tracked worktree has uncommitted changes
- WHEN the helper runs without an explicit development override
- THEN it exits nonzero
- AND it does not present the checkout as clean current-HEAD evidence

#### Scenario: Bad receipts are not selected [r[automated-current-head-release-evidence-index.fail-closed.bad-receipt]]
- GIVEN a receipt has failed steps, no passed steps, invalid JSON, or references a missing log/summary/results artifact
- WHEN the helper scans local harness receipts
- THEN that receipt is rejected
- AND the index reports the corresponding evidence class as missing or invalid rather than passed

### Requirement: Deterministic output artifacts [r[automated-current-head-release-evidence-index.outputs]]
The helper MUST write deterministic JSON and Markdown artifacts under ignored `target/release-evidence/current-head/`.

#### Scenario: JSON and Markdown artifacts are emitted [r[automated-current-head-release-evidence-index.outputs.artifacts]]
- GIVEN acceptable repository state and local receipts
- WHEN the helper completes
- THEN it writes `index.json` with schema `clankers.current_head_release_evidence_index.v1`
- AND it writes `index.md` with the same payload HEAD, selected receipts, missing evidence classes, and non-claims

### Requirement: Harness receipts bind to payload commit [r[bind-harness-receipts-to-payload-commit.receipt-payload]]
Harness result receipts MUST record deterministic Git payload metadata captured once at harness start so downstream evidence can distinguish current-HEAD validation from historical local receipts.

#### Scenario: Harness result includes payload metadata [r[bind-harness-receipts-to-payload-commit.receipt-payload.emitted]]
- GIVEN a developer runs any `./scripts/test-harness.sh` mode
- WHEN the harness writes `results.json`
- THEN the receipt includes a top-level `payload.commit` equal to the HEAD being tested
- AND it records payload branch, describe string, tracked dirty state, upstream, and ahead/behind status when available

### Requirement: Evidence index verifies receipt payload commits [r[bind-harness-receipts-to-payload-commit.index-verification]]
The current-head evidence index MUST mark selected receipts as payload-commit verified only when their recorded payload commit matches the indexed HEAD and the receipt payload was captured from a clean tracked worktree.

#### Scenario: Matching clean payload receipt is current-head proof [r[bind-harness-receipts-to-payload-commit.index-verification.matching]]
- GIVEN a passed receipt records `payload.commit` equal to the current index HEAD
- AND the receipt records `payload.tracked_dirty=false`
- WHEN the evidence-index helper selects that receipt
- THEN it reports `payload_commit_verified=true`

#### Scenario: Legacy, dirty, or mismatched payload receipts are not overclaimed [r[bind-harness-receipts-to-payload-commit.index-verification.mismatch]]
- GIVEN a selected receipt has no payload metadata, a different payload commit, or `payload.tracked_dirty=true`
- WHEN the evidence-index helper writes the index
- THEN it reports `payload_commit_verified=false`
- AND it does not describe that receipt as current-HEAD validation

### Requirement: Payload binding documentation [r[bind-harness-receipts-to-payload-commit.docs]]
Release-readiness documentation MUST explain the payload metadata fields and the transition behavior for older receipts that lack payload metadata.

#### Scenario: Documentation describes legacy receipt semantics [r[bind-harness-receipts-to-payload-commit.docs.legacy]]
- GIVEN an operator reads the release-readiness reference
- WHEN it describes the current-head evidence index
- THEN it states that receipts lacking payload metadata may be selected as historical local evidence but cannot be marked payload-commit verified
