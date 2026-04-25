## Purpose

Defines the local-preservation policy for workspace crates that remain independent packages inside the clankers repository rather than being extracted to standalone repositories.

## Requirements

### Requirement: Workspace-local preservation
The system MUST keep the remaining six target crates as independent members of the clankers workspace rather than extracting them to standalone GitHub repositories.
r[workspace-crate-preservation.local]

#### Scenario: Target crates remain local
r[workspace-crate-preservation.local-targets]
- GIVEN the target crates `clankers-nix`, `clankers-matrix`, `clankers-zellij`, `clankers-protocol`, `clankers-db`, and `clankers-hooks`
- WHEN this change is completed
- THEN each target crate remains under `crates/`
- AND each target crate remains referenced by the root workspace configuration
- AND root workspace path dependencies remain valid

### Requirement: Standalone repository prohibition
The system MUST NOT create standalone GitHub repositories, push split branches, publish crates, configure standalone CI, or replace workspace path dependencies with git dependencies for the six target crates in this change.
r[workspace-crate-preservation.no-standalone-repos]

#### Scenario: No external repo mechanics are performed
r[workspace-crate-preservation.no-external-mechanics]
- GIVEN a target crate remains in the workspace
- WHEN tasks for this change are reviewed
- THEN there are no tasks that create a GitHub repository
- AND there are no tasks that push a split branch
- AND there are no tasks that publish a crate with `cargo publish`
- AND there are no tasks that configure standalone CI outside the clankers workspace
- AND there are no tasks that replace the local crate with a git dependency
- AND there are no tasks that add or remove a thin re-export wrapper

#### Scenario: External publishing is out of scope
r[workspace-crate-preservation.no-external-publishing]
- GIVEN the revised scope is workspace-local preservation
- WHEN the change is implemented
- THEN no `git subtree split` command is required
- AND no `git push` to a new target repository is required
- AND no standalone CI configuration, standalone CI badge, standalone repository README, or crate publishing step is required

### Requirement: Leaf crate local contracts
The system SHALL verify local preservation contracts for the leaf crates that were previously extraction candidates.
r[workspace-crate-preservation.leaf-contracts]

#### Scenario: Nix contract is preserved locally
r[workspace-crate-preservation.nix-contract]
- GIVEN `crates/clankers-nix/` remains in the workspace
- WHEN verification evidence is recorded
- THEN evidence checks that `crates/clankers-nix/Cargo.toml` still defines the `eval` and `refscan` feature flags
- AND evidence checks that `crates/clankers-nix/Cargo.toml` still pins snix dependencies to revision `8fe3bade2013befd5ca98aa42224fa2a23551559`

#### Scenario: Matrix contract is preserved locally
r[workspace-crate-preservation.matrix-contract]
- GIVEN `crates/clankers-matrix/` remains in the workspace
- WHEN verification evidence is recorded
- THEN evidence checks that the Matrix SDK dependency still includes the `e2e-encryption`, `sqlite`, and `rustls-tls` features

#### Scenario: Zellij contract is preserved locally
r[workspace-crate-preservation.zellij-contract]
- GIVEN `crates/clankers-zellij/` remains in the workspace
- WHEN verification evidence is recorded
- THEN evidence checks that the iroh dependency still enables the `address-lookup-mdns` feature

### Requirement: Infrastructure crate local contracts
The system SHALL verify local preservation contracts for infrastructure crates that were previously extraction candidates.
r[workspace-crate-preservation.infrastructure-contracts]

#### Scenario: Protocol contract is preserved locally
r[workspace-crate-preservation.protocol-contract]
- GIVEN `crates/clankers-protocol/` remains in the workspace
- WHEN verification evidence is recorded
- THEN evidence includes a workspace build or focused package check that compiles the protocol crate
- AND evidence checks that `crates/clankers-protocol/src/event.rs` defines `pub enum DaemonEvent`
- AND evidence checks that `crates/clankers-protocol/src/command.rs` defines `pub enum SessionCommand`
- AND evidence checks that `crates/clankers-protocol/src/control.rs` defines `pub enum ControlResponse`
- AND evidence checks that `crates/clankers-protocol/src/frame.rs` defines `write_frame` and `read_frame` for length-prefix-plus-JSON framing

#### Scenario: Database contract is preserved locally
r[workspace-crate-preservation.db-contract]
- GIVEN `crates/clankers-db/` remains in the workspace
- WHEN verification evidence is recorded
- THEN evidence includes a workspace build or focused package check that compiles the database crate
- AND evidence checks that `crates/clankers-db/src/lib.rs` defines `pub struct Db`
- AND evidence checks that `crates/clankers-db/src/schema.rs` defines `migrate` and `version`
- AND evidence checks that `crates/clankers-db/src/schema.rs` opens local redb table definitions through `open_table`

#### Scenario: Hooks contract is preserved locally
r[workspace-crate-preservation.hooks-contract]
- GIVEN `crates/clankers-hooks/` remains in the workspace
- WHEN verification evidence is recorded
- THEN evidence includes a workspace build or focused package check that compiles the hooks crate
- AND evidence checks that `crates/clankers-hooks/src/point.rs` defines `pub enum HookPoint`
- AND evidence checks that `crates/clankers-hooks/src/dispatcher.rs` defines `HookPipeline`
- AND evidence checks that `crates/clankers-hooks/src/verdict.rs` defines `pub enum HookVerdict`

### Requirement: Generated artifact refresh decision
The system SHALL explicitly decide whether generated artifact refresh is required for the revised local-preservation scope.
r[workspace-crate-preservation.generated-artifacts]

#### Scenario: No generated artifacts are refreshed for no-op crate preservation
r[workspace-crate-preservation.generated-artifacts-none]
- GIVEN this change does not move crates, rename packages, remove wrappers, regenerate docs, or change user-visible TUI output
- WHEN final verification is recorded
- THEN evidence states that no generated artifact refresh is required
- AND future in-workspace rename or API changes must decide their own generated artifact refresh requirements

### Requirement: Preflight evidence preservation
The system SHALL preserve the original extraction preflight audit as historical evidence while applying the revised local-workspace decision.
r[workspace-crate-preservation.preflight-evidence]

#### Scenario: Preflight audit remains available
r[workspace-crate-preservation.preflight-audit]
- GIVEN `evidence/preflight-audit.md` records dependency-source, sibling-dirt, and snapshot-impact findings from the original extraction analysis
- WHEN this change is completed
- THEN that audit evidence remains available under the change evidence directory
- AND revised scope evidence states that remote extraction mechanics are no longer used

### Requirement: Future rename isolation
Any future rename of the remaining `clankers-*` package names MUST be handled in a separate focused change.
r[workspace-crate-preservation.future-renames]

#### Scenario: This change preserves names
r[workspace-crate-preservation.names-preserved]
- GIVEN the existing workspace package names start with `clankers-`
- WHEN this change completes
- THEN the package names and import paths remain unchanged
- AND no caller migration to `clanker_*` import paths is required by this change
