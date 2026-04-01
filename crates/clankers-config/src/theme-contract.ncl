# Theme contract for clankers TUI.
#
# Import this in your theme .ncl file to get type checking and defaults:
#
#   (import "clankers://theme") & {
#     bg = [0, 0, 0],
#     highlight = [80, 200, 255],
#   }
#
# Or write a plain record — missing fields fall back to the dark theme.
#
# Colors are RGB arrays: [red, green, blue] with values 0–255.

let Color = Array Number in

{
  # ── Base colors ─────────────────────────────────────
  bg              | Color | default = [30, 30, 30],
  fg              | Color | default = [220, 220, 220],
  border          | Color | default = [80, 80, 80],
  highlight       | Color | default = [100, 180, 255],
  error           | Color | default = [255, 100, 100],

  # ── Message role colors ─────────────────────────────
  userMsg         | Color | default = [120, 200, 120],
  assistantMsg    | Color | default = [200, 200, 200],
  systemMsg       | Color | default = [150, 150, 150],
  thinkingMsg     | Color | default = [150, 130, 200],

  # ── Block chrome ────────────────────────────────────
  blockBorder         | Color | default = [60, 60, 60],
  blockBorderFocused  | Color | default = [100, 180, 255],
  blockTimestamp      | Color | default = [100, 100, 100],

  # ── Markdown rendering ─────────────────────────────
  mdCodeBlock     | Color | default = [180, 220, 140],
  mdCodeFence     | Color | default = [100, 100, 100],
  mdInlineCodeFg  | Color | default = [230, 190, 80],
  mdInlineCodeBg  | Color | default = [45, 45, 45],
  mdListMarker    | Color | default = [100, 100, 100],
  mdBlockquote    | Color | default = [160, 160, 160],
  mdHrule         | Color | default = [80, 80, 80],

  # ── Search highlights ──────────────────────────────
  searchMatch     | Color | default = [120, 90, 30],
  searchCurrent   | Color | default = [220, 180, 40],
}
