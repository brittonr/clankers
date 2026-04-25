# crate-extraction-3 — Design

## Context

The original `crate-extraction-3` design continued a standalone repository extraction program for six remaining crates:

- `clankers-nix`
- `clankers-matrix`
- `clankers-zellij`
- `clankers-protocol`
- `clankers-db`
- `clankers-hooks`

That plan required GitHub repository creation, subtree splits, publishing split branches, thin workspace wrappers, caller migrations, and final wrapper removal. The user decision for this continuation is now explicit: do not create separate GitHub repositories for these crates; keep them in this workspace as separate crates.

## Goals / Non-Goals

**Goals**
- Preserve the six crates as independent workspace members.
- Remove external-repository extraction work from this change.
- Keep useful local verification contracts for crate boundaries.
- Make future in-workspace rename/API work explicit and separate.

**Non-Goals**
- Create GitHub repositories.
- Push split branches.
- Replace workspace path crates with git dependencies.
- Add migration wrappers.
- Rename crates in this change.
- Publish crates independently.

## Decisions

### 1. Keep the six targets as workspace-local crates

**Choice:** The six crates remain independent packages under `crates/` and continue to build as part of the clankers workspace.

**Rationale:** This preserves modular boundaries without adding release/publishing overhead or remote dependency management. It also avoids the repository-sprawl and push-permission concerns of the original external extraction plan.

**Implementation:** No code move is required. The workspace already contains these crates as separate packages.

### 2. Stop external extraction mechanics here

**Choice:** Remove tasks and requirements for subtree splits, GitHub repo creation, standalone CI badges, wrapper crates, and git-dependency migration.

**Rationale:** Those mechanics only apply to standalone repositories. Keeping crates in the workspace makes them unnecessary and potentially harmful busywork.

**Implementation:** Rewrite OpenSpec artifacts to local preservation semantics, then archive this change.

### 3. Preserve local crate-boundary evidence

**Choice:** Keep the verification intent from the original plan only where it applies to this no-op preservation scope:

- feature/pin preservation for `clankers-nix`
- Matrix SDK feature preservation for `clankers-matrix`
- iroh/mDNS feature preservation for `clankers-zellij`
- local ownership of daemon/client protocol types and framing code for `clankers-protocol`
- local ownership of redb schema/table APIs for `clankers-db`
- local ownership of hook dispatch/runtime types for `clankers-hooks`

**Rationale:** Those are still valuable crate-boundary facts even without repository extraction. Deeper behavioral changes belong in future focused changes.

**Implementation:** Specs describe what evidence must remain true while the crates stay in the workspace.

### 4. Defer renames to focused future changes

**Choice:** Do not rename `clankers-*` packages to `clanker-*` in this change.

**Rationale:** In-workspace renames still have broad import-path and package-name effects. They should be done only when there is a specific product need, with focused tests and migration tasks.

**Implementation:** Preserve existing package names/import paths.

## Architecture

### Before

```text
clankers workspace
└── crates/
    ├── clankers-nix/
    ├── clankers-matrix/
    ├── clankers-zellij/
    ├── clankers-protocol/
    ├── clankers-db/
    └── clankers-hooks/
```

### After

```text
clankers workspace
└── crates/
    ├── clankers-nix/       # unchanged workspace crate
    ├── clankers-matrix/    # unchanged workspace crate
    ├── clankers-zellij/    # unchanged workspace crate
    ├── clankers-protocol/  # unchanged workspace crate
    ├── clankers-db/        # unchanged workspace crate
    └── clankers-hooks/     # unchanged workspace crate
```

No standalone repos are created. No git dependencies replace path dependencies.

## Verification

The verification target is the workspace itself. Evidence is stored under `openspec/changes/crate-extraction-3/evidence/`.

### Scenario-to-evidence mapping

- `r[workspace-crate-preservation.local-targets]`: run directory and root `Cargo.toml` checks for all six crates.
- `r[workspace-crate-preservation.no-external-mechanics]` and `r[workspace-crate-preservation.no-external-publishing]`: inspect tasks/design and record that there are no GitHub repository, subtree split, split push, `cargo publish`, standalone CI, git dependency migration, or wrapper tasks.
- `r[workspace-crate-preservation.nix-contract]`: grep `crates/clankers-nix/Cargo.toml` for `eval`, `refscan`, and snix revision `8fe3bade2013befd5ca98aa42224fa2a23551559`.
- `r[workspace-crate-preservation.matrix-contract]`: grep `crates/clankers-matrix/Cargo.toml` for Matrix SDK features `e2e-encryption`, `sqlite`, and `rustls-tls`.
- `r[workspace-crate-preservation.zellij-contract]`: grep `crates/clankers-zellij/Cargo.toml` for iroh feature `address-lookup-mdns`.
- `r[workspace-crate-preservation.protocol-contract]`: include workspace build or package check evidence, plus grep `crates/clankers-protocol/src/{event.rs,command.rs,control.rs,frame.rs}` for `DaemonEvent`, `SessionCommand`, `ControlResponse`, `write_frame`, and `read_frame`.
- `r[workspace-crate-preservation.db-contract]`: include workspace build or package check evidence, plus grep `crates/clankers-db/src/{lib.rs,schema.rs}` for `pub struct Db`, `migrate`, `version`, and `open_table`.
- `r[workspace-crate-preservation.hooks-contract]`: include workspace build or package check evidence, plus grep `crates/clankers-hooks/src/{point.rs,dispatcher.rs,verdict.rs}` for `HookPoint`, `HookPipeline`, and `HookVerdict`.
- `r[workspace-crate-preservation.generated-artifacts-none]`: record that no generated artifact refresh is required because this change does not move crates, rename packages, remove wrappers, regenerate docs, or change user-visible TUI output.
- `r[workspace-crate-preservation.preflight-audit]`: preserve `evidence/preflight-audit.md` with dependency-source, sibling-dirt, and snapshot-impact findings from the original extraction analysis.
- `r[workspace-crate-preservation.names-preserved]`: record that package names and import paths remain unchanged.

### Build evidence

The broader OpenSpec drain already produced `cargo check --lib` evidence after code changes. Since this revised scope makes only OpenSpec/evidence changes, no new runtime regression is expected. Focused package checks may be added if later artifact edits touch runtime code.

## Risks / Trade-offs

**Leaving reusable code in the workspace**
→ Acceptable. The user explicitly prefers local workspace crates over separate GitHub repos for these targets.

**Future rename pressure**
→ Mitigate by requiring a focused future OpenSpec if package/import names need to change.

**Spec drift from the original external extraction plan**
→ Mitigate by rewriting delta specs before archive so main specs do not claim external repositories are required.
