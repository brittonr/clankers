# snix-integration — Tasks

No dependencies on other open changes.

## Phase 1: nix-compat parsing (clankers-nix crate + NixTool enhancements)

- [x] Create `crates/clankers-nix/Cargo.toml` with `nix-compat` path dep
- [x] Add `clankers-nix` to workspace members in root `Cargo.toml`
- [x] Write `error.rs` — `NixError` with snafu context selectors
- [x] Write `store_path.rs` — `NixPath` type + `parse_store_path()` + `extract_store_paths()`
- [x] Write `flakeref.rs` — `ParsedFlakeRef` type + `parse_flake_ref()` + `detect_flake()`
- [x] Write `derivation.rs` — `DerivationInfo` type + `read_derivation()` + `dependency_summary()`
- [x] Write `lib.rs` — re-exports
- [x] Tests: store path parsing (valid, invalid, .drv, edge cases)
- [x] Tests: store path extraction from multiline text
- [x] Tests: flake ref parsing (path, github, git, indirect, with/without fragment)
- [x] Tests: flake ref rejection (malformed inputs)
- [x] Tests: derivation reading (use nix-compat's test fixtures or create minimal .drv files)
- [x] Tests: derivation env filtering (large vars excluded, key vars included)
- [x] Tests: dependency summary (bounded depth)
- [x] Integrate into NixTool: flake ref pre-validation before CLI spawn
- [x] Integrate into NixTool: parse output paths after successful build
- [x] Integrate into NixTool: optional derivation summary on build failure
- [x] Verify existing NixTool tests still pass unchanged
- [x] `cargo nextest run -p clankers-nix`

## Phase 2: in-process evaluation (NixEvalTool)

- [x] Add `snix-eval` and `snix-serde` as optional deps behind `eval` feature
- [x] Write `eval.rs` — `evaluate()` function using `EvaluationBuilder`
- [x] Write `eval.rs` — `introspect_flake()` convenience function
- [x] Write `eval.rs` — JSON serialization of Nix values
- [x] Write `eval.rs` — impure detection and CLI fallback logic
- [x] Write `eval.rs` — evaluation limits (step count, output size, timeout)
- [x] Create `src/tools/nix/eval_tool.rs` — `NixEvalTool` implementation
- [x] Register `NixEvalTool` at `ToolTier::Specialty` in `src/modes/common.rs`
- [ ] Conditionally register only when nix is detected (same gate as NixTool)
- [x] Tests: pure expression evaluation (arithmetic, attrsets, lists, strings)
- [x] Tests: impure fallback trigger (import, nixpkgs lookup)
- [x] Tests: value serialization (all Nix types → JSON)
- [x] Tests: flake introspection (needs a fixture flake directory)
- [x] Tests: evaluation limits (step count exceeded, output size exceeded)
- [x] Tests: timeout enforcement
- [x] Add `eval` feature to default features in clankers-nix
- [x] `cargo nextest run -p clankers-nix --features eval`

## Phase 3: store reference scanning

- [x] Add `snix-castore` as optional dep behind `refscan` feature (default-features = false)
- [x] Write `refscan.rs` — `scan_store_refs()` regex-based scanner
- [x] Write `refscan.rs` — `annotate_store_refs()` summary formatter
- [x] Add `annotateStoreRefs` config option to clankers-config Settings
- [x] Wire annotation into NixTool post-processing (when config enabled)
- [ ] Wire annotation into BashTool post-processing (when config enabled)
- [x] Tests: scanning text with 0, 1, many store paths
- [x] Tests: deduplication of repeated paths
- [x] Tests: annotation format
- [x] Tests: performance on large output (>100 KB)
- [x] Tests: skip scanning for >1 MB outputs
- [x] `cargo nextest run -p clankers-nix --features refscan`
