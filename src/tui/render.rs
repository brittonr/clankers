//! Top-level layout renderer
//!
//! Uses hypertile BSP tiling to split the terminal into panes and render
//! side-panels via the [`Panel`] trait, while the chat pane
//! (blocks + editor + status bar) is rendered directly.

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

use crate::tui::app::App;
use crate::tui::app::AppState;
use crate::tui::components::block_view;
use crate::tui::components::editor as editor_component;
use crate::tui::components::session_panel;
use crate::tui::components::slash_menu;
use crate::tui::components::status_bar::StatusBarData;
use crate::tui::components::status_bar::{self};
use crate::tui::panel::DrawContext;
use crate::tui::panes::PaneKind;
use crate::tui::widget_host;

/// Render the full application UI
pub fn render(frame: &mut Frame, app: &mut App) {
    // Advance animation tick (drives spinners and other animated elements)
    app.advance_tick();

    // Garbage-collect expired plugin notifications
    app.plugin_ui.gc_notifications();

    // Refresh git status periodically
    app.git_status.maybe_refresh();

    // Sync the cwd into file_activity_panel so it can shorten paths
    if let Some(fap) = app.panels.downcast_mut::<crate::tui::components::file_activity_panel::FileActivityPanel>(crate::tui::panel::PanelId::Files) {
        if fap.cwd != app.cwd {
            fap.cwd.clone_from(&app.cwd);
        }
    }

    // Refresh process panel entries from monitor
    if let Some(pp) = app.panels.downcast_mut::<crate::tui::components::process_panel::ProcessPanel>(crate::tui::panel::PanelId::Processes) {
        pp.refresh_entries();
    }

    // ── Compute BSP tiling layout ───────────────────────────────────

    app.tiling.compute_layout(frame.area());

    // ── Render each pane ────────────────────────────────────────────

    // Collect pane snapshots first (to avoid borrow conflicts with app).
    let pane_snapshots: Vec<_> = app.tiling.panes();
    let theme = app.theme.clone();
    let mut chat_area = Rect::default();
    let mut chat_focused = false;

    for pane in &pane_snapshots {
        match app.pane_registry.kind(pane.id) {
            Some(PaneKind::Panel(panel_id)) => {
                let panel_id = *panel_id;
                let focused = app.is_panel_focused(panel_id);
                let ctx = DrawContext {
                    theme: &theme,
                    focused,
                };
                let panel = app.panel_mut(panel_id);
                crate::tui::panel::draw_panel_scrolled(frame, panel, pane.rect, &ctx);
            }
            Some(PaneKind::Chat) => {
                chat_area = pane.rect;
                chat_focused = !app.has_panel_focus();
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

    // ── Main (chat) column layout ───────────────────────────────────

    let main_render_area = if chat_focused {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(chat_area);
        frame.render_widget(block, chat_area);
        inner
    } else {
        chat_area
    };

    render_main_column(frame, app, main_render_area);

    // ── Panel navigation hint ───────────────────────────────────────

    if chat_focused {
        let hint = Span::styled(" h/l:panels j/k:panes ", Style::default().fg(Color::DarkGray));
        let hint_len = hint.width() as u16;
        let hint_area = Rect {
            x: chat_area.x + chat_area.width.saturating_sub(hint_len + 1),
            y: chat_area.y,
            width: hint_len.min(chat_area.width),
            height: 1,
        };
        frame.render_widget(Paragraph::new(hint), hint_area);
    } else if let Some(focused_pane) = app.tiling.focused_pane() {
        // Show tiling hint on the focused panel's border
        if let Some(rect) = app.tiling.pane_rect(focused_pane) {
            let hint = Span::styled(" j/k h/l:nav []:size |/-:split ", Style::default().fg(Color::DarkGray));
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

    // ── Overlays (rendered on top of everything) ────────────────────

    session_panel::render_session_popup(frame, app, &app.theme.clone());
    app.model_selector.render(frame, frame.area());
    app.account_selector.render(frame, frame.area());
    app.session_selector.render(frame, frame.area());
    app.branch_switcher.render(frame, frame.area());
    app.branch_compare.render(frame, frame.area());
    app.merge_interactive.render(frame, frame.area());
    app.leader_menu.render(frame, frame.area());

    if !app.plugin_ui.notifications.is_empty() {
        widget_host::render_plugin_notifications(frame, &app.plugin_ui.notifications, frame.area());
    }
}

// ── Main column (chat blocks + editor + status bar) ─────────────────────────

fn render_main_column(frame: &mut Frame, app: &mut App, main_area: Rect) {
    let inner_width = main_area.width.saturating_sub(2) as usize;
    let indicator = match (app.state, app.input_mode) {
        (AppState::Streaming, _) => "… ",
        (_, crate::config::keybindings::InputMode::Normal) => "  ",
        (_, crate::config::keybindings::InputMode::Insert) => "> ",
    };
    let visual_lines = app.editor.visual_line_count(inner_width, indicator.len()) as u16;
    let editor_height = (visual_lines + 2).clamp(3, 10);

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
            Constraint::Length(editor_height),       // editor
            Constraint::Length(1),                   // status bar
        ]
    } else {
        vec![
            Constraint::Min(3),                // messages (blocks)
            Constraint::Length(editor_height), // editor
            Constraint::Length(1),             // status bar
        ]
    };

    let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints).split(main_area);

    let (messages_idx, plugin_idx, editor_idx, status_idx) = if plugin_panel_height > 0 {
        (0, Some(1), 2, 3)
    } else {
        (0, None, 1, 2)
    };

    // ── Save editor + status areas for mouse hit-testing ──────────

    app.editor_area = chunks[editor_idx];
    app.status_area = chunks[status_idx];

    // ── Messages (block-oriented rendering) ─────────────────────────

    // Build set of active block IDs for marking active branches
    let active_block_ids: std::collections::HashSet<usize> = app
        .blocks
        .iter()
        .filter_map(|e| match e {
            crate::tui::components::block::BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        })
        .collect();

    let branch_info: std::collections::HashMap<usize, crate::tui::components::block_view::BlockBranchInfo> = app
        .blocks
        .iter()
        .filter_map(|e| match e {
            crate::tui::components::block::BlockEntry::Conversation(b) => {
                let (sibling_index, sibling_total) = app.block_siblings(b.id);
                let children_count = app.block_children_count(b.id);
                // Collect child branch previews for branch points
                let child_branch_previews = if children_count > 1 {
                    app.all_blocks
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
                Some((b.id, crate::tui::components::block_view::BlockBranchInfo {
                    sibling_index,
                    sibling_total,
                    children_count,
                    show_id: app.show_block_ids,
                    child_branch_previews,
                }))
            }
            _ => None,
        })
        .collect();

    app.messages_area = chunks[messages_idx];

    if app.output_search.has_query() || app.output_search.active {
        app.output_search.update_matches(&app.rendered_lines);
    }

    let search_scroll_target = if app.output_search.scroll_to_current {
        app.output_search.scroll_to_current = false;
        app.output_search.current_match_row()
    } else {
        None
    };

    app.rendered_lines = block_view::render_blocks(
        frame,
        &app.blocks,
        app.focused_block,
        app.active_block.as_ref(),
        &app.streaming_thinking,
        &app.streaming_text,
        app.show_thinking,
        &app.theme,
        &mut app.scroll,
        &app.selection,
        chunks[messages_idx],
        &branch_info,
        &app.output_search,
        search_scroll_target,
        &app.active_tools,
        &app.progress_renderer,
        app.tick,
    );

    if app.output_search.active {
        app.output_search.render(frame, chunks[messages_idx]);
    }

    // ── Plugin widget panels ────────────────────────────────────────

    if let Some(idx) = plugin_idx {
        widget_host::render_plugin_panels(frame, &app.plugin_ui, &app.theme, chunks[idx]);
    }

    // ── Editor ──────────────────────────────────────────────────────

    let image_count = app.pending_images.len();
    let title = if image_count > 0 {
        format!("Input 📎 {} image{}", image_count, if image_count == 1 { "" } else { "s" })
    } else {
        "Input".to_string()
    };
    editor_component::render_editor(frame, &app.editor, chunks[editor_idx], indicator, app.theme.border, &title);

    let editor_inner_width = chunks[editor_idx].width.saturating_sub(2) as usize;
    let (cx, _cy) = app.editor.visual_cursor_position(editor_inner_width, indicator.len());
    let cursor_x = chunks[editor_idx].x + 1 + cx;
    slash_menu::render_slash_menu(frame, &app.slash_menu, &app.theme, chunks[editor_idx], cursor_x);

    // ── Status bar ──────────────────────────────────────────────────

    let plugin_spans = widget_host::plugin_status_spans(&app.plugin_ui);
    let context_span = app.context_gauge.status_bar_span();
    let git_span = app.git_status.status_bar_span();
    let process_span = app.panels
        .downcast_ref::<crate::tui::components::process_panel::ProcessPanel>(crate::tui::panel::PanelId::Processes)
        .and_then(|pp| pp.status_bar_span());
    let budget_status = app
        .cost_tracker
        .as_ref()
        .map_or(crate::routing::cost_tracker::BudgetStatus::NoBudget, |ct| {
            ct.budget_status()
        });
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
    };
    status_bar::render_status_bar(frame, &status_data, &app.theme, chunks[status_idx]);
}
