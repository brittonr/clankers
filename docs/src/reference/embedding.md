# Embedding Clankers

`clankers-runtime` is the Rust-facing embedding boundary for applications that want Clankers behavior without driving the CLI, TUI, daemon socket, ACP/MCP transport, or Matrix adapter.

## Runtime facade

Use `RuntimeBuilder` to construct a runtime with explicit host choices:

- `model_adapter(...)` for provider/router integration.
- `services(...)` for settings, auth, session, cache, project-context, skills, plugins, and checkpoint storage.
- `prompt_assembly(...)` for host context and filesystem-discovery policy.
- `tool_catalog(...)` for capability packs and host custom tools.
- `confirmation_broker(...)` for host-owned approvals.

A built `Runtime` creates `SessionHandle` values. A session handle exposes:

- `submit_prompt(PromptInput)`
- `take_events()` for ordered `SessionEvent` values
- `set_model(...)`
- `set_disabled_tools(...)`
- `interrupt()`
- `shutdown()`

The public event stream is semantic: prompt accepted, thinking/assistant deltas, tool start/finish, confirmation requested, cost update, completion, error, and shutdown. It intentionally avoids daemon protocol frames, TUI widget types, CLI argument structs, and ACP/MCP JSON-RPC envelopes.

## Prompt assembly

`PromptAssembler` accepts a `PromptAssemblyPolicy` and `PromptSources` rather than reading ambient project files by default.

- `PromptAssemblyPolicy::host_context_only()` disables filesystem discovery and context-reference expansion for embedders.
- `PromptAssemblyPolicy::desktop_default()` is the adapter-side policy for normal Clankers shells.
- `examples/prompt-assembly-kit/` is the checked copyable recipe for this brick: it assembles host-owned context, rejects ambient filesystem discovery, records unsupported context-reference metadata, and emits a deterministic BLAKE3 receipt hash over redacted prompt evidence.
- Provenance records safe labels/counts/summaries and redacts secret-like content markers.

## Tool capability packs

`ToolCatalog::embedding_safe()` publishes only read-only descriptors. Side-effecting packs are explicit opt-ins:

- `ReadOnly`
- `WorkspaceMutation`
- `ShellCommands`
- `Network`
- `ExternalProcesses`

Every descriptor declares its `SideEffectLevel` and whether confirmation is required. Host custom tools are registered through `ToolCatalogBuilder::custom_tool`, which rejects name collisions instead of silently overriding built-ins.

## Host-owned runtime services

`RuntimeServices::in_memory()` is the minimal no-ambient-path profile. It uses noop settings/auth/cache/project/skills/plugins/checkpoints, an in-memory session store, and disabled extension services.

Extension systems are split from ordinary stores through `ExtensionServices`:

- `provider_router` owns provider/router execution. The default disabled service does not autostart the desktop router daemon or contact provider backends.
- `auth_store` owns provider-scoped auth lookup, OAuth verifier persistence, and token-refresh persistence policy. The default disabled service cannot write login verifiers or refreshed credentials.
- `credential_pool` owns account selection, rotation, cooldown, and pool strategy. The default disabled service never reads desktop credential pools.
- `runtime` owns plugin, MCP, and gateway publication/execution. The default disabled service publishes no extension tools and does not launch plugin subprocesses, connect to MCP servers, inherit environment/header values, or deliver gateway artifacts.

Desktop Clankers adapters wrap existing `~/.clankers`, project `.clankers`, auth, plugin, MCP, gateway, and session defaults explicitly. CLI/TUI/daemon/ACP/MCP shells should opt into those adapters when they want current desktop behavior; embedders should supply their own services or keep the disabled profile.

Extension receipts and descriptors carry safe replay/debug metadata only: source class, action, status, timing, provider/server/tool labels, and error class. They must not contain API keys, OAuth/refresh tokens, authorization headers, environment values, raw provider request/response bodies, login-verifier secrets, credential file contents, raw plugin/MCP arguments, raw plugin output, or plugin state contents.

Catalog construction remains separate from extension execution. Building a catalog may include extension-backed descriptors only when the host supplied/enabled the matching runtime service; metadata queries must not start routers, OAuth flows, plugin subprocesses, MCP servers, or gateway delivery paths.

## Confirmation broker

Embedders supply a `ConfirmationBroker` that receives typed `ConfirmationRequest` values and returns `ConfirmationDecision` values. The default broker denies. Absent, unavailable, timed-out, and cancelled brokers fail closed.

- `examples/confirmation-broker-kit/` is the checked copyable recipe for this brick: it shows a host-owned approval broker, proves that denied/default/unavailable brokers do not execute the protected action, redacts secret-like request summaries, and emits a deterministic BLAKE3 receipt hash over safe decision evidence.

Request summaries and event metadata are bounded/redacted and must not include raw credentials, headers, environment values, provider payloads, or hidden prompt/context text.

## Current non-goals

The first runtime facade is an in-process Rust API and deterministic adapter seam. It does not replace the daemon protocol, provide C/FFI/web bindings, guarantee a specific provider backend, or make plugin/MCP subprocess runtimes mandatory for embedded applications.
