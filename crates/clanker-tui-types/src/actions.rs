//! Action types and name mappings for keybindings.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Actions — every semantic operation the TUI supports
// ---------------------------------------------------------------------------

/// Core actions that cannot be extended by plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreAction {
    // ── Mode switching ───────────────────────────────
    EnterInsert,
    EnterCommand,
    EnterNormal,

    // ── Core ──────────────────────────────────────────
    Submit,
    NewLine,
    Cancel,
    Quit,

    // ── Editor movement ──────────────────────────────
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,

    // ── Editor editing ───────────────────────────────
    DeleteBack,
    DeleteForward,
    DeleteWord,
    ClearLine,

    // ── History ──────────────────────────────────────
    HistoryUp,
    HistoryDown,

    // ── Scrolling ────────────────────────────────────
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,

    // ── Block navigation ─────────────────────────────
    FocusPrevBlock,
    FocusNextBlock,
    Unfocus,

    // ── Menu navigation (slash command autocomplete) ─
    MenuUp,
    MenuDown,
    MenuAccept,
    MenuClose,

    // ── Clipboard paste ──────────────────────────────
    PasteImage,
}

/// Semantic actions that keybindings can trigger.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Action {
    /// Core action (hardcoded, cannot be extended).
    #[serde(skip)]
    Core(CoreAction),
    /// Extended action — compile-time checked, no stringly-typed dispatch.
    Extended(ExtendedAction),
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// All extended actions as a proper enum. Eliminates stringly-typed dispatch
/// and gives compile-time exhaustiveness checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtendedAction {
    // Search
    SearchOutput,
    SearchNext,
    SearchPrev,
    // Block operations
    ToggleBlockCollapse,
    CollapseAllBlocks,
    ExpandAllBlocks,
    CopyBlock,
    RerunBlock,
    EditBlock,
    // Branch / panel navigation
    BranchPrev,
    BranchNext,
    // Toggles
    ToggleBlockIds,
    ToggleThinking,
    ToggleShowThinking,
    TogglePanelFocus,
    ToggleCostOverlay,
    ToggleSessionPopup,
    ToggleBranchPanel,
    // Panel operations
    PanelNextTab,
    PanelPrevTab,
    PanelScrollUp,
    PanelScrollDown,
    PanelClearDone,
    PanelKill,
    PanelRemove,
    // Selectors / menus
    OpenLeaderMenu,
    OpenModelSelector,
    OpenAccountSelector,
    OpenBranchSwitcher,
    OpenEditor,
    // Clipboard
    PasteImage,
    // Pane tiling
    PaneZoom,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneClose,
    PaneEqualize,
    PaneGrow,
    PaneShrink,
    PaneMoveLeft,
    PaneMoveRight,
    PaneMoveUp,
    PaneMoveDown,
    // Tool toggle
    OpenToolToggle,
    // Prompt improve
    TogglePromptImprove,
    // Auto-test
    ToggleAutoTest,
}

// Name mapping table for ExtendedAction.
const EXTENDED_ACTION_NAMES: &[(ExtendedAction, &[&str])] = &[
    (ExtendedAction::SearchOutput, &["search_output", "search", "find"]),
    (ExtendedAction::SearchNext, &["search_next", "next_match"]),
    (ExtendedAction::SearchPrev, &["search_prev", "prev_match"]),
    (ExtendedAction::ToggleBlockCollapse, &["toggle_block_collapse", "toggle_collapse"]),
    (ExtendedAction::CollapseAllBlocks, &["collapse_all_blocks", "collapse_all"]),
    (ExtendedAction::ExpandAllBlocks, &["expand_all_blocks", "expand_all"]),
    (ExtendedAction::CopyBlock, &["copy_block"]),
    (ExtendedAction::RerunBlock, &["rerun_block"]),
    (ExtendedAction::EditBlock, &["edit_block"]),
    (ExtendedAction::BranchPrev, &["branch_prev"]),
    (ExtendedAction::BranchNext, &["branch_next"]),
    (ExtendedAction::ToggleBlockIds, &["toggle_block_ids", "toggle_ids"]),
    (ExtendedAction::ToggleThinking, &["toggle_thinking"]),
    (ExtendedAction::ToggleShowThinking, &["toggle_show_thinking"]),
    (ExtendedAction::TogglePanelFocus, &["toggle_panel_focus", "panel_focus"]),
    (ExtendedAction::ToggleCostOverlay, &["toggle_cost_overlay", "cost_overlay"]),
    (ExtendedAction::ToggleSessionPopup, &["toggle_session_popup", "session_popup"]),
    (ExtendedAction::ToggleBranchPanel, &["toggle_branch_panel", "branch_panel"]),
    (ExtendedAction::PanelNextTab, &["panel_next_tab", "panel_next"]),
    (ExtendedAction::PanelPrevTab, &["panel_prev_tab", "panel_prev"]),
    (ExtendedAction::PanelScrollUp, &["panel_scroll_up"]),
    (ExtendedAction::PanelScrollDown, &["panel_scroll_down"]),
    (ExtendedAction::PanelClearDone, &["panel_clear_done", "panel_clear"]),
    (ExtendedAction::PanelKill, &["panel_kill"]),
    (ExtendedAction::PanelRemove, &["panel_remove"]),
    (ExtendedAction::OpenLeaderMenu, &["open_leader_menu", "leader_menu", "leader"]),
    (ExtendedAction::OpenModelSelector, &["open_model_selector", "model_selector"]),
    (ExtendedAction::OpenAccountSelector, &["open_account_selector", "account_selector"]),
    (ExtendedAction::OpenBranchSwitcher, &["open_branch_switcher", "branch_switcher"]),
    (ExtendedAction::OpenEditor, &["open_editor", "editor"]),
    (ExtendedAction::PasteImage, &["paste_image"]),
    (ExtendedAction::PaneZoom, &["pane_zoom", "zoom", "zoom_toggle"]),
    (ExtendedAction::PaneSplitHorizontal, &["pane_split_horizontal"]),
    (ExtendedAction::PaneSplitVertical, &["pane_split_vertical"]),
    (ExtendedAction::PaneClose, &["pane_close"]),
    (ExtendedAction::PaneEqualize, &["pane_equalize"]),
    (ExtendedAction::PaneGrow, &["pane_grow"]),
    (ExtendedAction::PaneShrink, &["pane_shrink"]),
    (ExtendedAction::PaneMoveLeft, &["pane_move_left"]),
    (ExtendedAction::PaneMoveRight, &["pane_move_right"]),
    (ExtendedAction::PaneMoveUp, &["pane_move_up"]),
    (ExtendedAction::PaneMoveDown, &["pane_move_down"]),
    (ExtendedAction::OpenToolToggle, &["open_tool_toggle", "tool_toggle"]),
    (ExtendedAction::TogglePromptImprove, &["toggle_prompt_improve", "prompt_improve"]),
    (ExtendedAction::ToggleAutoTest, &["toggle_auto_test", "auto_test"]),
];

impl ExtendedAction {
    /// Parse from a string name (for keymap config and leader menu).
    pub fn from_name(s: &str) -> Option<Self> {
        EXTENDED_ACTION_NAMES.iter().find(|(_, names)| names.contains(&s)).map(|(action, _)| *action)
    }

    /// Canonical string name (for serialization and display).
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(no_unwrap, reason = "all ExtendedAction variants are in EXTENDED_ACTION_NAMES")
    )]
    pub fn name(self) -> &'static str {
        EXTENDED_ACTION_NAMES
            .iter()
            .find(|(action, _)| *action == self)
            .map(|(_, names)| names[0])
            .expect("all ExtendedActions must have names")
    }
}

impl std::fmt::Display for ExtendedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// Serde: serialize as the canonical string name.
impl Serialize for ExtendedAction {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for ExtendedAction {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_name(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown extended action: {s}")))
    }
}

// ---------------------------------------------------------------------------
// ActionRegistry — tracks extended actions
// ---------------------------------------------------------------------------

/// Metadata for an extended action.
#[derive(Debug, Clone)]
pub struct ExtendedActionDef {
    pub name: String,
    pub description: String,
}

/// Registry for extended actions (plugins, user config).
#[derive(Debug, Clone, Default)]
pub struct ActionRegistry {
    actions: HashMap<String, ExtendedActionDef>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an extended action.
    pub fn register(&mut self, name: &str, description: &str) {
        self.actions.insert(name.to_string(), ExtendedActionDef {
            name: name.to_string(),
            description: description.to_string(),
        });
    }

    /// Get all registered actions.
    pub fn all(&self) -> impl Iterator<Item = &ExtendedActionDef> {
        self.actions.values()
    }
}

// ---------------------------------------------------------------------------
// Action parsing
// ---------------------------------------------------------------------------

/// Core action name mappings.
const CORE_ACTION_NAMES: &[(CoreAction, &[&str])] = &[
    (CoreAction::EnterInsert, &["enter_insert"]),
    (CoreAction::EnterCommand, &["enter_command"]),
    (CoreAction::EnterNormal, &["enter_normal"]),
    (CoreAction::Submit, &["submit"]),
    (CoreAction::NewLine, &["new_line", "newline"]),
    (CoreAction::Cancel, &["cancel"]),
    (CoreAction::Quit, &["quit"]),
    (CoreAction::MoveLeft, &["move_left"]),
    (CoreAction::MoveRight, &["move_right"]),
    (CoreAction::MoveHome, &["move_home"]),
    (CoreAction::MoveEnd, &["move_end"]),
    (CoreAction::DeleteBack, &["delete_back"]),
    (CoreAction::DeleteForward, &["delete_forward"]),
    (CoreAction::DeleteWord, &["delete_word"]),
    (CoreAction::ClearLine, &["clear_line"]),
    (CoreAction::HistoryUp, &["history_up"]),
    (CoreAction::HistoryDown, &["history_down"]),
    (CoreAction::ScrollUp, &["scroll_up"]),
    (CoreAction::ScrollDown, &["scroll_down"]),
    (CoreAction::ScrollPageUp, &["scroll_page_up", "page_up"]),
    (CoreAction::ScrollPageDown, &["scroll_page_down", "page_down"]),
    (CoreAction::ScrollToTop, &["scroll_to_top"]),
    (CoreAction::ScrollToBottom, &["scroll_to_bottom"]),
    (CoreAction::FocusPrevBlock, &["focus_prev_block", "prev_block"]),
    (CoreAction::FocusNextBlock, &["focus_next_block", "next_block"]),
    (CoreAction::Unfocus, &["unfocus"]),
    (CoreAction::MenuUp, &["menu_up"]),
    (CoreAction::MenuDown, &["menu_down"]),
    (CoreAction::MenuAccept, &["menu_accept"]),
    (CoreAction::MenuClose, &["menu_close"]),
];

pub fn parse_action(s: &str) -> Option<Action> {
    let normalized = s.to_lowercase().replace('-', "_");

    // Try core actions first.
    CORE_ACTION_NAMES
        .iter()
        .find(|(_, names)| names.contains(&normalized.as_str()))
        .map(|(action, _)| Action::Core(*action))
        .or_else(|| ExtendedAction::from_name(&normalized).map(Action::Extended))
}
