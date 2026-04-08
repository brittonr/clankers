## Clankers Development

Rust terminal coding agent. Workspace with ~30 crates under `crates/`.

### Build & Test

```bash
cargo build                    # debug build
cargo build --release          # release build
cargo nextest run              # run tests (preferred over cargo test)
cargo clippy -- -D warnings    # lint
```

### Architecture

- `src/` — main binary crate (CLI, TUI, modes, commands)
- `crates/clankers-agent/` — agent loop, system prompt, tool dispatch
- `crates/clankers-config/` — settings, paths, keybindings
- `crates/clankers-controller/` — SessionController (transport-agnostic agent driver)
- `crates/clankers-protocol/` — daemon↔client wire protocol (DaemonEvent, SessionCommand, frames)
- `crates/clankers-provider/` — LLM provider abstraction
- `crates/clankers-tui/` — terminal UI (ratatui-based)
- `crates/clankers-session/` — JSONL session persistence
- `crates/clankers-model-selection/` — complexity routing, cost tracking
- `crates/clankers-hooks/` — event hooks (pre-commit, session start, etc.)
- `crates/clankers-matrix/` — Matrix bridge for multi-agent chat

**Extracted crates** (standalone repos, direct git deps):
- `graggle` — order-independent merge algorithm for worktrees
- `clanker-actor` — Erlang-style actor system (ProcessRegistry, signals, supervisors)
- `clanker-scheduler` — cron/interval/one-shot schedule engine
- `clanker-loop` — loop/retry engine with output truncation
- `clanker-router` — multi-provider routing, fallback, caching, OAuth, RPC

### Daemon-Client Architecture

The daemon runs agent sessions as actor processes. Clients attach via Unix sockets (local) or iroh QUIC (remote).

**Key components:**
- `src/modes/daemon/` — daemon startup, socket bridge, agent process actor
- `src/modes/daemon/agent_process.rs` — wraps SessionController as a named actor
- `src/modes/daemon/socket_bridge.rs` — Unix socket control plane + SessionFactory
- `src/modes/daemon/quic_bridge.rs` — iroh QUIC remote access (ALPN: `clankers/daemon/1`)
- `src/modes/attach.rs` — TUI client that connects to daemon sessions
- `crates/clankers-controller/src/lib.rs` — SessionController (owns Agent + SessionManager)
- `crates/clankers-controller/src/transport.rs` — DaemonState, session socket listener

**Protocol:** 4-byte big-endian length prefix + JSON over Unix sockets or QUIC streams. Handshake → SessionInfo → ReplayHistory → streaming events.

**Actor system:** ProcessRegistry manages named actors with Erlang-style links, monitors, and `die_when_link_dies` cascading. SubagentTool/DelegateTool spawn in-process AgentProcess actors in daemon mode (subprocess fallback in standalone).

**Commands:**
```bash
clankers daemon start -d       # start background daemon
clankers daemon status         # show daemon info
clankers daemon create         # create a session
clankers attach [session-id]   # attach TUI to session
clankers attach --auto-daemon  # auto-start daemon + attach
clankers attach --remote <id>  # attach to remote daemon via iroh
clankers ps                    # list sessions
clankers daemon kill <id>      # kill a session
clankers daemon stop           # stop daemon
```

### Conventions

- Tiger style: functional core, imperative shell. Pure functions where possible.
- Error handling: `snafu` for error types, context selectors.
- Tests live next to code (`_tests.rs` suffix or `#[cfg(test)]` modules).
- Config paths: `~/.clankers/agent/` (global), `.clankers/` (project).
- Pi fallback: reads `~/.pi/agent/` for auth/settings when clankers versions missing.
- Anthropic OAuth request shaping lives in `crates/clankers-provider/src/anthropic/{api.rs,subscription_compat.rs}`. The provider prepends a Claude Code billing-header system block and rewrites clankers markers by default; disable with `CLANKERS_DISABLE_CLAUDE_SUBSCRIPTION_COMPAT=1` or override the block contents with `CLANKERS_ANTHROPIC_BILLING_HEADER`.

### Reference Repos

- `/home/brittonr/git/claude-code/` — Extracted TypeScript source of Anthropic's Claude Code CLI (v2.1.88). Use as reference for tool design, agent loop patterns, TUI architecture, provider abstractions, and CLI UX. Key dirs: `src/tools/`, `src/commands/`, `src/screens/`, `src/services/`, `src/state/`.

### Key Files

- `crates/clankers-agent/src/system_prompt.rs` — prompt assembly
- `crates/clankers-config/src/paths.rs` — path resolution
- `crates/clankers-config/src/settings.rs` — settings schema
- `src/main.rs` — CLI entrypoint and mode dispatch
- `src/modes/daemon/agent_process.rs` — AgentProcess actor + run_ephemeral_agent
- `src/modes/daemon/socket_bridge.rs` — control socket, SessionFactory, drain_and_broadcast
- `clanker-actor` (external) — ProcessRegistry (spawn, link, shutdown)
- `crates/clankers-controller/src/lib.rs` — SessionController (handle_command, feed_event)
