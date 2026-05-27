# Change: Allow Steel to propose orchestration-pack mutation

## Why

Repo-local Steel evolution packs make orchestration configurable without recompilation, but a human or Rust-only workflow would still need to edit the pack by hand. The higher-value loop is for Steel to evolve the orchestration it is using: update scripts, policies, gate selection, and workflow strategy based on receipts and repository evidence.

That should be allowed only inside a Rust-owned mutation cage. Steel can propose changes to orchestration material, but it must not gain raw filesystem, shell, git, credential, provider, daemon, TUI, native-tool, or capability-minting authority.

## What Changes

- Add a specific mutation class for repo-local Steel orchestration packs under `.clankers/steel/`.
- Let Steel produce typed orchestration patch proposals for its own pack scripts/policy/gate rules.
- Have Rust apply candidate changes only in an isolated worktree or staging area, run required gates, and activate the new pack only after validation.
- Record old/new pack hashes, patch hashes, gate receipts, rollback references, and human-checkpoint decisions.
- Treat host ABI or authority widening as an authority-kernel change that Steel cannot self-approve.

## Impact

- **Specs**: extend `steel-self-mutation-policy` with orchestration-pack mutation scenarios.
- **Depends on**: the repo-local evolution pack concept from `add-steel-repo-evolution-packs`.
- **Future code**: patch proposal schema, isolated apply path, gate runner integration, activation/rollback receipts, docs, and negative fixtures.
- **Non-goals**: no direct write-to-main, no automatic push, no Rust host capability minting, no raw shell/git/network/provider access from Steel, and no mutation outside reviewed pack roots in this slice.
