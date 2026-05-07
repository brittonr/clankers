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

`RuntimeServices::in_memory()` is the minimal no-ambient-path profile. It uses noop settings/auth/cache/project/skills/plugins/checkpoints and an in-memory session store.

Desktop Clankers adapters should wrap existing `~/.clankers`, project `.clankers`, auth, plugin, and session defaults explicitly instead of letting embedders inherit those paths by accident.

## Confirmation broker

Embedders supply a `ConfirmationBroker` that receives typed `ConfirmationRequest` values and returns `ConfirmationDecision` values. The default broker denies. Absent, unavailable, timed-out, and cancelled brokers fail closed.

Request summaries and event metadata are bounded/redacted and must not include raw credentials, headers, environment values, provider payloads, or hidden prompt/context text.

## Current non-goals

The first runtime facade is an in-process Rust API and deterministic adapter seam. It does not replace the daemon protocol, provide C/FFI/web bindings, guarantee a specific provider backend, or make plugin/MCP subprocess runtimes mandatory for embedded applications.
