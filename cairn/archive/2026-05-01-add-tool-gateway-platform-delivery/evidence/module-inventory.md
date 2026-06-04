Artifact-Type: module-inventory
Task-ID: inventory-tool-gateway-platform-delivery
Covers: r[tool-gateway-platform-delivery.capability], r[tool-gateway-platform-delivery.observability]
Generated: 2026-05-01T23:10:00Z

# Tool Gateway and Platform Delivery Module Inventory

## Existing owners

- `src/modes/common.rs`
  - Owns `ToolTier`, `ToolSet`, `ToolEnv`, `build_tiered_tools(...)`, and `build_all_tiered_tools(...)`.
  - This is the natural owner for first-pass gateway policy because standalone prompt, TUI, and daemon/session construction already flow through this shared catalog.
  - Gaps: tier enablement is enum-based, not named toolset policy; no normalized gateway summary or explicit delivery target model exists yet.

- `src/tools/`
  - Owns concrete tool implementations and `ToolResult::details` metadata.
  - Relevant first-pass integration points: `schedule` (scheduled task payloads), `mcp`/plugin tools (runtime-added catalog entries), `matrix_*` tools (platform bridge controls), and recently added safe metadata patterns in `external_memory` and `checkpoint`.
  - Gaps: no generic `delivery`/`gateway` module for validating delivery targets before a platform backend receives them.

- `src/modes/interactive.rs` and `src/modes/event_loop_runner/mod.rs`
  - Own the standalone/TUI schedule engine lifecycle and drain fired schedules into agent prompts.
  - `drain_schedule_events()` currently supports scheduled prompts with skills/scripts/model/toolset metadata but delivers back into the local TUI/session only.
  - Gaps: scheduled payloads do not yet carry a first-class delivery target; unsupported non-local delivery should be explicit.

- `src/modes/daemon/socket_bridge.rs` and `src/modes/daemon/agent_process.rs`
  - Own daemon session creation and build the same tool catalog via `ToolEnv` with daemon-specific channels, actors, schedule engine, and Matrix tier activation.
  - Gaps: delivery target validation is not centralized; daemon metadata should report target/action/status without recording raw payloads.

- `src/modes/matrix_bridge/mod.rs` and `src/modes/matrix_bridge/sendfile.rs`
  - Own Matrix platform ingress/egress, media download into session attachments, `<sendfile>...</sendfile>` extraction, sensitive-path checks, MIME guessing, and Matrix upload.
  - This is the existing platform delivery implementation to preserve, not rewrite.
  - Gaps: it is Matrix-only and response-text-tag driven; first-pass gateway should document that generic platform delivery is local/Matrix-aware only and unsupported elsewhere until more backends land.

- `clanker_scheduler` integration through `src/tools/schedule.rs`
  - Owns schedule CRUD and Hermes-compatible metadata fields such as `prompt`, `skills`, `script`, `model`, and `enabled_toolsets`.
  - Gaps: `enabled_toolsets` is stored as payload metadata for scheduled prompts but not enforced through a reusable gateway policy boundary.

- `crates/clankers-agent/src/tool.rs` and `crates/clanker-message/src/tool_result.rs`
  - Own the runtime tool trait/context and result details that get persisted/replayed.
  - Gaps: no shared delivery metadata schema yet; gateway should use `ToolResult::details` with safe fields only.

- `README.md` and `docs/src/reference/config.md`
  - Own user-facing tool/config documentation.
  - Gaps: no Tool Gateway/Platform Delivery section yet.

## Recommended first-pass boundary

1. Add a small Rust module for gateway policy and delivery target metadata rather than moving existing tool code.
2. Support a deterministic local-only inspection/validation surface first: named toolset parsing, local/session delivery target acceptance, and explicit unsupported errors for remote/platform targets that lack a backend.
3. Reuse `ToolEnv`/`ToolSet` for actual catalog publication and `ToolResult::details` for replay-safe metadata.
4. Keep Matrix sendfile delivery as the existing platform-specific backend; do not generalize it until policy tests and docs define the safe target contract.

## Metadata/redaction boundary

Gateway metadata may include action, status, backend, target kind, toolset names/counts, session id when available, and sanitized error strings. It must not include raw prompts, file contents, Matrix access tokens, room credentials, HTTP headers, or connection strings.
