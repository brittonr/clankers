## Phase 1: Contract and fixture shape

- [x] [serial] [covers=batch-trajectory-runner.batch-eval-runner-kit.boundary] [evidence=openspec validate brick-03-batch-eval-runner-kit --strict --json] Finalize the proposal, design, and delta spec for `batch-eval-runner-kit`.
- [x] [serial] [covers=batch-trajectory-runner.batch-eval-runner-kit.boundary] [evidence=source anchor readback: src/modes/batch.rs; docs/src/getting-started/quickstart.md; scripts/check-batch-eval-runner-kit.rs] Identify the minimal source anchors and decide whether the brick is an example, policy/manifest, generated inventory, receipt validator, focused test, or a combination.

## Phase 2: Implementation evidence

- [x] [serial] [covers=batch-trajectory-runner.batch-eval-runner-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' RUSTC_WRAPPER= cargo test --lib batch_eval_runner_kit_fixture_validates_manifest_resume_and_redaction] Implement the narrowest deterministic brick evidence for `batch-eval-runner-kit` with at least one positive path.
- [x] [parallel] [covers=batch-trajectory-runner.batch-eval-runner-kit.evidence] [evidence=batch_eval_runner_kit_fixture_validates_manifest_resume_and_redaction asserts resume skip, eval JSONL redaction, and s3:// remote input rejection] Add one fail-closed, denial, drift, or redaction case for the brick.
- [x] [parallel] [covers=batch-trajectory-runner.batch-eval-runner-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-batch-eval-runner-kit.rs; docs/src/getting-started/quickstart.md updated] Update docs, policy, generated inventory, or receipt schemas that advertise the brick.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=batch-trajectory-runner.batch-eval-runner-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-batch-eval-runner-kit.rs && cargo test --lib batch_eval_runner_kit_fixture_validates_manifest_resume_and_redaction] Run the focused verification for `batch-eval-runner-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=batch-trajectory-runner.batch-eval-runner-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=batch-trajectory-runner.batch-eval-runner-kit.boundary] [evidence=openspec validate batch-trajectory-runner --strict --json; archived 2026-05-19T02:33:22Z] Promote the spec delta, validate the canonical spec, and archive the change when complete.
