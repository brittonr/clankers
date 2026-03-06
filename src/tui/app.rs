//! Main application state machine

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ratatui::layout::Rect;

use crate::agent::events::AgentEvent;
use crate::config::keybindings::InputMode;
use crate::plugin::ui::PluginUIState;
use crate::provider::message::Content;
use crate::provider::streaming::ContentDelta;
use crate::tui::components::account_selector::AccountSelector;
use crate::tui::components::block::BlockEntry;
use crate::tui::components::block::ConversationBlock;
use crate::tui::components::editor::Editor;
use crate::tui::components::messages::MessageScroll;
use crate::tui::components::model_selector::ModelSelector;
use crate::tui::components::session_selector::SessionSelector;
use crate::tui::components::slash_menu::SlashMenu;
use crate::tui::panel::PanelId;
use crate::tui::selection::TextSelection;
use crate::tui::theme::Theme;

/// State for a currently-executing tool (used for live output rendering)
#[derive(Debug, Clone)]
pub struct ActiveToolExecution {
    /// Name of the tool (e.g. "bash")
    pub tool_name: String,
    /// When execution started
    pub started_at: Instant,
    /// Number of output lines received so far
    pub line_count: usize,
}

/// Application state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Streaming,
    Command,
    Dialog,
}

// Panel focus is tracked by `focused_panel: Option<PanelId>` and the hypertile
// BSP tiling engine (`tiling: Hypertile`). See App::focus_panel() / unfocus_panel().

/// Connection status to the clankers-router daemon
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterStatus {
    /// Connected to the router daemon via RPC
    Connected,
    /// Using in-process provider (no daemon)
    Local,
    /// Disconnected / unreachable
    Disconnected,
}

/// A message for display in the chat view
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_name: Option<String>,
    pub is_error: bool,
    /// Optional inline images (base64 data + media type) for terminal rendering
    pub images: Vec<DisplayImage>,
}

/// An image attached to a display message for inline terminal rendering
#[derive(Debug, Clone)]
pub struct DisplayImage {
    /// Base64-encoded image data
    pub data: String,
    /// MIME type (e.g. "image/png")
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    Thinking,
    System,
}

/// Saved tiling state while a pane is temporarily zoomed to full screen.
#[derive(Debug, Clone)]
pub struct ZoomState {
    /// The BSP tree before zooming.
    pub tiling: ratatui_hypertile::Hypertile,
    /// The pane registry before zooming.
    pub registry: super::panes::PaneRegistry,
    /// Which panel was focused before zooming (if any).
    pub focused_panel: Option<PanelId>,
    /// Which subagent pane was focused before zooming (if any).
    pub focused_subagent: Option<String>,
}

/// Main TUI application
pub struct App {
    pub state: AppState,
    pub input_mode: InputMode,
    pub theme: Theme,
    pub editor: Editor,
    /// Block-oriented conversation history (only the active branch is shown)
    pub blocks: Vec<BlockEntry>,
    /// All conversation blocks ever created (including branched-off blocks)
    pub all_blocks: Vec<ConversationBlock>,
    /// The in-progress block being streamed into
    pub active_block: Option<ConversationBlock>,
    /// Next block ID
    next_block_id: usize,
    /// Index of the currently focused block (for keyboard navigation)
    pub focused_block: Option<usize>,
    /// Accumulated streaming text (flushed into active block on boundaries)
    pub streaming_text: String,
    /// Accumulated streaming thinking (flushed into active block on boundaries)
    pub streaming_thinking: String,
    /// Current streaming content block index (tracks which block we're in)
    streaming_block_index: Option<usize>,
    pub scroll: MessageScroll,
    pub model: String,
    pub session_id: String,
    pub total_tokens: usize,
    pub total_cost: f64,
    /// Cost tracker for per-model cost display and budget status
    pub cost_tracker: Option<Arc<crate::routing::cost_tracker::CostTracker>>,
    pub cwd: String,
    pub should_quit: bool,
    /// Current text selection in the messages area
    pub selection: Option<TextSelection>,
    /// Plain-text lines from the last render (for selection extraction)
    pub rendered_lines: Vec<String>,
    /// The messages area Rect from the last render
    pub messages_area: Rect,
    /// Slash command autocomplete menu
    pub slash_menu: SlashMenu,
    /// Slash command registry (commands from builtins, plugins, user config)
    pub slash_registry: crate::slash_commands::SlashRegistry,
    /// Action registry (extended actions from plugins, user config)
    pub action_registry: crate::config::keybindings::ActionRegistry,
    /// PKCE verifier for in-progress OAuth login (set by `/login`, consumed by `/login <code>`)
    pub login_verifier: Option<(String, String)>, // (verifier, account_name)
    /// Pending branch operation: Some((fork_block_id, new_prompt)) if we need to
    /// truncate the agent's history and re-prompt after branching.
    pub pending_branch: Option<(usize, String)>,
    /// Branch checkpoint that was just executed — (checkpoint_msg_count, fork_message_ids).
    /// The event loop reads this to record the branch in the session file.
    pub last_branch_checkpoint: Option<usize>,
    /// Queued prompt to send after the current stream is aborted.
    pub queued_prompt: Option<String>,
    /// Whether extended thinking is currently enabled
    pub thinking_enabled: bool,
    /// Current thinking level
    pub thinking_level: crate::provider::ThinkingLevel,
    /// Whether to display thinking content in blocks
    pub show_thinking: bool,
    /// Available tool definitions (name, description, source)
    pub tool_info: Vec<(String, String, String)>,
    /// Plugin UI state (widgets, status segments, notifications)
    pub plugin_ui: PluginUIState,
    /// Panel manager (owns all side panels)
    pub panels: super::panel::PanelManager,
    /// Branch switcher overlay (quick fuzzy picker)
    pub branch_switcher: super::components::branch_switcher::BranchSwitcher,
    /// Branch comparison overlay (side-by-side diff)
    pub branch_compare: super::components::branch_compare::BranchCompareView,
    /// Interactive merge overlay (checkbox message selection)
    pub merge_interactive: super::components::merge_interactive::MergeInteractiveView,
    /// Context window gauge (token usage vs model limit)
    pub context_gauge: super::components::context_gauge::ContextGauge,
    /// Git status (branch + dirty indicator)
    pub git_status: super::components::git_status::GitStatus,
    // Legacy panel_tab/right_panel_tab/panel_focused deleted.
    /// Pending images attached via clipboard paste (base64-encoded PNG data)
    pub pending_images: Vec<PendingImage>,
    /// Whether a clipboard read is in progress (to avoid stacking requests)
    pub clipboard_pending: bool,
    /// Receiver for background clipboard reads
    pub clipboard_rx: Option<std::sync::mpsc::Receiver<crate::modes::clipboard::ClipboardResult>>,
    /// Whether the session/branch popup is visible
    pub session_popup_visible: bool,
    /// Whether to show block IDs in conversation view (toggled with Ctrl+I)
    pub show_block_ids: bool,
    /// Plan mode state
    pub plan_state: crate::modes::plan::PlanState,
    /// History search overlay (Ctrl+R)
    pub history_search: super::components::history_search::HistorySearch,
    /// Output search overlay (Ctrl+F / f)
    pub output_search: super::components::output_search::OutputSearch,
    /// Original system prompt (for `/system reset`)
    pub original_system_prompt: String,
    /// Flag: open external editor on next event loop tick
    pub open_editor_requested: bool,
    /// Model selector popup
    pub model_selector: ModelSelector,
    /// Account selector popup
    pub account_selector: AccountSelector,
    /// Session resume selector popup
    pub session_selector: SessionSelector,
    /// Active account name (for status bar display)
    pub active_account: String,
    /// Available model IDs (populated from provider)
    pub available_models: Vec<String>,
    /// Connection status to the clankers-router daemon
    pub router_status: RouterStatus,
    /// Leader key (Space) popup menu
    pub leader_menu: super::components::leader_menu::LeaderMenu,
    // ── Hypertile BSP tiling ────────────────────────────
    /// BSP tiling engine (replaces the old column-based PanelLayout).
    pub tiling: ratatui_hypertile::Hypertile,
    /// Maps hypertile PaneIds to their content type (Chat, Panel, Empty).
    pub pane_registry: super::panes::PaneRegistry,
    /// Which panel (if any) currently has focus.
    /// `None` means the chat pane is focused.
    pub focused_panel: Option<PanelId>,
    /// Which subagent pane (if any) currently has focus (by subagent ID).
    /// Mutually exclusive with `focused_panel`.
    pub focused_subagent: Option<String>,
    /// Per-subagent pane manager — each subagent gets its own BSP pane.
    pub subagent_panes: super::components::subagent_pane::SubagentPaneManager,
    /// Saved tiling state while a pane is zoomed to full screen.
    /// `None` means no pane is zoomed.
    pub zoom_state: Option<ZoomState>,
    // ── Mouse hit-test areas (updated each render) ───
    /// The editor/input area Rect from the last render
    pub editor_area: Rect,
    /// The status bar area Rect from the last render
    pub status_area: Rect,

    // ── Streaming tool output ────────────────────────
    /// Active tool executions keyed by call_id (for spinner/elapsed/line-count)
    pub active_tools: HashMap<String, ActiveToolExecution>,
    /// Structured progress renderer for active tool executions
    pub progress_renderer: super::components::progress_renderer::ProgressRenderer,
    /// Scrollable streaming output buffers for in-progress tools
    pub streaming_outputs: super::components::streaming_output::StreamingOutputManager,
    /// Which tool output (call_id) is currently focused for scroll control.
    /// Mutually exclusive with `focused_panel` and `focused_subagent`.
    pub focused_tool: Option<String>,
    /// Monotonic tick counter, incremented each render frame (drives spinner animation)
    pub tick: u64,
}

/// An image attached to the editor, waiting to be sent with the next prompt
#[derive(Debug, Clone)]
pub struct PendingImage {
    /// Base64-encoded image data
    pub data: String,
    /// MIME type (e.g. "image/png")
    pub media_type: String,
    /// Approximate size in bytes (of the raw image data)
    pub size: usize,
}

impl App {
    pub fn new(model: String, cwd: String, theme: Theme) -> Self {
        let context_gauge = super::components::context_gauge::ContextGauge::new(&model);
        let git_status = super::components::git_status::GitStatus::new(&cwd);
        Self {
            state: AppState::Idle,
            input_mode: InputMode::Normal,
            theme,
            editor: Editor::new(),
            blocks: Vec::new(),
            all_blocks: Vec::new(),
            active_block: None,
            next_block_id: 0,
            focused_block: None,
            streaming_text: String::new(),
            streaming_thinking: String::new(),
            streaming_block_index: None,
            scroll: MessageScroll::new(),
            model,
            session_id: String::new(),
            total_tokens: 0,
            total_cost: 0.0,
            cost_tracker: None,
            cwd,
            should_quit: false,
            selection: None,
            rendered_lines: Vec::new(),
            messages_area: Rect::default(),
            slash_menu: SlashMenu::new(),
            slash_registry: {
                use crate::slash_commands::{BuiltinSlashContributor, SlashContributor, SlashRegistry};
                let builtin = BuiltinSlashContributor;
                let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
                let (registry, _conflicts) = SlashRegistry::build(&contributors);
                // TODO: log conflicts to debug output
                registry
            },
            action_registry: crate::config::keybindings::ActionRegistry::new(),
            login_verifier: None,
            pending_branch: None,
            last_branch_checkpoint: None,
            queued_prompt: None,
            thinking_enabled: false,
            thinking_level: crate::provider::ThinkingLevel::Off,
            show_thinking: true,
            tool_info: Vec::new(),
            plugin_ui: PluginUIState::new(),
            panels: {
                let mut pm = super::panel::PanelManager::new();
                pm.register(Box::new(super::components::todo_panel::TodoPanel::new()));
                pm.register(Box::new(super::components::file_activity_panel::FileActivityPanel::new()));
                pm.register(Box::new(super::components::subagent_panel::SubagentPanel::new()));
                pm.register(Box::new(super::components::peers_panel::PeersPanel::new()));
                pm.register(Box::new(super::components::process_panel::ProcessPanel::new()));
                pm.register(Box::new(super::components::branch_panel::BranchPanel::new()));
                pm
            },
            branch_switcher: super::components::branch_switcher::BranchSwitcher::new(),
            branch_compare: super::components::branch_compare::BranchCompareView::new(),
            merge_interactive: super::components::merge_interactive::MergeInteractiveView::new(),
            context_gauge,
            git_status,

            pending_images: Vec::new(),
            clipboard_pending: false,
            clipboard_rx: None,
            session_popup_visible: false,
            show_block_ids: false,
            plan_state: crate::modes::plan::PlanState::Inactive,
            history_search: super::components::history_search::HistorySearch::new(),
            output_search: super::components::output_search::OutputSearch::new(),
            original_system_prompt: String::new(),
            open_editor_requested: false,
            model_selector: ModelSelector::new(Vec::new()),
            account_selector: AccountSelector::new(),
            session_selector: SessionSelector::new(),
            active_account: String::new(),
            available_models: Vec::new(),
            router_status: RouterStatus::Disconnected,
            leader_menu: super::components::leader_menu::LeaderMenu::new(),
            tiling: super::panes::default_tiling(),
            pane_registry: super::panes::default_registry(),
            focused_panel: None,
            focused_subagent: None,
            subagent_panes: super::components::subagent_pane::SubagentPaneManager::new(),
            zoom_state: None,
            editor_area: Rect::default(),
            status_area: Rect::default(),
            active_tools: HashMap::new(),
            progress_renderer: super::components::progress_renderer::ProgressRenderer::new(),
            streaming_outputs: super::components::streaming_output::StreamingOutputManager::new(),
            focused_tool: None,
            tick: 0,
        }
    }

    /// Rebuild the leader menu from all contributors.
    ///
    /// Call after plugin load/unload or settings change.
    pub fn rebuild_leader_menu(
        &mut self,
        plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
        settings: &crate::config::settings::Settings,
    ) {
        use super::components::leader_menu::BuiltinKeymapContributor;
        use super::components::leader_menu::MenuContributor;
        use super::components::leader_menu::SlashCommandContributor;

        let builtin = BuiltinKeymapContributor;
        let slash_commands = SlashCommandContributor::new(crate::slash_commands::builtin_commands());
        let hidden = settings.leader_menu.hidden_set();

        // Collect contributors into a vec of trait refs
        let pm_guard;
        let mut contributors: Vec<&dyn MenuContributor> = vec![&builtin, &slash_commands];

        if let Some(pm_arc) = plugin_manager {
            pm_guard = pm_arc.lock().unwrap();
            contributors.push(&*pm_guard);
        }

        contributors.push(&settings.leader_menu);

        let (menu, conflicts) = super::components::leader_menu::LeaderMenu::build(
            &contributors,
            &hidden,
        );

        for c in &conflicts {
            tracing::debug!(
                registry = c.registry,
                key = %c.key,
                winner = %c.winner,
                loser = %c.loser,
                "leader menu key conflict"
            );
        }

        self.leader_menu = menu;
    }

    /// Rebuild the slash command registry with plugin contributions.
    /// Call this after plugins are loaded.
    pub fn rebuild_slash_registry(
        &mut self,
        plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    ) {
        use crate::slash_commands::{BuiltinSlashContributor, SlashContributor, SlashRegistry};

        let builtin = BuiltinSlashContributor;
        
        // Collect contributors
        let pm_guard;
        let mut contributors: Vec<&dyn SlashContributor> = vec![&builtin];

        if let Some(pm_arc) = plugin_manager {
            pm_guard = pm_arc.lock().unwrap();
            contributors.push(&*pm_guard);
        }

        let (registry, conflicts) = SlashRegistry::build(&contributors);

        for c in &conflicts {
            tracing::debug!(
                registry = c.registry,
                key = %c.key,
                winner = %c.winner,
                loser = %c.loser,
                "slash command conflict"
            );
        }

        self.slash_registry = registry;
    }

    /// Get a panel by ID (immutable) for rendering.
    pub fn panel(&self, id: super::panel::PanelId) -> &dyn super::panel::Panel {
        self.panels.get(id).expect("unknown panel")
    }

    /// Get a panel by ID (mutable) for key handling.
    pub fn panel_mut(&mut self, id: super::panel::PanelId) -> &mut dyn super::panel::Panel {
        self.panels.get_mut(id).expect("unknown panel")
    }

    /// Close any detail/diff views on the focused panel before unfocusing.
    /// Panels like Subagents and Files have sub-views that should reset
    /// when the user exits the panel.
    pub fn close_focused_panel_views(&mut self) {
        if let Some(id) = self.focused_panel {
            self.panel_mut(id).close_detail_view();
        }
        // Subagent panes have no detail view to close, but clear the focus
    }

    // ── Focus helpers (bridge hypertile ↔ PanelId) ──────────────────

    /// Whether a side panel or subagent pane (not chat) currently has focus.
    pub fn has_panel_focus(&self) -> bool {
        self.focused_panel.is_some() || self.focused_subagent.is_some()
    }

    /// Focus a specific panel by `PanelId`. Updates both hypertile and
    /// the `focused_panel` tracker.
    pub fn focus_panel(&mut self, panel_id: PanelId) {
        if let Some(pane) = self.pane_registry.find_panel(panel_id) {
            let _ = self.tiling.focus_pane(pane);
            self.focused_panel = Some(panel_id);
            self.focused_subagent = None;
            self.focused_tool = None;
            self.streaming_outputs.unfocus_all();
        }
    }

    /// Focus a specific subagent pane by its string ID.
    pub fn focus_subagent(&mut self, subagent_id: &str) {
        if let Some(pane_id) = self.subagent_panes.pane_id_for(subagent_id) {
            let _ = self.tiling.focus_pane(pane_id);
            self.focused_subagent = Some(subagent_id.to_string());
            self.focused_panel = None;
            self.focused_tool = None;
            self.streaming_outputs.unfocus_all();
        }
    }

    /// Return focus to the chat pane (unfocus any panel, subagent, or tool output).
    pub fn unfocus_panel(&mut self) {
        let chat = self.pane_registry.chat_pane();
        let _ = self.tiling.focus_pane(chat);
        self.focused_panel = None;
        self.focused_subagent = None;
        self.focused_tool = None;
        self.streaming_outputs.unfocus_all();
    }

    /// Focus a specific tool's streaming output for scroll control.
    /// The `call_id` identifies the active tool execution.
    pub fn focus_tool(&mut self, call_id: &str) {
        self.focused_panel = None;
        self.focused_subagent = None;
        self.focused_tool = Some(call_id.to_string());
        self.streaming_outputs.focus(call_id);
    }

    /// Unfocus the currently focused tool output.
    pub fn unfocus_tool(&mut self) {
        self.focused_tool = None;
        self.streaming_outputs.unfocus_all();
    }

    /// Is the given panel currently focused?
    pub fn is_panel_focused(&self, panel_id: PanelId) -> bool {
        self.focused_panel == Some(panel_id)
    }

    /// Apply a hypertile tiling action (focus, resize, etc.) and sync
    /// our `focused_panel` tracker from the resulting hypertile state.
    pub fn apply_tiling_action(&mut self, action: ratatui_hypertile::HypertileAction) {
        self.tiling.apply_action(action);
        self.sync_focused_panel();
    }

    /// Sync `focused_panel` and `focused_subagent` from hypertile's current focus.
    pub fn sync_focused_panel(&mut self) {
        self.focused_panel = None;
        self.focused_subagent = None;
        if let Some(pane_id) = self.tiling.focused_pane() {
            match self.pane_registry.kind(pane_id) {
                Some(super::panes::PaneKind::Panel(panel_id)) => {
                    self.focused_panel = Some(*panel_id);
                }
                Some(super::panes::PaneKind::Subagent(id)) => {
                    self.focused_subagent = Some(id.clone());
                }
                _ => {} // Chat or Empty → no panel/subagent focus
            }
        }
    }

    /// Split the focused pane in the given direction.
    /// Creates a new empty pane and registers it.
    /// The chat pane (ROOT) cannot be split.
    pub fn split_focused_pane(&mut self, direction: ratatui::layout::Direction) {
        use super::panes::PaneKind;

        // Don't split the chat pane — it must remain a single pane.
        if let Some(focused) = self.tiling.focused_pane() {
            if self.pane_registry.is_chat(focused) {
                return;
            }
        }

        match self.tiling.split_focused(direction) {
            Ok(new_id) => {
                // The new pane starts as Empty. The old pane keeps its content.
                self.pane_registry.register(new_id, PaneKind::Empty);
                self.sync_focused_panel();
            }
            Err(_) => {} // Silently ignore (e.g. root-only tree)
        }
    }

    /// Close the focused pane and remove it from the registry.
    /// The chat pane (ROOT) cannot be closed.
    pub fn close_focused_pane(&mut self) {
        // Don't close the chat pane.
        if let Some(focused) = self.tiling.focused_pane() {
            if self.pane_registry.is_chat(focused) {
                return;
            }
        }

        match self.tiling.close_focused() {
            Ok(removed_id) => {
                self.pane_registry.unregister(removed_id);
                self.sync_focused_panel();
            }
            Err(_) => {} // Silently ignore (e.g. only one pane left)
        }
    }

    // ── Zoom (temporary full-screen focus) ────────────────────────

    /// Whether any pane is currently zoomed.
    pub fn is_zoomed(&self) -> bool {
        self.zoom_state.is_some()
    }

    /// Zoom the currently focused pane to fill the entire terminal.
    /// Saves the current tiling so it can be restored with `zoom_restore`.
    /// If already zoomed, this is a no-op.
    pub fn zoom_focused(&mut self) {
        if self.zoom_state.is_some() {
            return;
        }

        let Some(focused_pane) = self.tiling.focused_pane() else {
            return;
        };

        // Save current state.
        self.zoom_state = Some(ZoomState {
            tiling: self.tiling.clone(),
            registry: self.pane_registry.clone(),
            focused_panel: self.focused_panel,
            focused_subagent: self.focused_subagent.clone(),
        });

        // Build a single-pane tree with the focused pane at root.
        let mut zoomed = ratatui_hypertile::Hypertile::new();
        // Hypertile::new() creates ROOT as the only pane. We need to
        // make the registry map ROOT to whatever the focused pane held.
        let kind = self.pane_registry.kind(focused_pane).cloned()
            .unwrap_or(super::panes::PaneKind::Empty);

        let mut reg = super::panes::PaneRegistry::new();
        // ROOT is already Chat in a fresh registry — override it.
        reg.register(ratatui_hypertile::PaneId::ROOT, kind.clone());

        // If the zoomed pane was Chat, keep chat_pane as ROOT (already the case).
        // If it was a Panel, we need a registry where ROOT maps to that panel
        // and chat_pane is still ROOT (PaneRegistry enforces this). That's fine —
        // the renderer will see ROOT → Panel and skip the chat render path.

        let _ = zoomed.focus_pane(ratatui_hypertile::PaneId::ROOT);
        self.tiling = zoomed;
        self.pane_registry = reg;

        // Sync focused_panel/focused_subagent to match the zoomed pane's content.
        self.focused_subagent = None;
        match kind {
            super::panes::PaneKind::Panel(id) => self.focused_panel = Some(id),
            super::panes::PaneKind::Subagent(ref id) => {
                self.focused_panel = None;
                self.focused_subagent = Some(id.clone());
            }
            _ => self.focused_panel = None,
        }
    }

    /// Restore the tiling layout from before `zoom_focused` was called.
    /// No-op if not zoomed.
    pub fn zoom_restore(&mut self) {
        let Some(saved) = self.zoom_state.take() else {
            return;
        };
        self.tiling = saved.tiling;
        self.pane_registry = saved.registry;
        self.focused_panel = saved.focused_panel;
        self.focused_subagent = saved.focused_subagent;
    }

    /// Toggle zoom on the focused pane: zoom in if not zoomed, restore if zoomed.
    pub fn zoom_toggle(&mut self) {
        if self.is_zoomed() {
            self.zoom_restore();
        } else {
            self.zoom_focused();
        }
    }

    /// Advance the animation tick (called once per render frame)
    pub fn advance_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Get the current spinner character for animated indicators
    pub fn spinner_char(&self) -> char {
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        // Divide tick by 3 to slow down (~150ms per frame at 50ms poll rate)
        SPINNER[(self.tick as usize / 3) % SPINNER.len()]
    }

    /// Push a standalone system message (not part of any block)
    pub fn push_system(&mut self, content: String, is_error: bool) {
        self.blocks.push(BlockEntry::System(DisplayMessage {
            role: MessageRole::System,
            content,
            tool_name: None,
            is_error,
            images: Vec::new(),
        }));
        self.scroll.scroll_to_bottom();
    }

    /// Start a new conversation block for the given user prompt.
    /// `agent_msg_count` is the number of agent messages *before* this block's
    /// user message is appended (used for branching truncation).
    pub fn start_block(&mut self, prompt: String, agent_msg_count: usize) {
        self.finalize_active_block();

        // Determine parent: the last conversation block on the visible list
        let parent_id = self.blocks.iter().rev().find_map(|e| match e {
            BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        });

        let mut block = ConversationBlock::new(self.next_block_id, prompt);
        block.parent_block_id = parent_id;
        block.agent_msg_checkpoint = agent_msg_count;
        self.next_block_id += 1;
        self.active_block = Some(block);
    }

    /// Finalize the active block and move it to the completed list
    pub fn finalize_active_block(&mut self) {
        // Flush any in-progress streaming content first
        self.flush_streaming_thinking();
        self.flush_streaming_text();

        if let Some(mut block) = self.active_block.take() {
            block.streaming = false;
            // Store in both the active view and the full block history
            self.all_blocks.push(block.clone());
            self.blocks.push(BlockEntry::Conversation(block));

            // Refresh branch panel if it has entries (i.e., has been opened before)
            if let Some(bp) = self.panels.downcast_ref::<super::components::branch_panel::BranchPanel>(super::panel::PanelId::Branches) {
                if !bp.entries.is_empty() {
                    let active_ids: std::collections::HashSet<usize> = self
                        .blocks
                        .iter()
                        .filter_map(|e| match e {
                            BlockEntry::Conversation(b) => Some(b.id),
                            _ => None,
                        })
                        .collect();
                    let all_blocks = self.all_blocks.clone();
                    if let Some(bp) = self.panels.downcast_mut::<super::components::branch_panel::BranchPanel>(super::panel::PanelId::Branches) {
                        bp.refresh(&all_blocks, &active_ids);
                    }
                }
            }
        }
    }

    /// Flush accumulated streaming thinking into the active block
    fn flush_streaming_thinking(&mut self) {
        if !self.streaming_thinking.is_empty() {
            let content = std::mem::take(&mut self.streaming_thinking);
            if let Some(ref mut block) = self.active_block {
                block.responses.push(DisplayMessage {
                    role: MessageRole::Thinking,
                    content,
                    tool_name: None,
                    is_error: false,
                    images: Vec::new(),
                });
            }
        }
    }

    /// Flush accumulated streaming text into the active block
    fn flush_streaming_text(&mut self) {
        if !self.streaming_text.is_empty() {
            let content = std::mem::take(&mut self.streaming_text);
            if let Some(ref mut block) = self.active_block {
                block.responses.push(DisplayMessage {
                    role: MessageRole::Assistant,
                    content,
                    tool_name: None,
                    is_error: false,
                    images: Vec::new(),
                });
            }
        }
    }

    /// Handle an agent event, routing it into the active block
    pub fn handle_agent_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::AgentStart => {
                self.state = AppState::Streaming;
                self.streaming_text.clear();
                self.streaming_thinking.clear();
                self.streaming_block_index = None;
            }
            AgentEvent::AgentEnd { .. } => {
                self.finalize_active_block();
                self.state = AppState::Idle;
                self.scroll.scroll_to_bottom();
            }
            AgentEvent::ContentBlockStart { index, content_block } => {
                // A new content block is starting — flush any previous streaming buffers
                // so each content block becomes its own DisplayMessage
                match content_block {
                    Content::Thinking { .. } => {
                        // New thinking block: flush any prior text
                        self.flush_streaming_text();
                    }
                    Content::Text { .. } => {
                        // New text block: flush any prior thinking
                        self.flush_streaming_thinking();
                    }
                    Content::ToolUse { .. } => {
                        // Tool use: flush everything
                        self.flush_streaming_thinking();
                        self.flush_streaming_text();
                    }
                    _ => {
                        self.flush_streaming_thinking();
                        self.flush_streaming_text();
                    }
                }
                self.streaming_block_index = Some(*index);
            }
            AgentEvent::ContentBlockStop { index: _ } => {
                // Content block finished — flush its buffer
                self.flush_streaming_thinking();
                self.flush_streaming_text();
                self.streaming_block_index = None;
            }
            AgentEvent::MessageUpdate { delta, .. } => match delta {
                ContentDelta::TextDelta { text } => {
                    self.streaming_text.push_str(text);
                    if self.scroll.auto_scroll {
                        self.scroll.scroll_to_bottom();
                    }
                }
                ContentDelta::ThinkingDelta { thinking } => {
                    self.streaming_thinking.push_str(thinking);
                    if self.scroll.auto_scroll {
                        self.scroll.scroll_to_bottom();
                    }
                }
                _ => {}
            },
            AgentEvent::ToolCall { tool_name, input, .. } => {
                // ContentBlockStop should have already flushed, but be safe
                self.flush_streaming_thinking();
                self.flush_streaming_text();
                if let Some(ref mut block) = self.active_block {
                    block.responses.push(DisplayMessage {
                        role: MessageRole::ToolCall,
                        content: tool_name.clone(),
                        tool_name: Some(tool_name.clone()),
                        is_error: false,
                        images: Vec::new(),
                    });
                }
                // Track file activity from tool calls
                self.track_file_activity(tool_name, input);
            }
            AgentEvent::ToolExecutionStart { call_id, tool_name } => {
                self.active_tools.insert(call_id.clone(), ActiveToolExecution {
                    tool_name: tool_name.clone(),
                    started_at: Instant::now(),
                    line_count: 0,
                });
            }
            AgentEvent::ToolExecutionUpdate { call_id, partial } => {
                let text = partial
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");

                // Update active tool line count
                if let Some(active) = self.active_tools.get_mut(call_id.as_str()) {
                    active.line_count += text.lines().count().max(1);
                }

                // Feed into streaming output buffer for scrollable display
                self.streaming_outputs.add_text(call_id, &text);

                if let Some(ref mut block) = self.active_block {
                    let found = block
                        .responses
                        .iter_mut()
                        .rev()
                        .find(|m| m.role == MessageRole::ToolResult && m.tool_name.as_deref() == Some(call_id));
                    if let Some(msg) = found {
                        if !msg.content.is_empty() {
                            msg.content.push('\n');
                        }
                        msg.content.push_str(&text);
                    } else {
                        block.responses.push(DisplayMessage {
                            role: MessageRole::ToolResult,
                            content: text,
                            tool_name: Some(call_id.clone()),
                            is_error: false,
                            images: Vec::new(),
                        });
                    }
                }
                if self.scroll.auto_scroll {
                    self.scroll.scroll_to_bottom();
                }
            }
            AgentEvent::ToolProgressUpdate { call_id, progress } => {
                self.progress_renderer.update(call_id, progress.clone());
            }
            AgentEvent::ToolResultChunk { call_id, chunk } => {
                // Feed chunks into the streaming output buffer for display.
                // The executor's accumulator also collects these for the final result.
                if chunk.content_type == "text" {
                    self.streaming_outputs.add_text(call_id, &chunk.content);
                }
            }
            AgentEvent::ToolExecutionEnd {
                call_id,
                result,
                is_error,
                ..
            } => {
                // Remove from active tools, progress renderer, and streaming output
                self.progress_renderer.remove(call_id);
                self.active_tools.remove(call_id.as_str());
                self.streaming_outputs.remove(call_id);
                // Clear focused tool if it was this one
                if self.focused_tool.as_deref() == Some(call_id) {
                    self.focused_tool = None;
                }

                // Collect text content
                let display = result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                // Collect image content (no longer silently dropped)
                let images: Vec<DisplayImage> = result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        crate::tools::ToolResultContent::Image { media_type, data } => Some(DisplayImage {
                            data: data.clone(),
                            media_type: media_type.clone(),
                        }),
                        _ => None,
                    })
                    .collect();

                if let Some(ref mut block) = self.active_block {
                    let found = block
                        .responses
                        .iter_mut()
                        .rev()
                        .find(|m| m.role == MessageRole::ToolResult && m.tool_name.as_deref() == Some(call_id));
                    if let Some(msg) = found {
                        msg.content = display;
                        msg.is_error = *is_error;
                        msg.tool_name = None;
                        msg.images = images;
                    } else {
                        block.responses.push(DisplayMessage {
                            role: MessageRole::ToolResult,
                            content: display,
                            tool_name: None,
                            is_error: *is_error,
                            images,
                        });
                    }
                }
            }
            AgentEvent::UsageUpdate {
                cumulative_usage,
                turn_usage,
                ..
            } => {
                self.total_tokens = cumulative_usage.total_tokens();
                // Pull real cost from tracker if available
                if let Some(ref ct) = self.cost_tracker {
                    self.total_cost = ct.total_cost();
                }
                if let Some(ref mut block) = self.active_block {
                    block.tokens = block.tokens.saturating_add(turn_usage.total_tokens());
                }
                // Update context gauge with cumulative input/output tokens
                self.context_gauge.update(
                    cumulative_usage.input_tokens,
                    cumulative_usage.output_tokens,
                    cumulative_usage.cache_creation_input_tokens,
                    cumulative_usage.cache_read_input_tokens,
                );
            }
            AgentEvent::UserInput { text, agent_msg_count } => {
                // Start a new block for this user input
                self.start_block(text.clone(), *agent_msg_count);
                self.scroll.scroll_to_bottom();
            }
            AgentEvent::SessionCompaction {
                compacted_count,
                tokens_saved,
            } => {
                self.push_system(
                    format!(
                        "Auto-compacted {} messages, saved ~{} tokens.",
                        compacted_count, tokens_saved,
                    ),
                    false,
                );
            }
            _ => {}
        }
    }

    // ── File activity tracking ─────────────────────────

    /// Extract file paths from tool call inputs and record them
    fn track_file_activity(&mut self, tool_name: &str, input: &serde_json::Value) {
        use super::components::file_activity_panel::FileOp;

        let op = match tool_name {
            "read" => FileOp::Read,
            "edit" => FileOp::Edit,
            "write" => FileOp::Write,
            _ => return,
        };

        // All file tools use "path" as the file argument
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            // For write, check if the file exists to distinguish create vs write
            let actual_op = if op == FileOp::Write && !std::path::Path::new(path).exists() {
                FileOp::Create
            } else {
                op
            };
            if let Some(fap) = self.panels.downcast_mut::<super::components::file_activity_panel::FileActivityPanel>(super::panel::PanelId::Files) {
                fap.record(path.to_string(), actual_op);
            }
        }
    }

    // ── Block navigation ─────────────────────────────

    /// Focus the previous block
    pub fn focus_prev_block(&mut self) {
        let conv_ids: Vec<usize> = self.conversation_block_ids();
        if conv_ids.is_empty() {
            return;
        }
        match self.focused_block {
            None => {
                self.focused_block = conv_ids.last().copied();
            }
            Some(current) => {
                if let Some(pos) = conv_ids.iter().position(|&id| id == current) {
                    if pos > 0 {
                        self.focused_block = Some(conv_ids[pos - 1]);
                    }
                    // At the first block — stay put
                } else {
                    self.focused_block = conv_ids.last().copied();
                }
            }
        }
        self.scroll.auto_scroll = false;
    }

    /// Focus the next block
    pub fn focus_next_block(&mut self) {
        let conv_ids: Vec<usize> = self.conversation_block_ids();
        if conv_ids.is_empty() {
            return;
        }
        match self.focused_block {
            None => {
                // Start from the bottom (most recent block) since the user
                // is already scrolled to the bottom when unfocused.
                self.focused_block = conv_ids.last().copied();
                self.scroll.auto_scroll = false;
            }
            Some(current) => {
                if let Some(pos) = conv_ids.iter().position(|&id| id == current) {
                    if pos + 1 < conv_ids.len() {
                        self.focused_block = Some(conv_ids[pos + 1]);
                    } else {
                        // Past the last block — unfocus and return to auto-scroll
                        self.focused_block = None;
                        self.scroll.scroll_to_bottom();
                    }
                } else {
                    self.focused_block = conv_ids.last().copied();
                    self.scroll.auto_scroll = false;
                }
            }
        }
    }

    /// Toggle collapse on the focused block
    pub fn toggle_focused_block(&mut self) {
        if let Some(id) = self.focused_block {
            for entry in &mut self.blocks {
                if let BlockEntry::Conversation(block) = entry
                    && block.id == id
                {
                    block.toggle_collapse();
                    return;
                }
            }
        }
    }

    /// Collapse all conversation blocks
    pub fn collapse_all_blocks(&mut self) {
        for entry in &mut self.blocks {
            if let BlockEntry::Conversation(block) = entry {
                block.collapsed = true;
            }
        }
    }

    /// Expand all conversation blocks
    pub fn expand_all_blocks(&mut self) {
        for entry in &mut self.blocks {
            if let BlockEntry::Conversation(block) = entry {
                block.collapsed = false;
            }
        }
    }

    /// Copy the focused block's content to the clipboard
    pub fn copy_focused_block(&self) {
        if let Some(id) = self.focused_block {
            for entry in &self.blocks {
                if let BlockEntry::Conversation(block) = entry
                    && block.id == id
                {
                    let mut text = String::new();
                    for msg in &block.responses {
                        if msg.role == MessageRole::Assistant {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(&msg.content);
                        }
                    }
                    if !text.is_empty() {
                        crate::tui::selection::copy_to_clipboard(&text);
                    }
                    return;
                }
            }
        }
    }

    /// Get the prompt from the focused block (for re-running)
    pub fn get_focused_block_prompt(&self) -> Option<String> {
        let id = self.focused_block?;
        for entry in &self.blocks {
            if let BlockEntry::Conversation(block) = entry
                && block.id == id
            {
                return Some(block.prompt.clone());
            }
        }
        None
    }

    /// Get IDs of all conversation blocks in order
    fn conversation_block_ids(&self) -> Vec<usize> {
        self.blocks
            .iter()
            .filter_map(|entry| match entry {
                BlockEntry::Conversation(block) => Some(block.id),
                _ => None,
            })
            .collect()
    }

    // ── Branching ────────────────────────────────────

    /// Get the sibling info for a block: (current_index, total_siblings)
    /// Siblings are blocks that share the same parent_block_id.
    pub fn block_siblings(&self, block_id: usize) -> (usize, usize) {
        let block = match self.all_blocks.iter().find(|b| b.id == block_id) {
            Some(b) => b,
            None => return (0, 1),
        };
        let parent = block.parent_block_id;
        let siblings: Vec<usize> =
            self.all_blocks.iter().filter(|b| b.parent_block_id == parent).map(|b| b.id).collect();
        let idx = siblings.iter().position(|&id| id == block_id).unwrap_or(0);
        (idx, siblings.len())
    }

    /// Count how many child blocks branch from the given block.
    /// Returns 0 for leaf blocks, >1 means this block is a branch point.
    pub fn block_children_count(&self, block_id: usize) -> usize {
        self.all_blocks.iter().filter(|b| b.parent_block_id == Some(block_id)).count()
    }

    /// Edit the focused block's prompt: pre-fill the editor and set up a
    /// pending branch operation. Returns true if a branch edit was initiated.
    pub fn edit_focused_block_prompt(&mut self) -> bool {
        let id = match self.focused_block {
            Some(id) => id,
            None => return false,
        };
        let block = match self.all_blocks.iter().find(|b| b.id == id) {
            Some(b) => b.clone(),
            None => return false,
        };
        // Pre-fill the editor with the prompt text
        self.editor.clear();
        for c in block.prompt.chars() {
            self.editor.insert_char(c);
        }
        // Store the pending branch info: we'll branch from this block's parent
        // using this block's agent_msg_checkpoint (the message count before it)
        self.pending_branch = Some((id, String::new())); // prompt will be filled on submit
        self.focused_block = None;
        true
    }

    /// If there's a pending branch, finalize it with the submitted prompt.
    /// Returns Some((checkpoint, prompt)) to tell the event loop to truncate and re-prompt.
    pub fn take_pending_branch(&mut self, submitted_prompt: &str) -> Option<(usize, String)> {
        let (fork_block_id, _) = self.pending_branch.take()?;
        let fork_block = self.all_blocks.iter().find(|b| b.id == fork_block_id)?;
        let checkpoint = fork_block.agent_msg_checkpoint;
        // Remove all blocks from the visible list that come at or after the fork point.
        let mut keep_up_to = self.blocks.len();
        for (i, entry) in self.blocks.iter().enumerate() {
            if let BlockEntry::Conversation(b) = entry
                && b.id == fork_block_id
            {
                keep_up_to = i;
                break;
            }
        }
        self.blocks.truncate(keep_up_to);

        // Signal the event loop to record a branch in the session file
        self.last_branch_checkpoint = Some(checkpoint);

        Some((checkpoint, submitted_prompt.to_string()))
    }

    /// Navigate to the previous sibling branch at the focused block
    pub fn branch_prev(&mut self) {
        if let Some(id) = self.focused_block
            && let Some(sibling_id) = self.adjacent_sibling(id, -1)
        {
            self.switch_to_branch(sibling_id);
        }
    }

    /// Navigate to the next sibling branch at the focused block
    pub fn branch_next(&mut self) {
        if let Some(id) = self.focused_block
            && let Some(sibling_id) = self.adjacent_sibling(id, 1)
        {
            self.switch_to_branch(sibling_id);
        }
    }

    /// Find the sibling block offset positions from the given block.
    fn adjacent_sibling(&self, block_id: usize, offset: isize) -> Option<usize> {
        let block = self.all_blocks.iter().find(|b| b.id == block_id)?;
        let parent = block.parent_block_id;
        let siblings: Vec<usize> =
            self.all_blocks.iter().filter(|b| b.parent_block_id == parent).map(|b| b.id).collect();
        let idx = siblings.iter().position(|&id| id == block_id)? as isize;
        let new_idx = idx + offset;
        if new_idx >= 0 && (new_idx as usize) < siblings.len() {
            Some(siblings[new_idx as usize])
        } else {
            None
        }
    }

    /// Switch the visible block list to show the branch containing `target_block_id`.
    /// Rebuilds `self.blocks` to show the path from root through target and all its descendants.
    pub fn switch_to_branch(&mut self, target_block_id: usize) {
        // Walk up from target to root to find the full ancestor path
        let mut path_up = Vec::new();
        let mut current = Some(target_block_id);
        while let Some(id) = current {
            path_up.push(id);
            current = self.all_blocks.iter().find(|b| b.id == id).and_then(|b| b.parent_block_id);
        }
        path_up.reverse(); // now root → ... → target

        // Walk down from target following the latest child at each level
        let mut path = path_up;
        let mut leaf = target_block_id;
        loop {
            // Find children of leaf (blocks whose parent_block_id == Some(leaf))
            let children: Vec<usize> =
                self.all_blocks.iter().filter(|b| b.parent_block_id == Some(leaf)).map(|b| b.id).collect();
            if let Some(&last_child) = children.last() {
                path.push(last_child);
                leaf = last_child;
            } else {
                break;
            }
        }

        // Rebuild self.blocks: keep system messages at their positions,
        // replace conversation blocks with the path
        let system_msgs: Vec<BlockEntry> =
            self.blocks.iter().filter(|e| matches!(e, BlockEntry::System(_))).cloned().collect();

        self.blocks.clear();
        // Re-add system messages that were before the first conversation block
        // For simplicity, put system messages first, then the branch path
        for sys in system_msgs {
            self.blocks.push(sys);
        }
        for &block_id in &path {
            if let Some(block) = self.all_blocks.iter().find(|b| b.id == block_id) {
                self.blocks.push(BlockEntry::Conversation(block.clone()));
            }
        }

        self.focused_block = Some(target_block_id);
        self.scroll.auto_scroll = false;
    }

    // ── Mouse hit-testing ─────────────────────────────

    /// Determine which UI region a screen coordinate falls in.
    pub fn hit_test(&self, col: u16, row: u16) -> HitRegion {
        // Check editor area first (it overlaps with "main column")
        if rect_contains(self.editor_area, col, row) {
            return HitRegion::Editor;
        }
        // Check status bar
        if rect_contains(self.status_area, col, row) {
            return HitRegion::StatusBar;
        }
        // Check panes via hypertile geometry
        for pane in self.tiling.panes() {
            if rect_contains(pane.rect, col, row) {
                match self.pane_registry.kind(pane.id) {
                    Some(super::panes::PaneKind::Panel(panel_id)) => {
                        return HitRegion::Panel(*panel_id);
                    }
                    Some(super::panes::PaneKind::Subagent(id)) => {
                        return HitRegion::Subagent(id.clone());
                    }
                    Some(super::panes::PaneKind::Chat) => {
                        // Fall through to messages/editor checks below
                    }
                    _ => {}
                }
            }
        }
        // Check messages area
        if rect_contains(self.messages_area, col, row) {
            return HitRegion::Messages;
        }
        HitRegion::None
    }

    /// Submit the current editor content and take any pending images
    pub fn submit_input(&mut self) -> Option<String> {
        self.slash_menu.hide();
        self.editor.submit()
    }

    /// Take pending images, clearing the list
    pub fn take_pending_images(&mut self) -> Vec<PendingImage> {
        std::mem::take(&mut self.pending_images)
    }

    /// Add an image attachment from clipboard data
    pub fn attach_image(&mut self, data: String, media_type: String, size: usize) {
        self.pending_images.push(PendingImage { data, media_type, size });
    }

    /// Remove all pending image attachments
    pub fn clear_pending_images(&mut self) {
        self.pending_images.clear();
    }

    /// Update the slash menu based on current editor content
    pub fn update_slash_menu(&mut self) {
        let content = self.editor.content().join("\n");
        if self.editor.line_count() == 1 && content.starts_with('/') && !content.contains('\n') {
            self.slash_menu.update(&self.slash_registry, &content);
        } else {
            self.slash_menu.hide();
        }
    }

    /// Accept the selected slash menu item, replacing editor content
    pub fn accept_slash_completion(&mut self) -> bool {
        if let Some((insert_text, trailing_space)) = self.slash_menu.accept() {
            self.editor.clear();
            let cmd = format!("/{}", insert_text);
            for c in cmd.chars() {
                self.editor.insert_char(c);
            }
            if trailing_space {
                self.editor.insert_char(' ');
            }
            true
        } else {
            false
        }
    }
}

// ── Hit-testing helpers ──────────────────────────────────────────────────────

/// Which UI region a mouse event landed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitRegion {
    /// The main messages / chat area
    Messages,
    /// The text editor / input area
    Editor,
    /// A side panel
    Panel(PanelId),
    /// A subagent's dedicated pane
    Subagent(String),
    /// The status bar
    StatusBar,
    /// Outside any tracked region
    None,
}

/// Check whether a screen coordinate (col, row) is inside a `Rect`.
fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x && col < area.x + area.width && row >= area.y && row < area.y + area.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains_inside() {
        let area = Rect::new(10, 5, 20, 10);
        assert!(rect_contains(area, 10, 5)); // top-left corner
        assert!(rect_contains(area, 15, 8)); // middle
        assert!(rect_contains(area, 29, 14)); // bottom-right (just inside)
    }

    #[test]
    fn test_rect_contains_outside() {
        let area = Rect::new(10, 5, 20, 10);
        assert!(!rect_contains(area, 9, 5)); // left of area
        assert!(!rect_contains(area, 10, 4)); // above area
        assert!(!rect_contains(area, 30, 5)); // right edge (exclusive)
        assert!(!rect_contains(area, 10, 15)); // bottom edge (exclusive)
    }

    #[test]
    fn test_rect_contains_zero_size() {
        let area = Rect::new(5, 5, 0, 0);
        assert!(!rect_contains(area, 5, 5)); // zero-size rect contains nothing
    }

    #[test]
    fn test_hit_region_editor_wins_over_messages() {
        // Editor and messages are both in the "main column" — editor should
        // win because it's checked first.
        let theme = crate::tui::theme::Theme::dark();
        let mut app = App::new("test".to_string(), "/tmp".to_string(), theme);
        app.messages_area = Rect::new(20, 0, 60, 40);
        app.editor_area = Rect::new(20, 35, 60, 5);
        app.status_area = Rect::new(20, 40, 60, 1);
        // Compute hypertile layout so pane rects are populated
        app.tiling.compute_layout(Rect::new(0, 0, 100, 41));

        // Click in the editor area
        assert_eq!(app.hit_test(30, 37), HitRegion::Editor);
        // Click in the messages area (above editor)
        assert_eq!(app.hit_test(30, 10), HitRegion::Messages);
        // Click in a panel (Todo is in the left column)
        assert_eq!(app.hit_test(5, 5), HitRegion::Panel(PanelId::Todo));
        assert_eq!(app.hit_test(5, 25), HitRegion::Panel(PanelId::Files));
        // Click on status bar
        assert_eq!(app.hit_test(30, 40), HitRegion::StatusBar);
    }
}
