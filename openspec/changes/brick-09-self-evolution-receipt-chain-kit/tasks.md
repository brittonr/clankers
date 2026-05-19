## Phase 1: Contract and fixture shape

- [ ] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.boundary] [evidence=openspec validate brick-09-self-evolution-receipt-chain-kit --strict --json] Finalize the proposal, design, and delta spec for `self-evolution-receipt-chain-kit`.
- [ ] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.boundary] [evidence=source anchor readback] Identify the minimal source anchors and decide whether the brick is an example, policy/manifest, generated inventory, receipt validator, focused test, or a combination.

## Phase 2: Implementation evidence

- [ ] [serial] [covers=self-evolution-control.self-evolution-receipt-chain-kit.evidence] [evidence=focused Rust/example/checker command] Implement the narrowest deterministic brick evidence for `self-evolution-receipt-chain-kit` with at least one positive path.
- [ ] [parallel] [covers=self-evolution-control.self-evolution-receipt-chain-kit.evidence] [evidence=negative fixture or fail-closed assertion] Add one fail-closed, denial, drift, or redaction case for the brick.
- [ ] [parallel] [covers=self-evolution-control.self-evolution-receipt-chain-kit.drift] [evidence=docs/policy/generated inventory update or documented no-op] Update docs, policy, generated inventory, or receipt schemas that advertise the brick.

## Phase 3: Validation and archive

- [ ] [depends:implementation] [covers=self-evolution-control.self-evolution-receipt-chain-kit.evidence] [evidence=focused verification command] Run the focused verification for `self-evolution-receipt-chain-kit` and capture the command in the archive note.
- [ ] [depends:implementation] [covers=self-evolution-control.self-evolution-receipt-chain-kit.drift] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [ ] [depends:implementation] [covers=self-evolution-control.self-evolution-receipt-chain-kit.boundary] [evidence=openspec validate self-evolution-control --strict --json] Promote the spec delta, validate the canonical spec, and archive the change when complete.
