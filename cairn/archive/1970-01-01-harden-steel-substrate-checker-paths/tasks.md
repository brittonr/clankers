# Tasks: Harden Steel Substrate Checker Paths

## Phase 1: Implementation

- [x] [serial] I1: Add active-to-archived task path resolution and active-to-canonical spec path resolution in `scripts/check-steel-tool-plugin-substrate.rs`, while preserving existing marker validation and receipt hashing. [covers=r[steel-tool-plugin-substrate.checker-paths.active-archive-resolution],r[steel-tool-plugin-substrate.checker-paths.receipt-artifacts]]

## Phase 2: Verification

- [x] [serial] V1: Run `./scripts/check-steel-tool-plugin-substrate.rs` and capture evidence that it writes `target/steel-tool-plugin-substrate/receipt.json` using archived task plus canonical spec artifacts after the active change directory is absent. [covers=r[steel-tool-plugin-substrate.checker-paths.active-archive-resolution],r[steel-tool-plugin-substrate.checker-paths.receipt-artifacts]] [evidence=evidence/v1-checker.md]
- [x] [serial] V2: Run Cairn gates/validation for `harden-steel-substrate-checker-paths` before archive. [covers=r[steel-tool-plugin-substrate.checker-paths.active-archive-resolution],r[steel-tool-plugin-substrate.checker-paths.receipt-artifacts]] [evidence=evidence/v2-cairn.md]
