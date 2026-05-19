## Phase 1: Contract and fixture shape

- [x] [serial] [covers=slash-command-composition.slash-command-routing-kit.boundary] [evidence=openspec validate brick-04-slash-command-routing-kit --strict --json] Finalize the proposal, design, and delta spec for `slash-command-routing-kit`.
- [x] [serial] [covers=slash-command-composition.slash-command-routing-kit.boundary] [evidence=source anchor readback: src/slash_commands/mod.rs; src/slash_commands/tests.rs; src/modes/attach/commands.rs; docs/src/reference/commands.md; scripts/check-slash-command-routing-kit.rs] Identify the minimal source anchors and decide whether the brick is an example, policy/manifest, generated inventory, receipt validator, focused test, or a combination.

## Phase 2: Implementation evidence

- [x] [serial] [covers=slash-command-composition.slash-command-routing-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' RUSTC_WRAPPER= cargo test --lib slash_command_routing_kit_detects_conflicts_and_prompt_template_fallback] Implement the narrowest deterministic brick evidence for `slash-command-routing-kit` with at least one positive path.
- [x] [parallel] [covers=slash-command-composition.slash-command-routing-kit.evidence] [evidence=slash_command_routing_kit_detects_conflicts_and_prompt_template_fallback asserts plugin conflict winner, prompt-template fallback, invalid dotted command rejection, and >64-char command rejection] Add one fail-closed, denial, drift, or redaction case for the brick.
- [x] [parallel] [covers=slash-command-composition.slash-command-routing-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-slash-command-routing-kit.rs; docs/src/reference/commands.md updated] Update docs, policy, generated inventory, or receipt schemas that advertise the brick.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=slash-command-composition.slash-command-routing-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-slash-command-routing-kit.rs && cargo test --lib slash_command_routing_kit_detects_conflicts_and_prompt_template_fallback] Run the focused verification for `slash-command-routing-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=slash-command-composition.slash-command-routing-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=slash-command-composition.slash-command-routing-kit.boundary] [evidence=openspec validate slash-command-composition --strict --json; archived 2026-05-19T02:38:32Z] Promote the spec delta, validate the canonical spec, and archive the change when complete.
