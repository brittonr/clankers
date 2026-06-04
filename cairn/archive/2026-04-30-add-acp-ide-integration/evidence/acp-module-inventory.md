# ACP IDE Integration Module Inventory

## Existing implementation

- There is no existing ACP implementation or dependency in the repository. Repository searches for `ACP` and `Agent Client Protocol` only find this OpenSpec change and unrelated text.
- Clankers already has a transport-agnostic session/controller core that should be adapted rather than bypassed:
  - `crates/clankers-controller/src/command.rs` owns `SessionCommand` handling and prompt lifecycle dispatch.
  - `crates/clankers-controller/src/event_processing.rs`, `transport_convert.rs`, and `persistence.rs` translate agent events, persist session output, and shape daemon-visible events.
  - `crates/clankers-protocol/src/command.rs`, `event.rs`, `types.rs`, and `frame.rs` define the current daemon command/event protocol and image/content payloads.
- Existing daemon and attach paths provide useful adapter references:
  - `src/modes/daemon/agent_process.rs` owns the session actor loop.
  - `src/modes/daemon/socket_bridge.rs` drains controller events and broadcasts protocol events.
  - `src/modes/attach.rs` is the existing client-side command bridge, including prompt/image forwarding.
- `src/modes/common.rs` centralizes tool configuration and registration. ACP should reuse this path when it starts sessions so IDE sessions observe the same built-in, plugin, MCP, browser, and policy surfaces as other modes.
- `crates/clankers-session/src/entry.rs` and `crates/clankers-session/src/lib.rs` already support custom session entries through `SessionEntry::Custom` and `SessionManager::record_custom`, which is suitable for normalized ACP connection/session metadata.

## Proposed ownership

- CLI entrypoint: add an explicit ACP command under `src/commands/` and wire it from `src/cli.rs`/`src/main.rs`, instead of hiding ACP behind daemon attach.
- ACP adapter: add a small module such as `src/modes/acp.rs` or `src/modes/acp_server.rs` that owns ACP JSON-RPC/framing, maps ACP requests to `SessionCommand`, and maps `DaemonEvent`/controller output back to ACP notifications.
- Protocol boundaries: keep ACP-specific request/response types local to the adapter unless stable enough for a crate. Do not change core `clankers-protocol` framing just to mirror ACP.
- Session lifecycle: reuse `SessionController` and existing agent/session construction rather than creating a parallel agent loop.
- Config: put durable ACP settings in `crates/clankers-config/src/settings.rs` only if the first implementation exposes configurable listen/stdio/session behavior. Otherwise keep the first slice CLI-only with explicit unsupported config errors.
- Observability: record sanitized connection/session metadata via `SessionManager::record_custom("acp_ide_integration", ...)` when a persisted session is available.

## First-pass supported and unsupported cases to decide next

Supported first pass should be intentionally narrow:

- Run an ACP-compatible stdio adapter for one clankers session.
- Accept a prompt/new-turn request and stream normalized progress/output derived from existing controller events.
- Return explicit unsupported errors for IDE features not yet bridged, such as arbitrary terminal management, editor-native diff application, background task panels, or multi-workspace sessions.

Unsupported cases must be visible rather than silently dropped because ACP crosses process, project, and editor trust boundaries.

## Risk notes

- ACP can expand into editor-native terminals, diff management, tool activity, cancellation, permissions, and multi-session routing. The adapter must start with a minimal protocol subset.
- The current daemon protocol is clankers-specific; ACP should be an adapter over controller/session semantics, not a replacement transport.
- The implementation must preserve existing tool policy, sandboxing, disabled tools, credential isolation, session cwd boundaries, and replay semantics.
- Any persisted metadata must avoid credentials, raw headers, environment values, or provider-specific opaque blobs.

## Targeted checks for later tasks

- `CARGO_TARGET_DIR=target cargo nextest run -p clankers-config acp_ --no-fail-fast` if config is added.
- `CARGO_TARGET_DIR=target cargo nextest run -p clankers-controller acp --no-fail-fast` for command/event mapping helpers if they land in the controller crate.
- `CARGO_TARGET_DIR=target cargo nextest run -p clankers --test acp_ide_integration --no-fail-fast` for adapter integration coverage.
- `CARGO_TARGET_DIR=target cargo check --tests -p clankers -p clankers-config -p clankers-controller -p clankers-protocol` for shared lifecycle changes.
