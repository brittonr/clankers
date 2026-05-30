# Tasks: Steel Executor Runtime Smoke

## Phase 0: Audit

- [x] [serial] R1. Audit the existing embedded controller Steel runtime smoke and checker to find missing executor assertions. [covers=r[steel-executor-runtime-smoke.executor-visible]] [evidence=evidence/runtime-smoke-audit.md]

## Phase 1: Implementation

- [x] [serial] I1. Assert daemon-visible `executor=RustNative` in comparison-mode controller smoke and `executor=SteelScheme` in default-settings controller smoke. [covers=r[steel-executor-runtime-smoke.executor-visible.comparison], r[steel-executor-runtime-smoke.executor-visible.default]]
- [x] [serial] I2. Update runtime-smoke checker and docs to require executor evidence without changing authority semantics. [covers=r[steel-executor-runtime-smoke.executor-visible]]

## Phase 2: Verification

- [x] [serial] V1. Run focused embedded-controller Steel runtime smoke tests and the runtime-smoke checker. [covers=r[steel-executor-runtime-smoke.executor-visible.comparison], r[steel-executor-runtime-smoke.executor-visible.default]] [evidence=evidence/focused-runtime-smoke.md]
- [x] [serial] V2. Run formatting/diff hygiene plus Cairn gates, sync/archive, and validation. [covers=r[steel-executor-runtime-smoke.executor-visible]] [evidence=evidence/cairn-validation.md]
