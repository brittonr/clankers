# Tasks

## Implementation
- [x] [serial] r[automated-current-head-release-evidence-index.harness-command] Add `./scripts/test-harness.sh evidence-index` and inventory documentation.
- [x] [serial] r[automated-current-head-release-evidence-index.generator] Add Rust helper that gathers Git state, lifecycle state, and latest valid harness receipts.
- [x] [serial] r[automated-current-head-release-evidence-index.fail-closed] Fail closed by default on dirty tracked state, failed/missing receipts, and missing referenced receipt artifacts while allowing an explicit development override.
- [x] [serial] r[automated-current-head-release-evidence-index.outputs] Emit deterministic JSON and Markdown index artifacts under `target/release-evidence/current-head/`.

## Verification
- [x] [parallel] r[automated-current-head-release-evidence-index.harness-command.listed] Update harness contract tests for the new mode and list documentation.
- [x] [parallel] r[automated-current-head-release-evidence-index.outputs.artifacts] Run the helper/harness smoke and inspect generated artifacts.
- [x] [serial] r[automated-current-head-release-evidence-index.fail-closed.dirty] Validate and gate the Cairn package before archive.
