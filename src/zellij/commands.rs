//! Zellij CLI/IPC command wrappers

use std::process::Command;

/// Run a zellij action command.
/// Stdout/stderr are suppressed to avoid corrupting the TUI.
fn zellij_action(args: &[&str]) -> std::io::Result<()> {
    let status = Command::new("zellij")
        .arg("action")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("zellij action failed: {:?}", args)));
    }
    Ok(())
}

/// Pane placement strategy
pub enum PanePosition {
    /// Floating overlay pane
    Floating,
    /// Tiled pane in the given direction
    Direction(&'static str),
    /// Stacked pane (Zellij collapses them into a tab-like stack)
    Stacked,
}

/// Open a new pane with a command
pub fn new_pane(
    name: Option<&str>,
    command: &str,
    args: &[&str],
    position: PanePosition,
    cwd: Option<&str>,
    close_on_exit: bool,
) -> std::io::Result<()> {
    let mut cmd = Command::new("zellij");
    cmd.arg("action").arg("new-pane");
    match position {
        PanePosition::Floating => {
            cmd.arg("--floating");
        }
        PanePosition::Direction(dir) => {
            cmd.args(["--direction", dir]);
        }
        PanePosition::Stacked => {
            cmd.arg("--stacked");
        }
    }
    if close_on_exit {
        cmd.arg("--close-on-exit");
    }
    if let Some(n) = name {
        cmd.args(["--name", n]);
    }
    if let Some(dir) = cwd {
        cmd.args(["--cwd", dir]);
    }
    cmd.arg("--").arg(command).args(args);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let status = cmd.status()?;
    if !status.success() {
        return Err(std::io::Error::other("new-pane failed"));
    }
    Ok(())
}

/// Close the focused pane
pub fn close_pane() -> std::io::Result<()> {
    zellij_action(&["close-pane"])
}

/// Return focus to the previous pane (used after spawning a new pane)
pub fn focus_previous_pane() -> std::io::Result<()> {
    zellij_action(&["focus-previous-pane"])
}

/// Focus a pane by name (uses write-chars to search)
pub fn focus_pane_by_name(name: &str) -> std::io::Result<()> {
    // Zellij doesn't have direct "focus by name" - use go-to-tab-name or pipe
    // For now, this is best-effort via the zellij action system
    zellij_action(&["go-to-tab-name", name])
}

/// Rename the current pane
pub fn rename_pane(name: &str) -> std::io::Result<()> {
    zellij_action(&["rename-pane", name])
}

/// Write characters to the focused pane
pub fn write_chars(chars: &str) -> std::io::Result<()> {
    zellij_action(&["write-chars", chars])
}

/// Toggle floating panes
pub fn toggle_floating() -> std::io::Result<()> {
    zellij_action(&["toggle-floating-panes"])
}

/// Send a pipe message to a plugin.
///
/// Stdout/stderr are captured to prevent Zellij error messages from
/// corrupting the TUI when the target plugin doesn't exist.
pub fn pipe_message(plugin: &str, name: &str, payload: &str) -> std::io::Result<()> {
    let mut cmd = Command::new("zellij");
    cmd.arg("pipe").args(["--plugin", plugin]).args(["--name", name]).arg("--");
    cmd.arg(payload);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let status = cmd.status()?;
    if !status.success() {
        return Err(std::io::Error::other("pipe failed"));
    }
    Ok(())
}
