Evidence-ID: steel-tool-plugin-substrate.V5.subagents
Task-ID: V5
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.subagents.lifecycle-preserved, steel-tool-plugin-substrate.subagents.denied-no-spawn
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V5 Subagent/Delegate Evidence

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers subagent --lib
```

Result: passed.

Observed output:

```text
running 2 tests
test tools::subagent::tests::subagent_tool_reports_subagent_backend_for_steel_substrate ... ok
test tools::delegate::tests::delegate_tool_reports_subagent_backend_for_steel_substrate ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1002 filtered out
```

The tests prove both `SubagentTool` and `DelegateTool` advertise executor kind `subagent` to the Steel substrate. The executor remains Rust-owned: actor/subprocess/remote routing, process monitor, watchdog, panel events, cancellation, and session construction are still inside the existing tools.
