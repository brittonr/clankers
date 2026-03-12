# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|
| 2026-03-11 | user | `use_worktrees` defaulted to `true` — writes went to `.git/clankers-worktrees/` and users never saw changes in their repo. `--no-worktree` CLI flag was dead code (never wired in). | Changed default to `false` so writes go to the actual repo. Wired `--no-worktree` flag into settings override. Worktrees are opt-in via config. |
| 2026-03-09 | self | Missed serde_json dep when extracting agent_defs (identity.rs uses it) | Always run `rg` for ALL external crate usages including transitive ones like serde_json, not just explicit `use` at top of mod.rs |
| 2026-03-09 | self | `cargo fix --lib` removed `DbWorktreeExt` import from registry.rs (thought it was unused) — broke tests later | After `cargo fix`, verify extension trait imports are still present in files that use the trait in test modules (glob `use super::*` pulls them in for tests) |
| 2026-03-09 | self | Extracting procmon: ProcessEvent enum fields didn't match what the code actually constructs | Always grep for actual struct literal construction sites before defining a replacement type — don't guess field names from the original |
| 2026-03-09 | self | `#[cfg(test)]` methods on extracted crate become invisible to downstream integration tests | For test helpers on extracted crates, use unconditional `pub` instead of `#[cfg(test)]` — downstream tests need them. Same for `Db::in_memory()`. |
| 2026-03-09 | self | `map_err(db_err)` as tail expression returns `Result<T, DbError>`, not `Result<T, Error>` — type mismatch | When `db_err` returns a different type than the function's Result, wrap in `Ok(expr.map_err(db_err)?)` to trigger `From` conversion via `?` |
| 2026-03-09 | self | Tried to impl Serialize/Deserialize for InputMode in main crate after moving the type to clankers-tui-types — orphan rule prevents it | Add serde derives directly on the type in the types crate (it already depends on serde). Don't try to add trait impls for foreign types. |
| 2026-03-09 | self | Left `#[derive(Debug, Clone, PartialEq, Eq)]` above a `pub use` re-export — `derive` only applies to struct/enum/union definitions | When replacing a type def with a re-export, remove ALL attributes above the original definition (derives, doc comments that became stale) |
| 2026-03-09 | self | Switched slash_commands CompletionItem import to tui-types version, but they have different field types (`&'static str` vs `String`) | Before switching imports, verify the types are structurally identical. Different field types break From impls and constructors. |
| 2026-03-09 | self | ThinkingLevel had `to_config()` method referencing `ThinkingConfig` from clankers-router — can't move to types crate | When moving types with methods that depend on other crate types, extract those methods as standalone functions (e.g., `thinking_level_to_config()`) in the original module |
| 2026-03-09 | self | Moving `PluginUIState` to types crate broke `apply()` method which referenced `PluginUIAction` (staying in main crate) | When a type's methods reference types from the original crate, convert those methods to free functions that take `&mut self` as first arg |
| 2026-03-09 | self | Adding `use std::time::Instant` to progress.rs caused duplicate error — it was already pulled in via `pub use display::*` in lib.rs | Glob re-exports (`pub use module::*`) bring all public items including re-exported std types; check for conflicts before adding imports in sibling modules |
| 2026-03-09 | self | Moved `rebuild_leader_menu`/`rebuild_slash_registry` from App to interactive.rs, broke PTY integration tests (validate_tui). Blank screen. | validate_tui tests spawn the actual binary via PTY — any change that breaks initialization order or startup flow causes blank screens. Always run validate_tui tests before committing App initialization changes. Reverted to keeping rebuild methods on App. |
| 2026-03-09 | self | `app.slash_registry` can't become `Box<dyn CompletionSource>` because event_handlers.rs calls `registry.dispatch()` (not a CompletionSource method), and `std::mem::take` needs Default | RESOLVED: moved SlashRegistry out of App into EventLoopRunner. App gets `Box<dyn CompletionSource>`, runner passes `&SlashRegistry` to dispatch functions. No more borrow conflict. |
| 2026-03-09 | self | Moving rebuild_leader_menu out of App broke PTY tests when also changing slash_registry field type | Do one thing at a time: first move SlashRegistry out (keep rebuild_leader_menu), then move rebuild_leader_menu. Both changes individually pass PTY tests. |
| 2026-03-09 | self | Process panel deeply coupled to `TrackedProcess` internals (snapshots, meta, state) | ProcessDataSource trait in tui-types returns Vec<ProcessSnapshot> — pre-computes all display fields. Main crate's ProcessMonitor implements the trait with tracked_to_snapshot() helper. |
| 2026-03-09 | self | sed-based `crate::error::Error::Variant { message: X }` → `err_fn(X)` conversion left mismatched braces (`X }` instead of `X)`) | For struct-literal→fn-call conversions, don't sed. Read each call site and fix with targeted edits. The ` }` vs `)` closing is easy to miss. |
| 2026-03-09 | self | Provider crate `credential_manager.rs` tests needed `reqwest/blocking` feature — not in workspace default | When extracting crates, check test code for feature-gated imports (e.g. `reqwest::blocking::Client`) and add features to the new crate's Cargo.toml |
| 2026-03-09 | self | rpc_e2e test implemented `Provider` trait returning `clankers::error::Result` — broke when Provider::complete now returns `ProviderError` | After extracting a trait's error type to a new crate, grep all trait impl sites in tests/ and integration tests too, not just src/ |
| 2026-03-07 | self | delegate_task workers for all 5 cleanup tasks reported success but no changes persisted to disk | Always redo multi-file refactors directly after worker "success" — verify files exist before moving on |
| 2026-03-07 | self | Tried `pub use clankers_router::Cost` but Cost isn't re-exported at root | Check `lib.rs` re-exports before assuming root-level access; use `clankers_router::provider::Cost` |
| 2026-03-07 | self | Stale `session/` dir (gitignored workspace copy) had old `agents::` paths after rename to `agent_defs::` — cargo test picked it up | Always `rm -rf session/` after renames; stale workspace copies interfere with cargo resolution |
| 2026-03-05 | self | Subagent parallel tasks for registry.rs and slash_commands silently failed (files not created) | Verify file existence after subagent work before depending on it; do critical edits directly |
| 2026-03-05 | self | Delegated handler extraction workers reported success but didn't persist changes | Workers may lose edits; always verify with `cargo check` and `grep` after delegation |
| 2026-03-05 | self | Python regex-based code transform was fragile (mangled `crate::tui::app::` paths, missed `if let Some(db)` bindings, double-prefixed `self.self.`) | For code extraction: use brace-counting for boundaries, then targeted `sed` fixes for the known replacement patterns. Don't try to be clever with one regex. |
| 2026-03-05 | self | Rust 2024 edition: `ref name` in match patterns causes "cannot explicitly borrow" | Drop `ref` in match patterns — Rust 2024 does implicit borrowing |
| 2026-03-06 | self | `PaneId::new()` is not const in ratatui-hypertile 0.1 — tried `pub const` pane IDs | Use functions (`pub fn todo() -> PaneId`) instead of `const` for non-ROOT pane IDs. Only `PaneId::ROOT` (uses `Self(0)` literal) is const. |
| 2026-03-06 | self | Old column-based h/l navigation tests assumed `l from right → main`. Hypertile spatial model: chat is in center, `l` goes right, `h` goes left | Tests for panel navigation must account for BSP spatial model — directional focus goes to the nearest pane in that direction, not column-side logic |
| 2026-03-06 | self | Old `dispatch()` had `"sh"` instead of `"shell"` — `/shell` fell through to prompt template handler | When adding/renaming commands in the `dispatch()` match table, verify the string matches `builtin_commands()` name exactly |
| 2026-03-06 | self | PTY tests sending Ctrl+J/K/N (`\x0a`/`\x0b`/`\x0e`) — crossterm doesn't reliably parse these as Ctrl+letter through PTYs | Use Up/Down arrows for menu navigation in PTY tests; HistoryUp/Down are handled by menu interceptor when menu is visible |
| 2026-03-06 | self | `/help` test expected "Available slash commands" header but it scrolled off with 37+ commands in a 50-row PTY | Size PTY large enough for content, or wait for text guaranteed to be visible (e.g. `/quit` near bottom of help list) |
| 2026-03-07 | self | Delegated workers for antipattern refactors reported success but changes didn't persist in the worktree | Always do large refactors directly, not via delegate_task. Workers may not persist changes to worktrees. |
| 2026-03-07 | self | Tried disjoint field borrow across function boundary for slash registry — compiler can't split borrows on `&mut App` received as parameter | Use `std::mem::take()` to temporarily move the field out, dispatch, then put it back. Works cleanly for `Default`-able types. |
| 2026-03-07 | self | `ctx.app.slash_registry.dispatch(..., &mut ctx)` fails — self-referential borrow through SlashContext | Extract registry with `std::mem::take()` before building SlashContext, restore after dispatch |
| 2026-03-07 | self | Both `delegate_task` and `subagent` parallel tasks report success on file refactors but changes don't persist to the main repo | Always do refactoring directly. delegate_task/subagent tools cannot reliably persist multi-file edits. Only use them for read-only analysis or single-file writes. |
| 2026-03-07 | self | Worker removed `use super::*` from git_ops test module (clippy said unused) but tests needed it | Clippy `unused_import` on `super::*` inside a non-`#[cfg(test)]` mod — the fix is adding `#[cfg(test)]`, not removing the import |
| 2026-03-07 | self | Subagent parallel workers for single-file clippy fixes worked reliably across 4 groups (no persistence issues) | Subagent parallel tasks DO work well for single-file mechanical edits (clippy fixes, dead code removal) — the persistence issue is mainly with multi-file refactors and worktrees |
| 2026-03-07 | self | Subagent parallel workers for single-file refactors (function extraction, module splits) worked reliably across 16 parallel batches | Subagent workers work great for: extract helpers from long functions, split file into directory module, move tests to separate file. Key: each task targets 1-2 files max, uses `cargo check` as gate. |
| 2026-03-08 | self | Subagent workers created new files (test directories, common.rs) but didn't `git add` them — had to catch untracked files manually | Always run `git status --short` after subagent file creation to catch untracked new files before committing |
| 2026-03-08 | self | Assumed high comment density = dead commented-out code, but analysis showed 90%+ were doc comments (`///`, `//!`) | Before removing "comments", distinguish doc comments from dead code. High comment % is fine if it's documentation. Look for patterns like `// fn`, `// let`, `// if` for actual dead code. |
| 2026-03-08 | self | TUI "near-duplicate" panels (subagent_panel vs subagent_pane, branch_panel vs branch_switcher vs branch_compare) are architecturally distinct | Don't assume same-domain components are duplicates. Read module-level doc comments first: overview list ≠ BSP pane, list panel ≠ fuzzy overlay ≠ diff view. |
| 2026-03-07 | self | Panel downcast `.expect()` calls aren't bugs (panels always registered at startup) but are noisy | Replace bare `.expect("panel")` with descriptive `.expect("X panel registered at startup")` or wrap in typed helper methods for readability |

## Corrections (continued)
| 2026-03-10 | self | Plugin extraction: `impl SlashContributor for PluginManager` is orphan when both types are in separate crates | Use wrapper types (`PluginSlashContributor<'a>(&'a PluginManager)`) defined in the main crate for orphan-rule-safe trait impls |
| 2026-03-10 | self | Plugin extraction: test helpers accessed `mgr.plugins` and `mgr.instances` (private fields) directly | Add `inject_instance()` and `get_mut()` public methods to PluginManager for test injection — avoids exposing the full HashMap |
| 2026-03-10 | self | Moving PluginEvent::matches(&AgentEvent) to extracted crate creates a crate dep on AgentEvent | Decouple with string tags: `AgentEvent::event_kind() -> &str` + `PluginEvent::matches_event_kind(&str)` — no cross-crate type dependency |

## Corrections (continued 2)
| 2026-03-10 | self | Session crate (clankers-session) doesn't depend on `tracing` — used `eprintln!` for safety-critical error messages instead | Check Cargo.toml deps before using tracing macros in extracted crates. Use `eprintln!` or add tracing as a dep. |

## User Preferences
- Don't care about backwards compat — fix the implementation properly
- Uses Fastmail, not third-party email services (SendGrid, Mailgun)
- Prefers direct solutions over abstraction layers
- Git library: stick with git2 (libgit2). Considered gix (gitoxide/pure Rust) but it has too many gaps for writes (no index staging, no worktree add/remove, no high-level merge/checkout). Revisit when gix matures.

## Patterns That Work
- ProcessPanel needs `with_monitor()` after App::new() — App is created early in interactive.rs, monitor is created later; wire it via `app.process_panel = ProcessPanel::new().with_monitor(monitor.clone())`
- Headless/daemon paths use `build_tools_with_events` (not `build_default_tools`) when you need to inject a ProcessMonitor
- `DisplayMessage` has `images: Vec<DisplayImage>` — every construction site needs the field or it won't compile
- `expand_at_refs_with_images` returns `ExpandedContent { text, images }` — keeps old `expand_at_refs` for backward compat
- Sixel rendering: `image::load_from_memory` → resize → quantize to 255 colors → encode as DCS escape sequences
- Image decode features in Cargo.toml: `["png", "jpeg", "gif", "webp"]` — needed for both Sixel and clipboard paste
- Plugin SDK at `crates/clankers-plugin-sdk/` with `prelude::*` re-export
- Plugins are standalone crates: `cdylib`, `[workspace]` opt-out, target `wasm32-unknown-unknown`
- Extism 1.13 host / extism-pdk 1.4.1 guest
- Extism built-in HTTP via `allowed_hosts` on Manifest + `extism_pdk::http::request`
- Extism config via `manifest.with_config_key()` host-side, `extism_pdk::config::get()` guest-side
- `plugin.json` is the manifest, `PluginManifest` struct in `src/plugin/manifest.rs`
- `PluginManager::load_wasm` creates `extism::Plugin::new(manifest, [], true)` — no host fns yet

## Patterns That Work (calendar plugin)
- Host injects `current_time` (YYYYMMDDTHHMMSSZ) and `current_time_unix` config keys during `load_wasm` — plugins read via `extism_pdk::config::get()`
- WASM has no clock — all time-aware features MUST use host-injected config, not stubs
- `fetch_event` uses PROPFIND Depth:0 to get both calendar-data AND getetag in one request (SDK HTTP doesn't expose response headers)
- Calendar discovery results cached in `thread_local!` — avoids PROPFIND on every tool call
- UID generation must include a timestamp or random component to avoid collisions
- `serde` must be a direct dep for derive macros even though SDK re-exports the crate
- `allowed_hosts` in plugin.json must list specific CalDAV server hostnames (not empty `[]`)
- Attendee allowlist pattern matches email plugin's recipient allowlist: exact, `*@domain`, `*`
- Event responses use `display`/`message` fields (not `context`) — host only reads those
- `commands: []` if no `handle_command` export — don't declare dead slash commands
- Plugin `build.sh` must use `~/.cargo-target/` path, not `./target/`

## Patterns That Work (merge/cherry-pick)
- `set_message_id()` helper handles all 7 `AgentMessage` variants (User, Assistant, ToolResult, BashExecution, Custom, BranchSummary, CompactionSummary) — no `System` variant exists
- `find_unique_messages()` on SessionTree uses HashSet of target IDs for O(1) filtering
- `merge_branch()` copies messages with new IDs, chains parent_id from target leaf, emits CustomEntry with kind "merge"
- `merge_selective()` filters unique messages by selected_ids set before copying
- `cherry_pick()` uses `collect_subtree()` (DFS) for `--with-children`, maps old→new IDs to preserve subtree structure
- `collect_subtree()` is a static method (`Self::`) not `&self` — clippy catches `self_only_used_in_recursion`
- Slash command `parts.contains(&"--with-children")` not `parts.iter().any(|p| *p == ...)` — clippy `manual_contains`
- Cargo test filter: use space-separated names not `\|` alternation (not regex)

## Patterns That Work (subagent panel)
- `SubagentPanel` Enter key emits `FocusSubagent(id)` — caller must check if BSP pane exists, fall back to `open_detail()` if not
- `focus_subagent()` silently does nothing when no BSP pane exists for the subagent ID — always guard with `pane_id_for()` check
- BSP panes only created up to `max_subagent_panes` (default 4) — entries beyond that have no pane, need inline detail view fallback
- Dismissed BSP panes (user pressed `q`) leave entries in overview panel — Enter must still work via detail view

## Patterns That Don't Work
- WASM plugins use a shared cargo target dir at `~/.cargo-target/`, not `./target/` — find built wasm there
- Plugin `serde_json` usage needs `use clankers_plugin_sdk::serde_json;` — not a direct dep
- Plugin config checks run in order of code — `from` address check runs before `jmap_token` if no `from` param
- PluginManager::load_wasm injects config_env from real env vars — tests for "missing config" error paths must bypass load_wasm and create raw Extism plugins with no config
- Fastmail JMAP `EmailSubmission/set` enforces from/identity match strictly for external sends but is lenient for intra-account sends — identity must match the from address (exact or wildcard `*@domain`)

## Patterns That Work (continued)
- `send_markdown()` on MatrixClient handles md→HTML + auto-chunking at 32KB; don't call `send_text()` for agent responses
- `clankers_matrix::markdown::md_to_html()` uses pulldown-cmark with tables, strikethrough, tasklists enabled
- `chunk_response()` splits at paragraph boundaries, never inside fenced code blocks

## Patterns That Work (UCAN auth)
- `clankers-auth` crate lives at `crates/clankers-auth/` — forked from aspen-auth
- Generic machinery (token, builder, verifier, error, constants, utils) ported unchanged from aspen-auth
- Capability enum is clankers-specific: Prompt, ToolUse, ShellExecute, FileAccess, BotCommand, SessionManage, ModelSwitch, Delegate
- RevocationStore trait replaces aspen's KV-backed store with redb (REVOKED_TOKENS_TABLE, AUTH_TOKENS_TABLE)
- `pattern_contains()` for delegation checks uses HashSet subset logic for comma-separated patterns
- `glob_match()` ported from aspen-auth for ShellExecute command pattern matching
- Worker delegates create files in worktrees, must copy back to main repo manually
- iroh 0.96 API is compatible with aspen-auth's 0.95.1 usage (SecretKey, PublicKey, Signature)
- CLI `token` commands use `redb::Database::create` directly, NOT through `Db` wrapper (Db's `begin_read/begin_write` are `pub(crate)`, invisible to `main.rs` binary)
- `revocation` module must be `pub mod` in clankers-auth lib.rs for main.rs to access `AUTH_TOKENS_TABLE` / `REVOKED_TOKENS_TABLE`
- Token info goes to stderr, base64 token goes to stdout (for piping: `TOKEN=$(clankers token create ...)`)
- Duration parsing: `30m`, `1h`, `24h`, `7d`, `365d`, `1y`
- `--read-only` = ToolUse("read,grep,find,ls") — must match parent's pattern for delegation
- Daemon auth layer: `AuthLayer` struct holds `TokenVerifier`, `RedbRevocationStore`, `Arc<redb::Database>`
- Identity must be loaded BEFORE auth layer (auth needs `identity.public_key()` for trusted root)
- `SessionStore::get_or_create()` takes `capabilities: Option<&[Capability]>` — filters tools at session creation
- `LiveSession::session_tools` stores the filtered tool set for reuse in temporary agents
- Borrow checker: extract tools/provider/settings from session/store into locals before constructing Agent
- `run_matrix_prompt` / `run_matrix_prompt_with_images` both take `capabilities: Option<&[Capability]>`
- `!token <base64>` bot command: verify → store in redb → restart session → confirm to user
- Rust 2024 edition: no `ref` in match patterns (implicit borrowing)

## Patterns That Work (sendfile path validation)
- `is_sendfile_path_allowed()` canonicalizes first (resolves symlinks + `../`), then checks deny-lists
- `dirs::home_dir()` for portable home detection — `dirs` crate already a dep
- Blocked dirs are relative to `$HOME`: `.ssh`, `.gnupg`, `.aws`, `.kube`, `.docker`, etc.
- Blocked filenames: `id_rsa`, `id_ed25519`, `.env`, `.env.local`, `.env.production`
- Blocked system paths: `/etc/shadow`, `/etc/gshadow`, `/etc/master.passwd`, `/etc/sudoers`
- Non-existent paths fail at `canonicalize()` — counts as blocked (can't verify safety)
- The check runs in `upload_sendfiles()` after exists/is_file but before `fs::read`

## Patterns That Work (proactive agent)
- `run_proactive_prompt()` is like `run_matrix_prompt()` but does NOT update `last_active` or `turn_count` — for heartbeat/trigger prompts that shouldn't prevent idle reaping
- `is_heartbeat_ok()` checks case-insensitive for "HEARTBEAT_OK" or "HEARTBEAT OK" — supports both underscore and space variants
- Trigger pipe uses `libc::mkfifo` directly (libc already a dep) — no need for nix crate
- Trigger pipe reader re-opens FIFO in a loop on EOF (writers come and go)
- `ensure_trigger_pipe()` is called after each Matrix prompt to lazily spawn the reader — avoids needing Matrix client inside `get_or_create()`
- `SessionKey::dir_name()` sanitizes `:`, `@`, `!` for filesystem paths
- `ProactiveConfig` struct passes heartbeat/trigger config from `run_daemon()` into `run_matrix_bridge()`
- Session heartbeat only runs for Matrix sessions (iroh has no persistent back-channel to push responses)
- Heartbeat scheduler skips sessions where HEARTBEAT.md is missing or empty

## Patterns That Work (hypertile BSP tiling)
- `ratatui-hypertile = "0.1"` with `serde` feature — BSP tiling engine replacing custom `PanelLayout`
- `Hypertile` struct owns the BSP tree; `PaneRegistry` maps `PaneId` → `PaneKind` (Chat, Panel(PanelId), Empty)
- Chat pane is `PaneId::ROOT` (id=0), always exists, cannot be closed
- Default layout: `Node::Split` tree matching old 3-column (left 20% Todo/Files, center 50% Chat, right 30% Subagents/Peers)
- `app.apply_tiling_action()` wraps `tiling.apply_action()` + syncs `focused_panel` from hypertile state
- `app.has_panel_focus()` / `app.focus_panel()` / `app.unfocus_panel()` / `app.is_panel_focused()` replace old `FocusTracker`
- `app.sync_focused_panel()` reads `tiling.focused_pane()` → looks up `PaneKind::Panel(id)` → sets `focused_panel`
- Render loop: `tiling.compute_layout(area)` then iterate `tiling.panes()` → dispatch by `PaneKind`
- Mouse hit-testing: iterate `tiling.panes()` checking `rect_contains(pane.rect, col, row)` per pane
- Preset layouts: `default_tiling()`, `focused_tiling()`, `wide_chat_tiling()`, `right_heavy_tiling()` return `(Hypertile, PaneRegistry)` tuples
- Navigation: `h`/`l` → `FocusDirection { Horizontal, Start/End }`, `j`/`k` → `FocusDirection { Vertical, Start/End }`, `Tab` → `FocusNext`
- Deleted `tui/layout.rs` entirely (313 lines) — `PanelLayout`, `FocusTracker`, `ColumnSide` all gone
- 5 flaky PTY tests (`slash_commands`, `slash_menu`) timeout intermittently — pre-existing, not layout regression

## Patterns That Work (per-subagent BSP panes)
- Each subagent gets its own BSP pane via `PaneKind::Subagent(String)` — bypasses the fixed `PanelId` enum entirely
- `SubagentPaneManager` in `subagent_pane.rs` owns all per-subagent state (output lines, scroll, status, PaneId)
- `SubagentPaneManager::create()` calls `tiling.state_mut().allocate_pane_id()` — guaranteed unique, no collision with well-known IDs 0–6
- `auto_split_for_subagent()` in `panes.rs` places new subagent panes by: existing subagent pane (stack vertically) → Subagents overview panel → chat pane (horizontal 75/25)
- `focused_subagent: Option<String>` on App — mutually exclusive with `focused_panel: Option<PanelId>`
- `sync_focused_panel()` updates BOTH `focused_panel` and `focused_subagent` from hypertile state
- BSP utilities (`remove_pane_from_tree`, `insert_pane_beside`, `nodes_equal`) extracted to `panes.rs` — shared by slash commands, subagent auto-split, dismiss
- SubagentEvents route to BOTH the overview `SubagentPanel` (list summary) AND the per-pane `SubagentPaneManager`
- Overview panel Enter key emits `PanelAction::FocusSubagent(id)` to focus the dedicated pane
- Subagent pane keys: `j/k` scroll, `g/G` top/bottom, `x` kill, `q` dismiss (close pane from BSP tree), `Esc` unfocus
- `HitRegion::Subagent(String)` for mouse click-to-focus and scroll-wheel
- `ZoomState` saves/restores `focused_subagent`
- Finished/errored subagent panes stay open until user dismisses with `q` — no auto-cleanup

## Patterns That Work (pane tiling/resize/move)
- Panel-focused keybindings for tiling: `[`/`]` resize, `|`/`-` split, `X` close, `=` equalize, `Shift+H/L/J/K` move/swap
- Chat pane (ROOT) cannot be split or closed — guard checks `pane_registry.is_chat(focused)` before split/close
- `split_focused_pane()` uses `tiling.split_focused()` then registers new pane as `PaneKind::Empty`
- `close_focused_pane()` uses `tiling.close_focused()` then `pane_registry.unregister(removed_id)`
- Leader menu `Space → p` opens pane submenu with all tiling operations as extended actions
- Extended action names: `pane_split_vertical`, `pane_split_horizontal`, `pane_close`, `pane_equalize`, `pane_grow`, `pane_shrink`, `pane_move_{left,right,up,down}`
- `MoveScope::Window` for move/swap — swaps pane IDs geometrically (requires computed layout). `MoveScope::Split` swaps siblings only.
- Render hint on focused panel border: `[]:size |/-:split X:close`

## Patterns That Work (streaming output)
- `StreamingOutput` in `src/tui/components/streaming_output.rs` — per-tool scrollable buffer with head/tail truncation
- `StreamingOutputManager` maps `call_id` → `StreamingOutput`, lives on App
- Both `ToolExecutionUpdate` and `ToolResultChunk` events feed the manager (TUI no longer ignores chunks)
- `render_response_message` takes `&mut StreamingOutputManager` — uses buffer instead of 8-line tail window
- Focused tools show 32 lines (`FOCUSED_OUTPUT_LINES`), unfocused show 8 lines (`LIVE_OUTPUT_MAX_LINES`)
- Stats footer appears when output exceeds compact view or tool is focused
- `focused_tool: Option<String>` on App — mutually exclusive with `focused_panel` and `focused_subagent`
- Focus/unfocus methods on App: `focus_tool(call_id)`, `unfocus_tool()` — clear other focus types
- Key dispatch: j/k scroll, g/G top/bottom, f toggle auto-follow, q/Esc unfocus
- Status bar shows `🔧 tool_name (N lines)` or `🔧 X tools (N lines)` during execution
- `StreamingConfig` defaults: max_lines=2000, head=200, tail=200, visible=16
- `render_blocks` now takes `&mut StreamingOutputManager` — passed through all render functions
- All 5 render/block functions updated: `render_blocks`, `render_conversation_block`, `render_active_block`, `render_response_message`, plus the tests

## Patterns That Work (panel scroll infrastructure)
- `PanelScroll` struct in `panel.rs`: offset, content_height, visible_height, scroll_up/down/set_dimensions
- Panel trait: `panel_scroll()` / `panel_scroll_mut()` return `Option<&PanelScroll>` (default None)
- Default `handle_scroll` uses `panel_scroll_mut()` — panels get mouse wheel for free by implementing 2 methods
- `content()` method: return `Option<Vec<Line>>` — if Some, `draw()` default renders with auto-scroll
- `draw_panel_scrolled()` updates dimensions + applies scroll offset — called from render loop
- Render loop clones theme to avoid borrow conflict between `&app.theme` and `app.panel_mut()`
- ListNav panels (todo, process, peers, branch) keep their own `handle_scroll` override — selection-based scroll is better UX

## Patterns That Work (branch panel)
- `BranchPanel` implements `Panel` trait at `src/tui/components/branch_panel.rs`
- `PanelId::Branches` added to panel registry, layout, and App
- Panel hidden by default: `PanelSlot::with_weight(PanelId::Branches, 0)` in default layout
- Toggle action uses `panel_layout.toggle_panel()` to show/hide + `focus.focus()` to activate
- Leaf detection: blocks with no children (via `has_children` HashSet from `parent_block_id`)
- Branch entries auto-refresh when `branch_panel.entries` is non-empty (lazy — only after first open)
- `PanelAction::SlashCommand(format!("/switch #{}", leaf_id))` bridges panel → slash command system
- `ListNav` from `panel.rs` handles wrapping selection, scroll offset, prefix spans

## Patterns That Work (session popup tree)
- `render_tree_node()` does DFS over `all_blocks` (not `blocks`) to show ALL branches
- Active path = blocks in `app.blocks` (HashSet<usize> for O(1) lookup)
- Active blocks get cyan `*` marker + full color text; inactive get DarkGray + DIM
- Tree connectors: `├─` (has next sibling), `└─` (last child), `│ ` (continuing parent)
- Child prefix: `"   "` if parent was last child, `"│  "` if parent has more siblings
- Root blocks get no connector/prefix (empty string)
- `BlockBranchInfo` is `Clone` not `Copy` — has `Vec<(usize, String, bool)>` for child_branch_previews
- `child_branch_previews` populated in render.rs from `all_blocks` filtered by `parent_block_id`
- `truncate_preview()` takes first line only, then truncates to max chars

## Patterns That Work (slash command handlers)
- `SlashContext<'a>` wraps `&'a mut App` + all other params — single struct passed to every handler
- `SlashHandler` trait: `fn handle(&self, args: &str, ctx: &mut SlashContext<'_>)` — each handler struct implements it
- `dispatch()` in `slash_commands/mod.rs` is a compact 38-line match routing `SlashAction` → handler
- `execute_slash_command()` is now a 10-line thin wrapper constructing `SlashContext` and calling `dispatch()`
- Handler files organized by domain in `src/slash_commands/handlers/` — 13 files, ~2,100 lines total
- Helpers like `parse_oauth_input`, `format_time_ago`, `resume_session_from_file` made `pub(crate)` in interactive.rs
- `AgentCommand` enum made `pub(crate)` so handlers can send agent commands
- In handler bodies: `ctx.app`, `ctx.cmd_tx`, `ctx.plugin_manager`, `ctx.panel_tx`, `ctx.db`, `ctx.session_manager`
- Watch for `if let Some(db) = &ctx.db` pattern — the inner `db` is a local binding, NOT `ctx.db`

## Patterns That Work (dynamic registry)
- `src/registry.rs` holds shared `PRIORITY_BUILTIN/PLUGIN/USER` constants + `Conflict` struct
- `MenuContributor` trait: `fn menu_items(&self) -> Vec<MenuContribution>` — builtins, plugins, user config all implement it
- `LeaderMenu::build(contributors, hidden)` collects, deduplicates by `(key, placement)`, highest priority wins, returns `(LeaderMenu, Vec<Conflict>)`
- `BuiltinKeymapContributor` replaces the old hardcoded `LeaderMenu::new()` — produces identical menu
- `LeaderMenu::new()` still works (calls `build` with just builtins) for backward compat in tests
- `App::rebuild_leader_menu()` takes plugin_manager + settings, locks PM mutex, collects all contributors
- `PluginManifest.leader_menu: Vec<PluginLeaderEntry>` — plugins declare menu entries in plugin.json
- `PluginManager` impl `MenuContributor` validates entries (ascii key, non-empty label, command starts with `/`)
- `Settings.leader_menu: LeaderMenuConfig` with `items` (add/override) and `hide` (remove) — `hidden_set()` converts to `HashSet<(char, MenuPlacement)>`
- `SlashCommand.leader_key: Option<LeaderBinding>` field added (all `None` for now, ready for Phase 2)
- Python script to bulk-add `leader_key: None,` to 37 SlashCommand literals: count brackets in `subcommands: vec![...]` to find closing line

## Patterns That Work (git2 in-process)
- `git2 = "0.20"` replaces shell-outs in tools (commit, review) and worktree module
- `git_ops.rs` has async wrappers (for tools) and `git_ops::sync` module (for worktree)
- `git2::Repository::worktree()` does NOT create parent dirs — must `create_dir_all` first
- `glob = "0.3"` for branch pattern matching (`list_branches`, `list_merged_branches`)
- Newer git defaults to `main` not `master` — tests must use `git init -b main`
- "Merged" check: `merge_base(branch_tip, HEAD) == branch_tip` means branch is merged
- `git gc` has no libgit2 equivalent — keep the shell-out (runs rarely, non-critical)
- `WorktreePruneOptions` needs `.working_tree(true).valid(true).locked(true)` for force remove
- `worktree_list` works by iterating `repo.worktrees()` names, opening each with `find_worktree`
- `diff_name_only` resolves two refs to trees and iterates `diff.get_delta()` for paths
- Test repos in session_bridge.rs still use `Command::new("git")` for setup — that's fine (test helpers)

## Patterns That Work (nix tool + Landlock)
- Nix daemon socket at `/nix/var/nix/daemon-socket/socket` needs **write** access for `connect()` — Landlock `/nix` as RO blocks it
- Fix: add nix-specific RW paths (`/nix/var/nix/daemon-socket`, `~/.cache/nix`, `~/.local/state/nix`) before the broad `/nix` RO rule — Landlock merges (union) permissions
- `nix build --log-format internal-json -L` produces `@nix {...}` JSON lines on stderr — existing parser in `nix.rs` handles activities, build logs, progress, phases
- `nom` (nix-output-monitor) was evaluated as wrapper but **rejected**: it emits TUI cursor control codes (`[1G`, `[2K`, `[1F`, `[?25l`) and box-drawing chars even when piped or `TERM=dumb` — cannot stream line-by-line
- `nix-bindings-rust` (cachix FFI) and `snix` (tvix as library) were evaluated — both too immature; `Store::realise()` is blocking with no build-log streaming callback; snix not on crates.io
- For streaming nix builds to subagent panes: internal-json parser is the right approach

## Domain Notes
- JMAP (RFC 8620/8621): pure HTTP+JSON email protocol, Fastmail is reference impl
- JMAP flow: GET /jmap/session → accountId + identityId, then POST /jmap/api/ with methodCalls
- Fastmail API tokens from Settings → Privacy & Security → API Tokens
- Sandbox `Permission::Net` exists in enum but is NOT enforced — `load_wasm` ignores permissions
- `host.rs` is a stub — `HostFunctions` struct does nothing, just lists UI action names
- matrix-sdk 0.9 `Room::typing_notice(bool)` takes a plain bool, not a Typing enum
- matrix-sdk 0.9 `Room::send_attachment(filename, content_type, data, AttachmentConfig::new())` for file upload
- matrix-sdk 0.9 `client.media().get_media_content(&MediaRequestParameters { source, format: MediaFormat::File }, true)` for download
- `ClankersEvent::Text` now has `room_id: String` — bridge no longer loses room context
- Image/File/Audio/Video MessageTypes map to `ClankersEvent::Media` with media_type string field
- `<sendfile>/path</sendfile>` tags in agent response are extracted, uploaded to Matrix, and stripped from text
- Use `clankers_matrix::ruma` (re-exported) in daemon.rs, NOT `matrix_sdk::ruma` directly
- `DaemonConfig` construction in main.rs — use `..Default::default()` for new fields to avoid breaking existing call sites
- Matrix bridge: BridgeEvent::TextMessage and ChatMessage can be unified with `|` pattern in match arms
- Worker delegates don't always persist file edits — verify changes after delegation

## Patterns That Work (TUI extraction)
- `clankers-tui-types` crate at `crates/clankers-tui-types/` — shared boundary types with no ratatui dep
- `clankers-tui` crate at `crates/clankers-tui/` — full TUI crate with all 64 files, depends on ratatui/crossterm/hypertile
- Main crate: `pub use clankers_tui as tui;` in lib.rs — zero API change for callers using `crate::tui::`
- Re-export pattern: original locations do `pub use clankers_tui_types::TypeName;` for backward compat
- External files (tools, modes, slash_commands) import directly from `clankers_tui_types::` 
- Types with ratatui deps (TodoStatus.color(), ListNav.prefix_span()) stay in TUI crate, not types crate
- InputMode needs `Serialize`/`Deserialize` — add derives in the types crate, not orphan impls in main crate
- `parse_action()` moved to types crate alongside Action/CoreAction/ExtendedAction (no external deps)
- Crate extraction: `pub(crate)` items accessed from main crate must become `pub` — found 23 items needing promotion
- Tests referencing main-crate types (e.g., `crate::slash_commands`) cannot live in the TUI crate — move to main crate
- `crate::tui::` → `crate::` sed replacement is safe (all external refs were eliminated in Phase 5)
- Git detects file moves as renames when content changes are minimal (< ~20% diff)

## Patterns That Work (decomposition round 2)
- interactive.rs decomposition: extract setup_session, build_agent_with_tools, agent command spawn → 3 new modules. interactive.rs drops from 941→534 lines.
- event_handlers.rs decomposition: extract handle_core_action() and handle_extended_action() → 2 new modules. event_handlers.rs drops from 933→467 lines.
- event_loop_runner/mod.rs: extract AuditTracker and loop_mode methods → 2 new modules. mod.rs drops from 714→563 lines.
- The agent command `tokio::spawn` block was 180 lines of nested match — extracting to agent_task.rs with per-command helper functions keeps each function under 70 lines.
- `run_prompt_with_abort()` generic over Future: same abort-during-streaming pattern used for both prompt() and prompt_with_images()
- Key insight: cli.rs (763 lines) is all clap derive types — declarative data, not logic. Splitting subcommand enums into separate files would be anti-ergonomic. Left it alone.

## Patterns That Work (Tiger Style hardening)
- Session tree traversals (walk_branch, find_latest_leaf, find_all_leaves): bounded by MAX_TRAVERSAL_DEPTH (50K) with cycle detection via visited set
- find_all_leaves: converted recursive DFS to iterative DFS with explicit stack — eliminates stack overflow risk on deep trees
- BreakCondition::check: added depth-bounded recursion via check_bounded() for Any/All nesting (16 levels max)
- parse_duration_secs: checked_mul + 365-day maximum prevents overflow in schedule tool
- parse_datetime: i64::try_from(secs) replaces bare `as i64` cast for defense-in-depth
- estimate_cost: debug_assert on rate signs + is_finite() check prevents NaN propagation
- AuditTracker: MAX_PENDING_CALLS (1024) warns on leaked tool calls; saturating u128→u64 for duration
- parse_oauth_input: MAX_INPUT_LEN (4096) rejects oversized payloads before parsing
- parse_command: MAX_COMMAND_NAME_LEN (64) rejects absurdly long command names
- format_time_ago: clamps negative durations (future timestamps → "just now")
- Verus specs scaffolded in clankers-loop/verus/ — proofs for loop state machine invariants (bounded termination, monotonic transitions, well-formedness). Pseudo-code until Verus is installed.
- Compile-time assertions: `const _: () = assert!(...)` for all Tiger Style constants

## Patterns That Work (code quality cleanup)
- CODE_ANALYSIS_REPORT claimed `serde_json` import was unused in settings.rs — wrong, it's used for `Value`, `json!()`, `from_str`, `from_value`
- CODE_ANALYSIS_REPORT claimed 3 duplicate settings merging functions — wrong, `merge_layers` orchestrates and `merge_into` does field-level merge, they're complementary
- `helix_normal_nav()` and `vim_normal_nav()` were literally identical (same key→action map, different entry ordering) — safe to unify
- Plugin WASM tests (89 tests) fail in worktrees because they need pre-built .wasm binaries — skip with `--skip plugin::tests`
- system_prompt.rs at 727 lines is fine — 350 impl + 377 thorough tests, well-decomposed already. Not every big file needs splitting.
- event_handlers.rs is fundamentally a big match statement routing actions — limited decomposition value beyond helper extraction for repeated patterns

## Patterns That Work (crate extraction: message + session)
- `clankers-message` crate at `crates/clankers-message/` — AgentMessage, Content, MessageId, StreamEvent, etc.
- `clankers-session` crate at `crates/clankers-session/` — SessionManager, SessionTree, store, merge, export
- Message types used by 31+ files — extracting them first unlocked session extraction
- `generate_id()` inlined from util::id into clankers-message (6 lines, avoids util dep), made `pub`
- `MessageId::generate()` now calls clankers-message's own `generate_id()`
- `to_merge_view()` stayed in main crate at `src/session/merge_view.rs` — bridges session↔tui-types
- `SessionError` uses simple struct like `DbError` — not snafu/thiserror, just `Display + Error`
- `From<SessionError> for crate::Error` in error.rs maps to `Error::Session`
- Store.rs replaced `snafu::ResultExt` with `.map_err(session_err)` — cleaner for extracted crate
- `pub(super)` in tree module became `pub(crate)` for crate-internal visibility (find_message, children)
- `set_message_id()` promoted to `pub` in session crate (needed by merge.rs)
- Re-export pattern: `src/session/mod.rs` does `pub use clankers_session::*;` + `pub mod merge_view;`
- `env!("CARGO_PKG_VERSION")` in SessionManager::create() gets session crate version — same as main (both 0.1.0)
- Git detects file moves as renames when content is mostly unchanged — most session files showed as `R` (rename)

## Patterns That Work (TUI extraction round 2)
- Keybindings engine (Keymap, KeyCombo, presets, defaults, parser) moved to `crates/clankers-tui/src/keymap/` — main crate keeps only `KeymapConfig` (settings-layer loading) + re-exports
- `pub use clankers_tui::keymap::*` in `src/config/keybindings/mod.rs` preserves all import paths
- Mouse and clipboard modules move directly to TUI crate — zero backend deps, only needed `pub(crate)` → `pub` visibility change
- Selectors with backend side-effects (model, account, session) return `(bool, Option<SelectorAction>)` instead of taking `cmd_tx` channel
- `SelectorAction` enum in `clankers-tui-types` covers `SetModel`, `SwitchAccount`, `ResumeSession` — the event loop maps these to `AgentCommand` in `dispatch_selector_action()`
- Selectors with only TUI side-effects (branch switcher, branch compare, merge interactive) keep simple `bool` return — no abstraction needed
- `ansi.rs` was NOT a good extraction candidate: `ansi_to_spans`/`ansi_to_lines` are dead outside their own tests, and `strip_ansi` callers are tools (not TUI code)
- Always check who actually calls a function before deciding to move it — grep for callers, not just the function definition

## Patterns That Work (TUI snapshot/screenshot testing)
- `insta` for text snapshots: structure-based extraction (`extract_structure()`) isolates panel borders, titles, status bar, input area — ignores volatile message content
- PTY harness screenshots: `vt100::Parser` → `ScreenCapture::from_pty()` → `render_screenshot()` → PNG with embedded 8×16 VGA font
- tmux harness: `tmux new-session -d -s NAME -x COLS -y ROWS` → `send-keys -l` for literals → `capture-pane -p` (text) / `capture-pane -e -p` (ANSI)
- Worktree line ("Worktree: clankers/main-HASH") appears/disappears depending on git session state — must normalize or use structure snapshots
- Status bar contains timing-dependent artifacts (cursor chars from previous commands) — strip single chars between `|` and border `│`
- `\s*([│┘┐┤└])` normalizes whitespace before border chars to single space — catches 0-space vs 1-space differences
- PTY-based screenshots are cleaner than tmux screenshots because vt100 parser has full state; tmux captures only emit ANSI escape sequences
- Visual tests save PNG screenshots to `tests/tui/captures/` (gitignored) and ANSI captures to same dir
- Snapshot files in `tests/tui/snapshots/` (tracked) — review with `cargo insta review`
- Model name normalized to `MODEL` in structure snapshots to avoid breaks when switching default model
- `normalize_screen_text()` replaces git counters, token counts, worktree IDs, commit hashes, model names

## Patterns That Work (new crate creation)
- New crates follow workspace convention: `edition.workspace = true`, `license.workspace = true`, workspace deps for serde/chrono/tokio/etc.
- `parking_lot` for `Mutex` in shared state (not `std::sync::Mutex`) — matches all other crates
- `broadcast::channel` for event dispatch — same pattern as AgentEvent, SubagentEvent
- `CancellationToken` from tokio-util for stopping background loops — same as agent turn loop
- Schedule test timing: pin `created_at` to a fixed past time to avoid sub-millisecond drift between `Utc::now()` and struct construction
- `chrono::Duration::from_millis` doesn't exist — use `std::time::Duration::from_millis(N)` for tick intervals
- Clippy catches: `value % n == 0` -> `value.is_multiple_of(n)`, nested `if let` + `if` -> collapsed `if let && ...`, manual `strip_prefix`/`strip_suffix`
- Fixed loop count check must come BEFORE generic max_iterations check — otherwise fixed loops get `Stopped` instead of `Completed`

## Patterns That Work (crate extraction batch 3)
- `clankers-prompts` at `crates/clankers-prompts/` — prompt template discovery (zero crate deps, serde+std only)
- `clankers-skills` at `crates/clankers-skills/` — skill directory scanning (zero crate deps, serde+std only)
- `clankers-plugin` at `crates/clankers-plugin/` — plugin manager core, manifest, sandbox, host, bridge, UI
- After extracting prompts/skills with only 1-3 callers each: eliminate the `src/prompts/` and `src/skills/` directories entirely, callers import directly from crate
- `ToolResult`/`ToolResultContent` canonical home is `clankers-message` (message protocol types), re-exported from `tools/mod.rs`
- `ResultChunk`/`TruncationConfig`/`ToolResultAccumulator` canonical home is `clankers-message` (result streaming protocol)
- `ProgressKind`/`ToolProgress` canonical home is `clankers-tui-types`, re-exported from `tools/progress.rs`
- `tools/progress.rs` is now pure re-exports (14 lines) — all type defs live in their canonical crates
- Plugin crate bridge: `PluginEvent::matches_event_kind(&str)` decoupled from AgentEvent
- Plugin contrib wrappers: `PluginMenuContributor<'a>` and `PluginSlashContributor<'a>` for orphan-rule compliance
- `PluginManager::active_plugin_infos()` iterator replaces direct `.plugins` field access
- `PluginManager::inject_instance()` + `get_mut()` for test helpers (replaces private field access)
- `AgentEvent::event_kind()` returns `&'static str` tag for plugin matching — empty string for events plugins don't subscribe to
- `registry.rs` eliminated (was 4 re-exports from `clankers-tui-types`) — callers import directly

## Patterns That Work (plugin system maturity)
- `filter_ui_actions()` gates on `ui` permission — strips UI actions from plugins without it, logs a warning
- `catch_unwind(AssertUnwindSafe(...))` in `call_plugin` isolates WASM panics per-plugin
- All plugin mutex locks use `unwrap_or_else(|p| p.into_inner())` — poison recovery everywhere
- `PluginManager::disable()` removes WASM instance + sets state; `enable()` re-loads WASM
- Disabled plugins persisted to `~/.config/clankers/disabled-plugins.json` via `save_disabled_plugins()`
- `init_plugin_manager()` skips loading WASM for plugins in the disabled set
- Host functions (`host.rs`) use permission checks per-call: `read_file`/`list_dir` need `fs:read`, `write_file` needs `fs:write`
- `process_host_calls()` parses `"host_calls"` array from plugin JSON responses — request-based host interaction
- `MessageUpdate` event dispatch was missing — just needed one more match arm in `dispatch_event_to_plugins`

## Patterns That Work (crate extraction: agent)
- `clankers-agent` at `crates/clankers-agent/` — Agent struct, AgentEvent, Tool trait, ToolContext, turn loop, compaction, builder, TTSR, system_prompt, context
- Tool trait + ToolContext moved FROM `src/tools/mod.rs` TO `clankers-agent::tool` — tool impls in main crate depend on `clankers-agent`
- `AgentEvent` moved to `clankers-agent::events` — all type deps (Usage, AgentMessage, ToolResult, ToolProgress, ProcessMeta) were already in extracted crates
- `ModelSwitchSlot` type alias + `model_switch_slot()` constructor in `clankers-agent::tool` — shared between agent turn loop and switch_model tool
- `AgentError` enum: Cancelled, ProviderStreaming, Agent — `From<AgentError> for Error` in main crate's error.rs
- `Agent::prompt()` returns `Result<(), AgentError>` — callers in main crate convert with `.map_err(Error::from)` or match on `AgentError::Cancelled`
- `PathPolicy` + `check_path` + `init_policy` moved to `clankers-util::path_policy` — decouples agent crate from sandbox
- `src/tools/sandbox/policy.rs` keeps only env sanitization (`sanitized_env`), path policy re-exported from `clankers-util`
- `src/agent/mod.rs` is 12-line re-export file: `pub use clankers_agent::*; pub use clankers_agent::{builder, compaction, ...};`
- `src/tools/mod.rs` re-exports `Tool`, `ToolContext`, `ToolResult`, etc. from `clankers_agent::tool` — zero API change for callers
- `openspec` feature flag on `clankers-agent` controls `clankers-specs` optional dep (same pattern as main crate)
- Git detects file moves as renames when content changes are minimal (< ~20% diff)

## Patterns That Work (loop system)
- Loop iteration/break logic lives in `clankers-loop` crate (`LoopEngine`, `BreakCondition`, `LoopDef`)
- `EventLoopRunner` owns a `LoopEngine` + `active_loop_id: Option<LoopId>` — single source of truth for loop state
- `/loop` slash command writes to `app.loop_status` (display-only projection); `EventLoopRunner` lazily registers with engine on first iteration via `ensure_loop_registered()`
- `parse_break_condition()` is the canonical parser in `clankers-loop` — supports `contains:`, `exit:`, `not_contains:`, `equals:`, `regex:`, bare text
- `signal_loop_success` tool call → `engine.signal_break(id)` → engine sets internal flag consumed by next `record_iteration()`
- `maybe_continue_loop()` feeds accumulated tool output to `engine.record_iteration()` which handles break conditions + max iterations
- `app.loop_status.iteration` synced from `engine.get(id).current_iteration` after each iteration for TUI display
- `/loop stop` clears `app.loop_status`; `maybe_continue_loop` detects the mismatch and cleans up engine state
- Pause: `ls.active = false` → `maybe_continue_loop()` returns early without re-sending; resume: toggle back + send prompt from slash handler
- `LoopTool` (wraps LoopEngine for shell commands) exists but is NOT registered in production — only in tests
- Leader menu loop submenu: `Space → L → p/s/i` for pause/stop/status — uses `LeaderAction::SlashCommand` to dispatch

## Patterns That Work (hook system)
- `clankers-hooks` at `crates/clankers-hooks/` — leaf crate, no deps on other clankers crates
- `HookPoint` enum: PrePrompt, PostPrompt, PreTool, PostTool, PreCommit, PostCommit, SessionStart, SessionEnd, TurnStart, TurnEnd, ModelChange, OnError
- `HookPayload` with tagged `HookData` enum (Tool, Prompt, Session, Git, Error, ModelChange, Empty)
- `HookVerdict`: Continue, Modify(Value), Deny{reason} — merge logic: Deny > Modify > Continue
- `HookPipeline` dispatches to registered `HookHandler` impls sorted by priority (lower = first)
- Priority constants: PRIORITY_GIT_HOOKS=100, PRIORITY_SCRIPT_HOOKS=200, PRIORITY_PLUGIN_HOOKS=300
- `ScriptHookHandler`: runs executables from .clankers/hooks/<hook-name>, JSON on stdin, env vars for context
- Script exit 0 = Continue, non-zero = Deny (for pre-hooks), stdout JSON = Modify
- `GitHookHandler`: runs .git/hooks/pre-commit and post-commit, standard git hook protocol
- `install_hook_shim()` / `uninstall_hook_shim()` manage clankers shims with backup/restore
- `PluginHookHandler`: wraps PluginManager as HookHandler, dispatches to WASM plugins via spawn_blocking
- `ToolContext.with_hooks(pipeline, session_id)` attaches pipeline for pre/post tool hooks in execution.rs
- Pre-tool hook fires BEFORE tool.execute(), can deny with error result or modify input JSON
- Post-tool hook fires after tool.execute() via fire_async (fire-and-forget)
- EventLoopRunner fires async hooks for SessionStart, SessionEnd, TurnStart, TurnEnd, ModelChange
- CommitTool fires PreCommit before git_ops::commit(), PostCommit after success
- `AgentEvent::event_kind()` expanded: session_start, session_end, tool_execution_start, model_change, usage_update, session_branch, session_compaction, user_cancel
- `PluginEvent` expanded from 9 to 17 variants — all new events are additive (backward compat)
- `HooksConfig` in settings.json: enabled (default true), hooks_dir, disabled_hooks, script_timeout_secs (default 10), manage_git_hooks (default false)
- `/hooks` slash command: status, list (shows installed scripts), install-git, uninstall-git
- `SlashContext` doesn't have settings — use `load_hooks_config()` helper (loads from merged settings files)

## Patterns That Work (Tiger Style modularization round 3)
- commands/rpc.rs: 591-line `run()` → 14 handler functions. Key: `parse_node_id()`, `truncate_id()`, `rpc_error()`, `parse_capabilities()` shared helpers eliminate duplication
- NixOutputState struct groups 5 mutable params into a single bundle — callers write `state.messages` not 5 separate args
- `push_bounded(vec, item, max)` drops 10% when full — amortizes O(n) drain instead of shifting on every push
- `emit_deduped(ctx, last, msg)` eliminates repeated 3-line dedup pattern in nix parser
- `collect_line()` / `drain_reader()` in bash.rs: eliminated 4x copy-paste of 7-line "process a line" block
- `search_file_into()` in grep.rs: shared between single-file and walker paths (eliminated `search_single_file`)
- LoopTool: `RunParams` struct + `parse_run_params()` pure function, `run_loop_iterations()` async body, `format_loop_result()` pure output
- process_agent_event decomposed to: `record_usage()`, `process_tool_events()`, `dispatch_to_plugins()`, `fire_lifecycle_hooks()`
- Daemon `run_daemon` decomposed into: `build_endpoint()`, `build_rpc_state()`, `spawn_*()` family (5 spawners)
- Plugin `/plugin` handler: generic `plugin_toggle(pm, name, enable, ctx)` handles both enable/disable with one function
- `plugin_show` uses `join_or_none()` closure to eliminate 4x identical `if empty { "none" } else { join }` blocks
- Assertion density in daemon/session_store.rs: evict_oldest pre/post, max_sessions invariant, filter_tools output bound
- Functions over 70 lines in src/: 41 → 31 (eliminated 10)
- Remaining 31 are mostly: test functions (270-line translator test), pure match routers (translate/event_handlers), setup functions (run_interactive), or linear builders (landlock rules)
- cli.rs at 763 lines is all declarative clap derive — splitting would reduce ergonomics with no safety gain

## Patterns That Work (token efficiency)
- `OutputTruncationConfig` in clankers-loop, applied in turn loop via `apply_output_truncation()` after `execute_tools_parallel()` returns
- `ToolTier` enum (Core/Orchestration/Specialty/Matrix) + `ToolSet` struct — `active_tools()` filters by tier, `all_tools()` returns everything
- `build_tiered_tools()` replaces `build_tools_with_env()` as canonical builder; backward-compat wrapper kept
- `build_all_tiered_tools()` adds plugin tools as Specialty tier
- Interactive: Core+Specialty+Orchestration (no Matrix). Headless: Core+Specialty. Daemon/RPC: all tiers.
- `--tools` flag accepts tier names (core, orchestration, specialty, matrix), "all", "core", "none", or comma-separated tool names
- `PromptFeatures` struct controls which system prompt sections are included; `build_default_system_prompt()` is the conditional builder
- `detect_nix()` runs `which nix` once at startup; result wired into PromptFeatures
- `default_system_prompt()` changed from `-> &'static str` to `-> String` — callers with `.to_string()` still work (redundant but harmless)
- `TurnConfig.output_truncation` field added; `Agent::output_truncation_config()` derives it from Settings
- AgentConfig now has optional `tiers: Option<Vec<String>>` field in frontmatter

## Patterns That Work (decomposition round 3 — large file splits)
- validate_tui.rs (692→362): PtyHarness + key_bytes → `pty_harness.rs`, tests → `validate_tui_tests.rs`, extract `execute_action()` + `check_assertions()` from `run_tui_test()`
- git_ops/mod.rs (686→260): diff functions → `diff.rs`, log/time → `log.rs`, tests → `tests.rs`. Shared types (GitError, Result, open_repo) stay in mod.rs
- sync_ops.rs (672→368): tests → `sync_ops_tests.rs` via `#[path = "sync_ops_tests.rs"]` attribute (non-mod.rs can't use bare `mod tests;`)
- commands/rpc.rs (731): already well-decomposed (20+ small handlers), no further split needed
- `#[path = "filename.rs"] #[cfg(test)] mod tests;` is the cleanest way to extract tests from non-mod.rs files
- TUI snapshots are environment-sensitive (git identity, terminal state) — use `cargo insta test --accept` in non-interactive environments, not `cargo insta review`

## Patterns That Work (auto-test)
- `Settings.auto_test_command: Option<String>` — persistent config in settings.json (`"autoTestCommand": "cargo nextest run"`)
- `App.auto_test_enabled: bool` + `App.auto_test_command: Option<String>` — runtime state, initialized from settings in interactive.rs
- `EventLoopRunner.auto_test_in_progress: bool` — prevents recursive triggers (auto-test turn → PromptDone → would trigger another auto-test)
- Auto-test fires in `handle_task_results()` on `PromptDone(None)` when no queued prompt and no active loop
- Sends a prompt to the agent so it sees test results and can fix failures
- `/autotest` slash command: toggle on/off, `set <cmd>`, `status`
- Adding a new slash command shifts visual snapshots (use `cargo insta accept --all`); count assertions now use `builtin_commands().len()` so they self-adjust
- Leader menu entries: add to `root_keymap_actions()` in builder.rs, register the `ExtendedAction` in actions.rs (enum + name table), dispatch in extended_actions.rs, mark global in event_handlers.rs
- Fixed 3 hardcoded `45` counts in slash_commands/tests.rs → `builtin_commands().len()` — no more breakage on command add/remove

## Domain Notes (daemon-client architecture)
- rkyv rejected for wire protocol: wrong tool (small text messages, not large structs), loses debuggability, versioning pain on enum changes
- Lunatic rejected as actor foundation: WASM process model mismatches native agent resources (HTTP clients, file handles, Arc providers). Wasmtime 41 conflicts with extism's wasmtime 37. Only ~400 lines of lunatic's process model are relevant.
- Native actor layer: steal Signal/Link/Monitor concepts from lunatic, implement on tokio tasks. ~500 lines, no WASM dependency.
- Protocol: serde_json + length-prefixed frames. Reuse existing write_frame/read_frame from iroh RPC.
- Transport: Unix domain sockets (local), iroh QUIC (remote). One control socket + one socket per session.
- SessionController: transport-agnostic agent orchestrator extracted from EventLoopRunner. Owns agent, session mgr, loop engine, hooks, audit.
- Embedded mode preserved as default. `--daemon` and `attach` are opt-in.
- Slash commands split: agent-side (model, session, auth) via SessionCommand, client-side (zoom, panel, theme) handled locally.
- OpenSpec for this work lives at `openspec/changes/daemon-client/`
- Automerge for session tree: session is already an append-only DAG with unique IDs + parent pointers — that's what Automerge stores natively. Eliminates ~300 lines of manual merge/cherry-pick code. Enables concurrent writes from multiple agents/clients.
- Automerge for todo list: eliminates todo_tx/todo_rx channel pair. Agent writes to doc, TUI reads from local replica. No oneshot round-trip.
- Automerge for napkin: concurrent corrections from multiple agents merge without file conflicts.
- Automerge NOT for: settings (LWW is correct), auth tokens (need immediate authority), streaming output (ephemeral, high-frequency).
- iroh-docs syncs Automerge documents between daemon and TUI clients. Complementary to DaemonEvent stream — events for real-time rendering, iroh-docs for persistence consistency.
- Aspen backend: SessionBackend trait with local (Automerge files) and aspen (KV+blobs) impls. Agent work as aspen jobs for distributed execution. Opt-in via --backend aspen.

## Patterns That Work (daemon-client Phase 0-2)
- `clankers-protocol` at `crates/clankers-protocol/` — wire types only, deps: serde + serde_json + tokio (for AsyncRead/AsyncWrite in frame helpers)
- `write_frame`/`read_frame` generic over `AsyncWrite`/`AsyncRead` — no transport dependency, works with UnixStream, QUIC streams, `Vec<u8>`, `Cursor`
- `SessionCommand` and `DaemonEvent` enums with full serde Serialize+Deserialize round-trip tests for every variant
- `ControlCommand`/`ControlResponse` for control socket (session listing, creation, attach)
- `Handshake` struct with protocol_version, client_name, optional token+session_id
- `FrameError` enum: Io, TooLarge, Json, Eof — UnexpectedEof maps to Eof for clean disconnect detection
- MAX_FRAME_SIZE = 10MB, validated on both read and write
- `clankers-actor` at `crates/clankers-actor/` — native tokio tasks, NOT WASM. Deps: tokio + dashmap + tracing only
- `ProcessId = u64`, monotonic via AtomicU64, never reused
- `Signal` enum uses `Box<dyn Any + Send>` for Message — no generic type parameter on the enum itself
- `ProcessRegistry` uses DashMap (thread-safe) for processes + names + links + monitors
- `registry.spawn()` takes a factory closure `FnOnce(ProcessId, UnboundedReceiver<Signal>) -> Future<Output = DeathReason>` — actor receives its own ID and signal channel
- Death notifications: linked processes get `LinkDied`, monitors get `ProcessDied` — via `on_process_exit()` callback in the spawned task wrapper
- `Supervisor` is strategy + restart rate tracking, NOT a running task itself — `run()` method takes signal_rx and restart_fn
- `clankers-controller` at `crates/clankers-controller/` — owns Agent, SessionManager, LoopEngine, HookPipeline, AuditTracker
- `ConfirmStore<T>` generic over response type — handles both bash confirms (bool) and todo responses (Value)
- `agent_event_to_daemon_event()` and `daemon_event_to_tui_event()` are the two conversion points — AgentEvent → DaemonEvent → TuiEvent
- `Content::Image { source: ImageSource::Base64 { media_type, data } }` — not flat fields
- `ToolProgress` has `kind: ProgressKind, message, timestamp: Instant` — Instant is not serializable, convert to JSON with message only
- Controller test mock: inline `MockProvider` struct with `#[async_trait]` — no public mock in clankers-provider
- Provider::complete return type: `clankers_provider::error::Result<()>` (not `clankers_provider::Result`)

## Patterns That Work (daemon-client Phase 3 — wiring)
- Controller gaps closed: SetThinkingLevel parses via `ThinkingLevel::from_str_or_budget`, SeedMessages converts to AgentMessage, auto-test via `maybe_auto_test()`/`clear_auto_test()`, ModelChange hook fires via `HookPayload::model_change()` (new constructor)
- `socket_bridge.rs` in `src/modes/daemon/` bridges clankers-controller's transport layer into the daemon
- `SessionFactory` holds provider/tools/settings/model/prompt — shared resources for creating SessionController instances
- `run_control_socket_with_factory()` replaces `run_control_socket()` — handles CreateSession by building Agent → ControllerConfig → SessionController
- `run_session_driver()` is the per-session event loop: reads SessionCommand from mpsc channel, feeds to controller, drains DaemonEvent via `drain_events()`, broadcasts to connected clients
- Session driver uses `tokio::select!` with 50ms sleep for background event draining (tool execution events arrive asynchronously)
- Control socket spawned alongside existing iroh/Matrix in `run_daemon()` via `spawn_socket_control_plane()`
- `daemon-sessions` CLI subcommand (list, status, create, kill, shutdown) talks to control socket
- `send_control()` helper: connect UnixStream → write_frame(ControlCommand) → read_frame(ControlResponse)
- Integration tests: `tests/socket_bridge.rs` exercises full round-trip (control connect → create session → session socket handshake → GetSystemPrompt → SystemPromptResponse → disconnect)
- Test isolation: `set_test_socket_dir()` sets XDG_RUNTIME_DIR to tempdir, must `create_dir_all(socket_dir())` before starting control socket
- `std::env::set_var` is unsafe in Rust 2024 — wrap in `unsafe {}` block with SAFETY comment
- Existing iroh chat/1, rpc/1, and Matrix transports completely unchanged — socket layer is additive
## Patterns That Work (TUI attach mode — Phase 4)
- `clankers attach [session-id]` — top-level CLI command, not a DaemonSessions subcommand (launches full TUI)
- `--new` flag creates a new session via CreateSession before attaching; `--model` sets the model for new sessions
- No session_id given: lists sessions via control socket, auto-attaches to the first one
- `src/modes/attach.rs` — ~950 lines, self-contained module with `run_attach()` entry point
- `ClientAdapter::connect()` does handshake, spawns reader/writer tasks; TUI reads events via `try_recv()` in the render loop
- First event after connect is always `SessionInfo { session_id, model, system_prompt_hash }`
- `ReplayHistory` sent immediately after connect — daemon replays all messages as `HistoryBlock` events, then `HistoryEnd`
- Event processing: `daemon_event_to_tui_event()` handles streaming/tool/session events → `app.handle_tui_event()`. Non-TUI events (ConfirmRequest, SystemMessage, SubagentStarted, etc.) handled in a separate match arm.
- Return-early pattern: try `daemon_event_to_tui_event()` first, return if Some; then match non-TUI events with wildcard `_ => {}` fallthrough
- Input flow: user types → `submit_input_attach()` → `is_client_side_command()` check → local (quit, detach, zoom, help) or forward (`SessionCommand::Prompt` / `SessionCommand::SlashCommand`)
- ConfirmRequest auto-approved in attach mode — daemon handles actual sandboxing
- TodoRequest auto-responded with empty JSON object — daemon handles actual todo panel state
- Key handler: full overlay support (model selector, leader menu, output search, slash menu, panels) but simplified action dispatch — no agent-side state mutations
- `handle_local_action()` covers mode switching, navigation, scrolling, editing, history, zoom, search, selectors, copy block, quit
- `handle_leader_action_attach()` routes LeaderAction::SlashCommand through is_client_side_command, LeaderAction::KeymapAction through handle_local_action
- `build_client_slash_registry()` reuses the full builtin registry for completion menu — commands that aren't client-side are forwarded to daemon
- `rebuild_leader_menu()` and `build_slash_registry()` promoted to `pub(crate)` in interactive.rs for attach mode reuse
- Editor methods: `move_home()`/`move_end()` (not `move_to_line_start/end`), `history_up()`/`history_down()` (not `history_prev/next`)
- Leader menu: `open()` not `show()`; Model selector: `open()` not `show()`
- `Direction` comes from `ratatui::layout::Direction`, `Towards` from `ratatui_hypertile::Towards` — FocusDirection takes `{ direction, towards }` struct
- CoreAction variants: `Cancel` (not `Abort`), `ScrollPageUp/Down` (not `HalfPageUp/Down`), `MoveLeft/Right/Home/End` (not `CursorLeft/Right/LineStart/LineEnd`), `DeleteWord` (not `DeleteWordBack`), `ClearLine` (not `ChangeLine`)
- ExtendedAction variants: `PaneZoom` (not `ToggleZoom`), `OpenLeaderMenu` (not `Leader`), `OpenModelSelector` (not `ModelSelector`), `SearchOutput` (not `OutputSearch`), `ToggleCostOverlay` (not `CostOverlay`), `ToggleSessionPopup` (not `SessionPopup`)
- No `CopyLastResponse`/`CopyLastCodeBlock` extended actions — only `CopyBlock` exists
- Terminal `init_terminal()`/`restore_terminal()` shared in `src/modes/common.rs` — both attach.rs and interactive.rs call `super::common::{init,restore}_terminal()`
- clippy: `if a && b { if c {} }` → `if a && b && c {}`, `args.to_string()` → `args.clone()` when args is already String
- `RawGuard` drop struct for session picker — ensures `disable_raw_mode` + `show_cursor` on panic

## Patterns That Work (history replay — proper rendering)
- `replay_history()` now serializes with `serde_json::to_value(msg)` (not `format!("{msg:?}")`) — AgentMessage derives Serialize with `#[serde(tag = "type")]`
- `agent_message_to_tui_events()` in convert.rs converts each AgentMessage variant to a sequence of TuiEvents
- User → `UserInput`, Assistant → `AgentStart` + content blocks + `AgentEnd`, ToolResult → `ToolDone`, BashExecution → `ToolDone`, CompactionSummary → `SessionCompaction`
- Assistant content blocks: Text → `ContentBlockStart(thinking=false)` + `TextDelta` + `ContentBlockStop`, Thinking → same with `thinking=true`, ToolUse → `ToolCall` + `ToolStart`
- Graceful fallback: `serde_json::from_value` failure on old-format blocks shows "📜 (unrecognized block)" system message
- `HistoryEnd` no longer pushes a system message — cleaner attach experience
- `extract_user_text()` and `extract_display_images()` are local helpers in convert.rs (not shared with `extract_tool_content` which takes `ToolResultContent`)

## Patterns That Work (session picker)
- `pick_session()` runs BEFORE `init_terminal()` — standalone raw-mode mini-TUI, not full ratatui
- `crossterm::style::Stylize` for `.bold()`, `.dim()`, `.reverse()` on styled content
- `crossterm::style::PrintStyledContent` for writing styled text to stdout
- Single-session auto-pick: `sessions.len() == 1` bypasses picker entirely
- `\r\n` line endings required in raw mode (no automatic LF→CRLF translation)
- Column layout: SESSION(10) MODEL(28) TURNS(5) LAST ACTIVE(20) CLIENTS — session ID truncated to 8 chars, model to 26
- Navigation: j/k or ↑/↓, Enter to select, q/Esc to cancel
## Patterns That Work (embedded-mode SessionController wiring)
- `SessionController::new_embedded(config)` creates a controller without an agent — events fed via `feed_event()`, outgoing via `take_outgoing()`
- `SessionController::new(agent, config)` wraps agent in `Option<Agent>` — daemon mode methods use `self.agent.as_ref()/.as_mut()`
- `handle_prompt()` uses `self.agent.take()` / `self.agent = Some(agent)` to avoid borrow conflicts between agent and self.emit()
- `drain_events()` collects from event_rx into a Vec first to avoid borrow conflict between rx and process_agent_event
- EventLoopRunner keeps direct `broadcast::Receiver<AgentEvent>` subscription for real-time TUI rendering
- Controller's `feed_event()` is called in the runner's `process_agent_event()` — handles audit, hooks, loop output accumulation
- Loop/auto-test decisions delegated via `controller.check_post_prompt()` → returns `PostPromptAction::{ContinueLoop, RunAutoTest, None}`
- `controller.sync_loop_from_tui(app.loop_status.as_ref())` syncs TUI loop state to controller before checking post-prompt
- Session persistence stays in the runner (on AgentEnd) — runner keeps `session_manager` for branch/merge access
- Plugin dispatch stays in the runner (needs AgentEvent, TUI-side concern)
- Usage/tool result recording to redb stays in the runner (needs db handle)
- Eliminated: audit.rs (124 lines), auto_test.rs (45 lines), loop_mode.rs (127 lines) from EventLoopRunner
- EventLoopRunner: 1529 → 1188 lines (341 lines removed)
- Next: bash confirm UI over protocol, subagent event routing, wire EventLoopRunner to SessionController for daemon mode (full mode), `clankers ps` command
