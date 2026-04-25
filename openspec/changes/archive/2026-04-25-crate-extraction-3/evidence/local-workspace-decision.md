Artifact-Type: verification-evidence
Task-ID: all
Covers: workspace-local-preservation, external-extraction-work-avoidance, local-contract-verification

# Workspace-Local Crate Decision Evidence

## Decision

User instruction on 2026-04-24: do not create separate GitHub repositories for the remaining crate-extraction targets; keep them in this workspace as separate crates.

## Scope Consequences

External extraction mechanics are no longer required for this change:

- no `git subtree split` branches
- no new GitHub repositories
- no split-branch pushes
- no standalone CI badge work
- no temporary migration wrappers
- no replacement of path crates with git dependencies

## Workspace Membership Check

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

## Local Contract Checks

Command:

```bash
grep -n '^\[features\]\|eval\|refscan' crates/clankers-nix/Cargo.toml
grep -n 'matrix-sdk' crates/clankers-matrix/Cargo.toml
grep -n 'address-lookup-mdns\|iroh' crates/clankers-zellij/Cargo.toml
grep -n 'pub enum HookPoint' crates/clankers-hooks/src/point.rs
```

Output excerpt:

```text
crates/clankers-nix/Cargo.toml: eval = ["dep:snix-eval", "dep:snix-serde"]
crates/clankers-nix/Cargo.toml: refscan = ["dep:snix-castore"]
crates/clankers-nix/Cargo.toml: snix rev = "8fe3bade2013befd5ca98aa42224fa2a23551559"
crates/clankers-matrix/Cargo.toml: matrix-sdk features = ["e2e-encryption", "sqlite", "rustls-tls"]
crates/clankers-zellij/Cargo.toml: iroh features = ["address-lookup-mdns"]
crates/clankers-hooks/src/point.rs: pub enum HookPoint
```

## Build Evidence

During the OpenSpec drain, `cargo check --lib` passed after the preceding implementation work (`pueue` task 83, isolated target `/tmp/clankers-check-target`). No runtime code changes are required by this scope resolution.

## Generated Artifact Hygiene

No crate moves, package renames, wrapper removals, generated docs updates, or user-visible TUI snapshot-affecting changes are performed by the revised scope. Therefore no generated artifact refresh is required by this change. Future in-workspace rename/API work must decide its own generated artifact refresh requirements.
