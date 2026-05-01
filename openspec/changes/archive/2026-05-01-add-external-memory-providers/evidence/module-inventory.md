# External Memory Providers Inventory

Generated during OpenSpec drain for `add-external-memory-providers`. This records module ownership and first-pass integration boundaries before defining the user-facing API surface.

## Existing local memory ownership

- `crates/clankers-db/src/memory.rs`: owns durable curated local memory entries (`MemoryEntry`, `MemoryScope`, `MemorySource`) and prompt-context formatting via `context_for` / `context_for_with_limits`. External providers should not replace curated local memory; they should adapt into or alongside a provider abstraction that can merge retrieved context with local memory.
- `src/tools/memory.rs`: owns the agent-facing `memory` tool semantics (`add`, `replace`, `remove`, `search`) and capacity enforcement. It currently requires `ToolContext` database access and returns user-visible errors when unavailable. External provider tool behavior should reuse this style: explicit configuration/policy errors, no silent fallback.
- `src/slash_commands/handlers/memory.rs`: owns interactive `/memory` UX for list/add/edit/remove/search/clear and `/system` prompt controls. TUI UX for external memory should live here or in a nearby handler after the config/provider surface is defined.

## Configuration and publication ownership

- `crates/clankers-config/src/settings.rs`: owns typed settings. It already has `memory: MemoryLimits`, `mcp: McpSettings`, `browser_automation: BrowserAutomationSettings`, and `default_capabilities`. Add external-memory provider settings here, disabled by default, with provider kind/name, endpoint, credential environment references, timeouts, and redaction-safe labels.
- `src/modes/common.rs`: owns built-in tool registration through `ToolEnv` and `build_tiered_tools`. It currently publishes the local `memory` tool as Specialty and conditionally publishes `browser` when settings validate. Publish external-memory capability only when config validates, following the browser/MCP pattern.

## Prompt/session integration ownership

- `src/modes/agent_setup.rs` and `src/modes/interactive.rs`: own interactive agent construction and initial system prompt setup. Provider-derived memory context should be injected here only through an explicit, bounded retrieval path.
- Prompt ingress modes (`src/modes/json.rs`, `src/modes/print.rs`, `src/modes/inline.rs`) and daemon/session paths (`src/modes/attach.rs`, `src/modes/daemon/socket_bridge.rs`, `src/modes/daemon/agent_process.rs`) own non-interactive/daemon prompt execution. These paths must either share the same retrieval helper or return explicit unsupported/configuration errors.
- `crates/clankers-session/src/lib.rs` plus mode event handlers/session managers own replay/debug metadata. Record normalized provider operation metadata only: source, provider kind/name, action, status, timing, counts, and redacted errors. Do not persist credentials, tokens, prompts, raw memory text, or raw remote payloads in metadata.

## Test ownership

- `crates/clankers-config` tests: config parsing/default/validation behavior.
- `src/tools/memory.rs` or a new provider module: unit tests for provider request/response normalization, disabled/unsupported config failures, and secret redaction.
- `tests/`: integration coverage for tool publication, one successful first-pass operation using a deterministic fake/local provider, and one configuration/policy failure.

## First-pass boundary recommendation

Implement a generic, disabled-by-default external memory provider abstraction and a deterministic local/fake adapter seam first, not all named providers. The primary path should retrieve/search external memories and return structured results. Unsupported provider kinds, missing endpoint/credential references, and disabled configuration should fail before provider contact with actionable errors.
