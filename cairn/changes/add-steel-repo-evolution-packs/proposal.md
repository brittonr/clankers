# Change: Add repo-local Steel evolution packs

## Why

Steel is now the default turn-planning seam, but the reviewed script/profile are bundled with the binary. That is good for a safe first default, but it means repo-specific orchestration improvements still require Rust edits or recompilation when the desired change is only policy, workflow, gate selection, or planning strategy.

Clankers should let each repository carry a repo-local Steel evolution pack that can be loaded at runtime, hash-bound in receipts, and constrained by a stable Rust host ABI. This gives repositories their own evolvable orchestration without granting Steel ambient file, shell, git, network, provider, credential, or capability-minting authority.

## What Changes

- Add a repo-local Steel evolution pack format rooted at `.clankers/steel/`.
- Use Nickel as the source configuration language for the pack contract, with exported typed data consumed by Rust.
- Define a stable host ABI for planning/evolution host calls that Rust owns and versions.
- Require pack discovery, validation, hash receipts, hot reload, and default-deny behavior when no pack is present.
- Keep Steel in a plan/policy role: it may emit typed evolution plans, gate requests, and patch proposals, while Rust remains the execution authority.

## Impact

- **Specs**: add `steel-repo-evolution-packs`.
- **Future code**: settings/path discovery, Nickel export validation, Steel runtime profile loading, receipt schema, docs, and focused checks.
- **Safety**: no new ambient authority; all effects cross Rust authorization and policy gates.
- **Non-goals**: no automatic write-to-main, no git push, no credential access, no raw shell, no provider calls from Steel, and no mutation of Rust host capabilities in this change.
