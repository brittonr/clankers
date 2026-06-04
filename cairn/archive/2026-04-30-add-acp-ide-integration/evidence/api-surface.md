# ACP IDE Integration API Surface

## User-facing surface

The first pass exposes ACP as an explicit CLI server mode:

```text
clankers acp serve [--session <id>] [--new] [--cwd <dir>] [--model <model>]
```

Semantics:

- `clankers acp serve` runs a foreground stdio ACP adapter intended to be launched by an ACP-compatible editor.
- The adapter owns ACP JSON-RPC/framing at the edge and maps supported ACP requests onto existing clankers session/controller primitives.
- `--session <id>` resumes a known clankers session when supported by the underlying controller/session store.
- `--new` forces a new session.
- Global clankers options such as `--cwd`, `--model`, `--provider`, `--account`, `--tools`, `--skill`, and capability/policy settings remain the source of truth for agent behavior.
- The command should fail with an actionable error if stdin/stdout cannot be used safely as the ACP transport.

No separate built-in LLM tool is introduced for ACP. ACP is an external host/editor transport for clankers sessions, not something the model calls from inside a turn.

## Adapter API boundary

The first implementation should keep ACP-specific types at the edge:

- `src/modes/acp.rs` or `src/modes/acp_server.rs` owns ACP request/response structs, framing, and translation.
- The adapter maps incoming prompt/new-turn requests to existing `SessionCommand::Prompt` behavior.
- The adapter maps existing controller/daemon output into ACP notifications or response payloads.
- Unsupported ACP methods return structured JSON-RPC/ACP errors with stable error codes/messages.
- Session metadata is recorded as `SessionEntry::Custom` with `kind = "acp_ide_integration"` when a session store is present.

## Supported first-pass cases

The first pass should support only the minimal useful editor integration:

- Start one foreground stdio ACP session from `clankers acp serve`.
- Accept a single-session prompt/new-turn style request and dispatch it through existing clankers controller/session paths.
- Stream or return normalized assistant/tool progress using existing event translation where practical.
- Include safe metadata: source, adapter protocol version when known, session id, cwd, method name, status, elapsed timing, and redacted error details.

## Explicit unsupported cases

The first pass should return explicit unsupported errors for:

- arbitrary IDE terminal creation/management;
- editor-native diff application or patch review UI;
- multiple concurrent workspaces/sessions on one adapter process;
- remote/network ACP transports beyond foreground stdio;
- editor-managed file synchronization beyond clankers' existing cwd and tool policy;
- provider credential exchange through ACP;
- binary/media streaming that does not map to existing protocol content blocks;
- cancellation semantics not yet backed by the controller/event loop.

## Config surface

Do not add durable `Settings` fields until implementation proves they are needed. The initial surface can be CLI-only because editor launchers can pass the command and global clankers options directly.

If later needed, configuration should live in `crates/clankers-config/src/settings.rs` under an `acp` section with defaults that keep ACP disabled unless explicitly launched.

## Tests implied by this surface

- CLI parsing exposes `clankers acp serve` and rejects conflicting `--session`/`--new` combinations if they cannot both apply.
- Adapter method dispatch returns a successful response for a mock prompt path.
- Unsupported ACP methods return actionable structured errors.
- Metadata recording redacts errors and stores normalized fields only.
