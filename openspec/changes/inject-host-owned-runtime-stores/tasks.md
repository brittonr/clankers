## Phase 1: Service boundary

- [x] [serial] Write the host-owned runtime stores OpenSpec package. [covers=embeddable-runtime-stores.host-owned-services] [evidence=openspec validate inject-host-owned-runtime-stores --strict]
- [ ] [serial] Define runtime service/config traits or structs for settings, auth, sessions, cache, project context, skills, plugins, and checkpoints. [covers=embeddable-runtime-stores.host-owned-services]
- [ ] [parallel] Add in-memory/noop service implementations for minimal embedded runtime tests. [covers=embeddable-runtime-stores.host-owned-services.no-ambient-paths]
- [ ] [parallel] Wrap existing Clankers path/auth/session/plugin defaults as explicit desktop adapters. [covers=embeddable-runtime-stores.host-owned-services.desktop-adapters]

## Phase 2: Capability and parity tests

- [ ] [serial] Add tests proving minimal embedded fake-provider prompts do not touch ambient global/project paths. [covers=embeddable-runtime-stores.host-owned-services.no-ambient-paths]
- [ ] [parallel] Add safe capability metadata for missing injected services. [covers=embeddable-runtime-stores.capability-metadata]
- [ ] [parallel] Add in-memory session replay parity coverage. [covers=embeddable-runtime-stores.parity.in-memory-session]
