# Steel Orchestration Pack Mutation

Steel may evolve actual repo-local orchestration by proposing patches to `.clankers/steel/` pack files. It still does not get raw filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session-mutation, or capability-minting authority.

## Flow

1. Steel reads safe receipts/context through the repo evolution host ABI.
2. Steel emits `clankers.steel.orchestration-patch.v1` with intent, target paths, expected pack hash, patch hash, selected gates, activation policy, and authority-change declarations.
3. Rust validates paths, before hashes, patch hash shape, required gates, activation policy, and authority-kernel boundaries before any write.
4. Rust writes candidate payloads only under an isolated staging directory/worktree after payload targets exactly match the validated target list.
5. Rust runs required Steel pack gates against the staged state.
6. Promotion copies staged files to the live pack only after the live before-hash and staged after-hash match the receipt, while first copying live files to a backup root.
7. A changed pack activates only on explicit reload or a later turn after receipt recording.
8. Rollback verifies current post-apply and backup hashes before restoring files from the backup root.

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

Raw write requests such as `raw_write:...`, `write_file:...`, or `fs.write:...` are denied before any side effect and recorded only as the safe authority class `raw_write`. Other authority-kernel changes are denied and must become a human/oracle checkpoint or ordinary coding-agent task.

## Metaprogramming

Steel macros and DSL expansion are allowed for orchestration planning, but Rust validates the expanded typed patch proposal. Dynamic code generation cannot add host calls or bypass the typed schema.

## Isolated staging

`stage_orchestration_patch_to_directory(...)` is the Rust-owned isolated apply seam. It first runs pure preflight validation, then writes each typed payload below the supplied staging root using the already-validated `.clankers/steel/` relative path. Payload sets must exactly match `target_paths`; path escapes, missing payloads, and extra payloads fail before promotion. The live working tree is not touched by staging.

`promote_staged_orchestration_pack_to_directory(...)` is the live apply seam. It hashes the current live target set, verifies the staged target set hash, copies live files to a backup root, then copies staged files to live. `rollback_orchestration_pack_to_directory(...)` restores only when current live files match the recorded post-apply hash and backup files match the recorded pre-apply hash.

## Receipts

Receipts include old/new pack hashes, patch hash, safe target metadata, policy/script hashes where available, selected gates, gate result hashes, activation decision, rollback reference, issue code, and receipt hash. Denied receipts redact malformed patch hashes, unsafe target paths, and raw authority-change payloads to bounded classes. They omit raw prompts, credentials, compact UCAN tokens, provider payloads, secret paths, unbounded patch bodies, and private transcript material.

## Verification

Run:

```text
./scripts/check-steel-orchestration-pack-mutation.rs
```

The checker covers valid update, path escape, stale before hash, raw write attempt, authority widening, required gate removal, failed validation, malformed schema, malformed patch hash, unsafe receipt content, stale rollback, and guarded rollback fixtures. It writes `target/steel-orchestration-pack-mutation/receipt.json`.
