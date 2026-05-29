Evidence-ID: steel-tool-plugin-substrate.V4.stdio
Task-ID: V4
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.stdio-plugins.lifecycle-preserved, steel-tool-plugin-substrate.stdio-plugins.sandbox-fail-closed
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V4 Stdio Plugin Evidence

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers stdio_runtime --lib
```

Result: passed.

Observed output excerpt:

```text
running 33 tests
test plugin::tests::stdio_runtime::live_stdio_tool_builds_and_executes_real_tool_adapter ... ok
test plugin::tests::stdio_runtime::cancelled_stdio_tool_call_sends_cancel_and_returns_cancelled_error ... ok
test plugin::tests::stdio_runtime::hung_stdio_tool_call_times_out ... ok
test plugin::tests::stdio_runtime::restricted_sandbox_denies_network_without_allow_network ... ok
test plugin::tests::stdio_runtime::restricted_sandbox_mode_fails_closed_when_backend_is_unavailable ... ok
test plugin::tests::stdio_runtime::mixed_runtime_host_preserves_extism_behavior_and_stdio_visibility ... ok

test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 971 filtered out
```

The stdio runtime suite covers lifecycle, progress/tool-call routing, cancellation, timeout, disconnect/shutdown, restricted sandbox behavior, and mixed WASM/stdio visibility. The substrate addition preserves this path by tagging stdio plugin tools as `stdio_plugin` while Rust still owns supervision and execution.
