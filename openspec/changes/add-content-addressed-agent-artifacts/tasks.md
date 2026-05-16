## Phase 1: Artifact identity core

- [x] [serial] Define `ArtifactHash`, artifact kinds, canonical envelope versions, and redaction classes. [covers=content-addressed-agent-artifacts.identity] ✅ 4m 26s (started: 2026-05-16T23:01:54Z → completed: 2026-05-16T23:06:20Z; evidence: `cargo test -p clankers-artifacts`)
- [x] [depends:identity-core] Add pure canonicalization and golden hash fixtures for prompts, tool descriptors, model requests, manifests, skills references, and session blocks. [covers=content-addressed-agent-artifacts.canonicalization] ✅ 2m 45s (started: 2026-05-16T23:06:39Z → completed: 2026-05-16T23:09:24Z; evidence: `cargo test -p clankers-artifacts`)

## Phase 2: Storage and receipts

- [x] [depends:identity-core] Implement immutable artifact storage and name-pointer metadata without changing existing JSONL export compatibility. [covers=content-addressed-agent-artifacts.store] ✅ 2m 33s (started: 2026-05-16T23:09:43Z → completed: 2026-05-16T23:12:16Z; evidence: `cargo test -p clankers-artifacts`)
- [x] [depends:artifact-store] Thread artifact hashes into model/tool/session/review receipts with redacted inspect output. [covers=content-addressed-agent-artifacts.receipts] ✅ 1m 33s (started: 2026-05-16T23:12:41Z → completed: 2026-05-16T23:14:14Z; evidence: `cargo test -p clankers-artifacts`)
- [x] [depends:artifact-store] Add `clankers inspect-hash <hash>` for present, missing, wrong-kind, and redacted artifacts. [covers=content-addressed-agent-artifacts.inspect] ✅ 8m 18s (started: 2026-05-16T23:14:14Z → completed: 2026-05-16T23:22:32Z; evidence: `cargo test -p clankers-artifacts`; `cargo test -p clankers --lib inspect_hash`; `cargo test -p clankers --lib inspect_hash_cli_parses_hash_store_and_kind`)

## Phase 3: Deterministic cache foundation

- [x] [depends:receipts] Add an opt-in pure-result cache keyed by artifact/input hashes and explicit deterministic-input declarations. [covers=content-addressed-agent-artifacts.pure-cache] ✅ 5m 10s (started: 2026-05-16T23:22:51Z → completed: 2026-05-16T23:28:01Z; evidence: `cargo test -p clankers-artifacts`)
- [ ] [depends:pure-cache] Add side-effect and invalidation tests covering changed inputs, hidden env denial, network/shell denial, and cache hit receipts. [covers=content-addressed-agent-artifacts.cache-rails]
- [ ] [serial] Run focused artifact-store tests, replay tests, and `cargo nextest run` subset; record acceptance receipts. [covers=content-addressed-agent-artifacts.validation]
