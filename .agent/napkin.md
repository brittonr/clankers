# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|
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
