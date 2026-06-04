# External Memory Providers API Surface

This document fixes the first-pass user-facing surface for `add-external-memory-providers` before implementation.

## First-pass capability

External memory providers are disabled by default. When enabled and valid, clankers publishes an agent tool named `external_memory`. The tool gives the agent a structured way to query an external personalization/memory backend while preserving the existing curated local `memory` tool and local prompt memory injection.

The first pass supports a deterministic local provider kind for testing and a generic HTTP-shaped provider configuration seam. Named providers such as Honcho, OpenViking, Mem0, Hindsight, Holographic, RetainDB, ByteRover, and Supermemory remain documented future provider kinds unless explicitly implemented later.

## Configuration surface

Add `externalMemory` to `Settings`:

```json
{
  "externalMemory": {
    "enabled": false,
    "provider": "local",
    "name": "project-memory",
    "endpoint": null,
    "credentialEnv": null,
    "timeoutMs": 30000,
    "maxResults": 8,
    "injectIntoPrompt": false
  }
}
```

Fields:

- `enabled`: publishes and activates the capability only when true.
- `provider`: first-pass enum. Supported: `local`; optional generic `http` seam may validate but can return unsupported until implemented. Unsupported provider names fail explicitly.
- `name`: safe provider label used in metadata; must be non-blank when enabled.
- `endpoint`: provider endpoint when the provider kind requires one; must be non-blank when present.
- `credentialEnv`: optional environment variable name containing credentials; metadata records only the variable name, never the value.
- `timeoutMs`: request timeout; must be greater than zero when set.
- `maxResults`: bounded result count; must be greater than zero when set.
- `injectIntoPrompt`: disabled by default. If true, prompt/session ingress can call the retrieval helper before model contact; failures are surfaced as actionable errors rather than silently ignored.

## Agent tool surface

Tool name: `external_memory`.

Actions:

- `search`: query the configured external provider.
- `status`: return provider/configuration status without contacting a remote provider unless required by the provider kind.

Input schema shape:

```json
{
  "action": "search",
  "query": "repo conventions",
  "limit": 5,
  "scope": "project"
}
```

Output is structured text/JSON-compatible content with:

- `source: "external_memory_provider"`
- `provider`: safe configured provider kind/name
- `action`
- `status`: `ok` or `error`
- `resultCount`
- bounded result snippets/labels when search succeeds
- safe, redacted error detail when it fails

## TUI / slash command surface

Extend `/memory` help with external-memory status/search only after the tool/config implementation lands:

- `/memory external status`
- `/memory external search <query>`

The slash UX should use the same provider adapter and metadata normalization as the tool path.

## CLI / daemon / prompt paths

No new top-level CLI command is required for the first pass. Existing prompt paths (`clankers -p`, `--mode json`, `--inline`), the interactive TUI, and daemon/session attach paths share the same config-driven publication and optional prompt-injection helper.

If `externalMemory.injectIntoPrompt = false`, external memories are available only through the explicit tool/slash path. If true, prompt ingress must bound query/results and record normalized metadata.

## Session observability

When the capability runs inside a persisted session, record custom metadata named `external_memory_provider` with only safe fields:

- `source`
- `providerKind`
- `providerName`
- `action`
- `status`
- `elapsedMs` or equivalent timing
- `resultCount`
- `errorKind` / redacted `error` when applicable

Do not record credentials, token values, request headers, raw prompts, raw memory contents, full provider payloads, or connection strings.

## Unsupported first-pass cases

The implementation must return explicit actionable errors for:

- capability disabled or missing required config
- unsupported provider kinds
- blank provider names, endpoints, credential environment names, or other invalid config
- non-positive timeout/result limits
- remote provider kinds not implemented in the first pass
- prompt injection requested in a mode that cannot safely record metadata
- provider errors that would otherwise leak secrets; errors must be redacted before returning or recording

## Test expectations

- Config defaults keep the capability disabled.
- Valid local provider config parses and validates.
- Invalid/unsupported config fails before provider contact.
- `external_memory` is published only when enabled and valid.
- Successful local/fake search returns structured, bounded results.
- Failure path returns an actionable, redacted error and records safe metadata when a session manager is available.
