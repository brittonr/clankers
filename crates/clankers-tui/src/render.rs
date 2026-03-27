//! Top-level layout renderer
//!
//! Uses a two-level layout approach:
//! 1. The main terminal area is split into two parts:
//!    - Upper area: BSP tiling for panels and chat content
//!    - Lower area: Fixed input editor and status bar
//!
//! 2. The BSP tiling system manages the dynamic layout of side panels and chat content, while input
//!    and status remain fixed at the bottom.
//!
//! This separation allows the input area and status bar to remain stable
//! regardless of panel configuration changes.

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::app::AppState;
use crate::components::block_view;
use crate::components::cost_overlay;
use crate::components::editor as editor_component;
use crate::components::session_panel;
use crate::components::slash_menu;
use crate::components::status_bar::StatusBarData;
use crate::components::status_bar::{self};
use crate::panel::DrawContext;
use crate::panes::PaneKind;
use crate::widget_host;

/// Render the full application UI
pub fn render(frame: &mut Frame, app: &mut App) {
    // Advance animation tick (drives spinners and other animated elements)
    app.advance_tick();

    // Garbage-collect expired plugin notifications
    app.plugin_ui.gc_notifications();

    // Refresh git status periodically
    app.git_status.maybe_refresh();

    // Sync the cwd into file_activity_panel so it can shorten paths
    if let Some(fap) = app
        .panels
        .downcast_mut::<crate::components::file_activity_panel::FileActivityPanel>(crate::panel::PanelId::Files)
        && fap.cwd != app.cwd
    {
        fap.cwd.clone_from(&app.cwd);
    }

    // Refresh process panel entries from monitor
    if let Some(pp) = app
        .panels
        .downcast_mut::<crate::components::process_panel::ProcessPanel>(crate::panel::PanelId::Processes)
    {
        pp.refresh_entries();
    }

    // ── Calculate input and status bar heights ──────────────────────

    let indicator = compute_input_indicator(app.state, app.input_mode);
    let inner_width = frame.area().width.saturating_sub(2) as usize;
    let visual_lines = app.editor.visual_line_count(inner_width, indicator.len()) as u16;
    let editor_height = (visual_lines + 2).clamp(3, 10);
    let status_bar_height = 1;

    // ── Split frame into main area and bottom area ──────────────────

    let main_constraints = vec![
        Constraint::Min(3),                                    // main area (panels + chat)
        Constraint::Length(editor_height + status_bar_height), // input + status bar
    ];

    let main_chunks =
        Layout::default().direction(Direction::Vertical).constraints(main_constraints).split(frame.area());

    let panels_and_chat_area = main_chunks[0];
    let bottom_area = main_chunks[1];

    // ── Split bottom area into input and status bar ─────────────────

    let bottom_constraints = vec![
        Constraint::Length(editor_height),     // input/editor
        Constraint::Length(status_bar_height), // status bar
    ];

    let bottom_chunks =
        Layout::default().direction(Direction::Vertical).constraints(bottom_constraints).split(bottom_area);

    app.editor_area = bottom_chunks[0];
    app.status_area = bottom_chunks[1];

    // ── Compute BSP tiling layout for panels and chat ──────────────

    app.layout.tiling.compute_layout(panels_and_chat_area);

    // ── Render panels and get chat area ─────────────────────────────

    let (chat_area, is_chat_focused) = render_side_panels(frame, app);

    // ── Render main chat area ───────────────────────────────────────

    let border_color = if is_chat_focused { Color::Cyan } else { Color::DarkGray };
    let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color));
    let chat_inner = block.inner(chat_area);
    frame.render_widget(block, chat_area);

    render_chat_content(frame, app, chat_inner);

    // ── Panel navigation hint ───────────────────────────────────────

    if is_chat_focused {
        let hint = Span::styled(" h/l:panels j/k:panes ", Style::default().fg(Color::DarkGray));
        let hint_len = hint.width() as u16;
        let hint_area = Rect {
            x: chat_area.x + chat_area.width.saturating_sub(hint_len + 1),
            y: chat_area.y,
            width: hint_len.min(chat_area.width),
            height: 1,
        };
        frame.render_widget(Paragraph::new(hint), hint_area);
    } else if let Some(focused_pane) = app.layout.tiling.focused_pane() {
        // Show tiling hint on the focused panel's border
        if let Some(rect) = app.layout.tiling.pane_rect(focused_pane) {
            let hint_text = if app.is_zoomed() {
                " z:unzoom "
            } else {
                " j/k h/l:nav []:size |/-:split z:zoom "
            };
            let hint = Span::styled(hint_text, Style::default().fg(Color::DarkGray));
            let hint_len = hint.width() as u16;
            if rect.width > hint_len + 2 {
                let hint_area = Rect {
                    x: rect.x + rect.width.saturating_sub(hint_len + 1),
                    y: rect.y,
                    width: hint_len.min(rect.width),
                    height: 1,
                };
                frame.render_widget(Paragraph::new(hint), hint_area);
            }
        }
    }

    // ── Render input/editor area ────────────────────────────────────

    render_editor_area(frame, app, app.editor_area, indicator);

    // ── Render status bar ───────────────────────────────────────────

    render_status_bar_area(frame, app);

    // ── Overlays (rendered on top of everything) ────────────────────

    render_chrome(frame, app);
}

/// Render side panels and return the chat area and focus state
fn render_side_panels(frame: &mut Frame, app: &mut App) -> (Rect, bool) {
    // Collect pane snapshots first (to avoid borrow conflicts with app).
    let pane_snapshots: Vec<_> = app.layout.tiling.panes();
    let theme = app.theme.clone();
    let mut chat_area = Rect::default();
    let mut is_chat_focused = false;

    for pane in &pane_snapshots {
        match app.layout.pane_registry.kind(pane.id) {
            Some(PaneKind::Panel(panel_id)) => {
                let panel_id = *panel_id;
                let focused = app.is_panel_focused(panel_id);
                let ctx = DrawContext { theme: &theme, focused };
                if let Some(panel) = app.panel_mut(panel_id) {
                    crate::panel::draw_panel_scrolled(frame, panel, pane.rect, &ctx);
                } else {
                    // Panel not registered - render empty placeholder
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red))
                        .title(Span::styled(" Error: Panel Not Found ", Style::default().fg(Color::Red)));
                    frame.render_widget(block, pane.rect);
                }
            }
            Some(PaneKind::Subagent(id)) => {
                let id = id.clone();
                let focused = app.layout.focused_subagent.as_deref() == Some(&id);
                let ctx = DrawContext { theme: &theme, focused };
                app.layout.subagent_panes.draw(&id, frame, pane.rect, &ctx);
            }
            Some(PaneKind::Chat) => {
                chat_area = pane.rect;
                is_chat_focused = !app.has_panel_focus();
            }
            Some(PaneKind::Empty) | None => {
                // Render placeholder for empty/unknown panes
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(Span::styled(" Empty ", Style::default().fg(Color::DarkGray)));
                frame.render_widget(block, pane.rect);
            }
        }
    }

    (chat_area, is_chat_focused)
}

/// Render chrome: overlays, status bar, session popup, etc.
fn render_chrome(frame: &mut Frame, app: &mut App) {
    session_panel::render_session_popup(frame, app, &app.theme.clone());
    cost_overlay::render_cost_overlay(
        frame,
        app.cost_tracker.as_ref(),
        app.overlays.cost_overlay_visible,
        &app.theme.clone(),
    );
    app.overlays.model_selector.render(frame, frame.area());
    app.overlays.account_selector.render(frame, frame.area());
    app.overlays.session_selector.render(frame, frame.area());
    app.overlays.tool_toggle.render(frame, frame.area());
    app.branching.switcher.render(frame, frame.area());
    app.branching.compare.render(frame, frame.area());

    // Bash confirm dialog (attach mode)
    if let Some(ref confirm) = app.overlays.confirm_dialog {
        crate::components::confirm::render_bash_confirm(frame, frame.area(), confirm);
    }
    app.branching.merge_interactive.render(frame, frame.area());
    app.overlays.leader_menu.render(frame, frame.area());

    if !app.plugin_ui.notifications.is_empty() {
        widget_host::render_plugin_notifications(frame, &app.plugin_ui.notifications, frame.area());
    }
}

// ── Chat content (messages/blocks + plugin panels) ──────────────────────────

fn render_chat_content(frame: &mut Frame, app: &mut App, chat_area: Rect) {
    let plugin_panel_height = if app.plugin_ui.widgets.is_empty() {
        0
    } else {
        let count = app.plugin_ui.widgets.len() as u16;
        (count * 5).min(15)
    };

    let constraints = if plugin_panel_height > 0 {
        vec![
            Constraint::Min(3),                      // messages (blocks)
            Constraint::Length(plugin_panel_height), // plugin widget panels
        ]
    } else {
        vec![
            Constraint::Min(3), // messages (blocks) take all space
        ]
    };

    let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints).split(chat_area);

    // ── Messages (block-oriented rendering) ─────────────────────────

    render_messages(frame, app, chunks[0]);

    // ── Plugin widget panels ────────────────────────────────────────

    if plugin_panel_height > 0 && chunks.len() > 1 {
        widget_host::render_plugin_panels(frame, &app.plugin_ui, &app.theme, chunks[1]);
    }
}

/// Render the status bar area
fn render_status_bar_area(frame: &mut Frame, app: &mut App) {
    let plugin_spans = widget_host::plugin_status_spans(&app.plugin_ui);
    let context_span = app.context_gauge.status_bar_span();
    let git_span = app.git_status.status_bar_span();
    let process_span = app
        .panels
        .downcast_ref::<crate::components::process_panel::ProcessPanel>(crate::panel::PanelId::Processes)
        .and_then(|pp| pp.status_bar_span());
    let budget_status = app
        .cost_tracker
        .as_ref()
        .map_or(clankers_tui_types::BudgetStatus::NoBudget, |ct| ct.budget_status());
    // Build tool activity summary for the status bar
    let tool_activity = if !app.streaming.active_tools.is_empty() {
        let count = app.streaming.active_tools.len();
        let total_lines: usize = app.streaming.active_tools.values().map(|t| t.line_count).sum();
        let label = if count == 1 {
            if let Some(tool) = app.streaming.active_tools.values().next() {
                format!(" 🔧 {} ({} lines) ", tool.tool_name, total_lines)
            } else {
                format!(" 🔧 1 tool ({} lines) ", total_lines)
            }
        } else {
            format!(" 🔧 {} tools ({} lines) ", count, total_lines)
        };
        Some(Span::styled(
            label,
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
    } else {
        None
    };

    let status_data = StatusBarData {
        cwd: &app.cwd,
        model: &app.model,
        total_tokens: app.total_tokens,
        total_cost: app.total_cost,
        state: &app.state,
        session_id: &app.session_id,
        input_mode: app.input_mode,
        thinking_enabled: app.thinking_enabled,
        thinking_level: app.thinking_level,
        plugin_spans,
        context_span,
        git_span,
        process_span,
        active_account: &app.active_account,
        router_status: app.router_status,
        budget_status,
        tool_activity,
        prompt_improve: app.prompt_improve,
        connection_mode: app.connection_mode.clone(),
        loop_status: app.loop_status.as_ref().map(|ls| {
            use ratatui::style::Color;
            use ratatui::style::Modifier;
            use ratatui::style::Style;
            use ratatui::text::Span;
            let label = if ls.active {
                format!(" {} {} ", app.spinner_char(), ls.label())
            } else {
                format!(" ⟳ {} ", ls.label())
            };
            Span::styled(label, Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD))
        }),
    };
    status_bar::render_status_bar(frame, &status_data, &app.theme, app.status_area);
}

/// Compute the input indicator based on app state and input mode
fn compute_input_indicator(state: AppState, input_mode: clankers_tui_types::InputMode) -> &'static str {
    match (state, input_mode) {
        (AppState::Streaming, _) => "… ",
        (_, clankers_tui_types::InputMode::Normal) => "  ",
        (_, clankers_tui_types::InputMode::Insert) => "> ",
    }
}

/// Render the messages/blocks area with conversation history
fn render_messages(frame: &mut Frame, app: &mut App, messages_area: Rect) {
    // Build set of active block IDs for marking active branches
    let active_block_ids: std::collections::HashSet<usize> = app
        .conversation
        .blocks
        .iter()
        .filter_map(|e| match e {
            crate::components::block::BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        })
        .collect();

    let branch_info: std::collections::HashMap<usize, crate::components::block_view::BlockBranchInfo> = app
        .conversation
        .blocks
        .iter()
        .filter_map(|e| match e {
            crate::components::block::BlockEntry::Conversation(b) => {
                let (sibling_index, sibling_total) = app.block_siblings(b.id);
                let children_count = app.block_children_count(b.id);
                // Collect child branch previews for branch points
                let child_branch_previews = if children_count > 1 {
                    app.conversation
                        .all_blocks
                        .iter()
                        .filter(|c| c.parent_block_id == Some(b.id))
                        .map(|c| {
                            let preview: String = c.prompt.chars().take(40).collect();
                            let preview = if c.prompt.len() > 40 {
                                format!("{}…", preview)
                            } else {
                                preview
                            };
                            let is_active = active_block_ids.contains(&c.id);
                            (c.id, preview, is_active)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                Some((b.id, crate::components::block_view::BlockBranchInfo {
                    sibling_index,
                    sibling_total,
                    children_count,
                    show_id: app.overlays.show_block_ids,
                    child_branch_previews,
                }))
            }
            _ => None,
        })
        .collect();

    app.messages_area = messages_area;

    if app.overlays.output_search.has_query() || app.overlays.output_search.active {
        app.overlays.output_search.update_matches(&app.rendered_lines);
    }

    let search_scroll_target = compute_search_scroll_target(&mut app.overlays.output_search);

    app.rendered_lines = block_view::render_blocks(
        frame,
        &app.conversation.blocks,
        app.conversation.focused_block,
        app.conversation.active_block.as_ref(),
        &app.streaming.thinking,
        &app.streaming.text,
        app.show_thinking,
        &app.theme,
        &mut app.conversation.scroll,
        &app.selection,
        messages_area,
        &branch_info,
        &app.overlays.output_search,
        search_scroll_target,
        &app.streaming.active_tools,
        &app.streaming.progress_renderer,
        &mut app.streaming.outputs,
        app.tick,
        &*app.highlighter,
    );

    if app.overlays.output_search.active {
        app.overlays.output_search.render(frame, messages_area);
    }
}

/// Compute the search scroll target position
fn compute_search_scroll_target(output_search: &mut crate::components::output_search::OutputSearch) -> Option<usize> {
    if output_search.scroll_to_current {
        output_search.scroll_to_current = false;
        output_search.current_match_row()
    } else {
        None
    }
}

/// Render the editor/input area with slash menu
fn render_editor_area(frame: &mut Frame, app: &mut App, editor_area: Rect, indicator: &str) {
    let image_count = app.pending_images.len();
    let mut title = if image_count > 0 {
        format!("Input 📎 {} image{}", image_count, if image_count == 1 { "" } else { "s" })
    } else {
        "Input".to_string()
    };
    if app.prompt_improve {
        title.push_str(" ✨ improve");
    }
    editor_component::render_editor(frame, &app.editor, editor_area, indicator, app.theme.border, &title);

    let editor_inner_width = editor_area.width.saturating_sub(2) as usize;
    let (cx, _cy) = app.editor.visual_cursor_position(editor_inner_width, indicator.len());
    let cursor_x = editor_area.x + 1 + cx;
    slash_menu::render_slash_menu(frame, &app.slash_menu, &app.theme, editor_area, cursor_x);
}
