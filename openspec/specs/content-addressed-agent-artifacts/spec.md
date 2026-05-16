# content-addressed-agent-artifacts Specification

## Purpose

Define stable content-addressed identities, immutable storage, safe inspection, and deterministic pure-result caching for normalized Clankers agent artifacts.

## Requirements
### Requirement: Immutable artifact identity

The system MUST assign stable content hashes to supported normalized agent artifacts and MUST treat the hash as the authoritative identity for immutable artifact contents.
r[content-addressed-agent-artifacts.identity]

#### Scenario: hash identifies normalized artifact content
r[content-addressed-agent-artifacts.identity.normalized-content]

- GIVEN two artifacts with the same semantic payload but different map ordering or display-only formatting
- WHEN Clankers canonicalizes and hashes both artifacts
- THEN both artifacts receive the same hash
- THEN the stored artifact envelope records the artifact kind and schema version used to compute the hash

#### Scenario: semantic changes alter the hash
r[content-addressed-agent-artifacts.identity.semantic-change]

- GIVEN a prompt, tool descriptor, model request, manifest, skill reference, or session block has a semantic field changed
- WHEN Clankers canonicalizes and hashes the changed artifact
- THEN the resulting hash differs from the previous hash

### Requirement: Canonical artifact envelopes

The system MUST hash versioned canonical envelopes for prompts, tool descriptors, model requests, plugin and MCP manifests, skill references, and session blocks.
r[content-addressed-agent-artifacts.canonicalization]

#### Scenario: unsupported fields are rejected or explicitly excluded
r[content-addressed-agent-artifacts.canonicalization.exclusions]

- GIVEN an artifact contains timestamps, volatile display labels, credentials, or host-local paths
- WHEN the artifact is canonicalized
- THEN each volatile or secret-bearing field is either excluded by documented rule, redacted before hashing, or rejected with an explicit error
- THEN the decision is covered by a fixture test

### Requirement: Immutable artifact store with mutable name pointers

The system MUST store artifact payloads immutably by hash while allowing human-readable names and aliases to point at hashes as metadata.
r[content-addressed-agent-artifacts.store]

#### Scenario: name update does not mutate stored artifact
r[content-addressed-agent-artifacts.store.name-pointer]

- GIVEN a prompt name currently points to one artifact hash
- WHEN the name is updated to point to a new prompt artifact
- THEN the old artifact remains retrievable by hash
- THEN receipts that referenced the old hash continue to resolve to the old artifact

### Requirement: Receipts expose artifact provenance safely

The system MUST include artifact hashes in model requests, tool calls, session blocks, replay records, and review receipts where those artifacts influenced execution.
r[content-addressed-agent-artifacts.receipts]

#### Scenario: replay resolves original artifacts
r[content-addressed-agent-artifacts.receipts.replay]

- GIVEN a persisted session contains model and tool receipts with artifact hashes
- WHEN replay or review inspection loads the session
- THEN it can resolve the exact prompt, tool descriptor, request envelope, and session block artifacts that were used
- THEN missing artifacts are reported as missing provenance rather than silently ignored

#### Scenario: inspect output stays redacted
r[content-addressed-agent-artifacts.receipts.redaction]

- GIVEN an artifact includes fields classified as secret or raw provider payload
- WHEN a user inspects the artifact hash through CLI/TUI/review output
- THEN Clankers returns safe metadata and redacted payload fields only
- THEN raw credentials, headers, environment values, and unredacted provider bodies are not printed

### Requirement: Hash-keyed pure-result cache

The system MUST cache deterministic results only when the operation declares complete deterministic inputs and an allowed no-hidden-effect profile; implementations may leave the cache disabled, but any cache hit must satisfy this requirement.
r[content-addressed-agent-artifacts.pure-cache]

#### Scenario: deterministic cache hit
r[content-addressed-agent-artifacts.pure-cache.hit]

- GIVEN an operation declares its artifact hashes, file input hashes, environment allowlist, tool version, and no disallowed effects
- WHEN the same deterministic operation runs again with identical declared inputs
- THEN Clankers MAY reuse the cached result
- THEN the receipt records the cache key and hit status

#### Scenario: hidden effect blocks cache use
r[content-addressed-agent-artifacts.pure-cache.hidden-effect-denied]

- GIVEN an operation reads undeclared environment, uses network, touches time, mutates files, or starts a process outside its allowed profile
- WHEN Clankers evaluates cache eligibility
- THEN the operation is not served from the pure-result cache
- THEN the receipt records the denied effect class without exposing secrets

