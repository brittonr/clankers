//! Local worker subprocess spawning and management

use tokio_util::sync::CancellationToken;

use crate::tools::ToolResult;
use crate::tui::components::subagent_event::SubagentEvent;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

/// Length of task preview for panel display
const TASK_PREVIEW_SHORT_LEN: usize = 60;
/// Length of full task preview for process monitor
const TASK_PREVIEW_FULL_LEN: usize = 200;

pub async fn run_worker_subprocess(
    worker_name: &str,
    task: &str,
    agent: Option<&str>,
    cwd: Option<&str>,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
) -> ToolResult {
    let sub_id = format!("worker:{}", worker_name);
    let task_preview: String = task.chars().take(TASK_PREVIEW_SHORT_LEN).collect();

    let mut child = match spawn_worker_process(worker_name, task, agent, cwd) {
        Ok(child) => child,
        Err(e) => return ToolResult::error(e),
    };

    let child_pid = child.id();
    register_with_process_monitor(process_monitor, child_pid, worker_name, task);

    if let Some(tx) = panel_tx {
        let _ = tx.send(SubagentEvent::Started {
            id: sub_id.clone(),
            name: worker_name.to_string(),
            task: task_preview,
            pid: child_pid,
        });
    }

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return ToolResult::error("Failed to capture stdout"),
    };
    let stderr_handle = child.stderr.take();

    let collected = match stream_worker_output(stdout, &sub_id, panel_tx, signal.clone()).await {
        Ok(output) => output,
        Err(e) => {
            let _ = child.kill().await;
            return e;
        }
    };

    handle_worker_exit(worker_name, child, stderr_handle, collected, &sub_id, panel_tx).await
}

/// Spawn the worker subprocess with proper configuration
fn spawn_worker_process(
    worker_name: &str,
    task: &str,
    agent: Option<&str>,
    cwd: Option<&str>,
) -> Result<tokio::process::Child, String> {
    let exe = resolve_clankers_exe().map_err(|e| format!("Cannot find clankers executable: {}", e))?;

    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg("--no-zellij").arg("-p").arg(task);

    if let Some(a) = agent {
        cmd.arg("--agent").arg(a);
    }

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Create a new process group so we can kill the entire tree
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    cmd.spawn().map_err(|e| format!("Failed to spawn worker '{}': {}", worker_name, e))
}

/// Register the spawned process with the process monitor
fn register_with_process_monitor(
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
    child_pid: Option<u32>,
    worker_name: &str,
    task: &str,
) {
    if let Some(monitor) = process_monitor
        && let Some(pid) = child_pid
    {
        let task_preview_full: String = task.chars().take(TASK_PREVIEW_FULL_LEN).collect();
        monitor.register(pid, crate::procmon::ProcessMeta {
            tool_name: "delegate".to_string(),
            command: format!("worker:{} {}", worker_name, task_preview_full),
            call_id: format!("worker:{}", worker_name),
        });
    }
}

/// Stream worker stdout to the panel and collect all output
async fn stream_worker_output(
    stdout: tokio::process::ChildStdout,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
) -> Result<String, ToolResult> {
    use tokio::io::AsyncBufReadExt;
    use tokio::io::BufReader;

    let mut reader = BufReader::new(stdout).lines();
    let mut collected = String::new();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(tx) = panel_tx {
                            let _ = tx.send(SubagentEvent::Output {
                                id: sub_id.to_string(),
                                line: line.clone(),
                            });
                        }
                        if !collected.is_empty() {
                            collected.push('\n');
                        }
                        collected.push_str(&line);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(ToolResult::error(format!("Worker read error: {}", e)));
                    }
                }
            }
            () = signal.cancelled() => {
                if let Some(tx) = panel_tx {
                    let _ = tx.send(SubagentEvent::Error {
                        id: sub_id.to_string(),
                        message: "Cancelled".into()
                    });
                }
                return Err(ToolResult::error("Worker cancelled".to_string()));
            }
        }
    }

    Ok(collected)
}

/// Handle worker process exit and produce the final ToolResult
async fn handle_worker_exit(
    worker_name: &str,
    mut child: tokio::process::Child,
    stderr_handle: Option<tokio::process::ChildStderr>,
    collected: String,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
) -> ToolResult {
    use tokio::io::BufReader;

    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => return ToolResult::error(format!("Wait error: {}", e)),
    };

    if status.success() {
        if let Some(tx) = panel_tx {
            let _ = tx.send(SubagentEvent::Done { id: sub_id.to_string() });
        }
        ToolResult::text(collected)
    } else {
        let stderr_text = if let Some(stderr) = stderr_handle {
            let mut buf = String::new();
            let mut reader = BufReader::new(stderr);
            let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await;
            buf
        } else {
            String::new()
        };
        let err_msg = format!(
            "Worker '{}' failed (exit {}):\nstdout: {}\nstderr: {}",
            worker_name, status, collected, stderr_text
        );
        if let Some(tx) = panel_tx {
            let _ = tx.send(SubagentEvent::Error {
                id: sub_id.to_string(),
                message: err_msg.clone(),
            });
        }
        ToolResult::error(err_msg)
    }
}

fn resolve_clankers_exe() -> Result<std::path::PathBuf, String> {
    // Try current_exe first — works when the binary hasn't been recompiled
    if let Ok(exe) = std::env::current_exe() {
        if exe.exists() {
            return Ok(exe);
        }
        tracing::debug!("current_exe() returned {:?} but file is deleted", exe);
    }

    // cargo test sets this env var
    if let Ok(exe) = std::env::var("CARGO_BIN_EXE_clankers") {
        let p = std::path::PathBuf::from(&exe);
        if p.exists() {
            return Ok(p);
        }
    }

    // Walk up from CWD to find the project root (contains Cargo.toml with [workspace])
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            for profile in &["debug", "release"] {
                let candidate = ancestor.join("target").join(profile).join("clankers");
                if candidate.exists() {
                    tracing::info!("Resolved clankers binary via fallback: {:?}", candidate);
                    return Ok(candidate);
                }
            }
            // Stop at the workspace root
            let cargo_toml = ancestor.join("Cargo.toml");
            if cargo_toml.exists()
                && std::fs::read_to_string(&cargo_toml).is_ok_and(|contents| contents.contains("[workspace]"))
            {
                break;
            }
        }
    }

    // Last resort: look in PATH
    if let Ok(output) = std::process::Command::new("which").arg("clankers").output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(std::path::PathBuf::from(path));
        }
    }

    Err("clankers binary not found (current_exe deleted and no fallback found)".to_string())
}
