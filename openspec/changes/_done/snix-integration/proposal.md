# snix-integration

## Intent

Use snix crates to replace the NixTool's shell-out-and-parse approach with
typed Nix operations.  Today the agent spawns `nix build`, `nix eval`,
`nix flake show` as child processes and scrapes their output.  Store paths
are opaque strings.  Derivations are black boxes.  Flake references are
unvalidated.

snix already has production-quality crates for parsing store paths,
derivations, NAR archives, flake references, and evaluating Nix expressions
in-process.  These are sibling crates in `../snix/` — same repo server,
same team.  Pulling them in replaces fragile string parsing with typed
structures and eliminates process spawning for evaluation tasks.

## Scope

### In Scope

- `nix-compat` — parse store paths, derivations, flake references, nixbase32
- `snix-eval` + `snix-serde` — in-process Nix evaluation, Nix→Rust deserialization
- `snix-castore` refscan — scan tool outputs for store path references
- New `crates/clankers-nix/` crate wrapping snix APIs for agent use
- Enhanced NixTool returning structured metadata alongside build output
- New NixEvalTool for in-process expression evaluation
- Flake introspection without spawning `nix flake show`

### Out of Scope

- Replacing the `nix` CLI for builds (snix-build is Linux-only, not mature enough)
- Replacing git with any other VCS
- Running a nix-daemon (snix-nix-daemon)
- Adopting snix-castore as general blob storage (redb already covers this)
- Adopting snix-store for path info caching (not managing a nix store)
- Adopting sanakirja (snix uses redb internally too — same choice we made)
- Nix-as-config-language for clankers settings (too niche for the user base)
- snix-glue (pulls in everything, only needed for a full nix reimplementation)

## Approach

Three phases, each independently useful.

**Phase 1:** Add `nix-compat` to the workspace.  Use it in the existing
NixTool to parse store paths from build output, validate flake references
before spawning nix, and optionally read `.drv` files to surface build
metadata.  No new tools — just better output from the existing one.

**Phase 2:** Add `snix-eval` and `snix-serde`.  Create a `NixEvalTool` that
evaluates Nix expressions in-process without spawning `nix eval`.  Use it
for fast flake introspection (available packages, devshells, checks) and
reading Nix-defined project metadata.

**Phase 3:** Wire refscan into tool output post-processing.  When any tool
output contains `/nix/store/...` paths, parse and annotate them.  Feed
parsed dependency info into agent context for better reasoning about
build artifacts.
