# Design: Drain Tigerstyle Violation Ledger

## Approach

The burn-down proceeds by allow site, not by ad hoc grep output. An allow site is a crate-level `cfg_attr(... allow(...))` block or a narrow local `allow(tigerstyle::...)` attribute. Each site may contain one or more lint violations. The task ledger records the full lint set for every site, and each implementation task drains or explicitly reclassifies the whole site.

A slice is complete only when the implementation removes the allow entry or replaces it with a narrower documented boundary, and the verification evidence shows the focused package tests plus full Tigerstyle audit result.

## Validation Contract

Every completed drain slice records evidence with:

- commit hash or working-tree status at validation time;
- exact commands run;
- exit status for each command;
- focused package test output or no-run output when the public API moved;
- full Tigerstyle output summary.

Standard commands:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p <package> -- --keep-going
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p <package> --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Package tests may be adjusted for non-library crates or plugin crates, but the evidence must name the replacement command.

## Violation Inventory

| Task | Source | Lints |
| --- | --- | --- |
| `I#root-lib-allow-site` | `src/lib.rs` | assertion_density, numeric_units, function_length, explicit_defaults, ambient_clock, usize_in_public_api, unbounded_collection_growth, raw_arithmetic_overflow, compound_condition, nested_conditionals, no_unwrap, no_panic, unbounded_channel, unbounded_loop, ambiguous_params, too_many_parameters, bool_naming, ignored_result, sentinel_fallback, unchecked_narrowing, platform_dependent_cast, no_recursion (narrowed: catch_all_on_enum drained) |
| `I#root-main-allow-site` | `src/main.rs` | assertion_density, numeric_units, function_length, explicit_defaults, ambient_clock, usize_in_public_api, unbounded_collection_growth, raw_arithmetic_overflow, compound_condition, nested_conditionals, no_unwrap, no_panic, unbounded_channel, unbounded_loop, ambiguous_params, too_many_parameters, bool_naming, ignored_result, sentinel_fallback, unchecked_narrowing, platform_dependent_cast, no_recursion (narrowed: catch_all_on_enum drained) |
| `I#agent-lib-allow-site` | `crates/clankers-agent/src/lib.rs` | assertion_density, numeric_units, usize_in_public_api, ambiguous_params, too_many_parameters, raw_arithmetic_overflow, ambient_clock, unbounded_loop, unbounded_collection_growth, explicit_defaults, bool_naming, sentinel_fallback |
| `I#controller-lib-allow-site` | `crates/clankers-controller/src/lib.rs` | assertion_density, compound_condition, ambient_clock, usize_in_public_api, sentinel_fallback, bool_naming, no_panic, no_unwrap, unbounded_channel, explicit_defaults, raw_arithmetic_overflow, unbounded_collection_growth, ambiguous_params, unchecked_narrowing, unbounded_loop |
| `I#plugin-lib-allow-site` | `crates/clankers-plugin/src/lib.rs` | assertion_density, numeric_units, function_length, explicit_defaults, ambient_clock, no_unwrap, unbounded_channel, ambiguous_params, too_many_parameters, bool_naming, unbounded_loop, unbounded_collection_growth, unchecked_narrowing, usize_in_public_api |
| `I#provider-lib-allow-site` | `crates/clankers-provider/src/lib.rs` | assertion_density, multi_lock_ordering, acronym_style, numeric_units, function_length, nested_conditionals, compound_condition, unbounded_collection_growth, raw_arithmetic_overflow, ambiguous_params, ignored_result, explicit_defaults, unbounded_loop, no_recursion, ambient_clock, no_unwrap, bool_naming, platform_dependent_cast, usize_in_public_api |
| `I#router-lib-allow-site` | `crates/clanker-router/src/lib.rs` | assertion_density, acronym_style, multi_lock_ordering, compound_condition, numeric_units, function_length, unbounded_collection_growth, raw_arithmetic_overflow, explicit_defaults, ambient_clock, ambiguous_params, ignored_result, too_many_parameters, no_unwrap, platform_dependent_cast, bool_naming, unbounded_loop, usize_in_public_api, unchecked_division, contradictory_time, unchecked_narrowing, catch_all_on_enum, no_recursion |
| `I#runtime-lib-allow-site` | `crates/clankers-runtime/src/lib.rs` | assertion_density, numeric_units, explicit_defaults, unbounded_collection_growth, raw_arithmetic_overflow, too_many_parameters, ambient_clock, usize_in_public_api, no_unwrap |
| `I#tui-lib-allow-site` | `crates/clankers-tui/src/lib.rs` | platform_dependent_cast, explicit_defaults, usize_in_public_api, ambiguous_params, raw_arithmetic_overflow, numeric_units, unbounded_collection_growth, ambient_clock, too_many_parameters, compound_condition, bool_naming, no_panic |
| `I#util-lib-allow-site` | `crates/clankers-util/src/lib.rs` | function_length, unbounded_loop, usize_in_public_api |
| `I#db-clock-allow-site` | `crates/clankers-db/src/lib.rs` | ambient_clock |
| `I#matrix-bridge-clock-allow-site` | `crates/clankers-matrix/src/bridge.rs` | ambient_clock |
| `I#matrix-protocol-clock-allow-site` | `crates/clankers-matrix/src/protocol.rs` | ambient_clock |
| `I#nix-derivation-recursion-allow-site` | `crates/clankers-nix/src/derivation.rs` | no_recursion |
| `I#nix-eval-clock-allow-site` | `crates/clankers-nix/src/eval.rs` | ambient_clock |
| `I#util-truncation-clock-allow-site` | `crates/clankers-util/src/truncation.rs` | ambient_clock |

## Decision: Keep Local Boundary Allows Reviewable

Some local allows may remain acceptable shell-boundary exceptions after review, especially clock reads that centralize timestamp generation. Those are still tracked in this ledger. If a local allow remains, its verification evidence must explain why the boundary is narrower than the original lint risk and must include the full Tigerstyle audit showing no unrelated regressions.
