## Context

Settings are loaded at startup from up to three JSON files (pi fallback, global, project) and merged with a shallow field-level `merge_into()` that copies top-level keys from source to target. Nested objects (hooks, memory, compression, keymap, routing, cost_tracking) are replaced wholesale — a project config that sets `hooks.disabledHooks` silently drops `hooks.enabled` from the global layer. All defaults live in Rust via `#[serde(default)]` annotations and `Default` impls, duplicating the schema across code and documentation.

The `nickel-lang` 2.0 crate provides a stable Rust embedding API: `Context::eval_deep_for_export()` evaluates Nickel source to an `Expr`, and `Context::expr_to_json()` serializes it to a JSON string. The result feeds directly into the existing `serde_json::from_str::<Settings>()` path.

## Goals / Non-Goals

**Goals:**
- Support `.ncl` settings files alongside `.json` with zero breakage to existing configs
- Fix the shallow merge problem — Nickel's `&` operator with `| default` annotations provides correct deep merge
- Ship a Nickel contract that encodes the full `Settings` schema so users get clear validation errors
- Provide CLI commands (`config init`, `config check`, `config export`) for working with Nickel configs
- Gate the Nickel evaluator behind a cargo feature flag so minimal builds avoid the dependency weight

**Non-Goals:**
- Replacing agent definition format (markdown + YAML frontmatter) — wrong domain for Nickel
- Replacing plugin manifests (`plugin.json`) — too simple to benefit
- Hot-reloading config — `nickel_lang::Context` is `!Send + !Sync`, and settings are loaded once at startup
- Making Nickel the only config format — JSON stays as the default, Nickel is opt-in

## Decisions

### 1. Nickel evaluates to JSON, existing serde pipeline unchanged

**Decision:** Nickel files are evaluated to a JSON string via `Context::expr_to_json()`, then parsed through the existing `serde_json::from_value::<Settings>()` path.

**Alternatives considered:**
- Walk the Nickel `Expr` tree directly into `Settings` fields → tight coupling to Nickel internals, loses the stable API boundary
- Use Nickel's TOML/YAML export → JSON is what the pipeline already speaks

**Rationale:** The eval-to-JSON approach means the Nickel integration is a preprocessor. Everything downstream (serde defaults, validation, the `Settings` struct) stays exactly as-is. If Nickel is ever removed, the only change is deleting the preprocessor.

### 2. Feature-gated dependency: `nickel` cargo feature

**Decision:** The `nickel-lang` dependency is behind `features = ["nickel"]` in `clankers-config/Cargo.toml`. The main binary enables it by default. Minimal/embedded builds can disable it.

**Alternatives considered:**
- Always-on dependency → penalizes users who don't use `.ncl` files
- Shell out to `nickel export` CLI → adds runtime dependency on `nickel` binary in PATH, slower, error handling is worse

**Rationale:** Embedding avoids the external tool dependency. Feature gating lets downstream crates (tests, CI, plugins) skip the weight when they don't need config loading.

### 3. File precedence: `.ncl` preferred, `.json` fallback

**Decision:** For each config layer (pi, global, project), check for `settings.ncl` first. If absent, fall back to `settings.json`. Layers can mix formats — a global `.ncl` with a project `.json` works fine because both produce `serde_json::Value` before merging.

**Alternatives considered:**
- Only support `.ncl` when all layers are `.ncl` → confusing partial-support behavior
- Require explicit format flag → unnecessary friction

**Rationale:** Each layer independently resolves to a `serde_json::Value`. The merge logic doesn't care which format produced it.

### 4. Nickel-native merge vs existing `merge_into()`

**Decision:** When all three layers are `.ncl`, use a single Nickel expression that imports and merges them: `(import "pi.ncl") & (import "global.ncl") & (import "project.ncl")`. This gives deep merge with priority. When any layer is `.json`, convert it to `serde_json::Value` and use the existing `merge_into()` — but fix `merge_into()` to do recursive object merge instead of field-level replacement regardless of Nickel.

**Alternatives considered:**
- Always use Nickel merge (convert JSON to Nickel source at runtime) → fragile string templating
- Only fix the JSON merge, skip Nickel merge → misses the contract/validation value

**Rationale:** Fixing the JSON merge is the right thing to do independent of Nickel. The Nickel merge path is a bonus that additionally gives users contracts and computed values.

### 5. Contract file shipped as a Rust `include_str!` constant

**Decision:** The Nickel contract (`settings-contract.ncl`) is embedded in the binary via `include_str!`. It's written to a temp file (or injected via `with_added_import_paths`) when evaluating user configs. Users can reference it as `import "clankers://settings"` — the loader resolves this pseudo-URL to the embedded contract.

**Alternatives considered:**
- Ship as a separate file in `~/.clankers/agent/` → needs install/update mechanism
- Don't ship a contract, let users write raw records → loses the validation value

**Rationale:** Embedding ensures the contract version always matches the binary version. No install step, no version skew.

### 6. CLI subcommands under `clankers config`

**Decision:** Three subcommands:
- `clankers config init` — writes a starter `settings.ncl` (from the embedded contract with comments) to the appropriate location (`--global` or project)
- `clankers config check` — evaluates the config and reports errors without starting a session
- `clankers config export` — evaluates and prints the merged JSON (all layers resolved)

**Alternatives considered:**
- Slash commands (`/config check`) → these are startup concerns, not in-session operations
- No CLI, just better error messages → misses the "generate starter config" use case

## Risks / Trade-offs

**[Dependency weight]** → `nickel-lang` pulls ~30-40 transitive crates including `malachite` (arbitrary precision math). Mitigation: feature-gated, measured at integration time. If binary size increase exceeds 5MB, reconsider the shell-out approach.

**[`!Send + !Sync` Context]** → Can't eval Nickel on a background thread without wrapping in a dedicated thread + channel. Mitigation: config loading happens once at startup on the main thread, before the async runtime starts. No impact on daemon/actor system.

**[User learning curve]** → Nickel is niche. Most users will stick with JSON. Mitigation: `config init` generates a well-commented starter file. The JSON path stays fully supported. Power users who want computed config or deep merge opt in.

**[Contract drift]** → The Nickel contract must stay in sync with the Rust `Settings` struct. Mitigation: a test that evaluates the contract with empty overrides, deserializes the JSON output to `Settings`, and asserts it matches `Settings::default()`. Any new field added to the struct without a corresponding contract entry breaks this test.

**[Nickel eval errors are opaque]** → Nickel's error messages reference source positions that may not map cleanly to the user's file when imports are involved. Mitigation: use `with_source_name()` so error messages reference the actual file path. Surface the full Nickel error diagnostic rather than wrapping it.

## Open Questions

- Should `config init` generate a minimal file (just the fields most likely to be customized) or the full contract with all fields commented out?
- Is it worth adding a `--format ncl` flag to `clankers daemon status` and similar introspection commands to output Nickel instead of JSON?
