## Why

Clankers already has a daemon/session protocol, an attachable TUI, confirmation events, session replay, and an MCP client/tool-publication surface. The next step is to make clankers controllable by external MCP clients over the same user-facing substrate as the TUI, then use that control plane as the safe outer loop for eval-driven self-evolution.

The key product constraint is parity: MCP must not become a hidden automation backdoor. If a human can submit a prompt, approve a dangerous command, interrupt a turn, change thinking level, or observe tool activity through the TUI, an MCP client should be able to request the same operation only by emitting the same session command and receiving the same daemon event stream.

## What Changes

- **MCP session control plane**: Add a local MCP server/bridge that exposes selected session-control tools and resources for clankers sessions.
- **User-substrate parity**: Route MCP operations through `SessionCommand`, daemon attach/control sockets, confirmation handling, session persistence, and normal daemon events rather than through TUI internals or private controller APIs.
- **Receipts and observability**: Return structured receipts for MCP mutations and expose session/event resources that are safe for audit and replay.
- **Self-evolution outer loop**: Add a disabled-by-default self-evolution workflow that drives clankers through the MCP session control plane, evaluates candidates in isolated workdirs/branches, and requires human approval before promotion.

## Capabilities

### New Capabilities

- `mcp-session-control-plane`: External MCP clients can observe and steer clankers sessions through the normal daemon/session substrate.
- `self-evolution-control`: A self-evolver can run baseline-vs-candidate experiments using clankers as the shell while preserving user-visible confirmations, logs, and promotion gates.

### Modified Capabilities

- `integrations-mcp`: MCP is extended beyond consuming external MCP server tools; clankers also publishes a constrained local MCP bridge for session control.
- `daemon-session-control`: Session commands/events become the explicit authority boundary for human, TUI, MCP, and future clients.

## In Scope

- A local stdio MCP bridge such as `clankers mcp serve` or an equivalent subcommand.
- Initial MCP tools: send prompt, interrupt/abort, set thinking level, set disabled tools/capabilities, approve/deny pending confirmations, compact history, query session status, and fetch recent events/history.
- Resources/prompts for session status, tool activity, confirmation queue, and self-evolution run summaries.
- Tests proving MCP and TUI/attach paths produce equivalent session commands and observable events.
- Self-evolution planning/execution interfaces that write candidates to isolated output/worktree locations and report metrics/receipts.

## Out of Scope

- Direct mutation of TUI `App` state from MCP.
- Raw PTY input injection, screen scraping, or private calls into `SessionController` that bypass daemon/session commands.
- Live mutation of installed skills, prompts, tools, or code during an active self-evolution run.
- Network-exposed MCP without an explicit later transport/auth policy.
- Automatic merge/install/promotion of self-evolved candidates without human approval.

## Impact

- **Files likely affected**: `src/cli.rs`, `src/main.rs`, `src/commands/`, `src/modes/`, `crates/clankers-protocol`, daemon attach/control code, tests, README/docs.
- **APIs**: May add session-command metadata/receipt helpers and a local MCP bridge API, but authority remains the existing session protocol.
- **Dependencies**: May add or reuse an MCP protocol crate; avoid coupling MCP to TUI internals.
- **Testing**: Protocol/unit tests for command mapping, integration tests with fake daemon/session sockets, parity tests against attach slash/user paths, self-evolution dry-run tests with fake evaluator/executor, and docs checks.
