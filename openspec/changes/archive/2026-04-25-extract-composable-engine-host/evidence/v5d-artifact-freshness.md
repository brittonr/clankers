Task-ID: V5d
Covers: embeddable-agent-engine.host-artifact-refresh, embeddable-agent-engine.host-artifact-freshness
Artifact-Type: validation-evidence

# V5d artifact freshness evidence

## Commands

- `unit2nix --workspace --force --no-check -o build-plan.json`: PASS (`Wrote build-plan.json`).
- `cargo xtask docs`: PASS (`docs built → docs/book/`).
- Artifact grep checks:
  - `Cargo.toml`: contains `crates/clankers-engine-host` and `crates/clankers-tool-host`.
  - `Cargo.lock`: contains `clankers-engine-host` and `clankers-tool-host` package entries.
  - `flake.nix`: contains `clankers-engine-host` and `clankers-tool-host` check entries.
  - `build-plan.json`: contains both host package IDs and source paths.
  - `docs/src/generated/crates.md`: contains both host crate sections.
  - `docs/src/generated/architecture.md`: contains `engine-host` and `tool-host` nodes/edges.
