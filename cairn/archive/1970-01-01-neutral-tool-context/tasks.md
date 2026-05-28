## Phase 1: Neutral context and adapters

- [x] [serial] I1: Define a neutral tool invocation context with call identity, cancellation, safe progress/event sink, capability metadata, and typed optional host services. [covers=r[neutral-tool-context.context-contract.neutral-fields]]
- [x] [serial] I2: Add adapter layers between the existing agent `Tool` trait and `clankers-tool-host::ToolExecutor` so migration can be incremental. [covers=r[neutral-tool-context.adapter-compatibility.old-new-bridge]]
- [x] [parallel] I3: Move storage, search, hook, and progress dependencies behind host service traits or neutral DTOs instead of direct `Db`, search-index, hook-pipeline, `AgentEvent`, or TUI DTO fields. [covers=r[neutral-tool-context.host-services.no-shell-fields]]
- [x] [serial] I4: Migrate a representative built-in read-only tool and one mutating/tool-progress path to the neutral context. [covers=r[neutral-tool-context.migration.representative-tools]]

## Phase 2: Verification

- [x] [parallel] V1: Add fixtures for successful tool execution, missing storage service, capability denial, cancellation, progress emission, and truncation through the neutral context. [covers=r[neutral-tool-context.verification.tool-fixtures]]
- [x] [serial] V2: Add or extend architecture rails to reject shell-only imports in reusable tool-host context modules and run focused tool/agent parity checks plus Cairn gates. [covers=r[neutral-tool-context.verification.boundary-rail]]
