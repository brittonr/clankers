## Phase 1: Service boundary

- [x] [serial] Write the host-owned runtime stores OpenSpec package. [covers=embeddable-runtime-stores.host-owned-services] [evidence=openspec validate inject-host-owned-runtime-stores --strict]
- [x] [serial] Define runtime service/config traits or structs for settings, auth, sessions, cache, project context, skills, plugins, and checkpoints. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-stores.host-owned-services] [evidence=clankers_runtime::RuntimeServices]
- [x] [parallel] Add in-memory/noop service implementations for minimal embedded runtime tests. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-stores.host-owned-services.no-ambient-paths] [evidence=clankers-runtime::tests::default_runtime_does_not_need_ambient_paths]
- [x] [parallel] Wrap existing Clankers path/auth/session/plugin defaults as explicit desktop adapters. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-stores.host-owned-services.desktop-adapters] [evidence=clankers_runtime::DesktopRuntimeServices marker]

## Phase 2: Capability and parity tests

- [x] [serial] Add tests proving minimal embedded fake-provider prompts do not touch ambient global/project paths. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-stores.host-owned-services.no-ambient-paths] [evidence=clankers-runtime::tests::default_runtime_does_not_need_ambient_paths]
- [x] [parallel] Add safe capability metadata for missing injected services. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-stores.capability-metadata] [evidence=RuntimeServices::capability_metadata]
- [x] [parallel] Add in-memory session replay parity coverage. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-stores.parity.in-memory-session] [evidence=clankers-runtime::tests::in_memory_session_replay_records_last_prompt]
