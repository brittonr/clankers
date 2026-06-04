Artifact-Type: verification-evidence
Evidence-ID: ce3-workspace-preservation-verification
Task-ID: V1,V2,V3,V4,V5
Creator: pi
Created: 2026-04-25
Status: complete
Covers: workspace-crate-preservation.local-targets, workspace-crate-preservation.no-external-mechanics, workspace-crate-preservation.no-external-publishing, workspace-crate-preservation.leaf-contracts, workspace-crate-preservation.nix-contract, workspace-crate-preservation.matrix-contract, workspace-crate-preservation.zellij-contract, workspace-crate-preservation.infrastructure-contracts, workspace-crate-preservation.protocol-contract, workspace-crate-preservation.db-contract, workspace-crate-preservation.hooks-contract, workspace-crate-preservation.generated-artifacts, workspace-crate-preservation.generated-artifacts-none, workspace-crate-preservation.future-renames, workspace-crate-preservation.names-preserved

# Workspace Preservation Verification

## Workspace membership

Command:

```bash
for c in clankers-nix clankers-matrix clankers-zellij clankers-protocol clankers-db clankers-hooks; do
  test -d crates/$c && echo "ok dir $c"
  grep -q "crates/$c" Cargo.toml && echo "ok workspace $c"
done
```

Output:

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

## External mechanics review

`tasks.md`, `proposal.md`, and `design.md` were rewritten so external repository mechanics appear only as explicit prohibitions/non-goals. No task asks the agent to run `git subtree split`, create/push a GitHub repository, run `cargo publish`, configure standalone CI, replace a path dependency with a git dependency, or add/remove a thin wrapper crate.

## Leaf crate contracts

Command:

```bash
rg -n 'eval =|refscan =|8fe3bade2013befd5ca98aa42224fa2a23551559' crates/clankers-nix/Cargo.toml
rg -n 'matrix-sdk.*e2e-encryption.*sqlite.*rustls-tls' crates/clankers-matrix/Cargo.toml
rg -n 'iroh.*address-lookup-mdns' crates/clankers-zellij/Cargo.toml
```

Output:

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

## Infrastructure crate ownership checks

Command:

```bash
rg -n 'pub enum DaemonEvent|pub enum SessionCommand|pub enum ControlResponse|pub async fn write_frame|pub async fn read_frame' \
  crates/clankers-protocol/src/event.rs \
  crates/clankers-protocol/src/command.rs \
  crates/clankers-protocol/src/control.rs \
  crates/clankers-protocol/src/frame.rs
rg -n 'pub struct Db|pub fn migrate|pub fn version|open_table' crates/clankers-db/src/lib.rs crates/clankers-db/src/schema.rs
rg -n 'pub enum HookPoint|pub struct HookPipeline|pub enum HookVerdict' \
  crates/clankers-hooks/src/point.rs \
  crates/clankers-hooks/src/dispatcher.rs \
  crates/clankers-hooks/src/verdict.rs
```

Output excerpt:

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

## Focused build check

The focused build proves all six target package paths resolve through the current root workspace graph.

Command:

```bash
CARGO_TARGET_DIR=/tmp/clankers-check-target cargo check -p clankers-nix -p clankers-matrix -p clankers-zellij -p clankers-protocol -p clankers-db -p clankers-hooks --lib
```

Result: success (`pueue` task 95).

Output excerpt:

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

## Generated artifacts and names

No generated artifact refresh is required. This change does not move crates, rename packages, remove wrappers, regenerate docs, or change user-visible TUI output. Package names and import paths remain unchanged. Future in-workspace rename or API changes must decide their own generated artifact refresh requirements.
