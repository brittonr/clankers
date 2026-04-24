# TUI Extraction Plan

Extract ~5,300 lines from `clankers-tui` into a new standalone workspace.
The new project has no clankers dependencies — pure ratatui + std crates
that clankers-tui re-imports.

## Workspace: `rattoolkit`

New git repo. Workspace with 7 crates:

```
rattoolkit/
├── Cargo.toml              # workspace root
├── crates/
│   ├── rat-image/          # terminal image protocols
│   ├── rat-editor/         # multi-line text editor
│   ├── rat-selection/      # mouse text selection + clipboard
│   ├── rat-markdown/       # markdown → ratatui spans
│   ├── rat-streaming/      # output buffer + incremental search
│   ├── rat-diff/           # unified diff viewer
│   └── rat-widgets/        # small generic components
```

---

## Crate Breakdown

### 1. `rat-image` — Terminal Image Protocols

**Source:** `components/image.rs` (498 lines)

**What:** Kitty graphics protocol, iTerm2 inline images, Sixel detection.
Image display/placement, caching resized images, protocol detection.

**Public API:**
- `ImageProtocol` enum (Kitty, ITerm2, Sixel)
- `detect_image_protocol() -> Option<ImageProtocol>`
- `display_kitty_image()`, `display_iterm2_image()`
- `ImageCache` (LRU cache of resized images keyed by hash+dimensions)
- `InlineImage` (placement: id, row, col, width, height)

**Deps:** `base64`, `image` (resize/format conversion)

**Seams to cut:** None. Zero `crate::` or `super::` imports. Zero consumers
inside clankers-tui (the `image::` hits in clipboard.rs are the `image` *crate*,
not this module). Lift-and-drop extraction.

**Consumers in clankers-tui:** The component re-export in `components/mod.rs`.
After extraction: `pub use rat_image;` or direct dep.

---

### 2. `rat-editor` — Multi-line Text Editor

**Source:** `components/editor/{mod,input,render,history}.rs` (836 lines)

**What:** Multi-line text editor with cursor movement, word-wise ops,
selection, history ring (100 entries), undo-style saved input, and
ratatui rendering.

**Public API:**
- `Editor` struct (lines, cursor_line, cursor_col, history)
- Movement: `move_{left,right,up,down}`, `move_word_{left,right}`,
  `home`, `end`, `page_{up,down}`
- Editing: `insert_char`, `backspace`, `delete_char`, `insert_newline`,
  `delete_word_{backward,forward}`, `kill_line`, `paste`
- History: `history_up`, `history_down`, `push_history`
- Query: `content() -> String`, `is_empty()`, `line_count()`
- `render_editor()` — ratatui Frame rendering function

**Deps:** `ratatui`, `unicode-width`

**Seams to cut:** None. All internal refs are `use super::Editor`.
render.rs takes a `Color` for the border — no Theme dependency.

**Consumers in clankers-tui:** `app/mod.rs` (owns `Editor` field),
`render.rs` (calls `render_editor`), `mouse.rs` (hit testing via
`HitRegion::Editor` — that enum stays in clankers-tui).

---

### 3. `rat-selection` — Mouse Text Selection

**Source:** `selection.rs` (424 lines)

**What:** Text position tracking, drag selection, visual-to-logical row
mapping (handles line wrapping), highlight rect generation for rendering
selected text, clipboard copy via OSC 52 / wl-copy.

**Public API:**
- `TextPos` struct (row, col) with Ord
- `TextSelection` struct (anchor, cursor, active)
  - `start()`, `update()`, `finish()`
  - `ordered() -> (TextPos, TextPos)`
  - `col_range_for_row()` — highlight range per visual row
  - `extract_text()` — get selected text from line buffer
- `screen_to_text_pos()` — screen coords → text position
- `visual_to_logical()` — visual row → (logical line, col offset)
- `copy_to_clipboard()` — Wayland wl-copy → OSC 52 fallback

**Deps:** `ratatui` (only `layout::Rect`), `base64` (for OSC 52)

**Seams to cut:** `copy_to_clipboard` uses `std::process::Command` for
wl-copy and `std::io::Write` for `/dev/tty`. No crate:: deps. The Rect
dependency is just for `screen_to_text_pos(area: Rect, ...)`.

**Consumers in clankers-tui:** `mouse.rs` (screen_to_text_pos,
TextSelection::start, copy_to_clipboard), `app/mod.rs` (Option<TextSelection>
field), `app/block_nav.rs` (copy_to_clipboard), `block_view/mod.rs`
(TextSelection in draw signature).

---

### 4. `rat-markdown` — Markdown → Ratatui Spans

**Source:** `components/markdown.rs` (730 lines)

**What:** Converts markdown text into styled `ratatui::text::Line`/`Span`
vectors. Supports fenced code blocks (with syntax highlighting callback),
headings, bullet/numbered lists, bold, italic, bold-italic, inline code,
links, blockquotes, horizontal rules, nested formatting.

**Public API:**
- `MarkdownStyle` struct (base, code_block, code_fence, inline_code,
  bold, italic, bold_italic, heading, subheading, list_marker,
  blockquote, inline_code_bg, hrule, link styles)
- `render_markdown(text, style, highlighter) -> Vec<Line>`
- `SyntaxHighlighter` trait (moved here from clanker-tui-types)
- `HighlightSpan` struct (text, optional fg color)
- `PlainHighlighter` (no-op impl)

**Deps:** `ratatui`, `unicode-width`

**Seams to cut:**
- `crate::theme::Theme` → only used in `MarkdownStyle::from_theme()`.
  Move that method to clankers-tui as a free fn or extension trait.
  The crate itself only needs `MarkdownStyle` (already self-contained).
- `clanker_tui_types::SyntaxHighlighter` → move the trait + HighlightSpan
  + PlainHighlighter (35 lines) into this crate. They belong here.

**Consumers in clankers-tui:** `block_view/render.rs` (render_markdown,
MarkdownStyle). After extraction, clanker-tui-types drops its `syntax.rs`
module and re-exports from rat-markdown instead.

---

### 5. `rat-streaming` — Output Buffer + Search

**Source:** `components/{streaming_output,output_search}.rs` (1,250 lines)

**What:** Two complementary modules:

**StreamingOutput** (670 lines) — Scrollable line buffer for tool output
with head/tail truncation. Auto-follow mode, configurable limits (2000
lines, 200 head + 200 tail), per-tool-call keyed manager.
- `StreamingConfig` (max_lines, head_lines, tail_lines, visible_lines)
- `StreamingOutput` (lines, truncated, scroll_offset, auto_follow)
  - `add_line()`, `total_lines()`, `visible_lines()`, `scroll_{up,down,to_top,to_bottom}`
  - `render_lines() -> Vec<Line>` (ratatui rendering)
- `StreamingOutputManager` — HashMap<String, StreamingOutput> keyed by call ID

**OutputSearch** (580 lines) — Incremental search over rendered lines
with substring (smart-case) and fuzzy (subsequence) matching.
- `SearchMode` enum (Exact, Fuzzy)
- `SearchMatch` struct (row, byte_start, byte_end)
- `OutputSearch` — search state, query, matches, current index
  - `type_char()`, `backspace()`, `update_matches()`, `next_match()`, `prev_match()`
  - `toggle_mode()`, `current_match_row()`
  - `render()` — search bar overlay
- `apply_search_highlights()` — inject match highlighting into Line spans

**Deps:** `ratatui`

**Seams to cut:** Zero crate:: imports in either file. Pure data structures
with ratatui rendering methods. Lift-and-drop.

**Consumers in clankers-tui:** `app/mod.rs` (StreamingOutputManager field,
OutputSearch in overlays), `block_view/mod.rs` (passed to draw fns),
`render.rs` (updates search matches, renders search overlay).

---

### 6. `rat-diff` — Unified Diff Viewer

**Source:** `components/diff_view.rs` (416 lines) + `components/scroll.rs` (67 lines)

**What:** Computes unified diffs using `similar` and renders them with
colored added/removed/context lines. Scrollable via FreeScroll.

**Public API:**
- `DiffLineKind` enum (FileHeader, HunkHeader, Added, Removed, Context, Info)
- `DiffLine` struct (kind, text)
- `DiffView` — diff state with scroll
  - `compute(path, original) -> DiffView` — diffs file on disk vs original
  - `from_texts(old, new, filename)` — diffs two strings
  - `new_file(path)` / `deleted_file(path)`
  - `scroll_{up,down,to_top,to_bottom}()`
  - `draw(frame, area, theme)` — ratatui rendering
- `FreeScroll` — generic `Cell<u16>` scroll offset with up/down/clamp ops

**Deps:** `ratatui`, `similar`

**Seams to cut:**
- `prelude::*` → replace with direct ratatui imports
- `FreeScroll` from `super::scroll` → co-extract into this crate (or rat-widgets)
- `draw()` takes `&Theme` → change to take individual colors or a `DiffStyle` struct

**Consumers in clankers-tui:** `file_activity_panel.rs` (DiffView field,
compute, draw, scroll methods). `subagent_panel.rs` also uses FreeScroll
directly.

**Decision:** FreeScroll (67 lines) goes into `rat-widgets` since
subagent_panel also uses it. `rat-diff` depends on `rat-widgets`.

---

### 7. `rat-widgets` — Generic Small Components

**Source:** 6 files (457 lines) + scroll.rs (67 lines) = ~524 lines

| Component | File | Lines | Description |
|-----------|------|-------|-------------|
| `FreeScroll` | `scroll.rs` | 67 | Cell-based scroll state |
| `SelectList` | `select_list.rs` | 77 | Selection list dialog |
| `InputDialog` | `input.rs` | 70 | Single-line input dialog |
| `Loader` | `loader.rs` | 38 | Spinner/loading indicator |
| `Notification` | `notification.rs` | 80 | Toast notifications with TTL |
| `TreeView`/`TreeNode` | `tree_view.rs` | 103 | Collapsible tree navigation |
| `ConfirmDialog` | `confirm.rs` | 122 | Yes/No confirmation dialog |

**Deps:** `ratatui`

**Seams to cut:**
- `confirm.rs` imports `crate::app::BashConfirmState` for one
  bash-specific render method. Split that method out — keep it in
  clankers-tui, extract just the generic `ConfirmDialog`.

**Consumers in clankers-tui:** Sparse. SelectList, InputDialog, Loader,
TreeView, TreeNode have zero external consumers (only mod.rs re-exports).
Notification is only used via PluginNotification in widget_host.rs (different
type). ConfirmDialog used via BashConfirmState in app/mod.rs.

---

## Dependency Graph

```
rat-image       (standalone)
rat-editor      (standalone)
rat-selection   (standalone)
rat-markdown    (standalone)
rat-streaming   (standalone)
rat-widgets     (standalone)
rat-diff        → rat-widgets (for FreeScroll)
```

No cycles. Only one inter-crate dep.

---

## Migration Steps

### Phase 1: Create workspace, move code

1. `cargo init --lib ratkit` + convert to workspace
2. Create each crate with `cargo new --lib crates/rat-{name}`
3. Copy source files, adjust `use` paths
4. For each crate, strip clankers-specific seams:
   - `rat-markdown`: drop `from_theme()`, move SyntaxHighlighter trait in
   - `rat-diff`: replace `prelude::*` with direct imports, add `DiffStyle` param
   - `rat-widgets/confirm`: remove BashConfirmState render method

### Phase 2: Wire up clankers-tui

5. Add `ratkit` workspace crates as dependencies of clankers-tui
   (path deps during dev → git deps after push)
6. Replace `mod` declarations with `pub use rat_*` re-exports
7. Add bridge code in clankers-tui:
   - `MarkdownStyle::from_theme()` as a local fn
   - Bash-specific confirm rendering stays local
   - `clanker-tui-types` drops `syntax.rs`, re-exports from rat-markdown
8. Run `cargo nextest run` — fix any breakage

### Phase 3: Cleanup

9. Remove dead source files from clankers-tui
10. Update clankers-tui Cargo.toml (drop `similar`, `base64` if only
    used by extracted code)
11. Push ratkit repo, convert path deps to git deps

---

## Line Count Summary

| Crate | Lines | Files |
|-------|-------|-------|
| rat-image | 498 | 1 |
| rat-editor | 836 | 4 |
| rat-selection | 424 | 1 |
| rat-markdown | 765 | 1 (+35 from tui-types/syntax.rs) |
| rat-streaming | 1,250 | 2 |
| rat-diff | 416 | 1 |
| rat-widgets | 524 | 7 |
| **Total** | **4,713** | **17** |

That's 27% of clankers-tui's 17,683 lines moved to reusable crates.
