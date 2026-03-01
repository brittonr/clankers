//! Pane creation and management

use super::commands;
use super::commands::PanePosition;

/// Spawn a pane running a clankers agent in the right-side subagent column.
///
/// The layout defines a stacked "subagents" area on the right (33%).
/// New panes stack into it. Focus returns to main after spawning.
pub fn spawn_agent_pane(
    name: &str,
    agent: Option<&str>,
    cwd: &str,
    extra_args: &[&str],
    close_on_exit: bool,
) -> std::io::Result<()> {
    let (cmd, prefix) = super::resolve_clankers_command();
    let mut cmd_args: Vec<String> = prefix;
    if let Some(a) = agent {
        cmd_args.extend(["--agent".to_string(), a.to_string()]);
    }
    cmd_args.extend(extra_args.iter().map(|s| s.to_string()));
    let cmd_args_ref: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();

    commands::new_pane(Some(name), &cmd, &cmd_args_ref, PanePosition::Stacked, Some(cwd), close_on_exit)?;
    let _ = commands::focus_previous_pane();
    Ok(())
}

/// Spawn a background watcher pane in the subagent column
pub fn spawn_watcher_pane(name: &str, command: &str, cwd: &str) -> std::io::Result<()> {
    commands::new_pane(Some(name), command, &[], PanePosition::Stacked, Some(cwd), false)?;
    let _ = commands::focus_previous_pane();
    Ok(())
}
