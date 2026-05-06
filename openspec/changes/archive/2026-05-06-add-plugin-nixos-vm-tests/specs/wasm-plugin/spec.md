## MODIFIED Requirements

### Requirement: openspec Plugin Runtime Coverage
The plugin MUST have durable checked-in runtime coverage in addition to any ad-hoc smoke scripts. That coverage MUST include Rust integration tests for the WASM runtime and, for packaged release readiness, MUST be complemented by the `vm-plugin-runtime` NixOS VM check that proves at least one packaged Extism plugin can be discovered and invoked after boot.

#### Scenario: Runtime coverage exercises positive and negative calls
- GIVEN the `openspec-plugin/tests/runtime.rs` integration test
- WHEN `cargo test --manifest-path openspec-plugin/Cargo.toml` runs
- THEN Extism loads the built plugin module
- AND the test exercises `describe`, `on_event`, and all five tools
- AND both positive and negative tool-call cases are covered

#### Scenario: Packaged Extism plugin is exercised in a NixOS VM [r[wasm-plugin.runtime-coverage.vm-packaged-extism]]
- GIVEN the `vm-plugin-runtime` NixOS VM check runs
- WHEN the VM discovers packaged shipped plugins
- THEN at least one safe Extism plugin manifest and WASM module are loaded from the packaged layout
- AND a deterministic tool invocation succeeds through the installed clankers host
