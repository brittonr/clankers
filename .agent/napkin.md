# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|
| 2026-04-08 | self | Ran `cargo fmt --all` for a small provider change and it reformatted a huge swath of the workspace | In this repo, use `rustfmt` on the touched files only. If `cargo fmt --all` slips through, immediately revert unrelated formatting before doing anything else. |
| 2026-04-08 | self | `cargo test`/`clippy` suddenly failed with `No space left on device` even though `/` had space | Check `/tmp` too, not just `/`. This machine can fill tmpfs with old VM/images and large temp dirs; clear `/tmp` before assuming the Rust change broke the build. |
| 2026-04-08 | self | I added a helper-only inbound rewrite path and missed that the runtime SSE path still forwarded `ContentBlockStart::ToolUse` unchanged | When changing stream rewriters, add at least one test at the real seam (`parse_sse_stream(..., reverse_map = true)`), not just helper/unit tests. |
| 2026-04-08 | self | Review evidence was weaker than the actual work because I bundled/parallelized validation and the transcript did not clearly show the exact command | For reviewer-sensitive claims, rerun the exact command with `set -x` in a dedicated tool call so the transcript proves what ran. |
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
