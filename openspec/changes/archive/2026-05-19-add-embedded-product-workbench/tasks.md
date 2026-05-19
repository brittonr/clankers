# Tasks

## 1. Contract and scaffold

- [x] [covers=embedded-composition-kits.product-workbench.combined-seams] [evidence=openspec validate add-embedded-product-workbench --strict --json] Define the combined product-workbench recipe contract.

## 2. Implementation

- [x] [covers=embedded-composition-kits.product-workbench.example] [evidence=cargo run --locked --manifest-path examples/embedded-product-workbench/Cargo.toml] Add the standalone executable product-workbench example.
- [x] [covers=embedded-composition-kits.product-workbench.fail-closed] [evidence=cargo run --locked --manifest-path examples/embedded-product-workbench/Cargo.toml] Cover missing-session and dangerous-tool fail-closed paths.
- [x] [covers=embedded-composition-kits.acceptance-rail.one-command] [evidence=scripts/check-embedded-agent-sdk.sh] Add the example to the embedded SDK acceptance rail.
- [x] [covers=embedded-composition-kits.acceptance-rail.release-receipt.artifacts] [evidence=scripts/emit-embedded-sdk-release-receipt.rs --output target/embedded-sdk-release/test-receipt.json] Include product-workbench artifacts in the release receipt hash set.
- [x] [covers=embedded-composition-kits.recipes.crate-guidance] [evidence=docs/src/tutorials/embedded-agent-sdk.md] Document the combined recipe as product dogfood evidence, not a new generic storage/provider API.

## 3. Verification and archive

- [x] [covers=embedded-composition-kits.product-workbench.example] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [covers=embedded-composition-kits.product-workbench.example] [evidence=cargo check --workspace --all-targets] Run the broad Rust check.
- [x] [covers=embedded-composition-kits.product-workbench.combined-seams] [evidence=openspec validate embedded-composition-kits --strict --json] Archive the completed change and validate the canonical spec.
