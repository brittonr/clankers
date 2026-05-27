use clankers_controller::client::ClientAdapter;
use clankers_controller::convert::daemon_event_to_tui_event;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use tracing::debug;

use super::commands::AttachParityTracker;
use crate::tui::app::App;

/// Drain available DaemonEvents from the client and apply them to App state.
pub(crate) fn drain_daemon_events(
    app: &mut App,
    client: &mut ClientAdapter,
    is_replaying_history: &mut bool,
    max_subagent_panes: usize,
    parity_tracker: &mut AttachParityTracker,
) {
    while let Some(event) = client.try_recv() {
        process_daemon_event(app, client, &event, is_replaying_history, max_subagent_panes, parity_tracker);
    }
}

/// Process a single DaemonEvent — update App state, handle non-TUI events.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential event handling logic")
)]
pub(crate) fn process_daemon_event(
    app: &mut App,
    client: &ClientAdapter,
    event: &DaemonEvent,
    is_replaying_history: &mut bool,
    max_subagent_panes: usize,
    parity_tracker: &mut AttachParityTracker,
) {
    if parity_tracker.should_suppress(event) {
        return;
    }

    // First, try the TuiEvent conversion for all streaming/tool/session events.
    if let Some(tui_event) = daemon_event_to_tui_event(event) {
        app.handle_tui_event(&tui_event);
        return;
    }

    // Handle events that don't map to TuiEvent.
    match event {
        // ── Session metadata ────────────────────────
        DaemonEvent::SessionInfo { model, .. } => {
            if !model.is_empty() {
                app.model.clone_from(model);
            }
        }
        DaemonEvent::ModelChanged { to, .. } => {
            app.model.clone_from(to);
            app.push_system(format!("Model changed to {to}"), false);
        }

        // ── System messages ─────────────────────────
        DaemonEvent::SystemMessage { text, is_error } => {
            app.push_system(text.clone(), *is_error);
        }

        // ── Prompt lifecycle ────────────────────────
        DaemonEvent::PromptDone { error } => {
            let has_queued_prompt = app.queued_prompt.is_some();
            if let Some(err) = error {
                if let Some(ref mut block) = app.conversation.active_block {
                    block.error = Some(err.clone());
                }
                app.finalize_active_block();
                if !has_queued_prompt {
                    app.push_system(format!("Error: {err}"), true);
                }
            } else {
                app.finalize_active_block();
            }
            // If the user typed something while the agent was busy, send it now.
            if let Some(text) = app.queued_prompt.take() {
                client.send(SessionCommand::ResetCancel);
                client.prompt(text);
            }
        }

        // ── Confirmation requests ───────────────────
        DaemonEvent::ConfirmRequest {
            request_id,
            command,
            working_dir,
        } => {
            app.overlays.confirm_dialog = Some(clankers_tui::app::BashConfirmState {
                request_id: request_id.clone(),
                command: command.clone(),
                working_dir: working_dir.clone(),
                approved: true, // default to Yes
            });
        }
        DaemonEvent::TodoRequest { request_id, action } => {
            // Todo actions are TUI-local state updates (add/update/remove items).
            // The daemon sends these for panel synchronization. Auto-respond since
            // attach mode doesn't own the todo panel state.
            debug!("todo request in attach mode: {action:?}");
            // Auto-respond with empty object — daemon handles the actual todo
            client.send(SessionCommand::TodoResponse {
                request_id: request_id.clone(),
                response: serde_json::json!({}),
            });
        }

        // ── Capability events ───────────────────────
        DaemonEvent::Capabilities { capabilities } => {
            if let Some(caps) = capabilities {
                app.push_system(format!("Capabilities: {}", caps.join(", ")), false);
            }
        }
        DaemonEvent::ToolBlocked { tool_name, reason, .. } => {
            app.push_system(format!("⛔ Tool blocked: {tool_name} — {reason}"), true);
        }

        // ── Subagent events ─────────────────────────
        DaemonEvent::SubagentStarted { id, name, task, pid } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.add(id.clone(), name.clone(), task.clone(), *pid);
            }
            // Create a dedicated BSP pane for this subagent (same as embedded mode)
            if max_subagent_panes > 0 && app.layout.subagent_panes.len() < max_subagent_panes {
                let pane_id = app.layout.subagent_panes.create(
                    id.clone(),
                    name.clone(),
                    task.clone(),
                    *pid,
                    &mut app.layout.tiling,
                );
                app.layout.pane_registry.register(pane_id, crate::tui::panes::PaneKind::Subagent(id.clone()));
                crate::tui::panes::auto_split_for_subagent(&mut app.layout.tiling, &app.layout.pane_registry, pane_id);
            }
        }
        DaemonEvent::SubagentOutput { id, line } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.append_output(id, line);
            }
            app.layout.subagent_panes.append_output(id, line);
        }
        DaemonEvent::SubagentDone { id } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.mark_done(id);
            }
            app.layout.subagent_panes.mark_done(id);
        }
        DaemonEvent::SubagentError { id, message } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.mark_error(id);
                panel.append_output(id, &format!("Error: {message}"));
            }
            app.layout.subagent_panes.mark_error(id);
        }

        // ── History replay ──────────────────────────
        DaemonEvent::HistoryBlock { block } => {
            if *is_replaying_history {
                match serde_json::from_value::<clanker_message::AgentMessage>(block.clone()) {
                    Ok(msg) => {
                        let events = clankers_controller::convert::agent_message_to_tui_events(&msg);
                        for tui_event in &events {
                            app.handle_tui_event(tui_event);
                        }
                    }
                    Err(_) => {
                        // Graceful fallback for old-format or unrecognized blocks
                        let preview = block.as_str().unwrap_or("(unrecognized block)");
                        let truncated = if preview.len() > 120 {
                            format!("{}...", &preview[..120])
                        } else {
                            preview.to_string()
                        };
                        app.push_system(format!("📜 {truncated}"), false);
                    }
                }
            }
        }
        DaemonEvent::HistoryEnd => {
            app.finalize_active_block();
            *is_replaying_history = false;
        }

        // ── Tool metadata ────────────────────────────
        DaemonEvent::ToolList { tools } => {
            app.tool_info = tools.iter().map(|t| (t.name.clone(), t.description.clone(), String::new())).collect();
        }
        DaemonEvent::DisabledToolsChanged { tools } => {
            app.disabled_tools = tools.iter().cloned().collect();
        }

        // ── State sync events ───────────────────────
        DaemonEvent::ThinkingLevelChanged { from, to } => {
            app.push_system(format!("Thinking: {from} → {to}"), false);
        }
        DaemonEvent::LoopStatus {
            active,
            iteration,
            max_iterations,
            break_condition,
        } => {
            if *active {
                let iter_str = match (iteration, max_iterations) {
                    (Some(i), Some(m)) => format!(" ({i}/{m})"),
                    (Some(i), None) => format!(" ({i})"),
                    _ => String::new(),
                };
                let cond_str = break_condition.as_deref().unwrap_or("");
                app.push_system(format!("Loop active{iter_str} {cond_str}"), false);
            } else {
                app.push_system("Loop finished".to_string(), false);
            }
        }
        DaemonEvent::AutoTestChanged { enabled, command } => {
            if *enabled {
                let cmd = command.as_deref().unwrap_or("(default)");
                app.push_system(format!("Auto-test enabled: {cmd}"), false);
            } else {
                app.push_system("Auto-test disabled".to_string(), false);
            }
            app.auto_test_enabled = *enabled;
            app.auto_test_command.clone_from(command);
        }
        DaemonEvent::CostUpdate { total_cost_usd, .. } => {
            app.push_system(format!("Session cost: ${total_cost_usd:.4}"), false);
        }

        // ── Ignored events ──────────────────────────
        // ── Plugin events ───────────────────────
        DaemonEvent::PluginWidget { plugin, widget } => {
            if let Some(widget_json) = widget {
                if let Ok(w) = serde_json::from_value::<clanker_tui_types::Widget>(widget_json.clone()) {
                    app.plugin_ui.widgets.insert(plugin.clone(), w);
                }
            } else {
                app.plugin_ui.widgets.remove(plugin);
            }
        }
        DaemonEvent::PluginStatus { plugin, text, color } => {
            if let Some(text) = text {
                app.plugin_ui.status_segments.insert(plugin.clone(), clanker_tui_types::StatusSegment {
                    text: text.clone(),
                    color: color.clone(),
                });
            } else {
                app.plugin_ui.status_segments.remove(plugin);
            }
        }
        DaemonEvent::PluginNotify { plugin, message, level } => {
            app.plugin_ui.notifications.push(clanker_tui_types::PluginNotification {
                plugin: plugin.clone(),
                message: message.clone(),
                level: level.clone(),
                created: std::time::Instant::now(),
            });
        }
        DaemonEvent::PluginList { plugins } => {
            app.daemon_plugins = Some(plugins.clone());
            // Display plugin list when it arrives (in response to /plugin)
            if plugins.is_empty() {
                app.push_system("No plugins loaded.".to_string(), false);
            } else {
                let mut lines = vec![format!("Loaded plugins ({}):", plugins.len())];
                for p in plugins {
                    let marker = match p.state.as_str() {
                        "Active" => "\u{2713}",
                        "Loaded" | "Starting" => "\u{25cb}",
                        "Backoff" => "↺",
                        "Disabled" => "−",
                        _ => "\u{2717}",
                    };
                    let kind = p.kind.as_deref().unwrap_or("unknown");
                    lines.push(format!("  {} {} v{} [{} / {}]", marker, p.name, p.version, kind, p.state));
                    let tools = if p.tools.is_empty() {
                        "none".to_string()
                    } else {
                        p.tools.join(", ")
                    };
                    lines.push(format!("      tools: {}", tools));
                    if let Some(error) = &p.last_error {
                        lines.push(format!("      last error: {}", error));
                    }
                }
                app.push_system(lines.join("\n"), false);
            }
        }

        DaemonEvent::SystemPromptResponse { .. } => {
            // We didn't request this — ignore
        }

        // Events already handled by daemon_event_to_tui_event above
        _ => {}
    }
}
