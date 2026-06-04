## Phase 1: Contract and fixture shape

- [x] [serial] [covers=embeddable-agent-engine.confirmation-broker-kit.boundary] [evidence=openspec validate brick-02-confirmation-broker-kit --strict --json] Finalize the proposal, design, and delta spec for `confirmation-broker-kit`. ✅ completed: 2026-05-19T02:22:54Z
- [x] [serial] [covers=embeddable-agent-engine.confirmation-broker-kit.boundary] [evidence=source anchor readback] Identified minimal anchors: `crates/clankers-runtime/src/confirmation.rs`, `crates/clankers-runtime/src/runtime.rs`, `docs/src/reference/embedding.md`, and `scripts/check-embedded-agent-sdk.sh`. The brick is a checked copyable example plus docs and embedded SDK rail, not a new extracted crate. ✅ completed: 2026-05-19T02:22:54Z

## Phase 2: Implementation evidence

- [x] [serial] [covers=embeddable-agent-engine.confirmation-broker-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo run --locked --manifest-path examples/confirmation-broker-kit/Cargo.toml] Implemented `examples/confirmation-broker-kit` with a positive approved action path and deterministic receipt hash `386df48f4689711a4fe434157c03b606a4ce96459bcde5dba9f26a00183f3530`. ✅ completed: 2026-05-19T02:22:54Z
- [x] [parallel] [covers=embeddable-agent-engine.confirmation-broker-kit.evidence] [evidence=negative fixture or fail-closed assertion] Added denial/default/unavailable broker assertions proving protected actions are not executed and secret-like confirmation summaries are redacted to `[REDACTED]`. ✅ completed: 2026-05-19T02:22:54Z
- [x] [parallel] [covers=embeddable-agent-engine.confirmation-broker-kit.drift] [evidence=docs/policy/generated inventory update or documented no-op] Documented `examples/confirmation-broker-kit/` in `docs/src/reference/embedding.md` and wired it into `scripts/check-embedded-agent-sdk.sh`; the script now defaults `TMPDIR` to `~/.cargo-target/tmp` for reproducible embedded verification on hosts with full `/tmp`. ✅ completed: 2026-05-19T02:22:54Z

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=embeddable-agent-engine.confirmation-broker-kit.evidence] [evidence=focused verification command] Ran `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo run --locked --manifest-path examples/confirmation-broker-kit/Cargo.toml`; output ended with `confirmation-broker-kit passed`. ✅ completed: 2026-05-19T02:22:54Z
- [x] [depends:implementation] [covers=embeddable-agent-engine.confirmation-broker-kit.drift] [evidence=cargo fmt --check && git diff --check] Ran `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --manifest-path examples/confirmation-broker-kit/Cargo.toml --check` and `git diff --check`. ✅ completed: 2026-05-19T02:22:54Z
- [x] [depends:implementation] [covers=embeddable-agent-engine.confirmation-broker-kit.boundary] [evidence=openspec validate embeddable-agent-engine --strict --json] Promoted the spec delta into `openspec/specs/embeddable-agent-engine/spec.md`; focused validation command is `openspec validate embeddable-agent-engine --strict --json`. ✅ completed: 2026-05-19T02:22:54Z
