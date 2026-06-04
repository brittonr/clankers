## Phase 1: Contract and fixture shape

- [x] [serial] [covers=durable-process-jobs.process-job-profile-kit.boundary] [evidence=openspec validate brick-10-process-job-profile-kit --strict --json] Finalize the proposal, design, and delta spec for `process-job-profile-kit`.
- [x] [serial] [covers=durable-process-jobs.process-job-profile-kit.boundary] [evidence=crates/clankers-runtime/src/process_jobs.rs; docs/src/reference/process-jobs.md] Identify the minimal source anchors and keep the brick as a runtime DTO/profile-policy fixture plus docs/spec drift rail, not a new backend or public app-edge integration.

## Phase 2: Implementation evidence

- [x] [serial] [covers=durable-process-jobs.process-job-profile-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime process_job_profile_kit_validates_manifest_policy_identity_and_redaction] Implement deterministic positive evidence that a manifest resolves to a backend-neutral `StartProcessJobRequest`, BLAKE3-native identity, notification policy, and safe metadata without backend dispatch.
- [x] [parallel] [covers=durable-process-jobs.process-job-profile-kit.fail-closed] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime process_job_profile_kit_validates_manifest_policy_identity_and_redaction] Add fail-closed assertions for secret-like environment keys and redacted secret command previews.
- [x] [parallel] [covers=durable-process-jobs.process-job-profile-kit.drift] [evidence=docs/src/reference/process-jobs.md; scripts/check-process-job-profile-kit.rs] Update docs and add a focused checker that ties source, docs, tests, and OpenSpec anchors together.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=durable-process-jobs.process-job-profile-kit.evidence] [evidence=./scripts/check-process-job-profile-kit.rs; cargo test -p clankers-runtime process_job_profile_kit_validates_manifest_policy_identity_and_redaction] Run the focused verification for `process-job-profile-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=durable-process-jobs.process-job-profile-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check; git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=durable-process-jobs.process-job-profile-kit.boundary] [evidence=openspec validate durable-process-jobs --strict --json; 2026-05-19T03:31:01Z] Promote the spec delta, validate the canonical spec, and archive the change when complete.
