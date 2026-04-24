# group-a-leaves Specification

## Purpose
TBD - created by archiving change crate-extraction-2. Update Purpose after archive.
## Requirements
### Requirement: plugin-sdk Extraction
The `clankers-plugin-sdk` crate MUST be extracted to `clanker-plugin-sdk`. It already declares its own `[workspace]` in Cargo.toml and targets `wasm32-unknown-unknown`. The extraction is a repo move with no API changes.

#### Scenario: Shared plugin SDK builds after extraction
- GIVEN `crates/clankers-plugin-sdk/` with `[workspace]` and `crate-type = ["rlib"]`
- WHEN it is extracted to the `clanker-plugin-sdk` repo
- THEN the crate compiles with `cargo build --target wasm32-unknown-unknown`
- AND the `extism-pdk` re-export still works
- AND the prelude module re-exports all protocol types
- AND existing plugins that depend on it can switch to the git dependency with only a Cargo.toml change
