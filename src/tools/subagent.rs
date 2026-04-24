//! Subagent tool — spawns ephemeral clankers instances for delegated tasks
//!
//! Supports single, parallel, and chained task modes. Output streams
//! to the subagent panel in the TUI only — NOT to the main agent's event bus.

use async_trait::async_trait;
use clanker_tui_types::SubagentEvent;
use serde_json::Value;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

pub struct SubagentTool {
    definition: ToolDefinition,
    panel_tx: Option<PanelTx>,
    process_monitor: Option<crate::procmon::ProcessMonitorHandle>,
    /// When set, spawn in-process agent actors instead of subprocesses.
    actor_ctx: Option<ActorContext>,
}

/// Context for in-process agent spawning (daemon mode).
#[derive(Clone)]
pub struct ActorContext {
    pub registry: clanker_actor::ProcessRegistry,
    pub factory: std::sync::Arc<crate::modes::daemon::socket_bridge::SessionFactory>,
}

impl Default for SubagentTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SubagentTool {
    pub fn new() -> Self {
        Self {
            panel_tx: None,
            process_monitor: None,
            actor_ctx: None,
            definition: Self::make_definition(),
        }
    }

    pub fn with_panel_tx(mut self, tx: PanelTx) -> Self {
        self.panel_tx = Some(tx);
        self
    }

    /// Attach a process monitor to track spawned subagents.
    pub fn with_process_monitor(mut self, monitor: crate::procmon::ProcessMonitorHandle) -> Self {
        self.process_monitor = Some(monitor);
        self
    }

    /// Enable in-process agent spawning (daemon mode).
    pub fn with_actor_ctx(mut self, ctx: ActorContext) -> Self {
        self.actor_ctx = Some(ctx);
        self
    }

    fn make_definition() -> ToolDefinition {
        ToolDefinition {
            name: "subagent".to_string(),
            description: "Spawn subagent(s) to work on tasks in parallel. Each subagent is a separate clankers instance. Output streams to the subagent panel. Three modes:\n- task: single task\n- tasks: parallel tasks (max 8, 4 concurrent)\n- chain: sequential tasks where {previous} is replaced with prior output".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Single task prompt"
                    },
                    "tasks": {
                        "type": "array",
                        "description": "Parallel tasks (max 8, 4 concurrent)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "task": { "type": "string", "description": "Task prompt" },
                                "agent": { "type": "string", "description": "Agent definition" }
                            },
                            "required": ["task"]
                        }
                    },
                    "chain": {
                        "type": "array",
                        "description": "Sequential chain. {previous} replaced with prior output.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "task": { "type": "string" },
                                "agent": { "type": "string" }
                            },
                            "required": ["task"]
                        }
                    },
                    "agent": {
                        "type": "string",
                        "description": "Agent definition name (default for all tasks)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory override"
                    }
                }
            }),
        }
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let default_agent = params.get("agent").and_then(|v| v.as_str()).map(String::from);
        let cwd = params.get("cwd").and_then(|v| v.as_str()).map(String::from);
        let panel_tx = self.panel_tx.clone();
        let call_id = ctx.call_id.clone();
        let signal = ctx.signal.clone();
        let process_monitor = self.process_monitor.as_ref();
        let actor_ctx = self.actor_ctx.as_ref();

        if let Some(task) = params.get("task").and_then(|v| v.as_str()) {
            let preview: String = task.chars().take(80).collect();
            ctx.emit_progress(&format!("subagent: {}", preview));
            run_single(
                task,
                default_agent.as_deref(),
                cwd.as_deref(),
                panel_tx.as_ref(),
                &call_id,
                signal,
                process_monitor,
                actor_ctx,
            )
            .await
        } else if let Some(tasks) = params.get("tasks").and_then(|v| v.as_array()) {
            ctx.emit_progress(&format!("subagent: {} parallel tasks", tasks.len()));
            run_parallel(
                tasks,
                default_agent.as_deref(),
                cwd.as_deref(),
                panel_tx.as_ref(),
                &call_id,
                signal,
                process_monitor,
                actor_ctx,
            )
            .await
        } else if let Some(chain) = params.get("chain").and_then(|v| v.as_array()) {
            ctx.emit_progress(&format!("subagent: {} chained steps", chain.len()));
            run_chain(
                chain,
                default_agent.as_deref(),
                cwd.as_deref(),
                panel_tx.as_ref(),
                &call_id,
                signal,
                process_monitor,
                actor_ctx,
            )
            .await
        } else {
            ToolResult::error("Must provide exactly one of: task, tasks, or chain")
        }
    }
}

// ── Single task ─────────────────────────────────────────────────────────────

async fn run_single(
    task: &str,
    agent: Option<&str>,
    cwd: Option<&str>,
    panel_tx: Option<&PanelTx>,
    call_id: &str,
    signal: CancellationToken,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
    actor_ctx: Option<&ActorContext>,
) -> ToolResult {
    let result = if let Some(ctx) = actor_ctx {
        crate::modes::daemon::agent_process::run_ephemeral_agent(
            &ctx.registry,
            &ctx.factory,
            task,
            agent,
            None,
            panel_tx,
            call_id,
            signal,
        )
        .await
    } else {
        spawn_subprocess(task, agent, cwd, panel_tx, call_id, signal, process_monitor).await
    };
    match result {
        Ok(output) => ToolResult::text(output),
        Err(e) => ToolResult::error(format!("Subagent failed: {}", e)),
    }
}

// ── Parallel tasks ──────────────────────────────────────────────────────────

async fn run_parallel(
    tasks: &[Value],
    default_agent: Option<&str>,
    cwd: Option<&str>,
    panel_tx: Option<&PanelTx>,
    call_id: &str,
    signal: CancellationToken,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
    actor_ctx: Option<&ActorContext>,
) -> ToolResult {
    if tasks.len() > 8 {
        return ToolResult::error("Maximum 8 parallel tasks allowed");
    }

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut handles = Vec::new();

    for (i, task_val) in tasks.iter().enumerate() {
        let task_text = match task_val.get("task").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return ToolResult::error(format!("Task {} missing 'task' field", i)),
        };
        let agent = task_val
            .get("agent")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| default_agent.map(String::from));
        let cwd = cwd.map(String::from);
        let sem = semaphore.clone();
        let sig = signal.clone();
        let ptx = panel_tx.cloned();
        let pmon = process_monitor.cloned();
        let actx = actor_ctx.cloned();
        let cid = format!("{}:parallel:{}", call_id, i);

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            if let Some(ctx) = &actx {
                crate::modes::daemon::agent_process::run_ephemeral_agent(
                    &ctx.registry,
                    &ctx.factory,
                    &task_text,
                    agent.as_deref(),
                    None,
                    ptx.as_ref(),
                    &cid,
                    sig,
                )
                .await
            } else {
                spawn_subprocess(&task_text, agent.as_deref(), cwd.as_deref(), ptx.as_ref(), &cid, sig, pmon.as_ref()).await
            }
        }));
    }

    let mut results = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok(Ok(output)) => results.push(format!("[Task {}]:\n{}", i + 1, output)),
            Ok(Err(e)) => results.push(format!("[Task {}]: ERROR: {}", i + 1, e)),
            Err(e) => results.push(format!("[Task {}]: PANIC: {}", i + 1, e)),
        }
    }

    ToolResult::text(results.join("\n\n"))
}

// ── Chain tasks ─────────────────────────────────────────────────────────────

async fn run_chain(
    chain: &[Value],
    default_agent: Option<&str>,
    cwd: Option<&str>,
    panel_tx: Option<&PanelTx>,
    call_id: &str,
    signal: CancellationToken,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
    actor_ctx: Option<&ActorContext>,
) -> ToolResult {
    let mut previous_output = String::new();

    for (i, step) in chain.iter().enumerate() {
        let task_template = match step.get("task").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::error(format!("Chain step {} missing 'task' field", i)),
        };
        let task_text = task_template.replace("{previous}", &previous_output);
        let agent = step.get("agent").and_then(|v| v.as_str()).or(default_agent);
        let step_cid = format!("{}:chain:{}", call_id, i);

        let result = if let Some(ctx) = actor_ctx {
            crate::modes::daemon::agent_process::run_ephemeral_agent(
                &ctx.registry,
                &ctx.factory,
                &task_text,
                agent,
                None,
                panel_tx,
                &step_cid,
                signal.clone(),
            )
            .await
        } else {
            spawn_subprocess(&task_text, agent, cwd, panel_tx, &step_cid, signal.clone(), process_monitor).await
        };
        match result {
            Ok(output) => previous_output = output,
            Err(e) => return ToolResult::error(format!("Chain step {} failed: {}", i, e)),
        }

        if signal.is_cancelled() {
            return ToolResult::error("Cancelled");
        }
    }

    ToolResult::text(previous_output)
}

// ── Subprocess spawning ─────────────────────────────────────────────────────

/// Maximum collected output size (bytes) to prevent unbounded memory growth.
const MAX_COLLECTED_BYTES: usize = 10 * 1024 * 1024; // 10 MB

// Tiger Style: compile-time bounds
const _: () = assert!(MAX_COLLECTED_BYTES > 0);
const _: () = assert!(MAX_COLLECTED_BYTES <= 100 * 1024 * 1024); // sanity: ≤ 100 MB

/// Derive a short display name and task preview from the call ID and task text.
fn subagent_display_names(call_id: &str, task: &str) -> (String, String, String) {
    let sub_id = call_id.to_string();
    let short_name = if sub_id.contains(':') {
        sub_id.rsplit(':').next().unwrap_or(&sub_id).to_string()
    } else {
        sub_id.chars().take(8).collect()
    };
    let task_preview: String = task.chars().take(60).collect();
    (sub_id, short_name, task_preview)
}

/// Build the clankers subprocess command.
fn build_subprocess_command(
    task: &str,
    agent: Option<&str>,
    cwd: Option<&str>,
) -> Result<tokio::process::Command, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find clankers executable: {}", e))?;

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

    // New process group — kill the entire tree on cancel
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    Ok(cmd)
}

/// Register a spawned child with the process monitor and send a Started event.
fn register_subprocess(
    child_pid: Option<u32>,
    task: &str,
    call_id: &str,
    sub_id: &str,
    short_name: &str,
    task_preview: &str,
    panel_tx: Option<&PanelTx>,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
) {
    if let Some(monitor) = process_monitor
        && let Some(pid) = child_pid
    {
        let full_preview: String = task.chars().take(200).collect();
        monitor.register(pid, crate::procmon::ProcessMeta {
            tool_name: "subagent".to_string(),
            command: format!("subagent: {}", full_preview),
            call_id: call_id.to_string(),
        });
    }

    if let Some(tx) = panel_tx {
        tx.send(SubagentEvent::Started {
            id: sub_id.to_string(),
            name: short_name.to_string(),
            task: task_preview.to_string(),
            pid: child_pid,
        }).ok();
    }
}

/// Stream stdout from a subprocess, forwarding lines to the panel.
///
/// Returns the collected output on success. Handles cancellation via the signal token.
async fn stream_subprocess_output(
    child: &mut tokio::process::Child,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: &CancellationToken,
) -> Result<String, String> {
    use tokio::io::AsyncBufReadExt;
    use tokio::io::BufReader;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let mut reader = BufReader::new(stdout).lines();
    let mut collected = String::new();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(tx) = panel_tx {
                            tx.send(SubagentEvent::Output {
                                id: sub_id.to_string(),
                                line: line.clone(),
                            }).ok();
                        }
                        // Tiger Style: bounded collection
                        if collected.len() < MAX_COLLECTED_BYTES {
                            if !collected.is_empty() {
                                collected.push('\n');
                            }
                            collected.push_str(&line);
                        }
                    }
                    Ok(None) => break,
                    Err(e) => return Err(format!("Read error: {}", e)),
                }
            }
            () = signal.cancelled() => {
                child.kill().await.ok();
                if let Some(tx) = panel_tx {
                    tx.send(SubagentEvent::Error {
                        id: sub_id.to_string(),
                        message: "Cancelled".into(),
                    }).ok();
                }
                return Err("Cancelled".to_string());
            }
        }
    }

    Ok(collected)
}

/// Spawn a clankers subprocess in print mode, streaming output to panel only.
async fn spawn_subprocess(
    task: &str,
    agent: Option<&str>,
    cwd: Option<&str>,
    panel_tx: Option<&PanelTx>,
    call_id: &str,
    signal: CancellationToken,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
) -> Result<String, String> {
    let (sub_id, short_name, task_preview) = subagent_display_names(call_id, task);

    let mut cmd = build_subprocess_command(task, agent, cwd)?;
    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;
    let child_pid = child.id();

    register_subprocess(child_pid, task, call_id, &sub_id, &short_name, &task_preview, panel_tx, process_monitor);

    let stderr_handle = child.stderr.take();
    let collected = stream_subprocess_output(&mut child, &sub_id, panel_tx, &signal).await?;

    let status = child.wait().await.map_err(|e| format!("Wait error: {}", e))?;

    if status.success() {
        if let Some(tx) = panel_tx {
            tx.send(SubagentEvent::Done { id: sub_id }).ok();
        }
        Ok(collected)
    } else {
        let stderr_text = read_stderr(stderr_handle).await;
        let err_msg = format!("Exit code: {}\nstdout: {}\nstderr: {}", status, collected, stderr_text);
        if let Some(tx) = panel_tx {
            tx.send(SubagentEvent::Error {
                id: sub_id,
                message: err_msg.clone(),
            }).ok();
        }
        Err(err_msg)
    }
}

/// Read remaining stderr from a child process (best-effort).
async fn read_stderr(stderr_handle: Option<tokio::process::ChildStderr>) -> String {
    use tokio::io::BufReader;

    let Some(stderr) = stderr_handle else {
        return String::new();
    };
    let mut buf = String::new();
    let mut reader = BufReader::new(stderr);
    tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await.ok();
    buf
}
