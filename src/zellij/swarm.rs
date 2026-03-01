//! Spawn agents in zellij panes

use super::pane;

/// Spawn a subagent in the right-side column.
/// The pane closes automatically when the task finishes.
pub fn spawn_subagent(agent: Option<&str>, task: &str, cwd: &str, model: Option<&str>) -> std::io::Result<()> {
    let id = crate::util::id::generate_id();
    let name = format!("clankers:sub:{}", id);
    let mut extra_args = vec!["-p", task];
    if let Some(m) = model {
        extra_args.extend(["--model", m]);
    }
    pane::spawn_agent_pane(&name, agent, cwd, &extra_args, true)
}

/// Spawn a worker in the right-side column with a task.
/// The pane closes automatically when the task finishes.
pub fn spawn_worker(worker_name: &str, task: &str, agent: Option<&str>, cwd: &str) -> std::io::Result<()> {
    let name = format!("clankers:worker:{}", worker_name);
    pane::spawn_agent_pane(&name, agent, cwd, &["-p", task], true)
}

/// Spawn a watcher in the right-side column.
pub fn spawn_watcher(watcher_name: &str, command: &str, cwd: &str) -> std::io::Result<()> {
    let name = format!("clankers:watch:{}", watcher_name);
    pane::spawn_watcher_pane(&name, command, cwd)
}
