# Tool Gateway Session Metadata Boundary

## First-pass replay/debug metadata

`src/tools/tool_gateway.rs` returns `ToolResult::details` for every `status` and `validate` execution by serializing `tool_gateway::GatewayValidation` from `src/tool_gateway.rs`.

The metadata is intentionally safe and normalized:

- `source`: fixed `tool_gateway` marker.
- `action`: fixed `validate` action for status/validation summaries.
- `status`: `success` or `unsupported`.
- `backend`: `local` or `matrix-existing-bridge`.
- `toolsets`: normalized toolset labels only (`core`, `orchestration`, `specialty`, `matrix`).
- `delivery_target`: normalized target label only (`local`, `session`, `matrix`, or unsupported target kind such as `https`).
- `supported`: boolean capability result.
- `error_kind`: stable error class when unsupported.
- `error_message`: flattened and bounded human message.

## Deliberately excluded

The first-pass gateway does not persist or replay:

- webhook URLs or full remote target strings,
- authorization headers or credential material,
- raw platform payloads,
- Matrix message contents or room identifiers,
- cloud storage object URLs or access tokens.

Unsupported target parsing records only the target kind/prefix. For example, `https://token@example.test/hook` becomes `delivery_target = "https"` with a generic unsupported message. This keeps session replay useful for debugging capability boundaries without storing secrets or platform payloads.

## Verification

Focused tests in `src/tools/tool_gateway.rs` assert supported local metadata and unsupported remote metadata, including sanitized output that does not retain the input credential-like URL. `CARGO_TARGET_DIR=target cargo nextest run -p clankers tool_gateway --no-fail-fast` passed with 7 tests.
