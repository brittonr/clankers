## Why

Clankers already relies on review gates, replay, daemon recovery, and routed provider/tool seams, but many receipts still identify prompts, tool schemas, model requests, skills, and session blocks by mutable names, paths, or JSONL position. That makes determinism claims harder to prove and obscures which exact prompt/tool/request definition produced a result.

Unison's content-addressed codebase model suggests a practical Clankers adaptation: store normalized agent artifacts by content hash and treat human names as metadata pointers. This gives reviewable provenance without replacing Rust source control or text editing.

## What Changes

- **Content-addressed artifact store**: Add immutable hashed storage for normalized prompts, tool descriptors, model requests, skills references, plugin/MCP manifests, and session blocks.
- **Name pointers and receipts**: Record mutable names as pointers to hashes and surface hashes in replay/review receipts.
- **Pure-result cache foundation**: Allow deterministic checks and pure tool results to key off artifact/input hashes instead of timestamp or path heuristics.

## Capabilities

### New Capabilities
- `content-addressed-agent-artifacts`: Immutable artifact identity, pointer metadata, receipts, and inspection.

### Modified Capabilities
- `embeddable-agent-engine`: Engine/model/tool requests may carry artifact hash identities where available.
- `tool-host-embedding`: Tool catalog descriptors may be normalized and hashed before publication.

## Impact

- **Files**: likely new storage/index module, session persistence integration, provider/tool request constructors, CLI inspection commands, tests.
- **APIs**: add artifact hash types and optional hash metadata to receipts/events without breaking existing JSONL readers.
- **Dependencies**: prefer existing hashing stack; no network dependency.
- **Testing**: deterministic normalization tests, hash stability fixtures, replay/inspect CLI tests, cache invalidation tests.
