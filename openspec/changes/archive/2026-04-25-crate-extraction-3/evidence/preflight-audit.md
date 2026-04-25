# Crate Extraction 3 Preflight Audit

Artifact-Type: audit-note
Evidence-ID: ce3-preflight-audit
Task-ID: V6
Covers: workspace-crate-preservation.preflight-evidence, workspace-crate-preservation.preflight-audit
Creator: pi
Created: 2026-04-24
Status: complete

## Dependency Source Audit

Audited these remaining extraction targets:

- `crates/clankers-nix`
- `crates/clankers-matrix`
- `crates/clankers-zellij`
- `crates/clankers-protocol`
- `crates/clankers-db`
- `crates/clankers-hooks`

Result: no target currently depends on an already-extracted clanker crate or a vendored workspace snapshot that needs a new root `[patch."<source-url>"]` entry before the first migration.

Notable dependencies to preserve during extraction:

- `clankers-nix`: snix git rev `8fe3bade2013befd5ca98aa42224fa2a23551559`, features `eval` and `refscan`.
- `clankers-matrix`: `matrix-sdk` features `e2e-encryption`, `sqlite`, and `rustls-tls`.
- `clankers-zellij`: `iroh` feature `address-lookup-mdns`.
- `clankers-protocol`, `clankers-db`, `clankers-hooks`: workspace/common crates only; no extracted/vendored source unification needed at preflight.

Existing root patches remain unrelated to the six targets at this stage:

- `[patch."https://github.com/brittonr/clanker-router"]`
- `[patch."ssh://git@github.com/brittonr/ratcore.git"]`

## Sibling Dependency Status

Sibling path repositories used by validation rails are not clean:

- `../subwayrat`: dirty `.agent/review-metrics.jsonl` plus rustc ICE text files under `crates/rat-branches/` and `crates/rat-markdown/`.
- `../ratcore`: dirty `.agent/review-metrics.jsonl`.
- `../openspec`: dirty local extraction/plugin work.

Treat failures involving those sibling repos as externally contaminated until their worktrees are cleaned or explicitly isolated.

## TUI Snapshot Impact Decision

The revised scope performs no crate renames, code moves, wrapper removals, or user-visible TUI output changes.

Snapshot refresh is not required for this local-workspace preservation change. Future rename or API changes must decide their own snapshot/generated-artifact refresh requirements.
