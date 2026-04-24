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

mod lifecycle;
mod protocol;
mod spawner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use clanker_tui_types::SubagentEvent;
use lifecycle::WorkerState;
use protocol::SharedEndpoint;
use protocol::run_remote_worker;
use serde_json::Value;
use serde_json::json;
// Public re-export for external use
pub use spawner::run_worker_subprocess;
use tokio::sync::Mutex;

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

/// Length of short node ID display (first N chars of node ID)
const NODE_ID_DISPLAY_LEN: usize = 12;

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
    /// When set, spawn in-process agent actors instead of subprocesses.
    actor_ctx: Option<crate::tools::subagent::ActorContext>,
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
            actor_ctx: None,
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

    /// Enable in-process agent spawning (daemon mode).
    pub fn with_actor_ctx(mut self, ctx: crate::tools::subagent::ActorContext) -> Self {
        self.actor_ctx = Some(ctx);
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
                &node_id[..NODE_ID_DISPLAY_LEN.min(node_id.len())]
            ));
            // Remote delegation via iroh RPC
            {
                let mut workers = self.workers.lock().await;
                workers.entry(worker_name.clone()).or_insert_with(|| {
                    WorkerState::new(
                        cwd.clone().unwrap_or_else(|| ".".to_string()),
                        agent.clone(),
                        Some(node_id.clone()),
                    )
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
            // Local worker
            {
                let mut workers = self.workers.lock().await;
                workers.entry(worker_name.clone()).or_insert_with(|| {
                    WorkerState::new(cwd.clone().unwrap_or_else(|| ".".to_string()), agent.clone(), None)
                });
            }

            if let Some(actx) = &self.actor_ctx {
                // In-process agent actor (daemon mode)
                match crate::modes::daemon::agent_process::run_ephemeral_agent(
                    &actx.registry,
                    &actx.factory,
                    &task,
                    agent.as_deref(),
                    None,
                    self.panel_tx.as_ref(),
                    &worker_name,
                    signal,
                )
                .await
                {
                    Ok(output) => ToolResult::text(output),
                    Err(e) => ToolResult::error(format!("Worker failed: {}", e)),
                }
            } else {
                // Subprocess fallback
                run_worker_subprocess(
                    &worker_name,
                    &task,
                    agent.as_deref(),
                    cwd.as_deref(),
                    self.panel_tx.as_ref(),
                    signal,
                    self.process_monitor.as_ref(),
                )
                .await
            }
        }
    }
}
