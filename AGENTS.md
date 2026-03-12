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
- `crates/clankers-provider/` — LLM provider abstraction
- `crates/clankers-router/` — multi-provider routing, fallback, caching
- `crates/clankers-tui/` — terminal UI (ratatui-based)
- `crates/clankers-session/` — JSONL session persistence
- `crates/clankers-model-selection/` — complexity routing, cost tracking
- `crates/clankers-hooks/` — event hooks (pre-commit, session start, etc.)
- `crates/clankers-merge/` — graggle merge algorithm for worktrees
- `crates/clankers-loop/` — loop/retry engine
- `crates/clankers-matrix/` — Matrix bridge for multi-agent chat

### Conventions

- Tiger style: functional core, imperative shell. Pure functions where possible.
- Error handling: `snafu` for error types, context selectors.
- Tests live next to code (`_tests.rs` suffix or `#[cfg(test)]` modules).
- Config paths: `~/.clankers/agent/` (global), `.clankers/` (project).
- Pi fallback: reads `~/.pi/agent/` for auth/settings when clankers versions missing.

### Key Files

- `crates/clankers-agent/src/system_prompt.rs` — prompt assembly
- `crates/clankers-config/src/paths.rs` — path resolution
- `crates/clankers-config/src/settings.rs` — settings schema
- `src/main.rs` — CLI entrypoint and mode dispatch
