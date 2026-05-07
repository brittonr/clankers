## ADDED Requirements

### Requirement: Host-owned runtime services [r[embeddable-runtime-stores.host-owned-services]]

The embeddable runtime MUST accept host-owned services or explicit default adapters for settings, credentials/auth, session persistence, cache/database state, project context resolution, skill roots, plugin roots, and checkpoint storage policy.

#### Scenario: minimal embedded runtime uses no ambient global paths [r[embeddable-runtime-stores.host-owned-services.no-ambient-paths]]

- GIVEN a host constructs the runtime with in-memory or noop services and no default filesystem adapter
- WHEN the host creates a session and runs a fake-provider prompt with no filesystem tools enabled
- THEN the runtime does not read or create `~/.clankers`, project `.clankers`, global auth files, plugin roots, cache DBs, or JSONL session files
- THEN any feature that requires an absent service is unavailable with an explicit unsupported/configuration error

#### Scenario: Clankers desktop defaults are opt-in adapters [r[embeddable-runtime-stores.host-owned-services.desktop-adapters]]

- GIVEN the CLI, TUI, or daemon starts with normal Clankers path behavior
- WHEN it constructs runtime services
- THEN it does so through explicit desktop/default adapters for paths, settings, auth stores, session storage, cache, skills, plugins, and checkpoints
- THEN the same services can be replaced by embedding hosts without changing agent turn execution logic

### Requirement: Store capability metadata [r[embeddable-runtime-stores.capability-metadata]]

The runtime MUST expose safe metadata describing which host services are configured and which feature surfaces are unavailable because their backing stores or resolvers are absent.

#### Scenario: missing service suppresses dependent features [r[embeddable-runtime-stores.capability-metadata.suppress-features]]

- GIVEN the host does not provide a plugin root service, browser backend, session store, checkpoint backend, or external memory provider
- WHEN runtime capabilities and tool publication are computed
- THEN dependent features are omitted or marked unsupported before runtime contact
- THEN metadata reports safe feature names and status classes without leaking full paths, credentials, or environment values

### Requirement: Store injection parity [r[embeddable-runtime-stores.parity]]

The system MUST verify that replacing default filesystem stores with injected stores preserves core prompt/session semantics for supported features.

#### Scenario: in-memory session store preserves replay contract [r[embeddable-runtime-stores.parity.in-memory-session]]

- GIVEN an in-memory session store implementation
- WHEN a prompt completes and the session is resumed through the embeddable runtime
- THEN conversation context is reconstructed from the injected store
- THEN no JSONL filesystem session path is required for that test
