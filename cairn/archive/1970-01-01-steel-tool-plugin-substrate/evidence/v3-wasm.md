Evidence-ID: steel-tool-plugin-substrate.V3.wasm
Task-ID: V3
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.wasm-plugins.policy-preserved, steel-tool-plugin-substrate.wasm-plugins.fail-closed
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V3 WASM Plugin Evidence

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers plugin_tool --lib
```

Result: passed.

Observed output excerpt:

```text
running 24 tests
test tools::plugin_tool::tests::plugin_tool_reports_wasm_and_stdio_backends_for_steel_substrate ... ok
test tools::plugin_tool::tests::execute_unloaded_plugin_returns_error ... ok
test tools::plugin_tool::tests::execute_echo_wraps_params_in_envelope ... ok
test tools::plugin_tool::tests::execute_reverse_wraps_params_in_envelope ... ok
test plugin::tests::tool_integration::build_all_tiered_tools_includes_plugin_tools ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 980 filtered out
```

The new backend-tag test proves WASM plugin tools report `wasm_plugin` to the substrate. Existing plugin-tool fixtures preserve envelope wrapping, missing/unloaded plugin failure, plugin tool discovery, and plugin integration behavior behind the Rust executor.
