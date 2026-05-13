use clankers_controller::client::ClientAdapter;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use tracing::warn;

use crate::slash_commands;
use crate::tui::app::App;

/// Attach-side slash routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachSlashRoute {
    CustomLocal,
    RegistryLocal,
    GetPlugins,
    ForwardToDaemon,
}

const ATTACH_CUSTOM_LOCAL_COMMANDS: &[&str] = &["quit", "q", "detach", "zoom", "help"];
const ATTACH_REGISTRY_LOCAL_COMMANDS: &[&str] = &[
    "status", "usage", "metrics", "insights", "version", "router", "cd", "shell", "export", "layout", "preview",
    "editor", "todo", "tools", "think", "compact", "compress",
];
const ATTACH_REGISTRY_LOCAL_EMPTY_ARG_COMMANDS: &[&str] = &["model", "role"];
const ATTACH_LOCAL_SESSION_SUBCOMMANDS: &[&str] = &["list", "ls", "delete", "rm", "purge"];

#[derive(Debug, Default)]
pub(crate) struct AttachParityTracker {
    thinking_ack_messages_to_suppress: usize,
    disabled_tools_messages_to_suppress: usize,
    manual_compactions_to_suppress: usize,
}

impl AttachParityTracker {
    pub(crate) fn expect_thinking_ack_message(&mut self) {
        self.thinking_ack_messages_to_suppress += 1;
    }

    pub(crate) fn expect_disabled_tools_message(&mut self) {
        self.disabled_tools_messages_to_suppress += 1;
    }

    pub(crate) fn expect_manual_compaction(&mut self) {
        self.manual_compactions_to_suppress += 1;
    }

    pub(super) fn should_suppress(&mut self, event: &DaemonEvent) -> bool {
        if self.should_suppress_thinking_ack_message(event) {
            return true;
        }

        if self.should_suppress_disabled_tools_message(event) {
            return true;
        }

        self.should_suppress_manual_compaction(event)
    }

    fn should_suppress_thinking_ack_message(&mut self, event: &DaemonEvent) -> bool {
        if self.thinking_ack_messages_to_suppress == 0 {
            return false;
        }

        let should_suppress = is_thinking_ack_message(event);
        if should_suppress {
            self.thinking_ack_messages_to_suppress -= 1;
        }
        should_suppress
    }

    fn should_suppress_disabled_tools_message(&mut self, event: &DaemonEvent) -> bool {
        if self.disabled_tools_messages_to_suppress == 0 {
            return false;
        }

        let should_suppress = matches!(
            event,
            DaemonEvent::SystemMessage { text, is_error: false } if text.starts_with("Disabled tools updated:")
        );
        if should_suppress {
            self.disabled_tools_messages_to_suppress -= 1;
        }
        should_suppress
    }

    fn should_suppress_manual_compaction(&mut self, event: &DaemonEvent) -> bool {
        if self.manual_compactions_to_suppress == 0 {
            return false;
        }

        let should_suppress = matches!(event, DaemonEvent::SessionCompaction { .. });
        if should_suppress {
            self.manual_compactions_to_suppress -= 1;
        }
        should_suppress
    }
}

/// Decide how attach mode should handle a slash command.
pub(crate) fn is_thinking_ack_message(event: &DaemonEvent) -> bool {
    matches!(event, DaemonEvent::SystemMessage { text, is_error: false } if text.starts_with("Thinking"))
}

pub(crate) fn route_attach_slash(command: &str, args: &str) -> AttachSlashRoute {
    if ATTACH_CUSTOM_LOCAL_COMMANDS.contains(&command) {
        return AttachSlashRoute::CustomLocal;
    }

    if command == "plugin" {
        return AttachSlashRoute::GetPlugins;
    }

    if ATTACH_REGISTRY_LOCAL_COMMANDS.contains(&command) {
        return AttachSlashRoute::RegistryLocal;
    }

    if ATTACH_REGISTRY_LOCAL_EMPTY_ARG_COMMANDS.contains(&command) && args.trim().is_empty() {
        return AttachSlashRoute::RegistryLocal;
    }

    if command == "session" && is_attach_local_session_command(args) {
        return AttachSlashRoute::RegistryLocal;
    }

    AttachSlashRoute::ForwardToDaemon
}

fn is_attach_local_session_command(args: &str) -> bool {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return true;
    }

    let subcommand = trimmed.split_whitespace().next().unwrap_or_default();
    ATTACH_LOCAL_SESSION_SUBCOMMANDS.contains(&subcommand)
}

/// Submit input in attach mode — some slash commands run locally,
/// the rest are forwarded to the daemon.
pub(super) fn submit_input_attach(
    app: &mut App,
    client: &ClientAdapter,
    text: &str,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    if let Some((command, args)) = slash_commands::parse_command(text) {
        dispatch_attach_slash(app, client, &command, &args, slash_registry, parity_tracker);
    } else {
        // Regular prompt — expand @file/context references, then send text plus any image blocks.
        let expanded = crate::util::at_file::expand_at_refs_with_images(text, &app.cwd);
        if !expanded.references.is_empty() {
            tracing::info!(
                metadata = %serde_json::json!({
                    "source": "context_references",
                    "cwd": app.cwd,
                    "references": expanded.references,
                }),
                "context references expanded before daemon prompt submission"
            );
        }
        let images = expanded
            .images
            .into_iter()
            .filter_map(|content| match content {
                crate::provider::message::Content::Image {
                    source: crate::provider::message::ImageSource::Base64 { media_type, data },
                } => Some(clankers_protocol::ImageData { data, media_type }),
                _ => None,
            })
            .collect();
        client.prompt_with_images(expanded.text, images);
    }
}

pub(crate) fn dispatch_attach_slash(
    app: &mut App,
    client: &ClientAdapter,
    command: &str,
    args: &str,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    match route_attach_slash(command, args) {
        AttachSlashRoute::CustomLocal => handle_client_side_slash(app, command, args),
        AttachSlashRoute::RegistryLocal => {
            handle_attach_registry_slash(app, client, command, args, slash_registry, parity_tracker)
        }
        AttachSlashRoute::GetPlugins => {
            client.send(SessionCommand::GetPlugins);
        }
        AttachSlashRoute::ForwardToDaemon => {
            client.send(SessionCommand::SlashCommand {
                command: command.to_string(),
                args: args.to_string(),
            });
        }
    }
}

/// Handle a client-side slash command locally.
pub(crate) fn handle_client_side_slash(app: &mut App, command: &str, args: &str) {
    match command {
        "quit" | "q" => {
            app.should_quit = true;
        }
        "detach" => {
            app.should_quit = true;
            app.push_system("Detaching from session.".to_string(), false);
        }
        "zoom" => {
            app.zoom_toggle();
        }
        "help" => {
            app.push_system("Attach mode — locally handled slash commands include:".to_string(), false);
            app.push_system(
                "  /status /usage /version /router /model (no args) /role (no args) /session [list|delete|purge] /cd /shell /export"
                    .to_string(),
                false,
            );
            app.push_system(
                "  /layout /preview /editor /todo /tools /think [level] /compact /compress /plugin".to_string(),
                false,
            );
            app.push_system(
                "  /think with no args cycles level; /plugin fetches daemon plugin inventory; /quit /detach /zoom stay client-side."
                    .to_string(),
                false,
            );
            app.push_system("  Unlisted commands generally forward to daemon.".to_string(), false);
        }
        _ => {
            app.push_system(format!("Client command /{command} not implemented in attach mode."), true);
        }
    }

    let _ = args;
}

fn handle_attach_registry_slash(
    app: &mut App,
    client: &ClientAdapter,
    command: &str,
    args: &str,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (panel_tx, _panel_rx) = tokio::sync::mpsc::unbounded_channel();
    let db: Option<crate::db::Db> = None;
    let mut session_manager = None;
    let mut ctx = slash_commands::handlers::SlashContext {
        app,
        cmd_tx: &cmd_tx,
        plugin_manager: None,
        panel_tx: &panel_tx,
        db: &db,
        session_manager: &mut session_manager,
    };
    slash_registry.dispatch(command, args, &mut ctx);
    flush_attach_agent_commands(app, client, &mut cmd_rx, command, parity_tracker);
}

fn flush_attach_agent_commands(
    app: &mut App,
    client: &ClientAdapter,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::modes::interactive::AgentCommand>,
    command: &str,
    parity_tracker: &mut AttachParityTracker,
) {
    while let Ok(agent_cmd) = cmd_rx.try_recv() {
        match agent_cmd {
            crate::modes::interactive::AgentCommand::SetThinkingLevel(level) => {
                bridge_attach_thinking_level_change(
                    app,
                    client,
                    parity_tracker,
                    SessionCommand::SetThinkingLevel {
                        level: level.label().to_string(),
                    },
                    level,
                );
            }
            crate::modes::interactive::AgentCommand::CycleThinkingLevel => {
                let next_level = app.thinking_level.next();
                bridge_attach_thinking_level_change(
                    app,
                    client,
                    parity_tracker,
                    SessionCommand::CycleThinkingLevel,
                    next_level,
                );
            }
            crate::modes::interactive::AgentCommand::SetDisabledTools(disabled) => {
                let tools = apply_standalone_disabled_tools(app, disabled);
                parity_tracker.expect_disabled_tools_message();
                client.send(SessionCommand::SetDisabledTools { tools });
            }
            crate::modes::interactive::AgentCommand::CompressContext => {
                parity_tracker.expect_manual_compaction();
                client.send(SessionCommand::CompactHistory);
            }
            other => {
                if let Some(session_command) = translate_attach_agent_command(other) {
                    client.send(session_command);
                } else {
                    warn!("attach local slash /{command} emitted unsupported agent command");
                }
            }
        }
    }
}

pub(super) fn bridge_attach_thinking_level_change(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    session_command: SessionCommand,
    level: crate::provider::ThinkingLevel,
) {
    apply_standalone_thinking_level(app, level);
    parity_tracker.expect_thinking_ack_message();
    client.send(session_command);
}

pub(super) fn apply_standalone_disabled_tools(
    app: &mut App,
    disabled: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut tools: Vec<String> = disabled.into_iter().collect();
    tools.sort();
    app.disabled_tools = tools.iter().cloned().collect();
    tools
}

pub(super) fn apply_standalone_thinking_level(app: &mut App, level: crate::provider::ThinkingLevel) {
    app.thinking_enabled = level.is_enabled();
    app.thinking_level = level;
    app.push_system(format_attach_thinking_message(level), false);
}

pub(crate) fn format_attach_thinking_message(level: crate::provider::ThinkingLevel) -> String {
    match level.budget_tokens() {
        Some(tokens) => format!("Thinking: {} ({} tokens)", level.label(), tokens),
        None => "Thinking: off".to_string(),
    }
}

pub(crate) fn confirm_bash_command(request_id: String, approved: bool) -> SessionCommand {
    SessionCommand::ConfirmBash { request_id, approved }
}

fn translate_attach_agent_command(agent_cmd: crate::modes::interactive::AgentCommand) -> Option<SessionCommand> {
    match agent_cmd {
        crate::modes::interactive::AgentCommand::SetModel(model) => Some(SessionCommand::SetModel { model }),
        _ => None,
    }
}
