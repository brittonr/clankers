## Phase 1: Contract and fixture shape

- [x] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.boundary] [evidence=openspec validate brick-09-self-evolution-receipt-chain-kit --strict --json] Finalize the reusable receipt-chain boundary: run, approval, application, rollback receipts are reusable validation evidence; CLI/app-edge execution and live mutation decisions remain product-owned.
- [x] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.chain] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test self_evolution_application] Pick deterministic fixtures that show run → approval → application → rollback receipt links without depending on live credentials, provider payloads, or hidden ambient sessions.

## Phase 2: Positive and negative evidence

- [x] [parallel] [covers=self-evolution-control.self-evolution-receipt-chain-kit.chain] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test self_evolution_application] Cover positive evidence for approval-without-apply, application preflight, and live backup/application receipt creation.
- [x] [parallel] [covers=self-evolution-control.self-evolution-receipt-chain-kit.fail-closed] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test self_evolution_application] Cover fail-closed receipt-chain guards: stale targets, mismatched/applied approvals, missing candidates, unsupported modes, and non-promotable receipts reject before mutation.

## Phase 3: Drift rail and docs

- [x] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.drift] [evidence=./scripts/check-self-evolution-receipt-chain-kit.rs] Add a focused drift rail that checks source, docs, tests, and spec anchors so receipt-chain contract changes update all evidence together.
- [x] [parallel] [covers=self-evolution-control.self-evolution-receipt-chain-kit.boundary] [evidence=docs/src/reference/request-lifecycle.md] Document the brick boundary, explicit chain order, fail-closed guards, and safe-evidence rules.

## Phase 4: Validation and archive

- [x] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check; ./scripts/check-self-evolution-receipt-chain-kit.rs; TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test self_evolution_application; openspec validate brick-09-self-evolution-receipt-chain-kit --strict --json; openspec validate self-evolution-control --strict --json; git diff --check] Run focused validation and record evidence in this task list.
- [x] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.evidence] [evidence=2026-05-19T03:26:27Z] Promote the spec delta into `openspec/specs/self-evolution-control/spec.md`, archive the change, and commit/push the drain slice.
