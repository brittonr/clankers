# Plugin runtime trait evidence

Evidence-ID: trait-seam-refactor-roadmap.plugin-runtime-trait
Artifact-Type: command-output-summary
Task-ID: V1
Covers: remaining-coupling-drain.trait-seam-refactors.plugin-runtime
Date: 2026-06-06
Status: PASS

## Implementation summary

- Added `crates/clankers-plugin/src/runtime.rs` with `PluginRuntimeLifecycle` and Extism/stdio implementations.
- Moved runtime-specific manager state into `ExtismRuntimeState` and `StdioRuntimeState` so WASM instances and stdio supervisors/live state are owned by runtime bags instead of flat manager fields.
- `PluginManager::{disable,enable,reload}` now delegates lifecycle behavior through `plugin_runtime_for_kind(...)`; Arc-aware enable/reload wrappers execute stdio startup after releasing the manager lock via `PluginRuntimeAfterGuardDrop`.

## Commands completed

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-plugin --lib plugin_runtime_dispatch_kit
```

## Relevant output

```text
running 1 test
test tests::plugin_runtime_dispatch_kit_keeps_non_extism_out_of_wasm_loader ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.00s
exit=0
```
