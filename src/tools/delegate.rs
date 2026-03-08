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

struct WorkerState {
    _cwd: String,
    _agent: Option<String>,
    /// If delegated to a remote peer, its node_id
    _remote_peer: Option<String>,
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
                    _cwd: cwd.clone().unwrap_or_else(|| ".".to_string()),
                    _agent: agent.clone(),
                    _remote_peer: Some(node_id.clone()),
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
                    _cwd: cwd.clone().unwrap_or_else(|| ".".to_string()),
                    _agent: agent.clone(),
                    _remote_peer: None,
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

    emit_started_event(panel_tx, &sub_id, worker_name, short_node, &task_preview);

    let remote = match parse_peer_node_id(node_id, short_node, panel_tx, &sub_id) {
        Ok(pk) => pk,
        Err(result) => return result,
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

    retry_remote_call(
        worker_name,
        short_node,
        endpoint,
        remote,
        &request,
        &sub_id,
        panel_tx,
        signal,
    )
    .await
}

/// Emit a "Started" event to the subagent panel
fn emit_started_event(panel_tx: Option<&PanelTx>, sub_id: &str, worker_name: &str, short_node: &str, task_preview: &str) {
    if let Some(tx) = panel_tx {
        let _ = tx.send(SubagentEvent::Started {
            id: sub_id.to_string(),
            name: format!("{} → {}", worker_name, short_node),
            task: task_preview.to_string(),
            pid: None,
        });
    }
}

/// Parse the peer node ID. Returns error ToolResult on failure.
fn parse_peer_node_id(
    node_id: &str,
    short_node: &str,
    panel_tx: Option<&PanelTx>,
    sub_id: &str,
) -> Result<::iroh::PublicKey, ToolResult> {
    match node_id.parse() {
        Ok(pk) => Ok(pk),
        Err(e) => {
            let msg = format!("Invalid peer node ID '{}': {}", short_node, e);
            if let Some(tx) = panel_tx {
                let _ = tx.send(SubagentEvent::Error {
                    id: sub_id.to_string(),
                    message: msg.clone(),
                });
            }
            Err(ToolResult::error(msg))
        }
    }
}

/// Retry the remote RPC call with exponential backoff
async fn retry_remote_call(
    worker_name: &str,
    short_node: &str,
    endpoint: &iroh::Endpoint,
    remote: ::iroh::PublicKey,
    request: &crate::modes::rpc::protocol::Request,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: CancellationToken,
) -> ToolResult {

    let mut last_err = String::new();

    for attempt in 0..=MAX_RETRIES {
        // Exponential backoff before retries
        if attempt > 0
            && let Err(result) = wait_for_retry(attempt, sub_id, panel_tx, &signal).await {
                return result;
            }

        // Attempt the RPC call
        match try_remote_call(endpoint, remote, request, sub_id, panel_tx, &signal).await {
            Ok(text) => {
                if let Some(tx) = panel_tx {
                    let _ = tx.send(SubagentEvent::Done { id: sub_id.to_string() });
                }
                return ToolResult::text(format!("[remote:{}] {}", short_node, text));
            }
            Err(RemoteCallError::ApplicationError(msg)) => {
                if let Some(tx) = panel_tx {
                    let _ = tx.send(SubagentEvent::Error {
                        id: sub_id.to_string(),
                        message: msg.clone(),
                    });
                }
                return ToolResult::error(msg);
            }
            Err(RemoteCallError::Cancelled) => {
                return ToolResult::error(format!("Remote worker '{}' cancelled", worker_name));
            }
            Err(RemoteCallError::ConnectionError(e)) => {
                last_err = e;
                tracing::warn!(
                    "Remote worker '{}' attempt {}/{} failed: {}",
                    worker_name,
                    attempt + 1,
                    MAX_RETRIES + 1,
                    last_err
                );
            }
        }
    }

    // All retries exhausted
    let msg = format!("Failed to reach peer '{}' after {} attempts: {}", short_node, MAX_RETRIES + 1, last_err);
    if let Some(tx) = panel_tx {
        let _ = tx.send(SubagentEvent::Error {
            id: sub_id.to_string(),
            message: msg.clone(),
        });
    }
    ToolResult::error(msg)
}

enum RemoteCallError {
    ConnectionError(String),
    ApplicationError(String),
    Cancelled,
}

/// Try a single remote RPC call with streaming notifications
async fn try_remote_call(
    endpoint: &iroh::Endpoint,
    remote: ::iroh::PublicKey,
    request: &crate::modes::rpc::protocol::Request,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: &CancellationToken,
) -> Result<String, RemoteCallError> {
    use crate::modes::rpc::iroh;

    let sub_id_clone = sub_id.to_string();
    let panel_tx_clone = panel_tx.cloned();

    let result = tokio::select! {
        res = iroh::send_rpc_streaming(endpoint, remote, request, move |notification| {
            handle_streaming_notification(notification, &sub_id_clone, panel_tx_clone.as_ref());
        }) => res.map(|(_, response)| response),
        () = signal.cancelled() => {
            if let Some(tx) = panel_tx {
                let _ = tx.send(SubagentEvent::Error {
                    id: sub_id.to_string(),
                    message: "Cancelled".into()
                });
            }
            return Err(RemoteCallError::Cancelled);
        }
    };

    match result {
        Ok(response) => {
            if let Some(result) = response.ok {
                let text = result.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(text.to_string())
            } else if let Some(err) = response.error {
                Err(RemoteCallError::ApplicationError(format!("Remote peer error: {}", err)))
            } else {
                Err(RemoteCallError::ApplicationError("Empty response from remote peer".to_string()))
            }
        }
        Err(e) => Err(RemoteCallError::ConnectionError(format!("{}", e))),
    }
}

/// Handle streaming RPC notifications (text deltas, tool calls, etc.)
fn handle_streaming_notification(notification: &Value, sub_id: &str, panel_tx: Option<&PanelTx>) {
    let method = notification.get("method").and_then(|v| v.as_str());

    match method {
        Some("agent.text_delta") => {
            if let Some(text) = notification.get("params").and_then(|p| p.get("text")).and_then(|v| v.as_str())
                && let Some(tx) = panel_tx
            {
                for line in text.split('\n') {
                    if !line.is_empty() {
                        let _ = tx.send(SubagentEvent::Output {
                            id: sub_id.to_string(),
                            line: line.to_string(),
                        });
                    }
                }
            }
        }
        Some("agent.tool_call") => {
            if let Some(tx) = panel_tx {
                let tool = notification
                    .get("params")
                    .and_then(|p| p.get("tool_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let _ = tx.send(SubagentEvent::Output {
                    id: sub_id.to_string(),
                    line: format!("[tool: {}]", tool),
                });
            }
        }
        Some("agent.tool_result") => {
            if let Some(tx) = panel_tx {
                let is_error = notification
                    .get("params")
                    .and_then(|p| p.get("is_error"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if is_error {
                    let _ = tx.send(SubagentEvent::Output {
                        id: sub_id.to_string(),
                        line: "[tool error]".to_string(),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Wait for retry with exponential backoff, checking for cancellation
async fn wait_for_retry(
    attempt: u32,
    sub_id: &str,
    panel_tx: Option<&PanelTx>,
    signal: &CancellationToken,
) -> Result<(), ToolResult> {
    let backoff = std::time::Duration::from_millis(RETRY_BACKOFF_MS * 2u64.pow(attempt - 1));
    if let Some(tx) = panel_tx {
        let _ = tx.send(SubagentEvent::Output {
            id: sub_id.to_string(),
            line: format!("Retry {}/{} after {:?}...", attempt, MAX_RETRIES, backoff),
        });
    }
    tokio::select! {
        () = tokio::time::sleep(backoff) => Ok(()),
        () = signal.cancelled() => {
            if let Some(tx) = panel_tx {
                let _ = tx.send(SubagentEvent::Error {
                    id: sub_id.to_string(),
                    message: "Cancelled".into()
                });
            }
            Err(ToolResult::error("Cancelled during retry backoff".to_string()))
        }
    }
}

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
    let task_preview: String = task.chars().take(60).collect();

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
        let task_preview_full: String = task.chars().take(200).collect();
        monitor.register(
            pid,
            crate::procmon::ProcessMeta {
                tool_name: "delegate".to_string(),
                command: format!("worker:{} {}", worker_name, task_preview_full),
                call_id: format!("worker:{}", worker_name),
            },
        );
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
