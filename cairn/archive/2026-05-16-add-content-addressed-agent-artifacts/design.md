## Context

The target is a Rust implementation of Unison-inspired content addressing for Clankers agent artifacts, not a new programming language or replacement for git. Source files remain text; artifact storage captures normalized runtime objects and links them to session/review receipts.

## Goals / Non-Goals

**Goals:**
- Provide stable cryptographic identities for prompts, tools, model requests, skills references, manifests, and session blocks.
- Make review/replay receipts answer "what exact artifact did this use?"
- Lay a safe foundation for deterministic pure-result caching.

**Non-Goals:**
- Store arbitrary Rust definitions by AST hash.
- Replace Cargo, git, OpenSpec, or JSONL session export.
- Claim all shell/network tools are pure or cacheable.

## Decision 1: Hash canonical normalized artifact envelopes

**Choice:** Each supported artifact is converted into a versioned canonical envelope before hashing. The envelope includes artifact kind, schema version, canonical payload, redaction class, and dependency hashes when applicable.

**Rationale:** Hashing raw serde output or display text is brittle. Versioned envelopes make hash changes intentional and reviewable.

**Alternative:** Hash raw JSONL/session records. Rejected because incidental ordering, timestamps, and redacted fields would create false drift.

**Implementation:** Add pure canonicalization functions with golden fixtures and a typed `ArtifactHash` newtype.

## Decision 2: Names are metadata pointers, hashes are authoritative identity

**Choice:** Human labels such as skill name, prompt name, tool name, provider alias, or session block ID remain queryable metadata but do not define immutable identity.

**Rationale:** This mirrors Unison's name-pointer model while preserving Clankers UX.

**Alternative:** Require users to invoke hashes everywhere. Rejected as hostile to normal CLI/TUI use.

## Decision 3: Pure-result caching is opt-in and proof-backed

**Choice:** Only operations with declared deterministic inputs and no disallowed effects may use the hash-keyed result cache.

**Rationale:** Incorrect caching is worse than no caching; Clankers must prove negative side-effect claims with sentinels or explicit allowlists.

**Alternative:** Cache all command/tool outputs by command line. Rejected because env, filesystem, network, and time inputs are often hidden.

## Risks / Trade-offs

**Hash drift** → Pin canonicalization fixtures and schema versions.

**Receipt bloat** → Store hashes inline and keep payloads in the artifact store.

**Secret leakage** → Hash redacted canonical forms for receipts and keep secret-bearing material outside inspect output.

## Validation Plan

- Golden hash fixtures for every artifact kind.
- Negative tests showing timestamps, display names, and map ordering do not affect hashes where excluded.
- CLI inspection tests for `inspect-hash` success, missing hash, and redacted artifact cases.
- Replay tests proving model/tool request receipts resolve the same artifact hashes.
