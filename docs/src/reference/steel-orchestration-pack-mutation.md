# Steel Orchestration Pack Mutation

Steel may evolve actual repo-local orchestration by proposing patches to `.clankers/steel/` pack files. It still does not get raw filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session-mutation, or capability-minting authority.

## Flow

1. Steel reads safe receipts/context through the repo evolution host ABI.
2. Steel emits `clankers.steel.orchestration-patch.v1` with intent, target paths, expected pack hash, patch hash, selected gates, activation policy, and authority-change declarations.
3. Rust validates paths, before hashes, patch hash shape, required gates, activation policy, and authority-kernel boundaries before any write.
4. Rust applies candidate changes only in isolated staging/worktree state.
5. Rust runs required Steel pack gates.
6. A changed pack activates only on explicit reload or a later turn after receipt recording.
7. Rollback verifies current post-apply and backup hashes before restoring files.

## Authority kernel

Steel may update scripts, gate selection, and repo-local policy that stays within the existing Rust host ABI. It may not self-approve:

- new host calls
- wider budgets
- new UCAN abilities
- broader path roots
- credential/provider/network access
- direct git commit or push
- disabling required gates for its own mutation
- Rust source capability changes

Those requests are denied as authority-kernel changes and must become a human/oracle checkpoint or ordinary coding-agent task.

## Metaprogramming

Steel macros and DSL expansion are allowed for orchestration planning, but Rust validates the expanded typed patch proposal. Dynamic code generation cannot add host calls or bypass the typed schema.

## Receipts

Receipts include old/new pack hashes, patch hash, safe target metadata, policy/script hashes where available, selected gates, gate result hashes, activation decision, rollback reference, issue code, and receipt hash. They omit raw prompts, credentials, compact UCAN tokens, provider payloads, secret paths, unbounded patch bodies, and private transcript material.

## Verification

Run:

```text
./scripts/check-steel-orchestration-pack-mutation.rs
```

The checker covers valid update, path escape, stale before hash, authority widening, required gate removal, failed validation, malformed schema, malformed patch hash, stale rollback, and guarded rollback fixtures. It writes `target/steel-orchestration-pack-mutation/receipt.json`.
