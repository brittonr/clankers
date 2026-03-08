//! Main application state machine

mod agent_events;
mod block_nav;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ratatui::layout::Rect;

use crate::config::keybindings::InputMode;
use crate::plugin::ui::PluginUIState;
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

/// Streaming state — tracks in-progress LLM/tool output
#[derive(Debug, Default)]
pub struct StreamingState {
    /// Accumulated streaming text (flushed into active block on boundaries)
    pub text: String,
    /// Accumulated streaming thinking (flushed into active block on boundaries)
    pub thinking: String,
    /// Current streaming content block index
    pub block_index: Option<usize>,
    /// Active tool executions keyed by call_id
    pub active_tools: HashMap<String, ActiveToolExecution>,
    /// Structured progress renderer for active tool executions
    pub progress_renderer: super::components::progress_renderer::ProgressRenderer,
    /// Scrollable streaming output buffers for in-progress tools
    pub outputs: super::components::streaming_output::StreamingOutputManager,
    /// Which tool output (call_id) is currently focused for scroll control
    pub focused_tool: Option<String>,
}

/// Conversation state — blocks, scroll, and focus
pub struct ConversationState {
    /// Block-oriented conversation history (only active branch shown)
    pub blocks: Vec<BlockEntry>,
    /// All conversation blocks ever created (including branched-off)
    pub all_blocks: Vec<ConversationBlock>,
    /// The in-progress block being streamed into
    pub active_block: Option<ConversationBlock>,
    /// Next block ID
    pub next_block_id: usize,
    /// Index of the currently focused block (for keyboard navigation)
    pub focused_block: Option<usize>,
    /// Message scroll state
    pub scroll: MessageScroll,
}

/// Branching state — fork, compare, merge overlays
pub struct BranchingState {
    /// Pending branch operation: Some((fork_block_id, new_prompt))
    pub pending_branch: Option<(usize, String)>,
    /// Branch checkpoint that was just executed
    pub last_branch_checkpoint: Option<usize>,
    /// Branch switcher overlay (quick fuzzy picker)
    pub switcher: super::components::branch_switcher::BranchSwitcher,
    /// Branch comparison overlay (side-by-side diff)
    pub compare: super::components::branch_compare::BranchCompareView,
    /// Interactive merge overlay (checkbox message selection)
    pub merge_interactive: super::components::merge_interactive::MergeInteractiveView,
}

/// Overlay/popup state — all modal dialogs and selectors
pub struct OverlayState {
    /// Whether the session/branch popup is visible
    pub session_popup_visible: bool,
    /// Whether the cost detail overlay is visible
    pub cost_overlay_visible: bool,
    /// Whether to show block IDs in conversation view
    pub show_block_ids: bool,
    /// Plan mode state
    pub plan_state: crate::modes::plan::PlanState,
    /// History search overlay (Ctrl+R)
    pub history_search: super::components::history_search::HistorySearch,
    /// Output search overlay (Ctrl+F / f)
    pub output_search: super::components::output_search::OutputSearch,
    /// Model selector popup
    pub model_selector: ModelSelector,
    /// Account selector popup
    pub account_selector: AccountSelector,
    /// Session resume selector popup
    pub session_selector: SessionSelector,
    /// Leader key (Space) popup menu
    pub leader_menu: super::components::leader_menu::LeaderMenu,
}

/// Tiling state — BSP layout and pane focus
pub struct TilingState {
    /// BSP tiling engine
    pub tiling: ratatui_hypertile::Hypertile,
    /// Maps hypertile PaneIds to their content type
    pub pane_registry: super::panes::PaneRegistry,
    /// Which panel (if any) currently has focus
    pub focused_panel: Option<PanelId>,
    /// Which subagent pane (if any) currently has focus
    pub focused_subagent: Option<String>,
    /// Per-subagent pane manager
    pub subagent_panes: super::components::subagent_pane::SubagentPaneManager,
    /// Saved tiling state while a pane is zoomed
    pub zoom_state: Option<ZoomState>,
}

/// Main TUI application
pub struct App {
    // Core state (keep flat)
    pub state: AppState,
    pub input_mode: InputMode,
    pub theme: Theme,
    pub editor: Editor,
    pub model: String,
    pub session_id: String,
    pub total_tokens: usize,
    pub total_cost: f64,
    pub cost_tracker: Option<Arc<crate::routing::cost_tracker::CostTracker>>,
    pub cwd: String,
    pub should_quit: bool,
    pub thinking_enabled: bool,
    pub thinking_level: crate::provider::ThinkingLevel,
    pub show_thinking: bool,
    pub tool_info: Vec<(String, String, String)>,
    pub original_system_prompt: String,
    pub open_editor_requested: bool,
    pub active_account: String,
    pub available_models: Vec<String>,
    pub router_status: RouterStatus,
    pub queued_prompt: Option<String>,
    pub login_verifier: Option<(String, String)>,
    pub tick: u64,

    // Grouped sub-states
    pub streaming: StreamingState,
    pub conversation: ConversationState,
    pub branching: BranchingState,
    pub overlays: OverlayState,
    pub layout: TilingState,

    // Components (keep flat — they're already self-contained)
    pub slash_menu: SlashMenu,
    pub slash_registry: crate::slash_commands::SlashRegistry,
    pub action_registry: crate::config::keybindings::ActionRegistry,
    pub plugin_ui: PluginUIState,
    pub panels: super::panel::PanelManager,
    pub context_gauge: super::components::context_gauge::ContextGauge,
    pub git_status: super::components::git_status::GitStatus,

    // Render/hit-test areas
    pub selection: Option<TextSelection>,
    pub rendered_lines: Vec<String>,
    pub messages_area: Rect,
    pub editor_area: Rect,
    pub status_area: Rect,

    // Clipboard
    pub pending_images: Vec<PendingImage>,
    pub clipboard_pending: bool,
    pub clipboard_rx: Option<std::sync::mpsc::Receiver<crate::modes::clipboard::ClipboardResult>>,
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
            // Core state
            state: AppState::Idle,
            input_mode: InputMode::Normal,
            theme,
            editor: Editor::new(),
            model,
            session_id: String::new(),
            total_tokens: 0,
            total_cost: 0.0,
            cost_tracker: None,
            cwd,
            should_quit: false,
            thinking_enabled: false,
            thinking_level: crate::provider::ThinkingLevel::Off,
            show_thinking: true,
            tool_info: Vec::new(),
            original_system_prompt: String::new(),
            open_editor_requested: false,
            active_account: String::new(),
            available_models: Vec::new(),
            router_status: RouterStatus::Disconnected,
            queued_prompt: None,
            login_verifier: None,
            tick: 0,

            // Grouped sub-states
            streaming: StreamingState {
                text: String::new(),
                thinking: String::new(),
                block_index: None,
                active_tools: HashMap::new(),
                progress_renderer: super::components::progress_renderer::ProgressRenderer::new(),
                outputs: super::components::streaming_output::StreamingOutputManager::new(),
                focused_tool: None,
            },
            conversation: ConversationState {
                blocks: Vec::new(),
                all_blocks: Vec::new(),
                active_block: None,
                next_block_id: 0,
                focused_block: None,
                scroll: MessageScroll::new(),
            },
            branching: BranchingState {
                pending_branch: None,
                last_branch_checkpoint: None,
                switcher: super::components::branch_switcher::BranchSwitcher::new(),
                compare: super::components::branch_compare::BranchCompareView::new(),
                merge_interactive: super::components::merge_interactive::MergeInteractiveView::new(),
            },
            overlays: OverlayState {
                session_popup_visible: false,
                cost_overlay_visible: false,
                show_block_ids: false,
                plan_state: crate::modes::plan::PlanState::Inactive,
                history_search: super::components::history_search::HistorySearch::new(),
                output_search: super::components::output_search::OutputSearch::new(),
                model_selector: ModelSelector::new(Vec::new()),
                account_selector: AccountSelector::new(),
                session_selector: SessionSelector::new(),
                leader_menu: super::components::leader_menu::LeaderMenu::new(),
            },
            layout: TilingState {
                tiling: super::panes::default_tiling(),
                pane_registry: super::panes::default_registry(),
                focused_panel: None,
                focused_subagent: None,
                subagent_panes: super::components::subagent_pane::SubagentPaneManager::new(),
                zoom_state: None,
            },

            // Components
            slash_menu: SlashMenu::new(),
            slash_registry: build_default_slash_registry(),
            action_registry: crate::config::keybindings::ActionRegistry::new(),
            plugin_ui: PluginUIState::new(),
            panels: register_default_panels(),
            context_gauge,
            git_status,

            // Render/hit-test areas
            selection: None,
            rendered_lines: Vec::new(),
            messages_area: Rect::default(),
            editor_area: Rect::default(),
            status_area: Rect::default(),

            // Clipboard
            pending_images: Vec::new(),
            clipboard_pending: false,
            clipboard_rx: None,
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
            match pm_arc.lock() {
                Ok(guard) => {
                    pm_guard = guard;
                    contributors.push(&*pm_guard);
                }
                Err(poisoned) => {
                    tracing::warn!("plugin manager mutex poisoned in rebuild_leader_menu, recovering");
                    pm_guard = poisoned.into_inner();
                    contributors.push(&*pm_guard);
                }
            }
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

        self.overlays.leader_menu = menu;
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
            match pm_arc.lock() {
                Ok(guard) => {
                    pm_guard = guard;
                    contributors.push(&*pm_guard);
                }
                Err(poisoned) => {
                    tracing::warn!("plugin manager mutex poisoned in rebuild_slash_registry, recovering");
                    pm_guard = poisoned.into_inner();
                    contributors.push(&*pm_guard);
                }
            }
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
    pub fn panel(&self, id: super::panel::PanelId) -> Option<&dyn super::panel::Panel> {
        self.panels.get(id)
    }

    /// Get a panel by ID (mutable) for key handling.
    pub fn panel_mut(&mut self, id: super::panel::PanelId) -> Option<&mut dyn super::panel::Panel> {
        if self.panels.get_mut(id).is_none() {
            tracing::error!(panel_id = ?id, "attempted to access unregistered panel");
        }
        self.panels.get_mut(id)
    }

    /// Close any detail/diff views on the focused panel before unfocusing.
    /// Panels like Subagents and Files have sub-views that should reset
    /// when the user exits the panel.
    pub fn close_focused_panel_views(&mut self) {
        if let Some(id) = self.layout.focused_panel
            && let Some(panel) = self.panel_mut(id)
        {
            panel.close_detail_view();
        }
        // Subagent panes have no detail view to close, but clear the focus
    }

    // ── Focus helpers (bridge hypertile ↔ PanelId) ──────────────────

    /// Whether a side panel or subagent pane (not chat) currently has focus.
    pub fn has_panel_focus(&self) -> bool {
        self.layout.focused_panel.is_some() || self.layout.focused_subagent.is_some()
    }

    /// Focus a specific panel by `PanelId`. Updates both hypertile and
    /// the `focused_panel` tracker.
    pub fn focus_panel(&mut self, panel_id: PanelId) {
        if let Some(pane) = self.layout.pane_registry.find_panel(panel_id) {
            let _ = self.layout.tiling.focus_pane(pane);
            self.layout.focused_panel = Some(panel_id);
            self.layout.focused_subagent = None;
            self.streaming.focused_tool = None;
            self.streaming.outputs.unfocus_all();
        }
    }

    /// Focus a specific subagent pane by its string ID.
    pub fn focus_subagent(&mut self, subagent_id: &str) {
        if let Some(pane_id) = self.layout.subagent_panes.pane_id_for(subagent_id) {
            let _ = self.layout.tiling.focus_pane(pane_id);
            self.layout.focused_subagent = Some(subagent_id.to_string());
            self.layout.focused_panel = None;
            self.streaming.focused_tool = None;
            self.streaming.outputs.unfocus_all();
        }
    }

    /// Return focus to the chat pane (unfocus any panel, subagent, or tool output).
    pub fn unfocus_panel(&mut self) {
        let chat = self.layout.pane_registry.chat_pane();
        let _ = self.layout.tiling.focus_pane(chat);
        self.layout.focused_panel = None;
        self.layout.focused_subagent = None;
        self.streaming.focused_tool = None;
        self.streaming.outputs.unfocus_all();
    }

    /// Unfocus the currently focused tool output.
    pub fn unfocus_tool(&mut self) {
        self.streaming.focused_tool = None;
        self.streaming.outputs.unfocus_all();
    }

    /// Is the given panel currently focused?
    pub fn is_panel_focused(&self, panel_id: PanelId) -> bool {
        self.layout.focused_panel == Some(panel_id)
    }

    /// Apply a hypertile tiling action (focus, resize, etc.) and sync
    /// our `focused_panel` tracker from the resulting hypertile state.
    pub fn apply_tiling_action(&mut self, action: ratatui_hypertile::HypertileAction) {
        self.layout.tiling.apply_action(action);
        self.sync_focused_panel();
    }

    /// Sync `focused_panel` and `focused_subagent` from hypertile's current focus.
    pub fn sync_focused_panel(&mut self) {
        self.layout.focused_panel = None;
        self.layout.focused_subagent = None;
        if let Some(pane_id) = self.layout.tiling.focused_pane() {
            match self.layout.pane_registry.kind(pane_id) {
                Some(super::panes::PaneKind::Panel(panel_id)) => {
                    self.layout.focused_panel = Some(*panel_id);
                }
                Some(super::panes::PaneKind::Subagent(id)) => {
                    self.layout.focused_subagent = Some(id.clone());
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
        if let Some(focused) = self.layout.tiling.focused_pane()
            && self.layout.pane_registry.is_chat(focused)
        {
            return;
        }

        if let Ok(new_id) = self.layout.tiling.split_focused(direction) {
            // The new pane starts as Empty. The old pane keeps its content.
            self.layout.pane_registry.register(new_id, PaneKind::Empty);
            self.sync_focused_panel();
        }
    }

    /// Close the focused pane and remove it from the registry.
    /// The chat pane (ROOT) cannot be closed.
    pub fn close_focused_pane(&mut self) {
        // Don't close the chat pane.
        if let Some(focused) = self.layout.tiling.focused_pane()
            && self.layout.pane_registry.is_chat(focused)
        {
            return;
        }

        if let Ok(removed_id) = self.layout.tiling.close_focused() {
            self.layout.pane_registry.unregister(removed_id);
            self.sync_focused_panel();
        }
    }

    // ── Zoom (temporary full-screen focus) ────────────────────────

    /// Whether any pane is currently zoomed.
    pub fn is_zoomed(&self) -> bool {
        self.layout.zoom_state.is_some()
    }

    /// Zoom the currently focused pane to fill the entire terminal.
    /// Saves the current tiling so it can be restored with `zoom_restore`.
    /// If already zoomed, this is a no-op.
    pub fn zoom_focused(&mut self) {
        if self.layout.zoom_state.is_some() {
            return;
        }

        let Some(focused_pane) = self.layout.tiling.focused_pane() else {
            return;
        };

        // Save current state.
        self.layout.zoom_state = Some(ZoomState {
            tiling: self.layout.tiling.clone(),
            registry: self.layout.pane_registry.clone(),
            focused_panel: self.layout.focused_panel,
            focused_subagent: self.layout.focused_subagent.clone(),
        });

        // Build a single-pane tree with the focused pane at root.
        let mut zoomed = ratatui_hypertile::Hypertile::new();
        // Hypertile::new() creates ROOT as the only pane. We need to
        // make the registry map ROOT to whatever the focused pane held.
        let kind = self.layout.pane_registry.kind(focused_pane).cloned()
            .unwrap_or(super::panes::PaneKind::Empty);

        let mut reg = super::panes::PaneRegistry::new();
        // ROOT is already Chat in a fresh registry — override it.
        reg.register(ratatui_hypertile::PaneId::ROOT, kind.clone());

        // If the zoomed pane was Chat, keep chat_pane as ROOT (already the case).
        // If it was a Panel, we need a registry where ROOT maps to that panel
        // and chat_pane is still ROOT (PaneRegistry enforces this). That's fine —
        // the renderer will see ROOT → Panel and skip the chat render path.

        let _ = zoomed.focus_pane(ratatui_hypertile::PaneId::ROOT);
        self.layout.tiling = zoomed;
        self.layout.pane_registry = reg;

        // Sync focused_panel/focused_subagent to match the zoomed pane's content.
        self.layout.focused_subagent = None;
        match kind {
            super::panes::PaneKind::Panel(id) => self.layout.focused_panel = Some(id),
            super::panes::PaneKind::Subagent(ref id) => {
                self.layout.focused_panel = None;
                self.layout.focused_subagent = Some(id.clone());
            }
            _ => self.layout.focused_panel = None,
        }
    }

    /// Restore the tiling layout from before `zoom_focused` was called.
    /// No-op if not zoomed.
    pub fn zoom_restore(&mut self) {
        let Some(saved) = self.layout.zoom_state.take() else {
            return;
        };
        self.layout.tiling = saved.tiling;
        self.layout.pane_registry = saved.registry;
        self.layout.focused_panel = saved.focused_panel;
        self.layout.focused_subagent = saved.focused_subagent;
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
        self.conversation.blocks.push(BlockEntry::System(DisplayMessage {
            role: MessageRole::System,
            content,
            tool_name: None,
            is_error,
            images: Vec::new(),
        }));
        self.conversation.scroll.scroll_to_bottom();
    }

    /// Start a new conversation block for the given user prompt.
    /// `agent_msg_count` is the number of agent messages *before* this block's
    /// user message is appended (used for branching truncation).
    pub fn start_block(&mut self, prompt: String, agent_msg_count: usize) {
        self.finalize_active_block();

        // Determine parent: the last conversation block on the visible list
        let parent_id = self.conversation.blocks.iter().rev().find_map(|e| match e {
            BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        });

        let mut block = ConversationBlock::new(self.conversation.next_block_id, prompt);
        block.parent_block_id = parent_id;
        block.agent_msg_checkpoint = agent_msg_count;
        self.conversation.next_block_id += 1;
        self.conversation.active_block = Some(block);
    }

    /// Finalize the active block and move it to the completed list
    pub fn finalize_active_block(&mut self) {
        // Flush any in-progress streaming content first
        self.flush_streaming_thinking();
        self.flush_streaming_text();

        if let Some(mut block) = self.conversation.active_block.take() {
            block.streaming = false;
            // Store in both the active view and the full block history
            self.conversation.all_blocks.push(block.clone());
            self.conversation.blocks.push(BlockEntry::Conversation(block));

            // Refresh branch panel if it has entries (i.e., has been opened before)
            if let Some(bp) = self.panels.downcast_ref::<super::components::branch_panel::BranchPanel>(super::panel::PanelId::Branches)
                && !bp.entries.is_empty()
            {
                let active_ids: std::collections::HashSet<usize> = self
                    .conversation.blocks
                    .iter()
                    .filter_map(|e| match e {
                        BlockEntry::Conversation(b) => Some(b.id),
                        _ => None,
                    })
                    .collect();
                let all_blocks = self.conversation.all_blocks.clone();
                if let Some(bp) = self.panels.downcast_mut::<super::components::branch_panel::BranchPanel>(super::panel::PanelId::Branches) {
                    bp.refresh(&all_blocks, &active_ids);
                }
            }
        }
    }

    // Agent event handling is in app/agent_events.rs
    // Block navigation and branching is in app/block_nav.rs

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
        for pane in self.layout.tiling.panes() {
            if rect_contains(pane.rect, col, row) {
                match self.layout.pane_registry.kind(pane.id) {
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

// ── Constructor helpers ──────────────────────────────────────────────────────

/// Register all default panels (todo, files, subagents, peers, processes, branches).
fn register_default_panels() -> super::panel::PanelManager {
    let mut pm = super::panel::PanelManager::new();
    pm.register(Box::new(super::components::todo_panel::TodoPanel::new()));
    pm.register(Box::new(super::components::file_activity_panel::FileActivityPanel::new()));
    pm.register(Box::new(super::components::subagent_panel::SubagentPanel::new()));
    pm.register(Box::new(super::components::peers_panel::PeersPanel::new()));
    pm.register(Box::new(super::components::process_panel::ProcessPanel::new()));
    pm.register(Box::new(super::components::branch_panel::BranchPanel::new()));
    pm
}

/// Build the default slash command registry with builtin commands.
fn build_default_slash_registry() -> crate::slash_commands::SlashRegistry {
    use crate::slash_commands::{BuiltinSlashContributor, SlashContributor, SlashRegistry};
    
    let builtin = BuiltinSlashContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    let (registry, conflicts) = SlashRegistry::build(&contributors);
    
    for c in &conflicts {
        tracing::debug!(
            registry = c.registry,
            key = %c.key,
            winner = %c.winner,
            loser = %c.loser,
            "slash command conflict (init)"
        );
    }
    
    registry
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
        app.layout.tiling.compute_layout(Rect::new(0, 0, 100, 41));

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
