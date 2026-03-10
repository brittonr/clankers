# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|
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

## Patterns That Work (code quality cleanup)
- CODE_ANALYSIS_REPORT claimed `serde_json` import was unused in settings.rs — wrong, it's used for `Value`, `json!()`, `from_str`, `from_value`
- CODE_ANALYSIS_REPORT claimed 3 duplicate settings merging functions — wrong, `merge_layers` orchestrates and `merge_into` does field-level merge, they're complementary
- `helix_normal_nav()` and `vim_normal_nav()` were literally identical (same key→action map, different entry ordering) — safe to unify
- Plugin WASM tests (89 tests) fail in worktrees because they need pre-built .wasm binaries — skip with `--skip plugin::tests`
- system_prompt.rs at 727 lines is fine — 350 impl + 377 thorough tests, well-decomposed already. Not every big file needs splitting.
- event_handlers.rs is fundamentally a big match statement routing actions — limited decomposition value beyond helper extraction for repeated patterns

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
