# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|
| 2026-04-08 | self | Ran `cargo fmt --all` for a small provider change and it reformatted a huge swath of the workspace | In this repo, use `rustfmt` on the touched files only. If `cargo fmt --all` slips through, immediately revert unrelated formatting before doing anything else. |
| 2026-04-08 | self | `cargo test`/`clippy` suddenly failed with `No space left on device` even though `/` had space | Check `/tmp` too, not just `/`. This machine can fill tmpfs with old VM/images and large temp dirs; clear `/tmp` before assuming the Rust change broke the build. |
| 2026-04-08 | self | I added a helper-only inbound rewrite path and missed that the runtime SSE path still forwarded `ContentBlockStart::ToolUse` unchanged | When changing stream rewriters, add at least one test at the real seam (`parse_sse_stream(..., reverse_map = true)`), not just helper/unit tests. |
| 2026-04-08 | self | Review evidence was weaker than the actual work because I bundled/parallelized validation and the transcript did not clearly show the exact command | For reviewer-sensitive claims, rerun the exact command with `set -x` in a dedicated tool call so the transcript proves what ran. |
| 2026-04-11 | self | Updated subwayrat pin and stopped after Cargo/test fixes; Nix still failed because unit2nix also needed fresh `crate-hashes.json` entries and `flake.nix` externalSources for subwayrat's new `../ratcore` sibling dep | After path-dep repo bumps, validate both `cargo ...` and `nix build .#clankers`. If Nix fails before build, check `crate-hashes.json` fixed-output hashes and sibling path deps mirrored in `externalSources`. |
| 2026-04-18 | self | Bumped extracted `clanker-router` rev and tried prefetch helpers first; `nurl`/flake hashes were not enough for unit2nix's fixed-output crate hash | After extracted-crate rev bumps, run `nix build .#clankers -L` early and copy the exact `got:` hash from the fixed-output mismatch into `crate-hashes.json`; that is the authoritative unit2nix hash. |
| 2026-04-18 | self | Tried to pass `clankers-provider::credential_manager::CredentialManager` into `clanker-router`'s `OpenAICodexProvider::new`; same name, different type | When discovery wires a routed backend directly, build `clanker_router::credential::CredentialManager` with `clanker_router::auth::AuthStorePaths` and the backend refresh fn instead of reusing the local manager type. |
| 2026-04-18 | self | New `build_router()` completion tests started failing with `skipped in cooldown` instead of the real backend error because they opened shared `~/.clankers/agent/cache.db` and inherited router cooldown state | For discovery tests that call `build_router()` and `complete()`, set `CLANKERS_NO_DAEMON=1` under an env guard so tests skip the shared cache DB and stay deterministic. |
| 2026-04-18 | self | Tried to drive routed Codex not-entitled/probe-failure behavior from current-repo discovery tests with `crates/clankers-provider::openai_codex::with_test_probe_hook`; it only affects the local helper module, not the git dependency backend | To mirror routed backend failures in current-repo tests, either use deterministic public inputs like invalid JWTs or wrap a fake `clanker_router::Provider` in `RouterCompatAdapter` and assert current-repo error shaping there. |
| 2026-04-18 | self | Trusted README/docs auth examples and missed that clap only supports `clankers auth login --provider ...`, not positional `clankers auth login openai` syntax shown in docs | When auth UX changes, compare examples against `src/cli.rs` clap shapes and add a docs/help acceptance test so positional-vs-flag drift gets caught. |
| 2026-04-18 | self | Wrote Codex request contract test that derived `expected_body` by calling `build_codex_request_body(...)`, which only proved function equals itself | For wire-contract fixtures, pin one explicit literal JSON fixture with representative history/assistant/tool/reasoning replay and compare built requests against that literal, then add a separate override test for mutable fields like verbosity. |
| 2026-04-18 | self | Helper-level Codex SSE tests covered state transitions but still missed real parser-entrypoint assurance | For stream normalization claims, pair unit fixtures over `handle_event(...)` with one raw-SSE runtime seam test that feeds `parse_codex_sse(...)` through a tiny local `TcpListener` server returning `text/event-stream`. |
| 2026-04-18 | self | Discovery tests used a copied fake JWT literal that looked plausible but decoded to invalid JSON, so routed Codex tests failed with a misleading 401/auth-parse path | In tests, generate fake JWT payloads with a base64url helper instead of copying opaque literals. Bad fixture tokens can silently turn entitlement-path tests into auth-parse tests. |
| 2026-04-18 | self | Added separate Codex probe/retry HTTP hooks with their own test lock and hit cross-test entitlement cache races/poisoned locks | For backend tests that share one entitlement cache or test-only URL/sleep hooks, serialize all of them on the same global mutex and use cleanup guards that reset overrides on panic. |
| 2026-04-18 | self | Manual live Codex smoke with a real ChatGPT account showed the frozen probe contract drifted: `gpt-5.1/5.2` ChatGPT-account models returned HTTP 400 unsupported-model, while `gpt-5.3-codex` / `gpt-5.3-codex-spark` only succeeded when `stream=true` | Before calling Codex support ready, run one sanitized live probe against a real account. Private/reference fixtures can drift; current ChatGPT-account path appears to require `stream=true` probes and newer `gpt-5.3-*` model IDs. |
| 2026-04-18 | self | Live Codex smoke still failed after fixing the probe because `RouterCompatAdapter` serialized `AgentMessage` enums directly, so extracted backends saw non-native message JSON and sent empty `input` payloads | For routed backends, convert `AgentMessage` to provider-native `{role, content}` JSON before building `clanker_router::CompletionRequest`. Do not rely on plain `serde_json::to_value(AgentMessage)` at the adapter boundary. |
| 2026-04-18 | self | Tried to satisfy router repin by pointing Cargo/Nix at `../clanker-router` and `/home/.../clanker-router`; review correctly rejected it as non-reproducible | For extracted-crate updates, use a real remote git rev or vendor the snapshot inside repo with recorded source commit. Never leave machine-local path overrides as final pin state. |
| 2026-04-14 | self | Used `openspec status --change <new-name>` right after `openspec new change` and CLI claimed the new change did not exist | For fresh changes, use `openspec list`, `openspec instructions ... --change <name>`, or `openspec validate <name>` to confirm scaffolding before assuming creation failed. |
| 2026-04-14 | self | Took orchestration docs/comments at face value and assumed `loop`/`switch_model` were agent tools | Verify `src/modes/common.rs` actual tool registration before describing orchestration surface; README/comments currently overstate it. |
| 2026-04-17 | self | Took compaction docs/help text at face value and assumed `/compact` triggers real summarization | Verify live path before describing compaction: standalone `/compact` / `AgentCommand::CompressContext` is stubbed, controller `compact` only compacts stale tool results, real auto-compaction lives in `Agent::handle_auto_compaction()`. |
| 2026-04-17 | self | Reused a fixed temp-path helper binary for stdio runtime tests; after a failed `rustc` rebuild, later tests reused stale old binary because source matched but binary still existed | For self-compiled test helpers, key the temp build dir/binary path by source hash or otherwise invalidate stale binaries after failed rebuilds. Fixed-path caches can hide new behaviors. |
| 2026-04-17 | self | Deadlocked a plugin discovery test by holding `PluginManager` mutex guard while calling `build_protocol_plugin_summaries(&manager)`, which locks same mutex again | Drop plugin-manager guards before calling facade/summary helpers that take `Arc<Mutex<PluginManager>>`. Mixed direct-lock + helper-lock paths can self-deadlock in tests. |
| 2026-04-17 | self | Deadlocked env-var tests by taking two `EnvVarGuard::set(...)` locks in one scope; the guard serializes env mutation with one global mutex and is not reentrant | Use one env-var guard per test scope, or replace it with a multi-var guard if a test must mutate several vars at once. |
| 2026-04-17 | self | Derived stdio plugin state dir as `global_dir.parent()/plugin-state` unconditionally; ad-hoc test plugin roots then spilled state under `/tmp/plugin-state` instead of the test root | If plugin root basename is literally `plugins`, sibling `plugin-state` is right. Otherwise keep plugin state under `<plugin-root>/plugin-state` so tests and nonstandard roots stay self-contained. |
| 2026-04-17 | self | Assumed Extism `dispatch_events(...)` messages would appear through `dispatch_event_to_plugins(...)`; SDK default output lacks `display: true`, so plugin dispatch quietly drops them | For mixed-runtime event tests, verify Extism event behavior via direct `on_event` calls or a plugin that explicitly sets display/UI fields. Don't expect SDK default `handled/message` output to surface as a user message. |
| 2026-04-17 | self | Tried to surface restricted-sandbox setup failure by returning `Err(...)` from stdio `pre_exec`; spawn error collapsed into generic `Invalid argument (os error 22)` and hid the real cause | For reviewable stdio bootstrap failures, write the message to child stderr inside `pre_exec` and `_exit(126)`. Supervisor stderr capture then preserves exact sandbox/setup failure text. |
| 2026-04-17 | self | Trusted a green rerun of `cron_schedule_sends_email` and missed that `cargo nextest run` still flakes because `tests/scheduled_email_live.rs` mutates global env and all live tests share one Fastmail mailbox | For live email tests, serialize with one global async mutex before `load_secrets()` / `load_email_plugin()`. Shared env + shared mailbox indexing timing can make full-suite nextest flaky even when single-test reruns pass. |
| 2026-04-19 | self | Treated daemon `schedule_fire` plugin responses as if `display: true` was required and kept the daemon live test coupled to Fastmail search indexing | For daemon schedule tests, assert through `src/modes/daemon::handle_schedule_event()` and surface `schedule_fire` plugin `message` fields even without `display: true`; leave actual mailbox delivery to the other live email tests. |
| 2026-04-17 | self | Archived an OpenSpec change by copying `MODIFIED` delta specs straight into `openspec/specs/`, which silently deleted unrelated baseline requirements from existing specs | For OpenSpec archive sync, treat `MODIFIED` deltas as patches over the current main spec. Merge the changed sections; do not replace the whole file unless the delta truly rewrites the full capability spec. |
| 2026-04-19 | self | Tried to archive completed legacy OpenSpec changes as if they were modern delta-spec changes; some old changes still use pre-delta `specs/*.md` layouts and fail/over-warn spec sync | For modern `specs/<capability>/spec.md` changes with `## ADDED` deltas, `openspec archive -y <change>` is safe. For legacy pre-delta layouts, archive with `openspec archive -y --skip-specs <change>` unless you first migrate/sync them manually. |
| 2026-04-19 | self | Took active OpenSpec `small-terminal-snapshot-stability` at face value as still-live blocker before checking recent test history and git log; current `main` already passed focused + broad reproductions and the accepted snapshot diff had landed in `ba564ecb` | For stale-seeming test blockers, run the focused + broader repro first, then inspect recent `git log -- <relevant test files>` before planning a fresh fix. Active change does not guarantee the failure still reproduces on current `main`. |
| 2026-04-19 | self | `cargo dylint` tigerstyle integration hit three gotchas in this repo: the nix rustup shim did not support `rustup +stable which cargo`, `workspace.metadata.dylint` rejected SCP-style SSH URLs like `git@github.com:...`, and Cargo/libgit2 could not authenticate the private SSH repo even though plain `git` worked | For SSH dylint metadata here, use `ssh://git@github.com/owner/repo.git`, keep `.cargo/config.toml` `net.git-fetch-with-cli = true`, and prepend a local rustup shim that handles optional `+toolchain`, `which cargo`, and `which rustc`. |
| 2026-04-19 | self | Misread user request "turn on all lints to error mode" as clippy/pedantic instead of tigerstyle-specific lint levels | When repo has both Clippy and tigerstyle lint surfaces, confirm which lint family user means before editing `Cargo.toml` lint tables. Tigerstyle severity belongs in `dylint.toml` `[tigerstyle.lint_levels]`, not workspace Clippy levels. |
| 2026-04-19 | self | cargo-tigerstyle driver could not parse a plain `libtigerstyle.so` path from `TIGERSTYLE_LINT_LIB`; this repo's wrapper needed the dylint-style `libtigerstyle@<toolchain>.so` filename even though the env carries a full path | When wiring tigerstyle manually, create and pass the `@<toolchain>`-suffixed shared library path, not only the bare `.so`, or the real dylint driver rejects it before linting. |
| 2026-04-21 | self | Assumed attach-mode slash forwarding would line up with standalone names; standalone registry exposes `/think` but daemon controller still only understands `/thinking`, so attached `/think` quietly fell off parity | For standalone-vs-attach parity work, compare actual slash registry command names against daemon `handle_slash_command_sync` before assuming forwarding is enough. Attach mode may need local bridging even when a daemon-side command exists under a different name. |
| 2026-04-21 | self | Thought AgentCommand→SessionCommand translation alone proved parity, but attach still showed extra daemon acks (`Thinking...`, `Disabled tools updated: ...`, manual compaction notices) that standalone never emitted | Local attach parity needs two layers: apply standalone-visible UI effects immediately, and suppress daemon follow-up acks that only exist for state sync. Reuse same suppression tracker in both local and remote attach loops. |
| 2026-04-21 | self | Assumed attach parity tests were un-runnable because default `cargo test --lib` hit mold undefined symbols, but repo tests do run here if linker is forced off mold | For root lib tests on this machine, use `CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib ...` before giving up on runtime evidence. |
| 2026-04-21 | self | Wrote attach `/help` copy that said "local parity commands" even though tests only proved a subset of the advertised local routes | In attach help/status text, say "locally handled" unless the full listed surface has explicit parity coverage. Keep user-facing claims aligned with deterministic tests. |
| 2026-04-21 | self | Made attach thinking-ack suppression broad enough to match both `ThinkingLevelChanged` and `SystemMessage("Thinking...")`, even though local `/think` bridge currently only gets the controller system-message ack | Keep attach suppression matchers as narrow as the actual bridged daemon contract, and pin that contract with a deterministic controller-event test before broadening suppression. |
| 2026-04-21 | self | Fixed explicit `/think <level>` bridge path and still missed no-arg `/think` cycle, which had a separate `AgentCommand::CycleThinkingLevel` branch with different session-command and no local parity update | When slash command has both explicit and no-arg/cycle paths, test both branches. Shared help text can hide separate bridge codepaths. |
| 2026-04-21 | self | Kept revising attach `/help` piecemeal and missed remaining local special cases (`/model`, `/role`, `/plugin`) after fixing `/think` and `/compress` | For attach help, diff the rendered help list against `route_attach_slash(...)` categories. If list is abbreviated, say "include" and "generally forward" instead of sounding exhaustive. |
| 2026-04-18 | self | Fixed `openai-codex` refresh by handling only the HTTP refresh path and missed the disk-shortcut/fallback-shortcut paths in `CredentialManager` | When refresh changes derived in-memory state (like Codex entitlement), mirror that invalidation on every refresh success path: HTTP refresh, disk-refreshed primary store, and fallback-store shortcut. |
| 2026-04-18 | self | One failing `openai-codex` entitlement test poisoned the shared test mutex and made later tests fail for the wrong reason | Test-only global mutex helpers should recover poison (`into_inner()`) so one assertion failure does not cascade into unrelated test noise. |
| 2026-04-14 | self | Tried to solve router auth-store plumbing in a downstream wrapper even though the reusable NixOS module was missing the real seam | Put generic `clanker-router` service flags in `nix/modules/clanker-router.nix`. `--auth-file` is a global flag, so it needs a first-class module option; `extraArgs` append after `serve` and cannot express it. |
| 2026-04-14 | self | Parallelized `openspec new change` with `openspec instructions ... --change <name>` and the dependent calls raced the scaffold | Treat OpenSpec scaffolding as sequential: create change first, then run status/instructions/validate in later tool calls. |
| 2026-03-15 | self | Delegated `DaemonEvent::SessionInfo` field fixes to worker; worker reverted my prior event.rs edits (new variants + ToolInfo struct) | Don't delegate edits to files you've already modified in this session. Workers can't see your uncommitted changes and may overwrite them. |
| recurring | self | `delegate_task`/`subagent` workers report success on multi-file refactors but changes don't persist | Workers are reliable for single-file edits and read-only analysis. Multi-file refactors: do directly. Always verify with `cargo check` + file existence after delegation. |
| recurring | self | Extracting crates: `pub(crate)` items accessed by main crate break | Grep all callers before extracting. Items used cross-crate must become `pub`. |
| recurring | self | Orphan rule: `impl ForeignTrait for ForeignType` in main crate | Use wrapper types (`MyWrapper<'a>(&'a Foreign)`) defined in the crate that owns the trait impl. |
| recurring | self | `#[cfg(test)]` methods invisible to downstream integration tests | Use unconditional `pub` for test helpers on extracted crates. Downstream tests need them. |
| recurring | self | `cargo fix --lib` removes extension trait imports it thinks are unused | After `cargo fix`, verify extension trait imports still present (glob `use super::*` pulls them in for test modules). |
| recurring | self | sed-based struct-literal→fn-call conversion leaves mismatched braces | For syntax-level transforms, read each call site and fix with targeted edits. Don't sed. |
| recurring | self | Moving types with methods that reference crate-internal types | Extract those methods as standalone functions or convert to free functions taking `&mut self`. |
| recurring | self | Assumed similar components are duplicates (panels with same-domain names) | Read module-level doc comments first. Overview list ≠ BSP pane ≠ fuzzy overlay ≠ diff view. |
| 2026-03-12 | self | `target/debug/clankers` was stale — `CARGO_TARGET_DIR=~/.cargo-target/` | Always use `$CARGO_TARGET_DIR/debug/clankers` or full path. `target/debug/` is a decoy. |
| 2026-03-12 | self | Background daemon passed `--model` after subcommand (`daemon start --model X`) | Top-level flags go BEFORE the subcommand: `clankers --model X daemon start`. |
| 2026-03-12 | self | `die_when_link_dies` default broke existing tests expecting `LinkDied` on failure | Tests that observe `LinkDied` on abnormal exits must use `spawn_opts(die_when_link_dies=false)`. |
| 2026-03-12 | self | Added field to `SessionFactory` struct broke integration tests | Always grep tests/ for struct literal construction when adding required fields. |
| 2026-03-12 | self | Used `GlobalPaths::detect()` / `ClankersPaths::new()` — actual API is `ClankersPaths::resolve()` | Check actual method names with grep before using path helper types. |
| 2026-03-09 | self | Glob re-exports (`pub use module::*`) bring all public items — conflicts with sibling imports | Check for conflicts before adding imports when a sibling module has glob re-exports. |
| 2026-03-09 | self | `map_err(db_err)` as tail returns wrong Result type | When helper returns a different error type, wrap: `Ok(expr.map_err(helper)?)` to trigger `From` via `?`. |
| 2026-03-10 | self | Plugin `serde` needs direct dep for derive macros even though SDK re-exports crate | Check Cargo.toml deps before using macros that need proc-macro resolution. |
| 2026-03-09 | self | Changed App initialization order → PTY tests show blank screen | PTY tests spawn the actual binary. Run validate_tui tests before committing App init changes. |

## User Preferences
- Don't care about backwards compat — fix the implementation properly
- Uses Fastmail, not third-party email services
- Prefers direct solutions over abstraction layers
- Git library: stick with git2. gix too immature for writes.
- Rust 2024 edition: no `ref` in match patterns, `std::env::set_var` is unsafe

## Patterns That Work

### Crate extraction
- Re-export pattern: original location does `pub use new_crate::*;` for zero API change
- External callers import directly from new crate; internal code uses re-exports
- Git detects file moves as renames when content changes < ~20% diff
- `#[path = "filename.rs"] #[cfg(test)] mod tests;` extracts tests from non-mod.rs files
- Always check who calls a function before deciding to move it — grep callers, not just definitions

### Decomposition
- Extract setup/builder/handler functions, not structural splits of declarative files (cli.rs is fine at 763 lines — it's all clap derives)
- Big match statement files (event_handlers.rs) have limited decomposition value beyond helper extraction
- system_prompt.rs at 727 lines: 350 impl + 377 tests, well-decomposed already. Not every big file needs splitting.

### OpenSpec review hardening
- If a spec adds behavior or regression claims, tasks need at least one explicit checkbox that verifies them. Grouping is fine; uncovered scenarios are not.
- If a design depends on a private/external reference implementation for wire behavior, freeze the contract in the artifact itself: endpoint, required headers, body fields, claim path, and retry/status semantics. Pair it with fixture or integration coverage.
- If proposal/design says docs/help or unchanged UX paths matter, tasks must include explicit acceptance/regression verification. "Update docs" alone is too weak.
- If a spec says a value is stable, derived, or reused, define concrete source field, transform, scope, and lifetime. Do not leave identifier semantics implicit.
- OpenSpec design gate evidence can truncate long artifacts before late verification bullets. Put a compact verification summary early in `design.md` so constructor/parity/request-fixture/docs/smoke checks stay visible.
- Do not claim stage passes or file edits unless this turn's transcript shows the gate output or git status proving them. Re-run before summarizing if needed.

### Tiger Style
- Session tree traversals: bounded by MAX_TRAVERSAL_DEPTH with cycle detection via visited set
- Convert recursive DFS to iterative DFS with explicit stack where unbounded depth possible
- `const _: () = assert!(...)` for compile-time assertions on safety constants
- `push_bounded(vec, item, max)` drops 10% when full — amortizes O(n) drain
- `debug_assert` on rate signs + `is_finite()` check prevents NaN propagation

### Conversation caching
- Compaction invalidates cache prefixes — skip compaction when prompt caching is active
- `build_context(compact: bool)` — compact only when `--no-cache` (i.e., `settings.no_cache`)
- `prompt-caching-2024-07-31` beta header needed in ALL Anthropic request paths (provider + router, OAuth + API key)
- Two `CompletionRequest` types: provider (`clankers-provider/src/lib.rs`) and router (`clankers-router/src/provider.rs`) — both need `no_cache` and `cache_ttl`
- Third `CompletionRequest` construction site in `clankers-provider/src/router.rs` (test module) — easy to miss
- `CacheControl::with_ttl(None)` = ephemeral (5m), `with_ttl(Some("1h"))` = 1-hour. TTL serialized only when `Some`.
- Clippy `collapsible_if`: `if !flag { if let Some(x) = ... }` → `if !flag && let Some(x) = ...`
- Clippy `format_push_string`: use `write!(string, ...)` not `string.push_str(&format!(...))`

### Provider auth plumbing
- `crates/clankers-provider/src/credential_manager.rs` used to assume provider=`anthropic` in disk reload, refresh save-back, and fallback selection. When adding a new OAuth provider, thread provider name into `CredentialManager` and use provider-scoped `AuthStoreExt` helpers (`active_account_name_for`, `set_provider_credentials`, `active_oauth_credentials_for`) or refresh will touch the wrong provider slot.
- Pending OAuth verifier/state needs provider+account isolation in both memory and disk. New auth flows should use `.login_verifiers/<provider>/<account>.json` and keep legacy `.login_verifier` fallback only for migration/compat reads.
- When `clankers-provider::CompletionRequest` gains a field, `cargo check` may miss constructor gaps in test/helper code. Run `cargo check --tests` to catch provider-side helper constructors too (`router.rs`, `anthropic/mod.rs`).
- `SessionController.session_id` and `App.session_id` are not enough for routed provider requests. `_session_id` comes from `Agent.session_id`, so controller-owned agents must be synced on construction/update or daemon/resume paths silently lose session metadata. Slash-driven session resume also needs post-dispatch `controller.set_session_id(app.session_id.clone())` in the event loop, not just key-handler/session-selector paths.
- For `_session_id`/resume claims, direct `run_turn_loop(..., "same-id")` tests are too weak. Add one test that resumes a persisted session through `resume_session_from_file`, then captures a router/RPC request and checks `_session_id` there.
- For request-shape regressions, add deterministic rails in source tests instead of relying on review memory: exact constructor-count inventory for `CompletionRequest {` sites plus shared-field serde projection parity between local/provider and router structs.
- Pre-backend `openai-codex` discovery needs two stopgaps: (1) skip RPC daemon path when local Codex auth exists, because extracted router does not know Codex yet, and (2) keep a fail-closed `openai-codex/...` sentinel in `RouterProvider` so explicit Codex prefixes never silently fall back to Anthropic.
- When provider credentials can come from `~/.pi` fallback, discovery/status code must not assume primary auth store owns the active account. Resolve credential source and status source together or fallback-only providers disappear from catalog/status.

### Event draining
- `broadcast::Receiver::try_recv()` returns `Err(Lagged(n))` when buffer overflows — NOT a terminal error
- After `Lagged`, receiver auto-resets to oldest available event — must `continue`, not `break`
- `while let Ok(event) = rx.try_recv()` is WRONG for broadcast receivers — breaks on Lagged, drops all remaining events
- Use explicit `loop { match try_recv() { Ok => push, Lagged => warn+continue, _ => break } }` instead
- Agent broadcast channel is 1024 capacity. A 4-turn tool loop can produce 1500+ events (text deltas + tool events)
- `drain_events` only runs AFTER `handle_command` returns — entire turn loop's events queue up

### Daemon-client architecture
- Protocol: serde_json + length-prefixed frames over Unix sockets (local) / iroh QUIC (remote)
- rkyv rejected: wrong tool for small text messages, loses debuggability
- Lunatic rejected: WASM process model mismatches native agent resources, wasmtime version conflicts
- Automerge for: session tree (append-only DAG), todo list, napkin. NOT for: settings (LWW), auth tokens, streaming output (ephemeral)
- `SessionController`: transport-agnostic, owns Agent + SessionManager + LoopEngine + HookPipeline + AuditTracker
- Embedded mode: events fed via `feed_event()`, outgoing via `take_outgoing()`. No agent needed.
- `agent_event_to_daemon_event()` and `daemon_event_to_tui_event()` are the two conversion points
- `handle_prompt()` uses `self.agent.take()` / `self.agent = Some(agent)` to avoid borrow conflicts
- `drain_events()` collects from event_rx into Vec first to avoid borrow conflict between rx and processing

### Attach mode
- `ClientAdapter.is_disconnected()` detects closed channel; reconnection via `try_reconnect()` with exponential backoff
- `run_attach_with_reconnect()` owns the reconnection state machine, replaces `run_attach_loop()`
- History replay: `agent_message_to_tui_events()` converts AgentMessage → TuiEvent sequences
- Session picker runs BEFORE `init_terminal()` — standalone raw-mode mini-TUI
- Input split: `is_client_side_command()` routes locally (quit, detach, zoom) vs forward to daemon
- BashConfirmState popup in attach mode — higher priority than other overlay intercepts
- **Remote attach via iroh QUIC**: `clankers attach --remote <node-id>`
  - `clankers/daemon/1` ALPN carries `DaemonRequest` discriminant as first frame
  - `DaemonRequest::Control` for one-shot commands, `DaemonRequest::Attach` for session streams
  - `QuicBiStream` combines iroh `SendStream`/`RecvStream` into single `AsyncRead+AsyncWrite`
  - iroh `SendStream::poll_write` returns `WriteError`, not `io::Error` — must map in `AsyncWrite` impl
  - `ClientAdapter::from_channels()` skips handshake for pre-negotiated QUIC streams
  - After `DaemonRequest::Attach` + `AttachResponse` + `SessionInfo`, stream is standard session protocol
  - Reuse `run_attach_with_reconnect()` event loop — reconnection won't work for remote (empty socket path), but disconnect detection works

### Auto-daemon mode (Phase 3)
- Default interactive mode (`clankers` no subcommand) routes through daemon when `use_daemon=true`
- `run_auto_daemon_attach()` in `src/modes/attach.rs` — ensure daemon → CreateSession → connect → TUI
- Session killed on quit (via `ControlCommand::KillSession`) — auto-daemon owns its session lifecycle
- `ConnectionMode` stays `Embedded` (no "ATTACHED" badge) — user shouldn't see implementation details
- CLI overrides: `--daemon` forces daemon mode, `--no-daemon` forces in-process
- Headless modes (`--print`, `--stdin`, `--mode json`) bypass daemon — no TUI, no daemon overhead
- `--thinking` forwarded as `SetThinkingLevel` command after connect
- `--model`, `--agent`, `--resume`, `--continue`, `--cwd` all forward through `CreateSession`
- `ensure_daemon_running()` uses tracing not eprintln — TUI takes over stdout immediately after

### TUI patterns
- `SlashContext<'a>` wraps `&'a mut App` + all params — single struct to every handler
- `std::mem::take()` to temporarily move a field out, dispatch, put back — for Default-able types
- Render loop: clone theme to avoid borrow conflict between `&app.theme` and `app.panel_mut()`
- Hypertile BSP: `PaneId::ROOT` is chat (always exists), `PaneKind::Subagent(String)` for per-subagent panes
- `allocate_pane_id()` for unique IDs — no collision with well-known IDs 0–6
- Conversation blocks currently stamp `ConversationBlock.timestamp` at TUI construction time (`ConversationBlock::new` uses `Local::now()`), so restore/attach replay will mint fresh block times unless metadata is plumbed from persisted message timestamps

### Plugin system
- Extism 1.13 host / extism-pdk 1.4.1 guest, WASM targets `wasm32-unknown-unknown`
- Plugin WASM tests (89 tests) fail in worktrees — skip with `--skip plugin::tests`
- `catch_unwind(AssertUnwindSafe(...))` isolates WASM panics; mutex locks use poison recovery everywhere
- WASM has no clock — time-aware features MUST use host-injected config keys
- Plugin `build.sh` must use `~/.cargo-target/` path, not `./target/`

### AgentEvent field names (common gotchas)
- `MessageUpdate`: field is `index` not `message_index`, delta is `ContentDelta`
- `TurnStart`/`TurnEnd`: use `index` not `turn_number`
- `Context`: only `messages` field (no `system_prompt`)
- `ModelChange` NOT forwarded via `agent_event_to_daemon_event()` — hooks only

### Daemon resilience
- iroh endpoint failure is non-fatal — daemon runs with control socket only
- Heartbeat endpoint failure is non-fatal — heartbeat disabled with warning
- `build_endpoint()` returns `Result` — caller `match`es to degrade gracefully

### Verus proofs — UCAN
- `verus/ucan_spec.rs`: 7 UCAN requirements, all specs + proofs pass
- Models: `PatternModel` (Wildcard|Items), `FileAccessModel` (prefix+read_only), `FileOp`, `ToolGate`
- Key proof: `prove_file_access_no_escalation` — uses `prefix_transitive` lemma for Seq<u8> prefix transitivity
- `prefix_transitive` lemma: `is_prefix_of(a,b) && is_prefix_of(b,c) ==> is_prefix_of(a,c)` — proved via element-wise reasoning through subrange
- Tracey config: UCAN source files in `include` (not `test_include`) because they carry both `impl` and `verify` annotations
- `src/capability_gate.rs` has mixed `impl`/`verify` in one file — keep in `include` only (include allows all annotation types)

### Verus proofs
- Bitvector proofs: `assert(...) by (bit_vector)` — must work entirely in fixed-width types, no `as u8`/`as u32` casts inside the block
- u8↔u32 roundtrip: prove separately with a lemma `(x as u8) as u32 == x` when `x == x & 0xff`, then use the lemma to bridge the gap between spec fns that go through u8 and bit_vector proofs that need u32
- Recursive spec fns: SMT solver won't auto-unfold recursive definitions — manually call inner `walk_branch_rec(t, parent, fuel-1)` and `assert(path =~= inner.push(entry))` to help unfolding
- `=~=` (extensional equality) needed for Seq comparisons, not `==`
- Build tree with explicit `Map::empty().insert(...)` chains, not `Map::new(|..| choose)` — the latter triggers low-confidence trigger warnings

### Nix tool
- Nix daemon socket needs **write** access — Landlock `/nix` as RO blocks `connect()`
- Fix: add nix-specific RW paths before broad `/nix` RO rule (Landlock merges permissions)
- `nom` (nix-output-monitor) rejected: emits TUI cursor control codes even when piped
- `nix build --log-format internal-json -L` produces parseable `@nix {...}` JSON on stderr

## Patterns That Don't Work
- WASM plugins with shared `./target/` dir — use `~/.cargo-target/`
- `Plugin.serde_json` via `use clankers_plugin_sdk::serde_json` — needs direct dep
- Workers for multi-file refactors — changes don't persist reliably

### Automerge session storage
- `automerge 0.7.4`: `AutoCommit` for single-writer, `Value::Scalar(s).to_str()` returns `Option<&str>`
- Document schema: root has 3 keys — `header` (Map), `messages` (Map), `annotations` (List)
- Messages stored as JSON strings in `message_json` field — write-once, no partial merge needed
- Annotations stored as JSON strings in `data` field with `kind` discriminator
- `save_incremental()` appends bytes to existing file; `load()` reads full + incremental chunks
- `doc.keys()` returns insertion-order iteration for maps
- `doc.length()` for list length, index with `doc.get(&obj, i)` for `usize` indices
- `AnnotationEntry` uses `#[serde(tag = "ann_type")]` NOT `"kind"` — `CustomEntry` already has a `kind` field, so `#[serde(tag = "kind")]` causes duplicate field error on deserialization
- `merge_branch` is annotation-only (no message cloning). `merge_selective` and `cherry_pick` still copy messages via `append_message`.
- JSONL backward compat: `open()` auto-migrates .jsonl → .automerge alongside (original untouched)
- External callers (`interactive.rs`, `session_setup.rs`) must use `record_resume()` not `store::append_entry()` — file is binary automerge now

### Nickel config
- `nickel_lang::Error` does NOT impl `Display` — use `Debug` formatting (`{:?}`)
- `nickel_lang::Context` is `!Send + !Sync` — eval on main thread only, before async runtime
- Nickel error formatting overflows default thread stack (~2MB) — needs `RUST_MIN_STACK=33554432` or `#[ignore]` for contract violation tests
- Contract file has `#` comments — can't wrap in `(CONTRACT)` parens because `#` comments extend to EOL and eat the closing paren. Use `let` or inline substitution instead.
- `AgentScope::default()` is `User` not `All` — contract defaults must match Rust struct defaults exactly
- Nickel `| optional` for `Option<T>` fields — omitted fields don't appear in JSON output, serde `#[serde(default)]` fills in `None`

### Tigerstyle lints
- dylint driver needs manual build in Nix: `RUSTUP_TOOLCHAIN=nightly cargo build` in temp dir, copy to `~/.dylint_drivers/`
- `cargo clean` wipes the lint library — must rebuild from `~/git/tigerstyle`
- `cfg_attr(dylint_lib = "tigerstyle", allow(...))` needs `check-cfg` in workspace Cargo.toml
- `let _ = expr` → `expr.ok()` ONLY works when expr returns `Result`; non-Result types need `let _ = expr`
- `write!(String, ...).ok()` is the correct fix for infallible String writes (not `.unwrap()`)
- Bool renames on local vars that feed struct field shorthand (`Struct { field }`) must not be renamed
- Bulk sed/perl for `let _ =` → `.ok()` is dangerous — must verify each call returns Result, not Option/unit/custom type
- Remaining 272 warnings are structural: bool struct fields, mutex expects, event loops, TUI guarded divisions

## Domain Notes
- JMAP (RFC 8620/8621): pure HTTP+JSON email, Fastmail is reference impl
- Matrix SDK 0.9: `Room::typing_notice(bool)`, `send_attachment()` for files, `ClankersEvent::Text` has `room_id`
- `<sendfile>/path</sendfile>` tags extracted, uploaded to Matrix, stripped from text
- PTY tests: 5 flaky tests (`slash_commands`, `slash_menu`) timeout intermittently — pre-existing
- `DaemonConfig` construction: use `..Default::default()` for new fields
- `PaneId::new()` is not const — use functions for non-ROOT pane IDs
