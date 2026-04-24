# crate-extraction-3 — Tasks

> **Legend:** `[ ]` not started · `[~]` in progress ⏱ · `[x]` done ✅ `<duration>`
>
> **Status:** Split out of `crate-extraction-2` on 2026-04-24. This change
> owns the remaining nix / matrix / zellij / protocol / db / hooks extractions.
>
> **Traceability:** `[covers=...]` uses dotted short IDs derived from the delta
> spec file and requirement name.

## Phase 0: Shared preflight

- [x] Audit the six extraction targets for dependencies on already-extracted or vendored crates; add any required root `[patch."<source-url>"]` entries before the first migration lands (`evidence/preflight-audit.md`: no new patches needed) [covers=extraction-protocol.shared-dependency-source-unification]
- [x] Verify sibling path dependencies used by validation rails are clean (or record external contamination before treating failures as extraction regressions) (`evidence/preflight-audit.md`: `../subwayrat`, `../ratcore`, and `../openspec` dirty; treat as external contamination) [covers=extraction-protocol.verification-preconditions]
- [x] Decide whether any planned rename can affect user-visible TUI snapshots; if so, include snapshot refresh in final cleanup evidence (`evidence/preflight-audit.md`: no expected snapshot impact; keep final refresh conditional) [covers=extraction-protocol.generated-artifact-refresh]

## Phase 1: nix (clankers-nix → clanker-nix)

Leaf extraction. Zero internal deps. snix git deps carry over.

- [ ] Create `clanker-nix` repo on GitHub [covers=extraction-protocol.history-preservation]
- [ ] `git subtree split -P crates/clankers-nix -b extract-nix` [covers=extraction-protocol.history-preservation]
- [ ] Verify the split branch contains commits that touched `crates/clankers-nix/` and preserves original commit messages/dates before pushing [covers=extraction-protocol.history-preservation]
- [ ] Push split branch to new repo [covers=extraction-protocol.history-preservation]
- [ ] Verify the new repo's `git log` retains the split branch's original commit messages/dates [covers=extraction-protocol.history-preservation]
- [ ] Rename crate in Cargo.toml (`name = "clanker-nix"`) [covers=extraction-protocol.namespace-rename,group-a-leaves.nix-extraction]
- [ ] Replace all `clankers_nix` / `clankers-nix` references in source, docs, and string literals except historical changelog entries [covers=extraction-protocol.namespace-rename,group-a-leaves.nix-extraction]
- [ ] Preserve feature flags: `eval`, `refscan` [covers=group-a-leaves.nix-extraction]
- [ ] Verify the extracted repo still pins snix rev `8fe3bade2013befd5ca98aa42224fa2a23551559` [covers=group-a-leaves.nix-extraction]
- [ ] Verify `cargo check` passes in the extracted repo with default features [covers=group-a-leaves.nix-extraction]
- [ ] Verify `cargo check --features eval` passes in the extracted repo [covers=group-a-leaves.nix-extraction]
- [ ] Verify `cargo check --features refscan` passes in the extracted repo [covers=group-a-leaves.nix-extraction]
- [ ] Add or preserve focused tests that prove store path parsing, flakeref validation, and derivation reading still work after rename [covers=group-a-leaves.nix-extraction]
- [ ] Add `README.md` with a one-line crate description, a minimal usage example, and a link back to the clankers project [covers=extraction-protocol.readme]
- [ ] Add `LICENSE` using AGPL-3.0-or-later, or record an explicit compatible-license decision if a different license is chosen [covers=extraction-protocol.licensing]
- [ ] Add standalone CI that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run` (or `cargo test` only if nextest is not configured) [covers=extraction-protocol.standalone-ci]
- [ ] Add and verify a README CI badge for the standalone repo [covers=extraction-protocol.standalone-ci,extraction-protocol.readme]
- [ ] In workspace: add git dep, thin re-export wrapper [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Verify existing workspace callers compile through the `clankers-nix` migration wrapper before direct caller migration [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Remove moved source files [covers=extraction-protocol.re-export-wrapper]
- [ ] Migrate remaining callers to `clanker_nix` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Remove the `clankers-nix` thin wrapper crate [covers=extraction-protocol.re-export-wrapper]
- [ ] `cargo check && cargo nextest run` on full workspace [covers=extraction-protocol.workspace-continuity,group-a-leaves.nix-extraction]

## Phase 2: matrix (clankers-matrix → clanker-matrix)

Leaf extraction. Zero internal deps. Heavy external deps (`matrix-sdk`).

- [ ] Create `clanker-matrix` repo on GitHub [covers=extraction-protocol.history-preservation]
- [ ] `git subtree split -P crates/clankers-matrix -b extract-matrix` [covers=extraction-protocol.history-preservation]
- [ ] Verify the split branch contains commits that touched `crates/clankers-matrix/` and preserves original commit messages/dates before pushing [covers=extraction-protocol.history-preservation]
- [ ] Push split branch to new repo [covers=extraction-protocol.history-preservation]
- [ ] Verify the new repo's `git log` retains the split branch's original commit messages/dates [covers=extraction-protocol.history-preservation]
- [ ] Rename crate in Cargo.toml (`name = "clanker-matrix"`) [covers=extraction-protocol.namespace-rename,group-a-leaves.matrix-extraction]
- [ ] Replace all `clankers_matrix` / `clankers-matrix` references in source, docs, and string literals except historical changelog entries [covers=extraction-protocol.namespace-rename,group-a-leaves.matrix-extraction]
- [ ] Preserve matrix-sdk features: `e2e-encryption`, `sqlite`, `rustls-tls` [covers=group-a-leaves.matrix-extraction]
- [ ] Verify the extracted repo still enables the full matrix-sdk feature set in `Cargo.toml` / `cargo metadata` [covers=group-a-leaves.matrix-extraction]
- [ ] Verify the `client`, `bridge`, `room`, and `protocol` modules compile in the extracted repo [covers=group-a-leaves.matrix-extraction]
- [ ] Verify E2E encryption support remains enabled through a manifest or feature-resolution check [covers=group-a-leaves.matrix-extraction]
- [ ] Add or preserve focused coverage proving markdown rendering still works after extraction [covers=group-a-leaves.matrix-extraction]
- [ ] Add `README.md` with a one-line crate description, a minimal usage example, and a link back to the clankers project [covers=extraction-protocol.readme]
- [ ] Add `LICENSE` using AGPL-3.0-or-later, or record an explicit compatible-license decision if a different license is chosen [covers=extraction-protocol.licensing]
- [ ] Add standalone CI that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run` (or `cargo test` only if nextest is not configured) [covers=extraction-protocol.standalone-ci]
- [ ] Add and verify a README CI badge for the standalone repo [covers=extraction-protocol.standalone-ci,extraction-protocol.readme]
- [ ] In workspace: add git dep, thin re-export wrapper [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Verify existing workspace callers compile through the `clankers-matrix` migration wrapper before direct caller migration [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Remove moved source files [covers=extraction-protocol.re-export-wrapper]
- [ ] Migrate remaining callers to `clanker_matrix` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Remove the `clankers-matrix` thin wrapper crate [covers=extraction-protocol.re-export-wrapper]
- [ ] `cargo check && cargo nextest run` on full workspace [covers=extraction-protocol.workspace-continuity,group-a-leaves.matrix-extraction]

## Phase 3: zellij (clankers-zellij → clanker-zellij)

Leaf extraction. Zero internal deps. iroh QUIC dep.

- [ ] Create `clanker-zellij` repo on GitHub [covers=extraction-protocol.history-preservation]
- [ ] `git subtree split -P crates/clankers-zellij -b extract-zellij` [covers=extraction-protocol.history-preservation]
- [ ] Verify the split branch contains commits that touched `crates/clankers-zellij/` and preserves original commit messages/dates before pushing [covers=extraction-protocol.history-preservation]
- [ ] Push split branch to new repo [covers=extraction-protocol.history-preservation]
- [ ] Verify the new repo's `git log` retains the split branch's original commit messages/dates [covers=extraction-protocol.history-preservation]
- [ ] Rename crate in Cargo.toml (`name = "clanker-zellij"`) [covers=extraction-protocol.namespace-rename,group-a-leaves.zellij-extraction]
- [ ] Replace all `clankers_zellij` / `clankers-zellij` references in source, docs, and string literals except historical changelog entries [covers=extraction-protocol.namespace-rename,group-a-leaves.zellij-extraction]
- [ ] Preserve iroh version alignment with the clankers workspace [covers=group-a-leaves.zellij-extraction]
- [ ] Preserve the `address-lookup-mdns` feature on the iroh dependency [covers=group-a-leaves.zellij-extraction]
- [ ] Verify P2P terminal streaming code compiles in the extracted repo [covers=group-a-leaves.zellij-extraction]
- [ ] Verify the extracted repo keeps iroh version alignment and the mDNS feature through manifest or feature-resolution evidence [covers=group-a-leaves.zellij-extraction]
- [ ] Add `README.md` with a one-line crate description, a minimal usage example, and a link back to the clankers project [covers=extraction-protocol.readme]
- [ ] Add `LICENSE` using AGPL-3.0-or-later, or record an explicit compatible-license decision if a different license is chosen [covers=extraction-protocol.licensing]
- [ ] Add standalone CI that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run` (or `cargo test` only if nextest is not configured) [covers=extraction-protocol.standalone-ci]
- [ ] Add and verify a README CI badge for the standalone repo [covers=extraction-protocol.standalone-ci,extraction-protocol.readme]
- [ ] In workspace: add git dep, thin re-export wrapper [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Verify existing workspace callers compile through the `clankers-zellij` migration wrapper before direct caller migration [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Remove moved source files [covers=extraction-protocol.re-export-wrapper]
- [ ] Migrate remaining callers to `clanker_zellij` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Remove the `clankers-zellij` thin wrapper crate [covers=extraction-protocol.re-export-wrapper]
- [ ] `cargo check && cargo nextest run` on full workspace [covers=extraction-protocol.workspace-continuity,group-a-leaves.zellij-extraction]

## Phase 4: protocol (clankers-protocol → clanker-protocol)

Infrastructure extraction. Zero internal deps. 2 reverse deps.

- [ ] Create `clanker-protocol` repo on GitHub [covers=extraction-protocol.history-preservation]
- [ ] `git subtree split -P crates/clankers-protocol -b extract-protocol` [covers=extraction-protocol.history-preservation]
- [ ] Verify the split branch contains commits that touched `crates/clankers-protocol/` and preserves original commit messages/dates before pushing [covers=extraction-protocol.history-preservation]
- [ ] Push split branch to new repo [covers=extraction-protocol.history-preservation]
- [ ] Verify the new repo's `git log` retains the split branch's original commit messages/dates [covers=extraction-protocol.history-preservation]
- [ ] Rename crate in Cargo.toml (`name = "clanker-protocol"`) [covers=extraction-protocol.namespace-rename,group-b-infrastructure.protocol-extraction]
- [ ] Replace all `clankers_protocol` / `clankers-protocol` references in source, docs, and string literals except historical changelog entries [covers=extraction-protocol.namespace-rename,group-b-infrastructure.protocol-extraction]
- [ ] Verify frame, command, control, event, and types modules all compile [covers=group-b-infrastructure.protocol-extraction]
- [ ] Verify `read_frame` and `write_frame` still use the existing tokio async 4-byte length-prefix-plus-JSON framing implementation [covers=group-b-infrastructure.protocol-extraction]
- [ ] Add checked-in pre-extraction wire fixtures for `DaemonEvent`, `SessionCommand`, `ControlRequest`, and `ControlResponse` [covers=group-b-infrastructure.protocol-wire-compatibility-verification]
- [ ] Verify serde serialization/deserialization for those fixtures remains semantically identical to the pre-extraction wire contract [covers=group-b-infrastructure.protocol-extraction,group-b-infrastructure.protocol-wire-compatibility-verification]
- [ ] Verify framing round-trips those fixtures without changing bytes or semantic meaning [covers=group-b-infrastructure.protocol-wire-compatibility-verification]
- [ ] Add `README.md` with a one-line crate description, a minimal usage example, and a link back to the clankers project [covers=extraction-protocol.readme]
- [ ] Add `LICENSE` using AGPL-3.0-or-later, or record an explicit compatible-license decision if a different license is chosen [covers=extraction-protocol.licensing]
- [ ] Add standalone CI that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run` (or `cargo test` only if nextest is not configured) [covers=extraction-protocol.standalone-ci]
- [ ] Add and verify a README CI badge for the standalone repo [covers=extraction-protocol.standalone-ci,extraction-protocol.readme]
- [ ] In workspace: add git dep, thin re-export wrapper [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Remove moved source files [covers=extraction-protocol.re-export-wrapper]
- [ ] Migrate root crate imports to `clanker_protocol` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Migrate `clankers-controller` imports to `clanker_protocol` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Verify the root crate and `clankers-controller` compile through the migration wrapper before removing it [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity,group-b-infrastructure.protocol-extraction]
- [ ] Remove the `clankers-protocol` thin wrapper crate [covers=extraction-protocol.re-export-wrapper]
- [ ] `cargo check && cargo nextest run` on full workspace (with emphasis on root crate + controller) [covers=extraction-protocol.workspace-continuity,group-b-infrastructure.protocol-extraction]

## Phase 5: db (clankers-db → clanker-db)

Infrastructure extraction. Zero internal deps. 2 reverse deps.

- [ ] Create `clanker-db` repo on GitHub [covers=extraction-protocol.history-preservation]
- [ ] `git subtree split -P crates/clankers-db -b extract-db` [covers=extraction-protocol.history-preservation]
- [ ] Verify the split branch contains commits that touched `crates/clankers-db/` and preserves original commit messages/dates before pushing [covers=extraction-protocol.history-preservation]
- [ ] Push split branch to new repo [covers=extraction-protocol.history-preservation]
- [ ] Verify the new repo's `git log` retains the split branch's original commit messages/dates [covers=extraction-protocol.history-preservation]
- [ ] Rename crate in Cargo.toml (`name = "clanker-db"`) [covers=extraction-protocol.namespace-rename,group-b-infrastructure.db-extraction]
- [ ] Replace all `clankers_db` / `clankers-db` references in source, docs, and string literals except historical changelog entries [covers=extraction-protocol.namespace-rename,group-b-infrastructure.db-extraction]
- [ ] Verify all 8 table modules compile: audit, memory, sessions, history, usage, file_cache, tool_results, registry [covers=group-b-infrastructure.db-extraction]
- [ ] Verify table definitions and read/write methods still work with focused storage tests after extraction [covers=group-b-infrastructure.db-extraction]
- [ ] Verify the schema module continues to define the full redb table set [covers=group-b-infrastructure.db-extraction]
- [ ] Verify the error module preserves typed errors and error conversions used by callers [covers=group-b-infrastructure.db-extraction]
- [ ] Add `README.md` with a one-line crate description, a minimal usage example, and a link back to the clankers project [covers=extraction-protocol.readme]
- [ ] Add `LICENSE` using AGPL-3.0-or-later, or record an explicit compatible-license decision if a different license is chosen [covers=extraction-protocol.licensing]
- [ ] Add standalone CI that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run` (or `cargo test` only if nextest is not configured) [covers=extraction-protocol.standalone-ci]
- [ ] Add and verify a README CI badge for the standalone repo [covers=extraction-protocol.standalone-ci,extraction-protocol.readme]
- [ ] In workspace: add git dep, thin re-export wrapper [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Remove moved source files [covers=extraction-protocol.re-export-wrapper]
- [ ] Migrate root crate imports to `clanker_db` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Migrate `clankers-agent` imports to `clanker_db` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Verify the root crate and `clankers-agent` compile through the migration wrapper before removing it [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity,group-b-infrastructure.db-extraction]
- [ ] Remove the `clankers-db` thin wrapper crate [covers=extraction-protocol.re-export-wrapper]
- [ ] `cargo check && cargo nextest run` on full workspace (with emphasis on root crate + agent) [covers=extraction-protocol.workspace-continuity,group-b-infrastructure.db-extraction]

## Phase 6: hooks (clankers-hooks → clanker-hooks)

Infrastructure extraction. Zero internal deps. 5 reverse deps.

- [ ] Create `clanker-hooks` repo on GitHub [covers=extraction-protocol.history-preservation]
- [ ] `git subtree split -P crates/clankers-hooks -b extract-hooks` [covers=extraction-protocol.history-preservation]
- [ ] Verify the split branch contains commits that touched `crates/clankers-hooks/` and preserves original commit messages/dates before pushing [covers=extraction-protocol.history-preservation]
- [ ] Push split branch to new repo [covers=extraction-protocol.history-preservation]
- [ ] Verify the new repo's `git log` retains the split branch's original commit messages/dates [covers=extraction-protocol.history-preservation]
- [ ] Rename crate in Cargo.toml (`name = "clanker-hooks"`) [covers=extraction-protocol.namespace-rename,group-b-infrastructure.hooks-extraction]
- [ ] Replace all `clankers_hooks` / `clankers-hooks` references in source, docs, and string literals except historical changelog entries [covers=extraction-protocol.namespace-rename,group-b-infrastructure.hooks-extraction]
- [ ] Add `Custom(String)` variant to `HookPoint` for extensibility [covers=group-b-infrastructure.hooks-extraction,group-b-infrastructure.hooks-custom-variant-behavior]
- [ ] Verify config, dispatcher, git, payload, point, script, and verdict modules compile [covers=group-b-infrastructure.hooks-extraction]
- [ ] Verify the extracted public API exposes `HookPipeline`, `HookHandler`, `HookVerdict`, `HookPoint`, `HookPayload`, `HookConfig`, and `GitHooks` [covers=group-b-infrastructure.hooks-extraction]
- [ ] Add dedicated positive and negative tests for `HookPoint::Custom(String)` serde round-trip and preservation of existing concrete hook-point round-trips [covers=group-b-infrastructure.hooks-custom-variant-behavior]
- [ ] Add dedicated dispatcher tests proving custom hook handlers can match `HookPoint::Custom(String)` without changing built-in hook behavior [covers=group-b-infrastructure.hooks-custom-variant-behavior]
- [ ] Add or preserve tokio-backed coverage proving the async `HookHandler` trait still works [covers=group-b-infrastructure.hooks-extraction]
- [ ] Add or preserve coverage proving script hook execution continues to work after extraction [covers=group-b-infrastructure.hooks-extraction]
- [ ] Audit existing clankers `HookPoint` matches and update exhaustive callers for the new variant [covers=group-b-infrastructure.hooks-custom-variant-behavior]
- [ ] Add `README.md` with a one-line crate description, a minimal usage example, and a link back to the clankers project [covers=extraction-protocol.readme]
- [ ] Add `LICENSE` using AGPL-3.0-or-later, or record an explicit compatible-license decision if a different license is chosen [covers=extraction-protocol.licensing]
- [ ] Add standalone CI that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run` (or `cargo test` only if nextest is not configured) [covers=extraction-protocol.standalone-ci]
- [ ] Add and verify a README CI badge for the standalone repo [covers=extraction-protocol.standalone-ci,extraction-protocol.readme]
- [ ] In workspace: add git dep, thin re-export wrapper [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity]
- [ ] Remove moved source files [covers=extraction-protocol.re-export-wrapper]
- [ ] Migrate root crate imports to `clanker_hooks` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Migrate `clankers-agent` imports to `clanker_hooks` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Migrate `clankers-config` imports to `clanker_hooks` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Migrate `clankers-controller` imports to `clanker_hooks` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Migrate `clankers-plugin` imports to `clanker_hooks` [covers=extraction-protocol.namespace-rename,extraction-protocol.re-export-wrapper]
- [ ] Verify the root crate, `clankers-agent`, `clankers-config`, `clankers-controller`, and `clankers-plugin` compile through the migration wrapper before removing it [covers=extraction-protocol.re-export-wrapper,extraction-protocol.workspace-continuity,group-b-infrastructure.hooks-extraction]
- [ ] Remove the `clankers-hooks` thin wrapper crate [covers=extraction-protocol.re-export-wrapper]
- [ ] `cargo check && cargo nextest run` on full workspace (with emphasis on all 5 reverse deps) [covers=extraction-protocol.workspace-continuity,group-b-infrastructure.hooks-extraction]

## Phase 7: Final cleanup

- [ ] Grep workspace for any remaining `clankers_nix`, `clankers_matrix`, `clankers_zellij`, `clankers_protocol`, `clankers_db`, `clankers_hooks` [covers=extraction-protocol.namespace-rename]
- [ ] Confirm no thin wrapper crates remain for the continuation extractions [covers=extraction-protocol.re-export-wrapper]
- [ ] Verify `Cargo.lock` records the six git-dependency migrations cleanly [covers=extraction-protocol.workspace-continuity]
- [ ] Update workspace `members` list in root Cargo.toml [covers=extraction-protocol.workspace-continuity]
- [ ] Update `AGENTS.md` extracted crates section [covers=extraction-protocol.readme]
- [ ] Update xtask crate list [covers=extraction-protocol.generated-artifact-refresh]
- [ ] Regenerate `build-plan.json` with `unit2nix --workspace --force --no-check -o build-plan.json` [covers=extraction-protocol.generated-artifact-refresh]
- [ ] Refresh generated docs with `cargo xtask docs` [covers=extraction-protocol.generated-artifact-refresh]
- [ ] Refresh affected snapshots only if a rename changed user-visible TUI output [covers=extraction-protocol.generated-artifact-refresh]
- [ ] `RUSTC_WRAPPER= cargo check && RUSTC_WRAPPER= cargo nextest run` — full workspace green [covers=extraction-protocol.workspace-continuity]
