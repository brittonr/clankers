//! Delegate task to persistent worker
//!
//! Workers run as clankers subprocesses or are sent to remote peers via iroh RPC.
//! Their output is streamed to the subagent panel in the TUI.
//!
//! ## Routing
//!
//! When a `peer` parameter is specified, the task is sent to that remote peer
//! via iroh RPC. Otherwise, a local clankers subprocess is spawned.
//!
//! The tool can also auto-route based on the peer registry:
//! - If `tag` is specified, finds a peer with that capability tag
//! - If `agent` is specified, finds a peer that has that agent definition

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;
use crate::tui::components::subagent_event::SubagentEvent;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

/// Lazily-initialized, shared iroh endpoint. Created once on first remote
/// delegation, then reused for all subsequent calls across the process lifetime.
type SharedEndpoint = Arc<tokio::sync::OnceCell<iroh::Endpoint>>;

pub struct DelegateTool {
    definition: ToolDefinition,
    /// Track active workers by name
    workers: Arc<Mutex<HashMap<String, WorkerState>>>,
    panel_tx: Option<PanelTx>,
    /// Path to peer registry (for remote routing)
    peer_registry_path: Option<PathBuf>,
    /// Path to identity key (for iroh endpoint)
    identity_path: Option<PathBuf>,
    /// Persistent endpoint shared across all remote delegations
    shared_endpoint: SharedEndpoint,
    /// Process monitor for tracking spawned workers
    process_monitor: Option<crate::procmon::ProcessMonitorHandle>,
}

#[allow(dead_code)]
struct WorkerState {
    cwd: String,
    agent: Option<String>,
    /// If delegated to a remote peer, its node_id
    remote_peer: Option<String>,
}

impl Default for DelegateTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DelegateTool {
    pub fn new() -> Self {
        Self {
            shared_endpoint: Arc::new(tokio::sync::OnceCell::new()),
            panel_tx: None,
            process_monitor: None,
            definition: ToolDefinition {
                name: "delegate_task".to_string(),
                description: "Delegate a task to a worker. Can route to a local subprocess or a remote clankers peer via iroh P2P.\n\nRouting:\n- Specify 'peer' (name or node_id) to target a specific remote peer\n- Specify 'tag' to auto-find a peer with that capability tag\n- Otherwise runs locally as a subprocess".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "worker": {
                            "type": "string",
                            "description": "Worker name (auto-created if new)"
                        },
                        "task": {
                            "type": "string",
                            "description": "Task prompt to send to the worker"
                        },
                        "agent": {
                            "type": "string",
                            "description": "Agent definition name to configure the worker"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory (defaults to current, local only)"
                        },
                        "peer": {
                            "type": "string",
                            "description": "Remote peer name or node_id to delegate to"
                        },
                        "tag": {
                            "type": "string",
                            "description": "Capability tag to auto-route to a matching peer"
                        }
                    },
                    "required": ["worker", "task"]
                }),
            },
            workers: Arc::new(Mutex::new(HashMap::new())),
            peer_registry_path: None,
            identity_path: None,
        }
    }

    pub fn with_panel_tx(mut self, tx: PanelTx) -> Self {
        self.panel_tx = Some(tx);
        self
    }

    /// Enable remote peer routing
    pub fn with_peer_routing(mut self, registry_path: PathBuf, identity_path: PathBuf) -> Self {
        self.peer_registry_path = Some(registry_path);
        self.identity_path = Some(identity_path);
        self
    }

    /// Attach a process monitor to track spawned workers.
    pub fn with_process_monitor(mut self, monitor: crate::procmon::ProcessMonitorHandle) -> Self {
        self.process_monitor = Some(monitor);
        self
    }

    /// List active worker names
    pub async fn list_workers(&self) -> Vec<String> {
        let workers = self.workers.lock().await;
        workers.keys().cloned().collect()
    }

    /// Resolve which peer to use: explicit peer > tag match > agent match > None (local)
    fn resolve_peer(&self, peer: Option<&str>, tag: Option<&str>, agent: Option<&str>) -> Option<String> {
        let registry_path = self.peer_registry_path.as_ref()?;
        let registry = crate::modes::rpc::peers::PeerRegistry::load(registry_path);

        // Explicit peer name or node_id
        if let Some(peer_ref) = peer {
            // Try as node_id first
            if registry.peers.contains_key(peer_ref) {
                return Some(peer_ref.to_string());
            }
            // Try as name
            if let Some(p) = registry.peers.values().find(|p| p.name == peer_ref) {
                return Some(p.node_id.clone());
            }
            // Treat as raw node_id not in registry
            return Some(peer_ref.to_string());
        }

        // Tag-based routing
        if let Some(tag) = tag {
            let matches = registry.find_by_tag(tag);
            if let Some(best) =
                matches.into_iter().filter(|p| p.capabilities.accepts_prompts).max_by_key(|p| p.last_seen)
            {
                return Some(best.node_id.clone());
            }
        }

        // Agent-based routing
        if let Some(agent_name) = agent {
            let matches = registry.find_by_agent(agent_name);
            if let Some(best) =
                matches.into_iter().filter(|p| p.capabilities.accepts_prompts).max_by_key(|p| p.last_seen)
            {
                return Some(best.node_id.clone());
            }
        }

        None // Fall back to local
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let worker_name = match params.get("worker").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => return ToolResult::error("Missing 'worker' parameter"),
        };
        let task = match params.get("task").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return ToolResult::error("Missing 'task' parameter"),
        };
        let agent = params.get("agent").and_then(|v| v.as_str()).map(String::from);
        let cwd = params.get("cwd").and_then(|v| v.as_str()).map(String::from);
        let peer = params.get("peer").and_then(|v| v.as_str()).map(String::from);
        let tag = params.get("tag").and_then(|v| v.as_str()).map(String::from);
        let signal = ctx.signal.clone();

        // Resolve routing: remote peer or local?
        let target_peer = self.resolve_peer(peer.as_deref(), tag.as_deref(), agent.as_deref());

        if let Some(node_id) = target_peer {
            ctx.emit_progress(&format!(
                "delegating '{}' to remote peer {}",
                worker_name,
                &node_id[..12.min(node_id.len())]
            ));
            // Remote delegation via iroh RPC
            {
                let mut workers = self.workers.lock().await;
                workers.entry(worker_name.clone()).or_insert(WorkerState {
                    cwd: cwd.clone().unwrap_or_else(|| ".".to_string()),
                    agent: agent.clone(),
                    remote_peer: Some(node_id.clone()),
                });
            }

            let identity_path = match &self.identity_path {
                Some(p) => p.clone(),
                None => return ToolResult::error("Remote delegation requires identity key path"),
            };

            run_remote_worker(
                &worker_name,
                &task,
                &node_id,
                &identity_path,
                self.shared_endpoint.clone(),
                self.panel_tx.as_ref(),
                signal,
            )
            .await
        } else {
            ctx.emit_progress(&format!("spawning local worker '{}'", worker_name));
            // Local subprocess
            {
                let mut workers = self.workers.lock().await;
                workers.entry(worker_name.clone()).or_insert(WorkerState {
                    cwd: cwd.clone().unwrap_or_else(|| ".".to_string()),
                    agent: agent.clone(),
                    remote_peer: None,
                });
            }

            run_worker_subprocess(&worker_name, &task, agent.as_deref(), cwd.as_deref(), self.panel_tx.as_ref(), signal, self.process_monitor.as_ref())
                .await
        }
    }
}

/// Maximum number of retry attempts for transient connection failures.
const MAX_RETRIES: u32 = 3;
/// Initial backoff delay between retries.
const RETRY_BACKOFF_MS: u64 = 500;

/// Run a worker via iroh RPC to a remote peer.
///
/// Uses a shared endpoint (created lazily on first call) and retries transient
/// connection failures with exponential backoff. Streams text deltas to the
/// subagent panel in real-time via `send_rpc_streaming`.
async fn run_remote_worker(
    worker_name: &str,
    task: &str,
    node_id: &str,
    identity_path: &std::path::Path,
    shared_endpoint: SharedEndpoint,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
) -> ToolResult {
    use crate::modes::rpc::iroh;
    use crate::modes::rpc::protocol::Request;

    let sub_id = format!("worker:{}", worker_name);
    let task_preview: String = task.chars().take(60).collect();
    let short_node = &node_id[..12.min(node_id.len())];

    if let Some(tx) = panel_tx {
        let _ = tx.send(SubagentEvent::Started {
            id: sub_id.clone(),
            name: format!("{} → {}", worker_name, short_node),
            task: task_preview,
            pid: None, // No local PID for remote workers
        });
    }

    let remote: ::iroh::PublicKey = match node_id.parse() {
        Ok(pk) => pk,
        Err(e) => {
            let msg = format!("Invalid peer node ID '{}': {}", short_node, e);
            if let Some(tx) = panel_tx {
                let _ = tx.send(SubagentEvent::Error {
                    id: sub_id,
                    message: msg.clone(),
                });
            }
            return ToolResult::error(msg);
        }
    };

    // Get or create the shared endpoint (once per process lifetime)
    let identity_path_owned = identity_path.to_path_buf();
    let endpoint = match shared_endpoint
        .get_or_try_init(|| async {
            let identity = iroh::Identity::load_or_generate(&identity_path_owned);
            iroh::start_endpoint(&identity).await
        })
        .await
    {
        Ok(ep) => ep,
        Err(e) => {
            let msg = format!("Failed to start iroh endpoint: {}", e);
            if let Some(tx) = panel_tx {
                let _ = tx.send(SubagentEvent::Error {
                    id: sub_id,
                    message: msg.clone(),
                });
            }
            return ToolResult::error(msg);
        }
    };
    let request = Request::new("prompt", serde_json::json!({ "text": task }));

    // Retry loop with exponential backoff for transient connection failures
    let mut last_err = String::new();
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let backoff = std::time::Duration::from_millis(RETRY_BACKOFF_MS * 2u64.pow(attempt - 1));
            if let Some(tx) = panel_tx {
                let _ = tx.send(SubagentEvent::Output {
                    id: sub_id.clone(),
                    line: format!("Retry {}/{} after {:?}...", attempt, MAX_RETRIES, backoff),
                });
            }
            tokio::select! {
                () = tokio::time::sleep(backoff) => {}
                () = signal.cancelled() => {
                    if let Some(tx) = panel_tx {
                        let _ = tx.send(SubagentEvent::Error { id: sub_id, message: "Cancelled".into() });
                    }
                    return ToolResult::error(format!("Remote worker '{}' cancelled", worker_name));
                }
            }
        }

        // Use streaming RPC so text deltas flow to the panel in real-time
        let sub_id_clone = sub_id.clone();
        let panel_tx_clone = panel_tx.cloned();

        let result = tokio::select! {
            res = iroh::send_rpc_streaming(endpoint, remote, &request, |notification| {
                // Stream text deltas to the subagent panel as they arrive
                if let Some(method) = notification.get("method").and_then(|v| v.as_str()) {
                    match method {
                        "agent.text_delta" => {
                            if let Some(text) = notification
                                .get("params")
                                .and_then(|p| p.get("text"))
                                .and_then(|v| v.as_str())
                                && let Some(ref tx) = panel_tx_clone
                            {
                                // Send each line separately for TUI rendering
                                for line in text.split('\n') {
                                    if !line.is_empty() {
                                        let _ = tx.send(SubagentEvent::Output {
                                            id: sub_id_clone.clone(),
                                            line: line.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                        "agent.tool_call" => {
                            if let Some(ref tx) = panel_tx_clone {
                                let tool = notification
                                    .get("params")
                                    .and_then(|p| p.get("tool_name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?");
                                let _ = tx.send(SubagentEvent::Output {
                                    id: sub_id_clone.clone(),
                                    line: format!("[tool: {}]", tool),
                                });
                            }
                        }
                        "agent.tool_result" => {
                            if let Some(ref tx) = panel_tx_clone {
                                let is_error = notification
                                    .get("params")
                                    .and_then(|p| p.get("is_error"))
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                if is_error {
                                    let _ = tx.send(SubagentEvent::Output {
                                        id: sub_id_clone.clone(),
                                        line: "[tool error]".to_string(),
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }) => res.map(|(_, response)| response),
            () = signal.cancelled() => {
                if let Some(tx) = panel_tx {
                    let _ = tx.send(SubagentEvent::Error { id: sub_id, message: "Cancelled".into() });
                }
                return ToolResult::error(format!("Remote worker '{}' cancelled", worker_name));
            }
        };

        match result {
            Ok(response) => {
                if let Some(result) = response.ok {
                    let text = result.get("text").and_then(|v| v.as_str()).unwrap_or("");

                    if let Some(tx) = panel_tx {
                        let _ = tx.send(SubagentEvent::Done { id: sub_id });
                    }

                    return ToolResult::text(format!("[remote:{}] {}", short_node, text));
                } else if let Some(err) = response.error {
                    // Application-level errors are not retryable
                    let msg = format!("Remote peer error: {}", err);
                    if let Some(tx) = panel_tx {
                        let _ = tx.send(SubagentEvent::Error {
                            id: sub_id,
                            message: msg.clone(),
                        });
                    }
                    return ToolResult::error(msg);
                }
                return ToolResult::error("Empty response from remote peer");
            }
            Err(e) => {
                // Connection-level errors are retryable
                last_err = format!("{}", e);
                tracing::warn!(
                    "Remote worker '{}' attempt {}/{} failed: {}",
                    worker_name,
                    attempt + 1,
                    MAX_RETRIES + 1,
                    last_err
                );
                continue;
            }
        }
    }

    // All retries exhausted
    let msg = format!("Failed to reach peer '{}' after {} attempts: {}", short_node, MAX_RETRIES + 1, last_err);
    if let Some(tx) = panel_tx {
        let _ = tx.send(SubagentEvent::Error {
            id: sub_id,
            message: msg.clone(),
        });
    }
    ToolResult::error(msg)
}

/// Run a worker as a clankers subprocess, streaming output to the panel.
pub async fn run_worker_subprocess(
    worker_name: &str,
    task: &str,
    agent: Option<&str>,
    cwd: Option<&str>,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
    process_monitor: Option<&crate::procmon::ProcessMonitorHandle>,
) -> ToolResult {
    use tokio::io::AsyncBufReadExt;
    use tokio::io::BufReader;

    let sub_id = format!("worker:{}", worker_name);
    let task_preview: String = task.chars().take(60).collect();

    let exe = match resolve_clankers_exe() {
        Ok(e) => e,
        Err(e) => return ToolResult::error(format!("Cannot find clankers executable: {}", e)),
    };

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

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return ToolResult::error(format!("Failed to spawn worker '{}': {}", worker_name, e)),
    };
    let child_pid = child.id();

    // Register process with monitor
    if let Some(monitor) = process_monitor
        && let Some(pid) = child_pid {
            let task_preview_full: String = task.chars().take(200).collect();
            monitor.register(pid, crate::procmon::ProcessMeta {
                tool_name: "delegate".to_string(),
                command: format!("worker:{} {}", worker_name, task_preview_full),
                call_id: format!("worker:{}", worker_name),
            });
        }

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

    let mut reader = BufReader::new(stdout).lines();
    let mut collected = String::new();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(tx) = panel_tx {
                            let _ = tx.send(SubagentEvent::Output {
                                id: sub_id.clone(),
                                line: line.clone(),
                            });
                        }
                        if !collected.is_empty() {
                            collected.push('\n');
                        }
                        collected.push_str(&line);
                    }
                    Ok(None) => break,
                    Err(e) => return ToolResult::error(format!("Worker '{}' read error: {}", worker_name, e)),
                }
            }
            () = signal.cancelled() => {
                let _ = child.kill().await;
                if let Some(tx) = panel_tx {
                    let _ = tx.send(SubagentEvent::Error { id: sub_id, message: "Cancelled".into() });
                }
                return ToolResult::error(format!("Worker '{}' cancelled", worker_name));
            }
        }
    }

    let status = child.wait().await.map_err(|e| format!("Wait error: {}", e));
    let status = match status {
        Ok(s) => s,
        Err(e) => return ToolResult::error(e),
    };

    if status.success() {
        if let Some(tx) = panel_tx {
            let _ = tx.send(SubagentEvent::Done { id: sub_id });
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
                id: sub_id,
                message: err_msg.clone(),
            });
        }
        ToolResult::error(err_msg)
    }
}

/// Resolve the clankers executable path, handling the case where the binary was
/// recompiled (and the old inode deleted) while the process is still running.
///
/// Tries in order:
/// 1. `std::env::current_exe()` — if the file still exists on disk
/// 2. `CARGO_BIN_EXE_clankers` env var (set by `cargo test`)
/// 3. `target/debug/clankers` relative to the cargo manifest dir
/// 4. `target/release/clankers` relative to the cargo manifest dir
/// 5. `clankers` in `$PATH`
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
