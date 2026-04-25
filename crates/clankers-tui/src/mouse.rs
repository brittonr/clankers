//! Mouse event handling for the interactive TUI.
//!
//! Handles mouse clicks (left/middle/right), dragging for text selection,
//! scroll wheel, and block collapse toggles.

#![allow(unexpected_cfgs)]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        compound_assertion,
        ignored_result,
        no_unwrap,
        no_panic,
        no_todo,
        unjustified_no_todo_allow,
        no_recursion,
        unchecked_narrowing,
        unchecked_division,
        unbounded_loop,
        catch_all_on_enum,
        explicit_defaults,
        unbounded_channel,
        unbounded_collection_growth,
        assertion_density,
        raw_arithmetic_overflow,
        sentinel_fallback,
        acronym_style,
        bool_naming,
        negated_predicate,
        numeric_units,
        float_for_currency,
        function_length,
        nested_conditionals,
        platform_dependent_cast,
        usize_in_public_api,
        too_many_parameters,
        compound_condition,
        unjustified_allow,
        ambiguous_params,
        ambient_clock,
        verified_purity,
        contradictory_time,
        multi_lock_ordering,
        reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"
    )
)]

use clanker_tui_types::AppState;
use clanker_tui_types::BlockEntry;
use clanker_tui_types::HitRegion;
use clanker_tui_types::InputMode;

use crate::app::App;
use crate::event::Button;

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        function_length,
        reason = "sequential setup/dispatch logic — splitting would fragment readability"
    )
)]
pub fn handle_mouse_down(app: &mut App, button: Button, col: u16, row: u16) {
    let region = app.hit_test(col, row);

    match button {
        Button::Left => {
            match region {
                HitRegion::PanelScrollbar(panel_id) => {
                    // Start scrollbar drag for panel
                    if let Some(info) = app.scrollbar_registry.panels.get(&panel_id) {
                        let new_pos = crate::scrollbar_registry::ScrollbarRegistry::position_from_mouse(info, row);

                        // Update scroll position
                        if let Some(panel) = app.panels.get_mut(panel_id)
                            && let Some(scroll) = panel.panel_scroll_mut()
                        {
                            scroll.offset = new_pos.min(info.content_length.saturating_sub(info.visible_height));
                        }

                        // Start drag tracking
                        app.scrollbar_drag = Some(crate::app::ScrollbarDrag {
                            region: region.clone(),
                            initial_position: new_pos,
                            initial_mouse_y: row,
                        });
                    }
                }
                HitRegion::MessagesScrollbar => {
                    // Start scrollbar drag for messages
                    if let Some(info) = &app.scrollbar_registry.messages {
                        let new_pos = crate::scrollbar_registry::ScrollbarRegistry::position_from_mouse(info, row);
                        app.conversation.scroll.offset =
                            new_pos.min(info.content_length.saturating_sub(info.visible_height));

                        // Start drag tracking
                        app.scrollbar_drag = Some(crate::app::ScrollbarDrag {
                            region: region.clone(),
                            initial_position: new_pos,
                            initial_mouse_y: row,
                        });
                    }
                }
                HitRegion::SubagentScrollbar(ref id) => {
                    // Start scrollbar drag for subagent
                    if let Some(info) = app.scrollbar_registry.subagents.get(id) {
                        let new_pos = crate::scrollbar_registry::ScrollbarRegistry::position_from_mouse(info, row);

                        // Update subagent scroll
                        if let Some(state) = app.layout.subagent_panes.get_mut(id) {
                            state.scroll.offset = new_pos.min(info.content_length.saturating_sub(info.visible_height));
                            state.auto_scroll = false;
                        }

                        // Start drag tracking
                        app.scrollbar_drag = Some(crate::app::ScrollbarDrag {
                            region: region.clone(),
                            initial_position: new_pos,
                            initial_mouse_y: row,
                        });
                    }
                }
                HitRegion::Messages => {
                    // Start text selection in the messages area
                    if let Some(pos) = crate::selection::screen_to_text_pos(
                        col,
                        row,
                        app.messages_area,
                        app.conversation.scroll.offset,
                        &app.rendered_lines,
                    ) {
                        app.selection = Some(crate::selection::TextSelection::start(pos));
                    } else {
                        app.selection = None;
                    }

                    // Switch to normal mode if we were focused on a panel
                    app.unfocus_panel();
                }
                HitRegion::Editor => {
                    // Click in editor → switch to insert mode and place cursor
                    app.selection = None;
                    app.unfocus_panel();
                    app.input_mode = InputMode::Insert;

                    // Compute cursor position from click coordinates
                    let inner_x = app.editor_area.x + 1; // left border
                    let inner_y = app.editor_area.y + 1; // top border
                    let inner_w = app.editor_area.width.saturating_sub(2) as usize;

                    if col >= inner_x && row >= inner_y {
                        let rel_col = col - inner_x;
                        let rel_row = row - inner_y;
                        let indicator_len = match (app.state, app.input_mode) {
                            (AppState::Streaming, _) => 2,
                            (_, InputMode::Normal) => 2,
                            (_, InputMode::Insert) => 2,
                        };
                        app.editor.click_to_cursor(rel_col, rel_row, inner_w, indicator_len);
                    }
                }
                HitRegion::Subagent(ref subagent_id) => {
                    // Click on a subagent pane → focus it
                    app.selection = None;
                    app.focus_subagent(subagent_id);
                }
                HitRegion::Panel(panel_id) => {
                    // Click on a panel → focus it
                    app.selection = None;
                    app.focus_panel(panel_id);
                    app.input_mode = InputMode::Normal;
                }
                HitRegion::StatusBar | HitRegion::None => {
                    app.selection = None;
                }
            }
        }
        Button::Middle => {
            // Middle-click: paste from system clipboard (X11/Wayland primary selection).
            // We use the same paste mechanism as Ctrl+V but only on click.
            if matches!(region, HitRegion::Editor) {
                app.input_mode = InputMode::Insert;
                super::clipboard::paste_from_clipboard(app);
            }
        }
        Button::Right => {
            // Right-click in messages area: toggle collapse of the clicked block
            if matches!(region, HitRegion::Messages)
                && let Some(pos) = crate::selection::screen_to_text_pos(
                    col,
                    row,
                    app.messages_area,
                    app.conversation.scroll.offset,
                    &app.rendered_lines,
                )
            {
                // Try to find which block this line belongs to and toggle it
                click_toggle_block(app, pos.row);
            }
        }
    }
}

/// Handle mouse drag (button held + moved).
pub fn handle_mouse_drag(app: &mut App, button: Button, _col: u16, row: u16) {
    if button != Button::Left {
        return;
    }

    // Handle scrollbar drag
    if let Some(ref drag_state) = app.scrollbar_drag {
        match &drag_state.region {
            HitRegion::PanelScrollbar(panel_id) => {
                if let Some(info) = app.scrollbar_registry.panels.get(panel_id) {
                    let new_pos = crate::scrollbar_registry::ScrollbarRegistry::position_from_mouse(info, row);
                    if let Some(panel) = app.panels.get_mut(*panel_id)
                        && let Some(scroll) = panel.panel_scroll_mut()
                    {
                        scroll.offset = new_pos.min(info.content_length.saturating_sub(info.visible_height));
                    }
                }
            }
            HitRegion::MessagesScrollbar => {
                if let Some(info) = &app.scrollbar_registry.messages {
                    let new_pos = crate::scrollbar_registry::ScrollbarRegistry::position_from_mouse(info, row);
                    app.conversation.scroll.offset =
                        new_pos.min(info.content_length.saturating_sub(info.visible_height));
                }
            }
            HitRegion::SubagentScrollbar(id) => {
                if let Some(info) = app.scrollbar_registry.subagents.get(id) {
                    let new_pos = crate::scrollbar_registry::ScrollbarRegistry::position_from_mouse(info, row);
                    if let Some(state) = app.layout.subagent_panes.get_mut(id) {
                        state.scroll.offset = new_pos.min(info.content_length.saturating_sub(info.visible_height));
                        state.auto_scroll = false;
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // Continue text selection in messages area
    if let Some(ref mut sel) = app.selection
        && let Some(pos) = crate::selection::screen_to_text_pos(
            _col,
            row,
            app.messages_area,
            app.conversation.scroll.offset,
            &app.rendered_lines,
        )
    {
        sel.update(pos);
    }
}

/// Handle mouse button release.
pub fn handle_mouse_up(app: &mut App, button: Button, col: u16, row: u16) {
    if button != Button::Left {
        return;
    }

    // Clear scrollbar drag state
    app.scrollbar_drag = None;

    if let Some(ref mut sel) = app.selection {
        if let Some(pos) = crate::selection::screen_to_text_pos(
            col,
            row,
            app.messages_area,
            app.conversation.scroll.offset,
            &app.rendered_lines,
        ) {
            sel.update(pos);
        }
        sel.finish();
        if !sel.is_empty() {
            let text = sel.extract_text(&app.rendered_lines);
            crate::selection::copy_to_clipboard(&text);
        } else {
            app.selection = None;
        }
    }
}

/// Handle mouse scroll wheel — dispatches to whichever region the cursor is over.
pub fn handle_mouse_scroll(app: &mut App, col: u16, row: u16, up: bool, lines: u16) {
    let region = app.hit_test(col, row);

    match region {
        HitRegion::Messages => {
            if up {
                app.conversation.scroll.scroll_up(lines as usize);
            } else {
                app.conversation.scroll.scroll_down(lines as usize);
            }
        }
        HitRegion::Panel(panel_id) => {
            if let Some(panel) = app.panel_mut(panel_id) {
                panel.handle_scroll(up, lines);
            }
        }
        HitRegion::Subagent(ref id) => {
            app.layout.subagent_panes.handle_scroll(id, up, lines);
        }
        HitRegion::Editor => {
            // Scroll in editor could navigate history (up/down),
            // but that would be confusing. Just scroll the messages.
            if up {
                app.conversation.scroll.scroll_up(lines as usize);
            } else {
                app.conversation.scroll.scroll_down(lines as usize);
            }
        }
        _ => {}
    }
}

/// Try to toggle the collapse state of the block at the given rendered line.
fn click_toggle_block(app: &mut App, text_row: usize) {
    // Walk through blocks and count rendered lines to find which block
    // the clicked row falls in. This is approximate — we use the block
    // header lines as a heuristic.
    let mut row_cursor: usize = 0;

    for entry in &app.conversation.blocks {
        if let BlockEntry::Conversation(block) = entry {
            // Each block has at least a header line
            let block_lines = if block.collapsed {
                2 // header + collapsed indicator
            } else {
                // header + responses + spacing
                2 + block.responses.iter().map(|r| r.content.lines().count() + 1).sum::<usize>()
            };

            if text_row >= row_cursor && text_row < row_cursor + block_lines {
                // Found the block — focus and toggle it
                app.conversation.focused_block = Some(block.id);
                app.toggle_focused_block();
                app.input_mode = InputMode::Normal;
                return;
            }
            row_cursor += block_lines;
        } else {
            // System messages: count their lines
            if let BlockEntry::System(msg) = entry {
                row_cursor += msg.content.lines().count() + 1;
            }
        }
    }
}
