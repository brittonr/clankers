## Phase 1: Spec baseline

- [x] [serial] Write the extension-runtime hosting OpenSpec package. [covers=embeddable-agent-engine.extension-services.host-owned] [evidence=openspec validate inject-extension-runtime-hosts --strict]

## Phase 2: Runtime service contracts

- [ ] [serial] Add host-facing extension service contracts for router/provider execution, auth-store access, credential-pool policy, and plugin/MCP runtime lifecycle. [covers=embeddable-agent-engine.extension-services.host-owned]
- [ ] [parallel] Add default-safe embedded-runtime tests proving router autostart, OAuth/login verifier writes, credential refresh persistence, plugin subprocesses, MCP servers, and gateway delivery are disabled unless explicitly enabled. [covers=embeddable-agent-engine.extension-services.default-safe]
- [ ] [parallel] Add safe extension receipt/metadata tests proving provider/router/auth/plugin debug data excludes tokens, headers, environment values, provider request bodies, and raw plugin payloads. [covers=embeddable-agent-engine.extension-services.safe-metadata]

## Phase 3: Clankers desktop adapters

- [ ] [serial] Wrap current Clankers router/auth/plugin defaults as explicit desktop adapter implementations over the extension-service contracts. [covers=embeddable-agent-engine.extension-services.desktop-adapter-parity]
- [ ] [parallel] Add parity fixtures proving normal CLI/TUI/daemon provider discovery, provider-scoped auth, credential-pool behavior, plugin/MCP publication, and fail-closed known-provider prefixes still match current behavior through the adapters. [covers=embeddable-agent-engine.extension-services.desktop-adapter-parity]
- [ ] [parallel] Document embedding extension policy, including how hosts opt into router/auth/plugin/MCP/gateway capabilities and what remains disabled by default. [covers=tool-host-embedding.extension-runtime.explicit-publication]
