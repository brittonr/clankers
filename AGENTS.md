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
- Subwayrat crates are Cargo path deps (`../subwayrat/...`) but also a separately pinned Nix flake input (`subwayrat-src`); when subwayrat adds crates or new transitive deps, update both `Cargo.lock` and `flake.lock` so sandboxed Nix builds see the same source. If subwayrat starts depending on new sibling path deps (for example `../ratcore` via `rat-inline`), mirror those in `flake.nix` `externalSources` too.
- Anthropic OAuth request shaping lives in `crates/clankers-provider/src/anthropic/{api.rs,subscription_compat.rs}`. The provider prepends a Claude Code billing-header system block and rewrites clankers markers by default; disable with `CLANKERS_DISABLE_CLAUDE_SUBSCRIPTION_COMPAT=1` or override the block contents with `CLANKERS_ANTHROPIC_BILLING_HEADER`.
- `crates/clankers-provider/src/{auth.rs,credential_manager.rs}` was originally Anthropic-only. For any new OAuth provider, thread the provider name through `CredentialManager` and use provider-scoped `AuthStore` helpers for reload/save/refresh fallback so refreshed tokens do not overwrite Anthropic slots.
- Pending OAuth login state now lives under `~/.clankers/agent/.login_verifiers/<provider>/<account>.json` with legacy fallback to `.login_verifier`; new auth flows should key verifier/state by provider+account, not one global file.
- `crates/clankers-provider::CompletionRequest` now carries `extra_params` to match `clanker-router::CompletionRequest`; when adding request builders, run `cargo check --tests` so helper/test constructors in `crates/clankers-provider/src/router.rs` and `crates/clankers-provider/src/anthropic/mod.rs` don’t silently miss the new field.
- Session-scoped provider metadata depends on `Agent.session_id`, not just `SessionController.session_id` or `App.session_id`. In daemon/controller-owned paths, call `agent.set_session_id(...)` when constructing or updating the controller or `_session_id` will be missing from routed requests. Slash/session-resume paths in the TUI also need to sync `SessionController::set_session_id(app.session_id.clone())` after the app swaps sessions.
- Keyed daemon sessions (`SessionKey::Matrix`, chat/1 iroh) recover through `get_or_create_keyed_session()`, not only attach-path `recover_session()`. If a key maps to a suspended placeholder, revive that exact session ID in place or Matrix/chat recovery will silently fork a new session and lose history.
- Review-sensitive `_session_id` work needs one runtime resume-path test, not just direct `run_turn_loop(...)` calls. `src/modes/event_loop_runner/key_handler.rs` now has a good pattern: resume persisted session via real helper, prompt through `RouterCompatAdapter`, assert captured router request keeps `_session_id`.
- For cross-crate request-shape drift, `crates/clankers-provider/src/lib.rs` now has good deterministic rails: (1) exact constructor-count inventory over router-bound `CompletionRequest {` sites, requiring `extra_params` in each snippet, and (2) provider-vs-router shared-field serde projection parity tests. Update those counts when adding real constructors.
- `crates/clankers-provider/src/openai_codex.rs` still owns local Codex auth/status helpers, but the extracted `clanker-router` now has the routed Codex backend. `build_router_with_rpc()` should not skip the daemon just because local `openai-codex` auth exists anymore.
- Codex discovery must use the same auth store that supplied the credential. If `resolve_provider_credential_with_fallback()` found `openai-codex` only in `~/.pi`, loading just the primary auth store will suppress the catalog by probing the wrong account or no account at all.
- Service auth path overrides: `CLANKERS_AUTH_FILE` points at a single auth.json, while `CLANKERS_AUTH_SEED_FILE` + `CLANKERS_AUTH_RUNTIME_FILE` cause `crates/clankers-config/src/paths.rs` to materialize the merged effective auth store into the runtime file at process start. Existing call sites still read `paths.global_auth`, so daemon modules should point that env pair at the managed router seed/runtime locations.
- `RouterProvider` now keeps a fail-closed sentinel for explicit `openai-codex/...` prefixes when the backend is absent. Unknown random prefixes still fall back to default; known-but-unavailable Codex prefixes must error instead of silently routing to Anthropic.
- Mixed plugin kinds: `PluginManifest` now validates `kind: stdio` launch policy at discovery time, but only `PluginKind::Extism` should flow through `load_wasm()` / `init_plugin_manager()`'s eager WASM load loop. If non-Extism kinds hit that path, they degrade into bogus missing-WASM errors instead of staying ready for their own runtime.
- Plugin runtime queries should go through `clankers_plugin::PluginHostFacade` when possible. It is the seam for active-plugin filtering, event subscriptions, runtime summaries, and future stdio/runtime mixing; `PluginManager` remains the low-level Extism store.

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
- `crate-hashes.json` — unit2nix git source hashes for first-party extracted crates; stale entries fail `nix build .#clankers` with fixed-output hash mismatch before workspace code even builds

### Orchestration Notes

- Daemon mode gives top-level `subagent`/`delegate_task` calls in-process actor spawning, but child factories intentionally set `registry: None` and `plugin_manager: None` in `src/modes/daemon/socket_bridge.rs`, so recursive children fall back to subprocesses and do not load plugins.
- `delegate_task` is not process-persistent today: `DelegateTool` stores `WorkerState` metadata, but each call still spawns a fresh local subprocess, ephemeral actor, or remote `prompt` RPC.
- Orchestration docs drift: `README.md` and `ToolTier` comments mention `loop` and `switch_model`, but `src/modes/common.rs` does not register either tool. Actual loop control is `/loop` plus controller state; only `signal_loop_success` is wired as an agent tool.
- Context compaction is split-brain today. Real auto-compaction runs in `crates/clankers-agent/src/lib.rs::handle_auto_compaction()` before each turn at 80% estimated context, keeping the first message plus last 10 and calling `compact_with_llm()` with truncation fallback. Standalone `/compact` / `AgentCommand::CompressContext` is still a stub in `src/modes/agent_task.rs`, while controller-side `compact` only rewrites stale tool results via `compact_messages()`.
