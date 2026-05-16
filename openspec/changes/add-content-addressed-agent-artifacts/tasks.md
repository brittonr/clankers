## Phase 1: Artifact identity core

- [ ] [serial] Define `ArtifactHash`, artifact kinds, canonical envelope versions, and redaction classes. [covers=content-addressed-agent-artifacts.identity]
- [ ] [depends:identity-core] Add pure canonicalization and golden hash fixtures for prompts, tool descriptors, model requests, manifests, skills references, and session blocks. [covers=content-addressed-agent-artifacts.canonicalization]

## Phase 2: Storage and receipts

- [ ] [depends:identity-core] Implement immutable artifact storage and name-pointer metadata without changing existing JSONL export compatibility. [covers=content-addressed-agent-artifacts.store]
- [ ] [depends:artifact-store] Thread artifact hashes into model/tool/session/review receipts with redacted inspect output. [covers=content-addressed-agent-artifacts.receipts]
- [ ] [depends:artifact-store] Add `clankers inspect-hash <hash>` for present, missing, wrong-kind, and redacted artifacts. [covers=content-addressed-agent-artifacts.inspect]

## Phase 3: Deterministic cache foundation

- [ ] [depends:receipts] Add an opt-in pure-result cache keyed by artifact/input hashes and explicit deterministic-input declarations. [covers=content-addressed-agent-artifacts.pure-cache]
- [ ] [depends:pure-cache] Add side-effect and invalidation tests covering changed inputs, hidden env denial, network/shell denial, and cache hit receipts. [covers=content-addressed-agent-artifacts.cache-rails]
- [ ] [serial] Run focused artifact-store tests, replay tests, and `cargo nextest run` subset; record acceptance receipts. [covers=content-addressed-agent-artifacts.validation]
