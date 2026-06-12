use clanker_tui_types::AppState;
use clankers_controller::client::ClientAdapter;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_tui::app::App;
use tracing::warn;

use crate::modes::session_command_policy;
use crate::modes::session_command_policy::SessionAckPolicy;
use crate::modes::session_command_policy::SessionCommandEffect;
use crate::modes::session_command_policy::SessionCommandIntent;
use crate::slash_commands;
use crate::slash_commands::effects::SlashEffect;
use crate::slash_commands::effects::SlashPluginEffect;

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
    #[cfg(test)]
    pub(crate) fn expect_thinking_ack_message(&mut self) {
        self.expect_ack(SessionAckPolicy::ThinkingLevel);
    }

    #[cfg(test)]
    pub(crate) fn expect_disabled_tools_message(&mut self) {
        self.expect_ack(SessionAckPolicy::DisabledTools);
    }

    pub(crate) fn expect_ack(&mut self, policy: SessionAckPolicy) {
        match policy {
            SessionAckPolicy::ThinkingLevel => self.thinking_ack_messages_to_suppress += 1,
            SessionAckPolicy::DisabledTools => self.disabled_tools_messages_to_suppress += 1,
            SessionAckPolicy::ManualCompaction => self.manual_compactions_to_suppress += 1,
        }
    }

    pub(crate) fn should_suppress(&mut self, event: &DaemonEvent) -> bool {
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

        let should_suppress = session_command_policy::ack_matches(SessionAckPolicy::ThinkingLevel, event);
        if should_suppress {
            self.thinking_ack_messages_to_suppress -= 1;
        }
        should_suppress
    }

    fn should_suppress_disabled_tools_message(&mut self, event: &DaemonEvent) -> bool {
        if self.disabled_tools_messages_to_suppress == 0 {
            return false;
        }

        let should_suppress = session_command_policy::ack_matches(SessionAckPolicy::DisabledTools, event);
        if should_suppress {
            self.disabled_tools_messages_to_suppress -= 1;
        }
        should_suppress
    }

    fn should_suppress_manual_compaction(&mut self, event: &DaemonEvent) -> bool {
        if self.manual_compactions_to_suppress == 0 {
            return false;
        }

        let should_suppress = session_command_policy::ack_matches(SessionAckPolicy::ManualCompaction, event);
        if should_suppress {
            self.manual_compactions_to_suppress -= 1;
        }
        should_suppress
    }
}

/// Decide how attach mode should handle a slash command.
#[cfg(test)]
pub(crate) fn is_thinking_ack_message(event: &DaemonEvent) -> bool {
    session_command_policy::is_thinking_ack_message(event)
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
    if app.state != AppState::Idle {
        app.queued_prompt = Some(text.to_string());
        if app.conversation.active_block.is_some() {
            client.abort();
        }
        return;
    }

    if let Some((command, args)) = slash_commands::parse_command(text) {
        dispatch_attach_slash(app, client, &command, &args, slash_registry, parity_tracker);
    } else {
        // Regular prompt — expand @file/context references, then send text plus any image blocks.
        let expanded = clankers_util::at_file::expand_at_refs_with_images(
            clankers_util::at_file::ExpandAtRefsRequest { text, cwd: &app.cwd },
        );
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
                clanker_message::Content::Image {
                    source: clanker_message::ImageSource::Base64 { media_type, data },
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
        AttachSlashRoute::CustomLocal => {
            apply_attach_slash_effects(
                app,
                client,
                parity_tracker,
                slash_commands::effects::attach_client_effects(command, args),
            );
        }
        AttachSlashRoute::RegistryLocal => {
            handle_attach_registry_slash(app, client, command, args, slash_registry, parity_tracker);
        }
        AttachSlashRoute::GetPlugins => {
            apply_attach_slash_effect(app, client, parity_tracker, slash_commands::effects::plugin_list_effect());
        }
        AttachSlashRoute::ForwardToDaemon => {
            apply_attach_slash_effect(
                app,
                client,
                parity_tracker,
                slash_commands::effects::forward_to_daemon_effect(command, args),
            );
        }
    }
}

/// Handle a client-side slash command locally.
#[cfg(test)]
pub(crate) fn handle_client_side_slash(app: &mut App, command: &str, args: &str) {
    let (client, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let client = ClientAdapter::from_channels(client, event_rx);
    let mut parity_tracker = AttachParityTracker::default();
    apply_attach_slash_effects(
        app,
        &client,
        &mut parity_tracker,
        slash_commands::effects::attach_client_effects(command, args),
    );
}

fn apply_attach_slash_effects(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    effects: Vec<SlashEffect>,
) {
    for effect in effects {
        apply_attach_slash_effect(app, client, parity_tracker, effect);
    }
}

fn apply_attach_slash_effect(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    effect: SlashEffect,
) {
    match effect {
        SlashEffect::Ui(effect) => slash_commands::effects::apply_ui_effect(app, effect),
        SlashEffect::Session(effect) => dispatch_session_command_effect(app, client, parity_tracker, effect),
        SlashEffect::SendSessionCommand(command) => {
            client.send(command);
        }
        SlashEffect::Plugin(SlashPluginEffect::List) => {
            client.send(SessionCommand::GetPlugins);
        }
        SlashEffect::Noop { message, is_error } => app.push_system(message, is_error),
    }
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
    let db: Option<clankers_db::Db> = None;
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
        if let Some(effect) = slash_commands::effects::agent_command_effect(agent_cmd, app.thinking_level) {
            apply_attach_slash_effect(app, client, parity_tracker, effect);
        } else {
            warn!("attach local slash /{command} emitted unsupported agent command");
        }
    }
}

pub(super) fn bridge_attach_thinking_level_change(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    command: SessionCommandIntent,
    level: clanker_message::ThinkingLevel,
) {
    let mut effect = session_command_policy::set_thinking_level_effect(level);
    effect.command = Some(command);
    dispatch_session_command_effect(app, client, parity_tracker, effect);
}

fn dispatch_session_command_effect(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    effect: SessionCommandEffect,
) {
    apply_local_session_effect(app, effect.local);
    parity_tracker.expect_ack(effect.ack);
    if let Some(command) = effect.command {
        client.send(slash_commands::effects::session_command_intent_to_protocol(command));
    }
}

fn apply_local_session_effect(app: &mut App, effect: Option<crate::modes::session_command_policy::LocalSessionEffect>) {
    slash_commands::effects::apply_local_session_effect(app, effect);
}

pub(super) fn dispatch_disabled_tools_change(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    disabled: impl IntoIterator<Item = String>,
) {
    let effect = session_command_policy::disabled_tools_effect(disabled);
    dispatch_session_command_effect(app, client, parity_tracker, effect);
}

#[cfg(test)]
pub(super) fn apply_standalone_thinking_level(app: &mut App, level: clanker_message::ThinkingLevel) {
    let effect = session_command_policy::set_thinking_level_effect(level);
    apply_local_session_effect(app, effect.local);
}

#[cfg(test)]
pub(crate) fn format_attach_thinking_message(level: clanker_message::ThinkingLevel) -> String {
    session_command_policy::thinking_level_message(level)
}

pub(crate) fn confirm_bash_command(request_id: String, approved: bool) -> SessionCommand {
    SessionCommand::ConfirmBash { request_id, approved }
}
