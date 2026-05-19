## Phase 1: Contract and fixture shape

- [ ] [serial] [covers=slash-command-composition.slash-command-routing-kit.boundary] [evidence=openspec validate brick-04-slash-command-routing-kit --strict --json] Finalize the proposal, design, and delta spec for `slash-command-routing-kit`.
- [ ] [serial] [covers=slash-command-composition.slash-command-routing-kit.boundary] [evidence=source anchor readback] Identify the minimal source anchors and decide whether the brick is an example, policy/manifest, generated inventory, receipt validator, focused test, or a combination.

## Phase 2: Implementation evidence

- [ ] [serial] [covers=slash-command-composition.slash-command-routing-kit.evidence] [evidence=focused Rust/example/checker command] Implement the narrowest deterministic brick evidence for `slash-command-routing-kit` with at least one positive path.
- [ ] [parallel] [covers=slash-command-composition.slash-command-routing-kit.evidence] [evidence=negative fixture or fail-closed assertion] Add one fail-closed, denial, drift, or redaction case for the brick.
- [ ] [parallel] [covers=slash-command-composition.slash-command-routing-kit.drift] [evidence=docs/policy/generated inventory update or documented no-op] Update docs, policy, generated inventory, or receipt schemas that advertise the brick.

## Phase 3: Validation and archive

- [ ] [depends:implementation] [covers=slash-command-composition.slash-command-routing-kit.evidence] [evidence=focused verification command] Run the focused verification for `slash-command-routing-kit` and capture the command in the archive note.
- [ ] [depends:implementation] [covers=slash-command-composition.slash-command-routing-kit.drift] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [ ] [depends:implementation] [covers=slash-command-composition.slash-command-routing-kit.boundary] [evidence=openspec validate slash-command-composition --strict --json] Promote the spec delta, validate the canonical spec, and archive the change when complete.
