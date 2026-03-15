# Crate Structure

## Purpose

Define the layout of `crates/clankers-nix/` and how it wraps snix crates
for use by tools and other clankers subsystems.

## Requirements

### New crate: clankers-nix

r[nix.crate.layout]
The system MUST create `crates/clankers-nix/` with the following modules:

```
crates/clankers-nix/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── error.rs         # NixError enum (snafu)
    ├── store_path.rs    # NixPath wrapper + parsing helpers
    ├── derivation.rs    # Derivation reading + metadata extraction
    ├── flakeref.rs      # Flake reference validation + introspection
    ├── refscan.rs       # Store path reference scanning in text
    ├── eval.rs          # In-process Nix evaluation (phase 2)
    └── tests/
        ├── store_path_tests.rs
        ├── derivation_tests.rs
        ├── flakeref_tests.rs
        ├── refscan_tests.rs
        └── eval_tests.rs
```

### Dependencies (phase 1)

r[nix.crate.deps-phase1]
Phase 1 MUST depend only on `nix-compat`:

```toml
[dependencies]
nix-compat = { path = "../../snix/snix/nix-compat", features = ["flakeref"] }
snafu      = { workspace = true }
serde      = { workspace = true }
serde_json = { workspace = true }
tracing    = { workspace = true }
```

### Dependencies (phase 2, additive)

r[nix.crate.deps-phase2]
Phase 2 MUST add `snix-eval` and `snix-serde` behind a feature flag:

```toml
[features]
default = []
eval = ["dep:snix-eval", "dep:snix-serde"]

[dependencies]
snix-eval  = { path = "../../snix/snix/eval", optional = true }
snix-serde = { path = "../../snix/snix/serde", optional = true }
```

### Dependencies (phase 3, additive)

r[nix.crate.deps-phase3]
Phase 3 MUST add `snix-castore` for refscan behind a feature flag:

```toml
[features]
refscan = ["dep:snix-castore"]

[dependencies]
snix-castore = { path = "../../snix/snix/castore", optional = true, default-features = false }
```

### What NOT to depend on

r[nix.crate.excluded-deps]
The crate MUST NOT depend on:

| Crate | Reason |
|---|---|
| `snix-store` | Not managing a nix store |
| `snix-build` | Builds go through the CLI |
| `snix-glue` | Drags in the full stack |
| `snix-nix-daemon` | Not implementing the daemon protocol |
| `sanakirja` | Not used by snix; redb already covers KV needs |

### Integration with existing crates

r[nix.crate.integration]
The following crates MAY depend on `clankers-nix`:

| Consumer | Uses | Purpose |
|---|---|---|
| `src/tools/nix/` | `store_path`, `flakeref`, `derivation` | Parse CLI output, validate inputs |
| `src/tools/nix/` | `eval` (phase 2) | NixEvalTool implementation |
| `clankers-agent` | `flakeref` | System prompt: detect flake projects |
| `clankers-agent` | `refscan` (phase 3) | Post-process tool outputs |
