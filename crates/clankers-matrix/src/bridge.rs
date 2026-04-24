//! Bridge between Matrix events and the clankers peer system.
//!
//! Translates incoming Matrix messages into peer registry updates and
//! RPC dispatch, and outgoing RPC requests into Matrix room messages.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::client::MatrixClient;
use crate::protocol::ClankersEvent;
use crate::protocol::RpcRequest;
use crate::protocol::RpcResponse;

/// A discovered clankers peer on Matrix.
#[derive(Debug, Clone)]
pub struct MatrixPeer {
    /// Matrix user ID
    pub user_id: String,
    /// Instance name
    pub instance_name: String,
    /// clankers version
    pub version: String,
    /// Capability tags
    pub tags: Vec<String>,
    /// Available agents
    pub agents: Vec<String>,
    /// Whether it accepts prompts
    pub accepts_prompts: bool,
    /// Available tools
    pub tools: Vec<String>,
    /// Current model
    pub model: Option<String>,
    /// Last announcement time
    pub last_seen: chrono::DateTime<Utc>,
}

/// Pending RPC request awaiting a response.
struct PendingRequest {
    /// Channel to send the response back to the caller
    response_tx: tokio::sync::oneshot::Sender<RpcResponse>,
    /// When the request was sent (for timeout)
    sent_at: std::time::Instant,
}

/// The Matrix bridge manages the bidirectional flow between Matrix rooms
/// and the clankers agent system.
pub struct MatrixBridge {
    /// Known clankers peers discovered via announcements
    peers: Arc<RwLock<HashMap<String, MatrixPeer>>>,

    /// Pending RPC requests (keyed by request ID)
    pending: Arc<RwLock<HashMap<String, PendingRequest>>>,

    /// Channel for events the agent should see (chat messages, etc.)
    agent_event_tx: mpsc::UnboundedSender<BridgeEvent>,

    /// Receiver for agent events
    agent_event_rx: Option<mpsc::UnboundedReceiver<BridgeEvent>>,
}

/// Events forwarded from the bridge to the agent/TUI.
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    /// A new clankers peer was discovered or updated
    PeerUpdate(MatrixPeer),

    /// A peer went stale (no announcement for a while)
    PeerStale(String),

    /// An incoming chat message for the agent
    ChatMessage {
        sender: String,
        instance_name: String,
        body: String,
        room_id: String,
    },

    /// A regular text message (from a human)
    TextMessage {
        sender: String,
        body: String,
        room_id: String,
    },

    /// A media message (image, file, audio, video) from a user
    MediaMessage {
        sender: String,
        body: String,
        filename: String,
        media_type: String,
        source: ruma::events::room::MediaSource,
        room_id: String,
    },

    /// An RPC request addressed to us
    IncomingRpc { request: RpcRequest, room_id: String },
}

impl MatrixBridge {
    /// Create a new bridge.
    pub fn new() -> Self {
        let (agent_event_tx, agent_event_rx) = mpsc::unbounded_channel();
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
            agent_event_tx,
            agent_event_rx: Some(agent_event_rx),
        }
    }

    /// Take the agent event receiver (can only be called once).
    pub fn take_event_rx(&mut self) -> Option<mpsc::UnboundedReceiver<BridgeEvent>> {
        self.agent_event_rx.take()
    }

    /// Get a snapshot of known peers.
    pub async fn peers(&self) -> Vec<MatrixPeer> {
        self.peers.read().await.values().cloned().collect()
    }

    /// Get a specific peer by user ID.
    pub async fn get_peer(&self, user_id: &str) -> Option<MatrixPeer> {
        self.peers.read().await.get(user_id).cloned()
    }

    /// Start processing events from a Matrix client subscription.
    ///
    /// This spawns a background task that reads from the client's event
    /// stream and updates the peer registry / dispatches events.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "event loop; bounded by channel close")
    )]
    pub fn start(&self, mut event_rx: broadcast::Receiver<ClankersEvent>, our_user_id: &str) {
        let peers = self.peers.clone();
        let pending = self.pending.clone();
        let agent_tx = self.agent_event_tx.clone();
        let our_id = our_user_id.to_string();

        tokio::spawn(async move {
            info!("Matrix bridge started");
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&event, &peers, &pending, &agent_tx, &our_id).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Matrix bridge lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Matrix event stream closed, bridge stopping");
                        break;
                    }
                }
            }
        });
    }

    /// Process a single incoming event.
    async fn handle_event(
        event: &ClankersEvent,
        peers: &Arc<RwLock<HashMap<String, MatrixPeer>>>,
        pending: &Arc<RwLock<HashMap<String, PendingRequest>>>,
        agent_tx: &mpsc::UnboundedSender<BridgeEvent>,
        our_user_id: &str,
    ) {
        match event {
            ClankersEvent::Announce(announce) => {
                // Skip our own announcements
                if announce.user_id == our_user_id {
                    return;
                }

                debug!("Peer announcement from {}: {}", announce.user_id, announce.instance_name);

                let peer = MatrixPeer {
                    user_id: announce.user_id.clone(),
                    instance_name: announce.instance_name.clone(),
                    version: announce.version.clone(),
                    tags: announce.tags.clone(),
                    agents: announce.agents.clone(),
                    accepts_prompts: announce.accepts_prompts,
                    tools: announce.tools.clone(),
                    model: announce.model.clone(),
                    last_seen: announce.timestamp,
                };

                agent_tx.send(BridgeEvent::PeerUpdate(peer.clone())).ok();
                peers.write().await.insert(announce.user_id.clone(), peer);
            }

            ClankersEvent::RpcRequest(request) => {
                // Check if this is addressed to us (or broadcast)
                if let Some(ref target) = request.target
                    && target != our_user_id
                {
                    return; // Not for us
                }

                agent_tx
                    .send(BridgeEvent::IncomingRpc {
                        request: request.clone(),
                        room_id: String::new(), // filled by the caller
                    })
                    .ok();
            }

            ClankersEvent::RpcResponse(response) => {
                // Match to a pending request
                if let Some(ref id) = response.id {
                    let id_str = match id {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };

                    if let Some(pending_req) = pending.write().await.remove(&id_str) {
                        pending_req.response_tx.send(response.clone()).ok();
                    }
                }
            }

            ClankersEvent::Chat(chat) => {
                if chat.user_id == our_user_id {
                    return;
                }

                agent_tx
                    .send(BridgeEvent::ChatMessage {
                        sender: chat.user_id.clone(),
                        instance_name: chat.instance_name.clone(),
                        body: chat.body.clone(),
                        room_id: String::new(),
                    })
                    .ok();
            }

            ClankersEvent::Text {
                sender, body, room_id, ..
            } => {
                if sender == our_user_id {
                    return;
                }

                agent_tx
                    .send(BridgeEvent::TextMessage {
                        sender: sender.clone(),
                        body: body.clone(),
                        room_id: room_id.clone(),
                    })
                    .ok();
            }

            ClankersEvent::Media {
                sender,
                room_id,
                body,
                filename,
                media_type,
                source,
                ..
            } => {
                if sender == our_user_id {
                    return;
                }

                agent_tx
                    .send(BridgeEvent::MediaMessage {
                        sender: sender.clone(),
                        body: body.clone(),
                        filename: filename.clone(),
                        media_type: media_type.clone(),
                        source: source.clone(),
                        room_id: room_id.clone(),
                    })
                    .ok();
            }
        }
    }

    /// Send an RPC request and wait for a response (with timeout).
    pub async fn send_rpc(
        &self,
        client: &MatrixClient,
        room_id: &str,
        request: &RpcRequest,
        timeout: std::time::Duration,
    ) -> Result<RpcResponse, String> {
        let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|e| format!("Invalid room ID: {e}"))?;

        // Register pending request
        let id_str = request
            .id
            .as_ref()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.pending.write().await.insert(id_str.clone(), PendingRequest {
            response_tx,
            sent_at: std::time::Instant::now(),
        });

        // Send the request
        client.send_rpc_request(&room_id, request).await.map_err(|e| format!("Send failed: {e}"))?;

        // Wait for response or timeout
        match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                self.pending.write().await.remove(&id_str);
                Err("Response channel closed".to_string())
            }
            Err(_) => {
                self.pending.write().await.remove(&id_str);
                Err(format!("RPC timed out after {:?}", timeout))
            }
        }
    }

    /// Clean up stale pending requests.
    pub async fn gc_pending(&self, max_age: std::time::Duration) {
        let now = std::time::Instant::now();
        self.pending.write().await.retain(|_, req| now.duration_since(req.sent_at) < max_age);
    }
}

impl Default for MatrixBridge {
    fn default() -> Self {
        Self::new()
    }
}
