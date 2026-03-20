## 1. Dependencies and Feature Gate

- [x] 1.1 Add `nickel-lang = { version = "2.0", optional = true }` to `crates/clankers-config/Cargo.toml` with a `nickel` feature flag
- [x] 1.2 Enable the `nickel` feature by default in the root `Cargo.toml` workspace dependency and in `src/` (main binary)
- [x] 1.3 Verify the dependency compiles and measure binary size delta with feature on vs off

## 2. Fix Deep Merge (independent of Nickel)

- [x] 2.1 Change `merge_into()` in `settings.rs` to recursively merge nested `serde_json::Value::Object` instead of replacing at the top level
- [x] 2.2 Add tests: nested object partial override (hooks), scalar override within nested object (memory), array replacement (disabledTools)
- [x] 2.3 Verify existing settings tests still pass with the deep merge change

## 3. Nickel Evaluator Module

- [x] 3.1 Create `crates/clankers-config/src/nickel.rs` module gated behind `#[cfg(feature = "nickel")]`
- [x] 3.2 Implement `eval_ncl_file(path: &Path) -> Result<serde_json::Value, String>` using `Context::eval_deep_for_export()` + `Context::expr_to_json()`
- [x] 3.3 Implement `eval_ncl_with_contract(path: &Path, contract: &str) -> Result<serde_json::Value, String>` that injects the embedded contract via import path resolution
- [x] 3.4 Add error formatting that preserves Nickel diagnostic messages with source file paths
- [x] 3.5 Write unit tests: valid ncl → JSON value, syntax error → descriptive error, type violation → contract error message

## 4. Embedded Settings Contract

- [x] 4.1 Write `crates/clankers-config/src/settings-contract.ncl` covering all `Settings` fields with types, defaults, and nested sub-contracts (hooks, memory, compression, keymap, routing, cost_tracking)
- [x] 4.2 Embed via `include_str!` in the nickel module as a constant
- [x] 4.3 Implement `clankers://settings` pseudo-URL resolution in the Nickel evaluator (via `with_added_import_paths` or custom resolution)
- [x] 4.4 Add contract-default sync test: evaluate contract with no overrides, deserialize to `Settings`, assert equals `Settings::default()`

## 5. Settings Loader Integration

- [x] 5.1 Add `.ncl` path variants to `ClankersPaths` (`global_settings_ncl`) and `ProjectPaths` (`settings_ncl`) in `paths.rs`
- [x] 5.2 Create `load_layer(ncl_path: Option<&Path>, json_path: &Path) -> Option<serde_json::Value>` that checks `.ncl` first, falls back to `.json`
- [x] 5.3 Refactor `Settings::load_with_pi_fallback()` to use `load_layer()` for each of the three layers
- [x] 5.4 Add integration tests: ncl-only layer, json-only layer, both exist (ncl wins), mixed-format merge across layers
- [x] 5.5 When `nickel` feature is disabled, `load_layer` skips the `.ncl` check — test this with `#[cfg(not(feature = "nickel"))]` path

## 6. CLI Config Commands

- [x] 6.1 Add `config` subcommand group to `src/main.rs` CLI (clap) with `init`, `check`, `export` sub-subcommands
- [x] 6.2 Implement `config init`: generate starter `settings.ncl` from embedded contract with field comments, support `--global` flag, refuse to overwrite existing file
- [x] 6.3 Implement `config check`: load all layers, report success or print Nickel/JSON error diagnostics, exit code 0/1
- [x] 6.4 Implement `config export`: load + merge all layers, pretty-print merged JSON to stdout, support `--global` flag for single-layer export
- [x] 6.5 Add tests for each subcommand (tempdir-based, verify output and exit codes)

## 7. Documentation and Polish

- [x] 7.1 Add a `docs/nickel-config.md` guide: why Nickel, how to write a `settings.ncl`, contract reference, migration from JSON
- [x] 7.2 Update `README.md` or relevant user-facing docs to mention `.ncl` config support
- [x] 7.3 Add napkin entry for Nickel-specific gotchas discovered during implementation
