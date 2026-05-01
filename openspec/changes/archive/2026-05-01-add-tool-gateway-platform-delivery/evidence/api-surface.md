Artifact-Type: api-surface
Task-ID: define-tool-gateway-platform-delivery-surface
Covers: r[tool-gateway-platform-delivery.capability], r[tool-gateway-platform-delivery.observability]
Generated: 2026-05-01T23:13:00Z

# Tool Gateway and Platform Delivery API Surface

## First-pass user-facing surface

### CLI

Add a top-level inspection command:

```text
clankers gateway status [--json]
clankers gateway validate --toolsets <LIST> [--deliver <TARGET>] [--json]
```

- `status` reports the supported local gateway policy: available toolsets, supported delivery targets, and unsupported targets.
- `validate` checks named toolsets and one delivery target without executing a tool or sending data.
- `--toolsets` accepts comma-separated names matching the shared catalog vocabulary: `core`, `orchestration`, `specialty`, and `matrix`.
- `--deliver` accepts first-pass delivery target strings:
  - `local`: local session/TUI delivery, supported.
  - `session`: alias for local session delivery, supported.
  - `matrix`: reports supported only when a Matrix bridge path is already active; generic CLI validation should return an explicit unsupported error because it cannot infer a room/client.
  - `platform:<name>` / `http:<...>` / `telegram:<...>` / `discord:<...>` / `sms:<...>` / `s3:<...>`: unsupported in this slice.

### TUI / standalone prompt / daemon session paths

- The shared `ToolSet` remains the enforcement mechanism for which tools are sent to the model.
- Gateway policy should be exposed as a Specialty `tool_gateway` tool so agents can inspect and validate toolset/delivery constraints from prompt, TUI, and daemon-owned sessions.
- `ToolEnv` should remain the wiring point for runtime capabilities; first-pass gateway validation must not create new long-lived runtime channels.

### Scheduling

- The existing `schedule` tool may continue to store `enabled_toolsets` metadata in payloads.
- First-pass gateway validation should validate `enabled_toolsets` values and expose unsupported delivery targets before a scheduled prompt is accepted for non-local delivery.
- Scheduled delivery remains local/session-only in this slice. Cross-platform scheduled delivery targets are explicit unsupported cases, not silent drops.

### Platform/media delivery

- Existing Matrix media ingress and `<sendfile>...</sendfile>` egress stay in `src/modes/matrix_bridge/*`.
- Generic platform delivery from CLI/agent is not implemented in the first pass beyond target validation and clear errors.
- The first backend identity should be `local`, with `matrix-existing-bridge` documented as a platform-specific bridge behavior rather than a general gateway backend.

## Unsupported first-pass cases

The first pass must return actionable unsupported errors for:

- unknown toolsets;
- empty toolset lists when validation expects a list;
- remote/platform delivery targets without a configured backend;
- Matrix delivery outside an active Matrix bridge session;
- HTTP/webhook delivery;
- cloud object storage delivery;
- direct credential/header based delivery from gateway APIs;
- delivery metadata containing raw prompt bodies, raw file contents, credentials, headers, or connection strings.

## Safe metadata shape

Use normalized replay/debug metadata with fields such as:

```json
{
  "source": "tool_gateway",
  "action": "validate",
  "status": "success|error|unsupported",
  "backend": "local",
  "toolsets": ["core", "specialty"],
  "delivery_target": "local",
  "supported": true,
  "error_kind": "unsupported_target",
  "error_message": "sanitized human-readable error"
}
```

Do not persist raw prompts, raw file contents, Matrix tokens, HTTP headers, credential env vars, or connection strings.

## Documentation targets

- `README.md` Built-in Tools should list `tool_gateway` under Specialty after the tool lands.
- `docs/src/reference/config.md` should state that Tool Gateway first-pass needs no new config and only validates local/session delivery plus toolset names.
