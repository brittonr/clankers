//! Declarative slash-command effects shared by standalone and attach shells.
//!
//! Handlers can still parse rich command input, but transport/UI loops should
//! interpret these effects instead of re-implementing command-specific policy.

use clankers_protocol::SessionCommand;

use crate::modes::interactive::AgentCommand;
use crate::modes::session_command_policy;
use crate::modes::session_command_policy::LocalSessionEffect;
use crate::modes::session_command_policy::SessionCommandEffect;
use clankers_provider::ThinkingLevel;
use clankers_tui::app::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SlashUiEffect {
    Quit,
    Detach,
    ToggleZoom,
    SystemMessage { text: String, is_error: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlashPluginEffect {
    List,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SlashEffect {
    Ui(SlashUiEffect),
    Session(SessionCommandEffect),
    SendSessionCommand(SessionCommand),
    Plugin(SlashPluginEffect),
    Noop { message: String, is_error: bool },
}

pub(crate) fn attach_client_effects(command: &str, args: &str) -> Vec<SlashEffect> {
    match command {
        "quit" | "q" => vec![SlashEffect::Ui(SlashUiEffect::Quit)],
        "detach" => vec![
            SlashEffect::Ui(SlashUiEffect::Detach),
            SlashEffect::Ui(SlashUiEffect::SystemMessage {
                text: "Detaching from session.".to_string(),
                is_error: false,
            }),
        ],
        "zoom" => vec![SlashEffect::Ui(SlashUiEffect::ToggleZoom)],
        "help" => attach_help_effects(),
        _ => vec![SlashEffect::Noop {
            message: format!("Client command /{command} not implemented in attach mode."),
            is_error: true,
        }],
    }
    .into_iter()
    .map(|effect| suppress_unused_args(effect, args))
    .collect()
}

pub(crate) fn plugin_list_effect() -> SlashEffect {
    SlashEffect::Plugin(SlashPluginEffect::List)
}

pub(crate) fn forward_to_daemon_effect(command: &str, args: &str) -> SlashEffect {
    SlashEffect::SendSessionCommand(SessionCommand::SlashCommand {
        command: command.to_string(),
        args: args.to_string(),
    })
}

pub(crate) fn agent_command_effect(
    agent_cmd: AgentCommand,
    current_thinking_level: ThinkingLevel,
) -> Option<SlashEffect> {
    match agent_cmd {
        AgentCommand::SetThinkingLevel(level) => {
            Some(SlashEffect::Session(session_command_policy::set_thinking_level_effect(level)))
        }
        AgentCommand::CycleThinkingLevel => {
            Some(SlashEffect::Session(session_command_policy::cycle_thinking_level_effect(current_thinking_level)))
        }
        AgentCommand::SetDisabledTools(disabled) => {
            Some(SlashEffect::Session(session_command_policy::disabled_tools_effect(disabled)))
        }
        AgentCommand::CompressContext => Some(SlashEffect::Session(session_command_policy::manual_compaction_effect())),
        AgentCommand::SetModel(model) => Some(SlashEffect::SendSessionCommand(SessionCommand::SetModel { model })),
        _ => None,
    }
}

pub(crate) fn apply_ui_effect(app: &mut App, effect: SlashUiEffect) {
    match effect {
        SlashUiEffect::Quit => app.should_quit = true,
        SlashUiEffect::Detach => app.should_quit = true,
        SlashUiEffect::ToggleZoom => app.zoom_toggle(),
        SlashUiEffect::SystemMessage { text, is_error } => app.push_system(text, is_error),
    }
}

pub(crate) fn apply_local_session_effect(app: &mut App, effect: Option<LocalSessionEffect>) {
    match effect {
        Some(LocalSessionEffect::ThinkingLevel { level, message }) => {
            app.thinking_enabled = level.is_enabled();
            app.thinking_level = level;
            app.push_system(message, false);
        }
        Some(LocalSessionEffect::DisabledTools { tools }) => {
            app.disabled_tools = tools.into_iter().collect();
        }
        None => {}
    }
}

pub(crate) fn apply_standalone_slash_effect(
    app: &mut App,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    effect: SlashEffect,
) {
    match effect {
        SlashEffect::Ui(effect) => apply_ui_effect(app, effect),
        SlashEffect::Session(effect) => apply_standalone_session_effect(app, cmd_tx, effect),
        SlashEffect::SendSessionCommand(command) => apply_standalone_session_command(cmd_tx, command),
        SlashEffect::Plugin(SlashPluginEffect::List) => app.push_system(
            "Plugin inventory is available from daemon attach; standalone plugin commands use the registry handler."
                .to_string(),
            false,
        ),
        SlashEffect::Noop { message, is_error } => app.push_system(message, is_error),
    }
}

fn apply_standalone_session_effect(
    app: &mut App,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    effect: SessionCommandEffect,
) {
    apply_local_session_effect(app, effect.local);
    if let Some(command) = effect.command {
        apply_standalone_session_command(cmd_tx, command);
    }
}

fn apply_standalone_session_command(
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    command: SessionCommand,
) {
    match command {
        SessionCommand::SetThinkingLevel { level } => {
            if let Some(level) = ThinkingLevel::from_str_or_budget(&level) {
                cmd_tx.send(AgentCommand::SetThinkingLevel(level)).ok();
            }
        }
        SessionCommand::CycleThinkingLevel => {
            cmd_tx.send(AgentCommand::CycleThinkingLevel).ok();
        }
        SessionCommand::SetDisabledTools { tools } => {
            cmd_tx.send(AgentCommand::SetDisabledTools(tools.into_iter().collect())).ok();
        }
        SessionCommand::CompactHistory => {
            cmd_tx.send(AgentCommand::CompressContext).ok();
        }
        SessionCommand::SetModel { model } => {
            cmd_tx.send(AgentCommand::SetModel(model)).ok();
        }
        _ => {}
    }
}

fn attach_help_effects() -> Vec<SlashEffect> {
    [
        "Attach mode — locally handled slash commands include:",
        "  /status /usage /version /router /model (no args) /role (no args) /session [list|delete|purge] /cd /shell /export",
        "  /layout /preview /editor /todo /tools /think [level] /compact /compress /plugin",
        "  /think with no args cycles level; /plugin fetches daemon plugin inventory; /quit /detach /zoom stay client-side.",
        "  Unlisted commands generally forward to daemon.",
    ]
    .into_iter()
    .map(|text| {
        SlashEffect::Ui(SlashUiEffect::SystemMessage {
            text: text.to_string(),
            is_error: false,
        })
    })
    .collect()
}

fn suppress_unused_args(effect: SlashEffect, _args: &str) -> SlashEffect {
    effect
}

#[cfg(test)]
mod tests {
    use clankers_protocol::SessionCommand;

    use super::*;

    #[test]
    fn attach_effects_cover_ui_plugin_session_forward_and_noop_shapes() {
        assert!(matches!(attach_client_effects("help", "").as_slice(), [SlashEffect::Ui(_), ..]));
        assert_eq!(plugin_list_effect(), SlashEffect::Plugin(SlashPluginEffect::List));
        assert_eq!(
            agent_command_effect(AgentCommand::SetThinkingLevel(ThinkingLevel::High), ThinkingLevel::Off)
                .expect("thinking effect"),
            SlashEffect::Session(session_command_policy::set_thinking_level_effect(ThinkingLevel::High))
        );
        assert_eq!(
            forward_to_daemon_effect("memory", "show"),
            SlashEffect::SendSessionCommand(SessionCommand::SlashCommand {
                command: "memory".to_string(),
                args: "show".to_string(),
            })
        );
        assert!(matches!(attach_client_effects("not-local", "").as_slice(), [SlashEffect::Noop {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn standalone_interpreter_applies_ui_session_and_noop_effects() {
        let mut app = App::new("test-model".to_string(), "/tmp".to_string(), crate::tui_config::detect_theme());
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();

        apply_standalone_slash_effect(
            &mut app,
            &cmd_tx,
            SlashEffect::Session(session_command_policy::set_thinking_level_effect(ThinkingLevel::High)),
        );
        assert_eq!(app.thinking_level, ThinkingLevel::High);
        assert!(matches!(cmd_rx.try_recv(), Ok(AgentCommand::SetThinkingLevel(ThinkingLevel::High))));

        apply_standalone_slash_effect(&mut app, &cmd_tx, SlashEffect::Noop {
            message: "deterministic no-op".to_string(),
            is_error: true,
        });
        assert!(!app.should_quit);
        apply_standalone_slash_effect(&mut app, &cmd_tx, SlashEffect::Ui(SlashUiEffect::Quit));
        assert!(app.should_quit);
    }
}
