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

- [ ] Add `snix-eval` and `snix-serde` as optional deps behind `eval` feature
- [ ] Write `eval.rs` — `evaluate()` function using `EvaluationBuilder`
- [ ] Write `eval.rs` — `introspect_flake()` convenience function
- [ ] Write `eval.rs` — JSON serialization of Nix values
- [ ] Write `eval.rs` — impure detection and CLI fallback logic
- [ ] Write `eval.rs` — evaluation limits (step count, output size, timeout)
- [ ] Create `src/tools/nix/eval_tool.rs` — `NixEvalTool` implementation
- [ ] Register `NixEvalTool` at `ToolTier::Specialty` in `src/modes/common.rs`
- [ ] Conditionally register only when nix is detected (same gate as NixTool)
- [ ] Tests: pure expression evaluation (arithmetic, attrsets, lists, strings)
- [ ] Tests: impure fallback trigger (import, nixpkgs lookup)
- [ ] Tests: value serialization (all Nix types → JSON)
- [ ] Tests: flake introspection (needs a fixture flake directory)
- [ ] Tests: evaluation limits (step count exceeded, output size exceeded)
- [ ] Tests: timeout enforcement
- [ ] Add `eval` feature to default features in clankers-nix
- [ ] `cargo nextest run -p clankers-nix --features eval`

## Phase 3: store reference scanning

- [ ] Add `snix-castore` as optional dep behind `refscan` feature (default-features = false)
- [ ] Write `refscan.rs` — `scan_store_refs()` regex-based scanner
- [ ] Write `refscan.rs` — `annotate_store_refs()` summary formatter
- [ ] Add `[nix] annotate_store_refs` config option to clankers-config
- [ ] Wire annotation into NixTool post-processing (when config enabled)
- [ ] Wire annotation into BashTool post-processing (when config enabled)
- [ ] Tests: scanning text with 0, 1, many store paths
- [ ] Tests: deduplication of repeated paths
- [ ] Tests: annotation format
- [ ] Tests: performance on large output (>100 KB)
- [ ] Tests: skip scanning for >1 MB outputs
- [ ] `cargo nextest run -p clankers-nix --features refscan`
