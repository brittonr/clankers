Evidence-ID: steel-tool-plugin-substrate.V6.dogfood
Task-ID: V6
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.verification.runtime-dogfood, steel-tool-plugin-substrate.catalog-policy.live-inventory, steel-tool-plugin-substrate.rollout.default-authorized-only
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V6 Deterministic Runtime Dogfood Evidence

Commands:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime steel_tool_substrate --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-config steel_tool_substrate --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent steel_tool_substrate --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers plugin_tool --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers stdio_runtime --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers subagent --lib
```

Result: passed.

Observed totals:

```text
clankers-runtime steel_tool_substrate: 3 passed
clankers-config steel_tool_substrate: 1 passed
clankers-agent steel_tool_substrate: 3 passed
clankers plugin_tool: 24 passed
clankers stdio_runtime: 33 passed
clankers subagent: 2 passed
```

The combined rail exercises the default-enabled Steel substrate DTO path, settings activation, built-in block-before-execute behavior, WASM/stdio backend tagging and plugin suites, stdio lifecycle/sandbox fixtures, and subagent/delegate backend tagging. The checker writes the deterministic receipt at `target/steel-tool-plugin-substrate/receipt.json`.
