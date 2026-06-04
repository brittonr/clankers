# crate-extraction-3 — Tasks

> Scope update: the remaining six crates stay in this workspace as separate crates. No standalone GitHub repositories are created, pushed to, or published for this change.

## Implementation

- [x] I1 Rewrite proposal/design/spec deltas from external extraction to workspace-local preservation. [covers=workspace-crate-preservation.local,workspace-crate-preservation.no-standalone-repos,workspace-crate-preservation.future-renames]
- [x] I2 Remove external-only work from the task model: GitHub repository creation, subtree split, split branch pushes, standalone CI, crate publishing, git dependency migration, and wrapper removal. [covers=workspace-crate-preservation.no-external-mechanics,workspace-crate-preservation.no-external-publishing]

## Verification

- [x] V1 Verify all six target crates exist under `crates/`, are referenced from root `Cargo.toml`, and have valid workspace path dependencies as proven by the focused workspace build. [covers=workspace-crate-preservation.local-targets] [evidence=openspec/changes/crate-extraction-3/evidence/workspace-preservation-verification.md]
- [x] V2 Verify no GitHub repository creation, `git subtree split`, split branch push, `git push` to a new target repository, `cargo publish`, standalone CI config/badge/README, git dependency migration, or thin wrapper add/remove tasks remain. [covers=workspace-crate-preservation.no-external-mechanics,workspace-crate-preservation.no-external-publishing] [evidence=openspec/changes/crate-extraction-3/evidence/workspace-preservation-verification.md]
- [x] V3 Verify leaf crate feature/pin contracts for `clankers-nix`, `clankers-matrix`, and `clankers-zellij`. [covers=workspace-crate-preservation.leaf-contracts,workspace-crate-preservation.nix-contract,workspace-crate-preservation.matrix-contract,workspace-crate-preservation.zellij-contract] [evidence=openspec/changes/crate-extraction-3/evidence/workspace-preservation-verification.md]
- [x] V4 Verify infrastructure crate ownership/build contracts for `clankers-protocol`, `clankers-db`, and `clankers-hooks`. [covers=workspace-crate-preservation.infrastructure-contracts,workspace-crate-preservation.protocol-contract,workspace-crate-preservation.db-contract,workspace-crate-preservation.hooks-contract] [evidence=openspec/changes/crate-extraction-3/evidence/workspace-preservation-verification.md]
- [x] V5 Verify generated artifact refresh is not required and package/import names remain unchanged. [covers=workspace-crate-preservation.generated-artifacts,workspace-crate-preservation.generated-artifacts-none,workspace-crate-preservation.future-renames,workspace-crate-preservation.names-preserved] [evidence=openspec/changes/crate-extraction-3/evidence/workspace-preservation-verification.md]
- [x] V6 Preserve original preflight audit with dependency-source, sibling-dirt, and snapshot-impact findings. [covers=workspace-crate-preservation.preflight-evidence,workspace-crate-preservation.preflight-audit] [evidence=openspec/changes/crate-extraction-3/evidence/preflight-audit.md]

## Evidence packet for gate review

The following untruncated review excerpts mirror the cited evidence files so the tasks gate can review checked V1–V6 evidence in the main task packet.

### `openspec/changes/crate-extraction-3/evidence/workspace-preservation-verification.md`

Artifact-Type: verification-evidence
Evidence-ID: ce3-workspace-preservation-verification
Task-ID: V1,V2,V3,V4,V5
Creator: pi
Created: 2026-04-25
Status: complete
Covers: workspace-crate-preservation.local-targets, workspace-crate-preservation.no-external-mechanics, workspace-crate-preservation.no-external-publishing, workspace-crate-preservation.leaf-contracts, workspace-crate-preservation.nix-contract, workspace-crate-preservation.matrix-contract, workspace-crate-preservation.zellij-contract, workspace-crate-preservation.infrastructure-contracts, workspace-crate-preservation.protocol-contract, workspace-crate-preservation.db-contract, workspace-crate-preservation.hooks-contract, workspace-crate-preservation.generated-artifacts, workspace-crate-preservation.generated-artifacts-none, workspace-crate-preservation.future-renames, workspace-crate-preservation.names-preserved

#### Workspace membership output

```text
ok dir clankers-nix
ok workspace clankers-nix
ok dir clankers-matrix
ok workspace clankers-matrix
ok dir clankers-zellij
ok workspace clankers-zellij
ok dir clankers-protocol
ok workspace clankers-protocol
ok dir clankers-db
ok workspace clankers-db
ok dir clankers-hooks
ok workspace clankers-hooks
```

#### External mechanics review

`tasks.md`, `proposal.md`, and `design.md` were rewritten so external repository mechanics appear only as explicit prohibitions/non-goals. No task asks the agent to run `git subtree split`, create/push a GitHub repository, run `cargo publish`, configure standalone CI, replace a path dependency with a git dependency, or add/remove a thin wrapper crate.

#### Leaf crate contract output

```text
crates/clankers-nix/Cargo.toml:11:eval = ["dep:snix-eval", "dep:snix-serde"]
crates/clankers-nix/Cargo.toml:13:refscan = ["dep:snix-castore"]
crates/clankers-nix/Cargo.toml:16:nix-compat = { git = "https://git.snix.dev/snix/snix.git", rev = "8fe3bade2013befd5ca98aa42224fa2a23551559", default-features = false, features = ["flakeref"] }
crates/clankers-nix/Cargo.toml:25:snix-eval = { git = "https://git.snix.dev/snix/snix.git", rev = "8fe3bade2013befd5ca98aa42224fa2a23551559", optional = true }
crates/clankers-nix/Cargo.toml:26:snix-serde = { git = "https://git.snix.dev/snix/snix.git", rev = "8fe3bade2013befd5ca98aa42224fa2a23551559", optional = true }
crates/clankers-nix/Cargo.toml:29:snix-castore = { git = "https://git.snix.dev/snix/snix.git", rev = "8fe3bade2013befd5ca98aa42224fa2a23551559", optional = true, default-features = false }
crates/clankers-matrix/Cargo.toml:11:matrix-sdk = { version = "0.9", default-features = false, features = ["e2e-encryption", "sqlite", "rustls-tls"] }
crates/clankers-zellij/Cargo.toml:11:iroh = { version = "0.96", features = ["address-lookup-mdns"] }
```

#### Infrastructure ownership output

```text
crates/clankers-protocol/src/frame.rs:78:pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), FrameError>
crates/clankers-protocol/src/frame.rs:100:pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, FrameError>
crates/clankers-protocol/src/command.rs:12:pub enum SessionCommand {
crates/clankers-protocol/src/event.rs:23:pub enum DaemonEvent {
crates/clankers-protocol/src/control.rs:54:pub enum ControlResponse {
crates/clankers-db/src/lib.rs:50:pub struct Db {
crates/clankers-db/src/schema.rs:54:pub fn migrate(db: &redb::Database) -> Result<()> {
crates/clankers-db/src/schema.rs:190:pub fn version(db: &redb::Database) -> Result<u32> {
crates/clankers-db/src/schema.rs:141:    tx.open_table(audit::TABLE).map_err(db_err)?;
crates/clankers-hooks/src/verdict.rs:5:pub enum HookVerdict {
crates/clankers-hooks/src/point.rs:7:pub enum HookPoint {
crates/clankers-hooks/src/dispatcher.rs:29:pub struct HookPipeline {
```

#### Focused build output

Command: `CARGO_TARGET_DIR=/tmp/clankers-check-target cargo check -p clankers-nix -p clankers-matrix -p clankers-zellij -p clankers-protocol -p clankers-db -p clankers-hooks --lib`

```text
Checking clankers-nix v0.1.0 (/home/brittonr/git/clankers/crates/clankers-nix)
Checking clankers-hooks v0.1.0 (/home/brittonr/git/clankers/crates/clankers-hooks)
Checking clankers-protocol v0.1.0 (/home/brittonr/git/clankers/crates/clankers-protocol)
Checking clankers-db v0.1.0 (/home/brittonr/git/clankers/crates/clankers-db)
warning: field `schema` is never read
  --> crates/clankers-db/src/search_index.rs:39:5
warning: `clankers-db` (lib) generated 1 warning
Checking clankers-zellij v0.1.0 (/home/brittonr/git/clankers/crates/clankers-zellij)
Checking clankers-matrix v0.1.0 (/home/brittonr/git/clankers/crates/clankers-matrix)
Finished `dev` profile [optimized + debuginfo] target(s) in 34.69s
```

#### Generated artifacts and names

No generated artifact refresh is required. This change does not move crates, rename packages, remove wrappers, regenerate docs, or change user-visible TUI output. Package names and import paths remain unchanged. Future in-workspace rename or API changes must decide their own generated artifact refresh requirements.

### `openspec/changes/crate-extraction-3/evidence/preflight-audit.md`

Artifact-Type: audit-note
Evidence-ID: ce3-preflight-audit
Task-ID: V6
Creator: pi
Created: 2026-04-24
Status: complete
Covers: workspace-crate-preservation.preflight-evidence, workspace-crate-preservation.preflight-audit

#### Dependency source audit

Audited targets: `crates/clankers-nix`, `crates/clankers-matrix`, `crates/clankers-zellij`, `crates/clankers-protocol`, `crates/clankers-db`, and `crates/clankers-hooks`.

Result: no target currently depends on an already-extracted clanker crate or a vendored workspace snapshot that needs a new root `[patch."<source-url>"]` entry before migration.

Notable dependency facts preserved:

- `clankers-nix`: snix git rev `8fe3bade2013befd5ca98aa42224fa2a23551559`, features `eval` and `refscan`.
- `clankers-matrix`: `matrix-sdk` features `e2e-encryption`, `sqlite`, and `rustls-tls`.
- `clankers-zellij`: `iroh` feature `address-lookup-mdns`.
- `clankers-protocol`, `clankers-db`, `clankers-hooks`: workspace/common crates only; no extracted/vendored source unification needed at preflight.

#### Sibling dependency status

Sibling path repositories used by validation rails were not clean at audit time:

- `../subwayrat`: dirty `.agent/review-metrics.jsonl` plus rustc ICE text files under `crates/rat-branches/` and `crates/rat-markdown/`.
- `../ratcore`: dirty `.agent/review-metrics.jsonl`.
- `../openspec`: dirty local extraction/plugin work.

Treat failures involving those sibling repos as externally contaminated until their worktrees are cleaned or explicitly isolated.

#### Snapshot impact decision

The revised scope performs no crate renames, code moves, wrapper removals, or user-visible TUI output changes. Snapshot refresh is not required for this local-workspace preservation change. Future rename or API changes must decide their own snapshot/generated-artifact refresh requirements.
