## Why

The settings layer uses plain JSON files with a shallow field-level merge (`merge_into`). This creates real problems: a project that overrides `hooks.disabledHooks` silently loses `hooks.enabled = true` from the global config because the entire `hooks` object gets replaced. JSON also lacks comments, computed values, and any form of validation before deserialization — users get cryptic serde errors when they typo a field name. Nickel is a configuration language built for exactly this: typed contracts, deep merge with priority annotations, comments, and computed values. It has a stable Rust embedding crate (`nickel-lang` 2.0) that evaluates to JSON, so the existing serde pipeline stays intact.

## What Changes

- Add `nickel-lang` 2.0 crate as a dependency of `clankers-config`
- Settings loader checks for `.ncl` files alongside `.json` — prefers `.ncl` when present, falls back to `.json` for backward compatibility
- Ship a Nickel contract file (`settings-contract.ncl`) that encodes the full `Settings` schema with types, defaults, and validation rules
- The 3-layer merge (pi fallback → global → project) uses Nickel's native `&` merge operator when all layers are `.ncl`, or falls back to the existing JSON merge when any layer is `.json`
- `clankers config init` subcommand generates a starter `settings.ncl` from the contract with comments explaining each field
- `clankers config check` subcommand validates a `.ncl` config without starting a session (catches contract violations, typos, type errors)
- `clankers config export` subcommand evaluates `.ncl` to JSON and prints it (debugging aid)
- No changes to agent definition loading (markdown+YAML frontmatter), plugin manifests, or hook scripts — those stay as-is

## Capabilities

### New Capabilities
- `nickel-settings-loader`: Load and evaluate `.ncl` settings files through the Nickel evaluator, producing `serde_json::Value` for the existing deserialization pipeline
- `nickel-settings-contract`: Ship a Nickel contract encoding the `Settings` schema — types, defaults, and validation — so config errors surface at load time with clear messages
- `nickel-config-commands`: CLI subcommands for initializing, checking, and exporting Nickel config files

### Modified Capabilities

_(none — the JSON path is preserved unchanged)_

## Impact

- **`crates/clankers-config/`**: New `nickel.rs` module for `.ncl` file detection and evaluation. `settings.rs` gains a branch in the load path. `paths.rs` adds `.ncl` path variants alongside existing `.json` paths.
- **`src/main.rs` / CLI**: New `config` subcommand with `init`, `check`, `export` sub-subcommands.
- **Dependencies**: `nickel-lang` 2.0 (pulls in `nickel-lang-core`, `malachite`, `codespan-reporting`). Adds ~30-40 transitive crates. Only used at startup — no runtime cost after settings are loaded.
- **Binary size**: Nickel evaluator adds weight. Could be gated behind a `nickel` cargo feature flag so minimal builds skip it.
- **Backward compatibility**: Zero breakage. Existing `.json` configs keep working. `.ncl` is opt-in.
