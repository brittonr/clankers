Evidence-ID: steel-tool-plugin-substrate.V2.builtins
Task-ID: V2
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.rust-builtins.semantic-parity, steel-tool-plugin-substrate.rust-builtins.denied-no-effect, steel-tool-plugin-substrate.rollout.comparison-oracle
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V2 Built-in Evidence

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent steel_tool_substrate --lib
```

Result: passed.

Observed output excerpt:

```text
running 3 tests
test turn::steel_tool_substrate::tests::disabled_executor_is_removed_from_profile ... ok
test turn::steel_tool_substrate::tests::default_settings_enable_all_executor_kinds ... ok
test turn::execution::tests::steel_tool_substrate_blocks_before_direct_tool_execution ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 181 filtered out
```

The execution test uses a panic-on-execute built-in and a block-mode substrate profile with `rust_builtin` removed, proving the substrate blocks before the direct `Tool::execute` path.
