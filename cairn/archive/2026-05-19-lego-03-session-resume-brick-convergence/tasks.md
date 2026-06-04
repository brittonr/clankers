# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.session-resume-brick] [evidence=openspec validate lego-03-session-resume-brick-convergence --strict --json] Finalize the delta spec and design for this lego slice. ✅ completed: 2026-05-19T03:57:28Z; focused change validation passed.

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.session-resume-brick] [evidence=focused Rust/example check] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates. ✅ completed: 2026-05-19T03:57:28Z; added `examples/embedded-session-store/session-resume-evidence.json`, `scripts/check-session-resume-brick.rs`, and verified both `embedded-session-store` and `embedded-product-workbench` examples preserve restored follow-up context and fail closed for missing sessions.
- [x] [parallel] [covers=embedded-composition-kits.session-resume-brick] [evidence=policy/embedded-lego update or documented no-op] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts. ✅ completed: 2026-05-19T03:57:28Z; `policy/embedded-lego/lego-contracts.json` now points to the checked session-resume evidence fixture and `scripts/check-embedded-lego-contracts.rs` validates that reference.
- [x] [parallel] [covers=embedded-composition-kits.session-resume-brick] [evidence=BLAKE3 receipt/hash assertion or documented no-op] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection. ✅ completed: 2026-05-19T03:57:28Z; `scripts/check-session-resume-brick.rs` writes `target/embedded-sdk-release/session-resume-brick-receipt.json` with BLAKE3 hashes for fixture/source/docs/spec evidence.

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.session-resume-brick] [evidence=scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed. ✅ completed: 2026-05-19T03:57:28Z; `TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-embedded-agent-sdk.sh` passed.
- [x] [depends:implementation] [covers=embedded-composition-kits.session-resume-brick] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks. ✅ completed: 2026-05-19T03:57:28Z; `cargo fmt --check` and `git diff --check` passed.
- [x] [depends:implementation] [covers=embedded-composition-kits.session-resume-brick] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation. ✅ completed: 2026-05-19T03:57:28Z; `openspec validate embedded-composition-kits --strict --json` passed before archive.
