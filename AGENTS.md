## Clankers Development

Rust terminal coding agent. Workspace with ~30 crates under `crates/`.

### Build & Test

```bash
cargo build                    # debug build
cargo build --release          # release build
cargo nextest run              # run tests (preferred over cargo test)
cargo clippy -- -D warnings    # lint
./scripts/verify.sh            # repo validation rails (verus/tracey + no_std core bundle)
./xtask/tigerstyle.sh          # tigerstyle dylint run (pulls pinned tigerstyle-rs over SSH)
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
- Private SSH git dependencies need Cargo CLI fetches here. Keep `.cargo/config.toml` `[net] git-fetch-with-cli = true` or cargo/libgit2 will fail to authenticate even when plain `git` over SSH works.
- Tigerstyle uses the first-class `cargo-tigerstyle` runner now, not `cargo dylint` directly. Root `Cargo.toml` must keep `[workspace.metadata.tigerstyle]` (`default_scope`, `cargo_check_args`) and `./xtask/tigerstyle.sh` bootstraps a compatible dylint driver plus the `libtigerstyle@<toolchain>.so` path for the runner.
- Anthropic OAuth request shaping lives in `crates/clankers-provider/src/anthropic/{api.rs,subscription_compat.rs}`. The provider prepends a Claude Code billing-header system block and rewrites clankers markers by default; disable with `CLANKERS_DISABLE_CLAUDE_SUBSCRIPTION_COMPAT=1` or override the block contents with `CLANKERS_ANTHROPIC_BILLING_HEADER`.
- `crates/clankers-provider/src/{auth.rs,credential_manager.rs}` was originally Anthropic-only. For any new OAuth provider, thread the provider name through `CredentialManager` and use provider-scoped `AuthStore` helpers for reload/save/refresh fallback so refreshed tokens do not overwrite Anthropic slots.
- Pending OAuth login state now lives under `~/.clankers/agent/.login_verifiers/<provider>/<account>.json` with legacy fallback to `.login_verifier`; new auth flows should key verifier/state by provider+account, not one global file.
- `crates/clankers-provider::CompletionRequest` now carries `extra_params` to match `clanker-router::CompletionRequest`; when adding request builders, run `cargo check --tests` so helper/test constructors in `crates/clankers-provider/src/router.rs` and `crates/clankers-provider/src/anthropic/mod.rs` don’t silently miss the new field.
- Session-scoped provider metadata depends on `Agent.session_id`, not just `SessionController.session_id` or `App.session_id`. In daemon/controller-owned paths, call `agent.set_session_id(...)` when constructing or updating the controller or `_session_id` will be missing from routed requests. Slash/session-resume paths in the TUI also need to sync `SessionController::set_session_id(app.session_id.clone())` after the app swaps sessions.
- Embedded prompt correlation now seeds from `AgentEvent::BeforeAgentStart` inside `crates/clankers-controller/src/event_processing.rs` when no pending prompt slot exists. If you touch embedded prompt lifecycle, keep `finish_embedded_prompt()` consuming reducer-owned `pending_prompt` state instead of clearing busy/loop state from ambient TUI state.
- Keyed daemon sessions (`SessionKey::Matrix`, chat/1 iroh) recover through `get_or_create_keyed_session()`, not only attach-path `recover_session()`. If a key maps to a suspended placeholder, revive that exact session ID in place or Matrix/chat recovery will silently fork a new session and lose history.
- Review-sensitive `_session_id` work needs one runtime resume-path test, not just direct `run_turn_loop(...)` calls. `src/modes/event_loop_runner/key_handler.rs` now has a good pattern: resume persisted session via real helper, prompt through `RouterCompatAdapter`, assert captured router request keeps `_session_id`.
- For cross-crate request-shape drift, `crates/clankers-provider/src/lib.rs` now has good deterministic rails: (1) exact constructor-count inventory over router-bound `CompletionRequest {` sites, requiring `extra_params` in each snippet, and (2) provider-vs-router shared-field serde projection parity tests. Update those counts when adding real constructors.
- `crates/clankers-provider/src/openai_codex.rs` still owns local Codex auth/status helpers, but the extracted `clanker-router` now has the routed Codex backend. `build_router_with_rpc()` should not skip the daemon just because local `openai-codex` auth exists anymore.
- When wiring a `clanker-router` backend directly inside `crates/clankers-provider/src/discovery.rs`, use `clanker_router::credential::CredentialManager` plus `clanker_router::auth::AuthStorePaths`; the local `clankers-provider::credential_manager::CredentialManager` is a different type and will not satisfy routed backend constructors.
- `crates/clankers-provider/src/discovery.rs` tests that call `build_router()` and then `complete()` should usually set `CLANKERS_NO_DAEMON=1`; otherwise they may open the shared `~/.clankers/agent/cache.db` and inherit router cooldown state from earlier failures, producing misleading `skipped in cooldown` errors.
- The local `crates/clankers-provider::openai_codex::with_test_probe_hook(...)` only affects the legacy/local helper module, not the routed `clanker-router` Codex backend inside the git dependency. To mirror routed backend auth/probe failures in current-repo tests, use deterministic public inputs (for example invalid JWTs) or fake `clanker_router::Provider` implementations behind `RouterCompatAdapter`.
- Codex discovery must use the same auth store that supplied the credential. If `resolve_provider_credential_with_fallback()` found `openai-codex` only in `~/.pi`, loading just the primary auth store will suppress the catalog by probing the wrong account or no account at all.
- Auth docs/help drift risk: clap auth commands use `--provider ...`, not positional provider args. When auth UX changes, update README/docs/slash help together and keep an acceptance test (for example `tests/openai_codex_help_docs.rs`) so examples stay aligned with `src/cli.rs`.
- For request-contract tests against extracted backends, do not build the expected JSON by calling the same body-builder under test. Pin one explicit literal fixture with representative history/reasoning/tool replay, then compare initial/retry/refresh requests against that literal and cover overrides in a separate test.
- For SSE normalization claims in extracted backends, helper-level state-machine tests are not enough. Add one runtime seam test that feeds raw `text/event-stream` bytes through the real parser entrypoint (for example `parse_codex_sse(...)`) via a tiny local `TcpListener` server, then assert normalized stream events there too.
- For routed Codex tests that need a valid OAuth token shape but no live credential, generate fake JWT payloads with a base64url helper instead of copying opaque token literals. A malformed fixture token can redirect the test into JWT/auth parse failures or 401s and hide the entitlement path you meant to exercise.
- OpenAI Codex backend tests share one global entitlement cache plus test-only URL/sleep overrides. Serialize those tests on one shared mutex and use RAII cleanup guards that clear overrides on panic, or parallel runs will poison locks / leak fake endpoints into unrelated assertions.
- Sanitized live Codex smoke against a real ChatGPT account found contract drift versus the frozen fixtures: `gpt-5.1/5.2` ChatGPT-account Codex models returned HTTP 400 unsupported-model, while `gpt-5.3-codex` and `gpt-5.3-codex-spark` accepted requests only when `stream=true`. Do one real-account probe before declaring Codex transport/model contracts stable.
- `RouterCompatAdapter` cannot forward `AgentMessage` via plain `serde_json::to_value(...)` for routed backends. Extracted providers expect provider-native `{role, content}` message JSON, so reuse the RPC-style content/message conversion or live routed backends can send empty Codex/OpenAI inputs even when clankers had a prompt.
- Do not satisfy extracted-crate pin tasks with host-local overrides like `../clanker-router` or absolute-path flake inputs. If the needed extracted commit is not on the remote yet, vendor a snapshot inside this repo (and record the source commit) or use a real remote git pin.
- Service auth path overrides: `CLANKERS_AUTH_FILE` points at a single auth.json, while `CLANKERS_AUTH_SEED_FILE` + `CLANKERS_AUTH_RUNTIME_FILE` cause `crates/clankers-config/src/paths.rs` to materialize the merged effective auth store into the runtime file at process start. Existing call sites still read `paths.global_auth`, so daemon modules should point that env pair at the managed router seed/runtime locations.
- `RouterProvider` now keeps a fail-closed sentinel for explicit `openai-codex/...` prefixes when the backend is absent. Unknown random prefixes still fall back to default; known-but-unavailable Codex prefixes must error instead of silently routing to Anthropic.
- Mixed plugin kinds: `PluginManifest` now validates `kind: stdio` launch policy at discovery time, but only `PluginKind::Extism` should flow through `load_wasm()` / `init_plugin_manager()`'s eager WASM load loop. If non-Extism kinds hit that path, they degrade into bogus missing-WASM errors instead of staying ready for their own runtime.
- Plugin runtime queries should go through `clankers_plugin::PluginHostFacade` when possible. It is the seam for active-plugin filtering, event subscriptions, runtime summaries, and future stdio/runtime mixing; `PluginManager` remains the low-level Extism store.
- `init_plugin_manager_for_mode(...)` configures stdio startup mode (`standalone` vs `daemon`) and launches stdio plugins only when a Tokio runtime is present. Sync tests/commands that call plain `init_plugin_manager(...)` still discover stdio manifests but intentionally leave them `Loaded` instead of forcing a runtime-start error.
- Stdio enable/reload flows need Arc-aware wrappers (`clankers_plugin::enable_plugin`, `reload_plugin`, `reload_all_plugins`). Calling `PluginManager::enable()`/`reload()` while holding the mutex cannot spawn a stdio supervisor without deadlocking on the same `Arc<Mutex<PluginManager>>`; the wrapper must drop the guard before `start_stdio_plugin(...)`.
- Stdio live tool collision checks depend on `PluginManager::set_stdio_reserved_tool_names(...)` during plugin-manager init. Seed that set from the real built-in tool list or stdio plugins can silently steal built-in names until tool construction time.
- `PluginTool` now fronts both WASM and stdio runtimes. Stdio calls go through `start_stdio_tool_call` / `cancel_stdio_tool_call` / `abandon_stdio_tool_call`; keep cancellation semantics in the tool adapter so turn cancellation always surfaces as cancelled even if the plugin later replies normally.
- Daemon sessions need periodic tool-list sync against the shared plugin host, not just `SetDisabledTools` rebuilds. Live stdio registrations/restarts happen outside command handling, so `run_agent_actor` should refresh tools from the rebuilder on its tick and emit a fresh `DaemonEvent::ToolList` when inventory changes.
- Stdio plugin `ui` / `display` frames are asynchronous runtime output, not synchronous `dispatch_event_to_plugins(...)` return values. Queue them in the shared plugin manager, then drain them on the standalone event-loop tick and daemon actor tick into the existing `PluginWidget` / `PluginStatus` / `PluginNotify` / `SystemMessage` flows.
- Stdio launch policy now applies at spawn time for `inherit` mode: resolve `stdio.command` to an absolute path before launch, `env_clear()` the child, and pass only manifest-allowlisted environment variables. Current host-runtime exception set is effectively empty because command lookup happens in the parent, not via child `PATH`.
- Restricted stdio writable roots should stay project-root-relative. Reject absolute paths / `..` escapes in runtime policy resolution so a manifest cannot smuggle extra host write access through `writable_roots`.
- When deriving stdio plugin state dirs from the plugin root, `.../plugins` should map to sibling `.../plugin-state`, but ad-hoc test roots that directly contain plugin dirs should use `<root>/plugin-state` instead of leaking into the parent temp directory.
- `clanker-plugin-sdk::dispatch_events(...)` returns `{handled,message}` without `display: true`; `src/modes/plugin_dispatch.rs` only surfaces Extism event messages when `display` is set. Mixed-runtime tests should verify Extism event behavior via direct `on_event` calls or explicit display-aware output, not by assuming `dispatch_event_to_plugins(...)` will show SDK default messages.
- `schedule_fire` is special: `plugins/clankers-email` returns `{handled,message}` JSON without `display: true`, and daemon schedule handling needs that `message` surfaced anyway for logs/tests. `src/modes/daemon::handle_schedule_event()` is the stable seam; do not make the daemon schedule test depend on Fastmail search indexing when direct plugin dispatch or other live tests already cover delivery.
- Linux `restricted` stdio sandbox now uses Landlock for bounded writable roots plus a seccomp socket-creation deny filter when effective network access is false. If either restriction cannot be applied, fail closed and leave the plugin in `Error`.
- When archiving OpenSpec changes with `MODIFIED` delta specs, do not wholesale-copy the delta into `openspec/specs/`. Merge the modified sections onto the existing main spec or unrelated baseline requirements/scenarios will be silently dropped.
- Current OpenSpec tasks gate supports typed traceability. Once a change adds `ID:` lines in specs, use dotted lowercase requirement/scenario IDs, typed task IDs (`I#`, `V#`, `H#`, `R#`), `[covers=...]` tags on typed tasks, and `[evidence=...]` on every `V#`/`H#` task. Evidence paths must exist, and `H#` evidence must be `Artifact-Type: oracle-checkpoint` with the required labeled sections.
- During the 2026-04-22 archive of `no-std-functional-core`, upstream `openspec validate` / `openspec archive` misparsed typed-ID delta specs: `openspec validate no-std-functional-core` kept emitting `ADDED ... must contain SHALL or MUST`, while `openspec change show no-std-functional-core --json --deltas-only` showed each requirement body parsed as `text: "ID: ..."`. If archive blocks only on that exact signature and repo-specific tasks gates plus validation evidence are already green, confirm with `openspec change show <change> --json --deltas-only` and use `openspec archive --no-validate <change>` only as a last resort.
- `openspec archive` moves change directories but does not rewrite archived `[evidence=...]` links inside `tasks.md`. If you need post-archive task auditability, retarget those evidence paths to `openspec/changes/archive/<date>-<change>/...` or add an archive-local task-audit note.
- This environment does not ship prebuilt `thumbv7em-none-eabi` std artifacts. For `no_std` bare-metal rails here, `cargo check -Zbuild-std=core,alloc --target thumbv7em-none-eabi` works; plain `cargo check --target thumbv7em-none-eabi` fails with missing `core`.
- Do not rely on returning `Err(...)` from stdio-plugin `pre_exec` hooks for diagnostics. Tokio/std process spawn can collapse that into generic `Invalid argument (os error 22)`. For reviewable runtime errors, write the sandbox/bootstrap failure to child stderr and `_exit(126)` so supervision captures the real cause.
- `tests/scheduled_email_live.rs` mutates process env with unsafe `set_var` and hits one shared Fastmail mailbox. Serialize the live-email tests with a global async mutex or full `cargo nextest run` can flake on env races / mailbox indexing timing.
- For stdio supervisor loops, do not `tokio::select! { biased; event = rx.recv() => ... child.wait() => ... }` with a bare `rx.recv()`. Once the sender side closes, `recv()` returns `None` immediately forever and starves `child.wait()`. Use `Some(event) = rx.recv()` or otherwise disable the branch after closure.
- Stdio supervisor teardown/restart needs per-run IDs. If you remove a supervisor handle before the old task exits, the old task can otherwise remove/clear a newly started replacement and clobber its state on disable→enable or reload races.

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
- `crate-hashes.json` — unit2nix git source hashes for first-party extracted crates; stale entries fail `nix build .#clankers` with fixed-output hash mismatch before workspace code even builds. For extracted-crate pin bumps, rerun `nix build .#clankers -L` and use the reported `got:` hash to refresh `crate-hashes.json` before chasing Rust errors.

### Orchestration Notes

- Daemon mode gives top-level `subagent`/`delegate_task` calls in-process actor spawning, but child factories intentionally set `registry: None` and `plugin_manager: None` in `src/modes/daemon/socket_bridge.rs`, so recursive children fall back to subprocesses and do not load plugins.
- `delegate_task` is not process-persistent today: `DelegateTool` stores `WorkerState` metadata, but each call still spawns a fresh local subprocess, ephemeral actor, or remote `prompt` RPC.
- Orchestration docs drift: `README.md` and `ToolTier` comments mention `loop` and `switch_model`, but `src/modes/common.rs` does not register either tool. Actual loop control is `/loop` plus controller state; only `signal_loop_success` is wired as an agent tool.
- Context compaction is split-brain today. Real auto-compaction runs in `crates/clankers-agent/src/lib.rs::handle_auto_compaction()` before each turn at 80% estimated context, keeping the first message plus last 10 and calling `compact_with_llm()` with truncation fallback. Standalone `/compact` / `AgentCommand::CompressContext` is still a stub in `src/modes/agent_task.rs`, while controller-side `compact` only rewrites stale tool results via `compact_messages()`.
- Attach slash parity now hinges on `src/modes/attach.rs::route_attach_slash(...)` plus the AgentCommand→SessionCommand bridge. Do not assume standalone slash names match daemon `handle_slash_command_sync()`; `/think` vs daemon-side `thinking` already drifted once.
- Local attach parity is two-part: bridge the standalone action to daemon state sync, then suppress daemon follow-up acks that would otherwise add extra UI noise (`Thinking...`, `Disabled tools updated: ...`, manual `SessionCompaction`). `src/modes/attach_remote.rs` must thread the same `AttachParityTracker` as local attach or QUIC attach regresses separately.
- Attach-side `/think` bridging has two distinct paths: explicit level → `SessionCommand::SetThinkingLevel`, no-arg cycle → `SessionCommand::CycleThinkingLevel`. Both apply the standalone-local thinking update first and both currently get a non-error controller `DaemonEvent::SystemMessage` starting with `Thinking`, not `DaemonEvent::ThinkingLevelChanged`. Keep suppression matcher narrow and test both paths.
- Disabled-tools parity should follow the same rule as thinking parity: apply the local attached-state update before budgeting suppression of the daemon `Disabled tools updated: ...` ack, even if the slash handler already mutated `app.disabled_tools` upstream.
- Remote attach reconnects must reset `AttachParityTracker` before draining new daemon events or stale suppression budgets can hide the first post-reconnect ack. Keep one deterministic `attach_remote.rs` regression test on that seam.
- Attach help/output must stay in lockstep with `route_attach_slash(...)`: if the list is abbreviated, say "include" / "generally forward" instead of presenting it as exhaustive. Keep no-arg `model`/`role` locals and special `plugin` fetch handling covered by deterministic tests.
- Root `cargo test --lib` currently hits a mold linker bug on this machine (`anon.caf19289...` unresolved inside `libclankers_tui`). A working local override is `CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib ...`; plain default-linker runs can fail even when `cargo check --tests` succeeds.
- `tests/tui/visual.rs::snapshot_small_terminal` is stable only after the extracted structure settles and the eight-cell Todo empty-state row is normalized. Do not treat raw row-1 text (`Noankers`, `Noonr/.c`, etc.) as a layout contract; keep that stabilization seam when refreshing `small_12x50_structure`.
- `crates/clankers-agent` does not depend on `clankers-core` at runtime. Keep `CoreThinkingLevel`/other core-type translation in `clankers-controller`, and use an explicit `clankers-core` `[dev-dependencies]` entry only for agent-side tests that exercise shared core contracts.
