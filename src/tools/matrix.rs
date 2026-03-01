//! Matrix communication tools — send/receive messages, list rooms, discover peers.
//!
//! These tools allow the agent to interact with other clankers instances and
//! humans over the Matrix protocol. Requires the `matrix` feature and a
//! configured Matrix account (`~/.clankers/matrix.json`).

#[cfg(feature = "matrix")]
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
#[cfg(feature = "matrix")]
use tokio::sync::RwLock;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

/// Shared Matrix state, injected at startup when the `matrix` feature is enabled.
///
/// The tools hold a reference to this state and use it to send/receive messages.
/// The actual Matrix client connection is managed by the TUI/mode layer.
#[cfg(feature = "matrix")]
pub struct MatrixState {
    pub client: Arc<RwLock<clankers_matrix::MatrixClient>>,
    pub bridge: Arc<clankers_matrix::bridge::MatrixBridge>,
}

// ════════════════════════════════════════════════════════════════════
//  matrix_send — Send a message to a Matrix room
// ════════════════════════════════════════════════════════════════════

pub struct MatrixSendTool {
    definition: ToolDefinition,
    #[cfg(feature = "matrix")]
    state: Option<Arc<MatrixState>>,
}

impl Default for MatrixSendTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixSendTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "matrix_send".to_string(),
                description: "Send a message to a Matrix room. Use this to communicate with \
                    other clankers instances or humans in a Matrix room. Messages are visible to \
                    all room members (both clankers agents and regular Matrix clients)."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": {
                            "type": "string",
                            "description": "Matrix room ID (e.g. '!abc123:matrix.org') or alias (e.g. '#clankers-collab:matrix.org')"
                        },
                        "message": {
                            "type": "string",
                            "description": "Message text to send (supports markdown)"
                        }
                    },
                    "required": ["room_id", "message"]
                }),
            },
            #[cfg(feature = "matrix")]
            state: None,
        }
    }

    #[cfg(feature = "matrix")]
    pub fn with_state(mut self, state: Arc<MatrixState>) -> Self {
        self.state = Some(state);
        self
    }
}

#[async_trait]
impl Tool for MatrixSendTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        #[cfg(not(feature = "matrix"))]
        {
            return ToolResult::error("Matrix support not enabled. Rebuild with `--features matrix`.");
        }

        #[cfg(feature = "matrix")]
        {
            let state = match &self.state {
                Some(s) => s,
                None => return ToolResult::error("Matrix not connected. Use /matrix login first."),
            };

            let room_id = match _params.get("room_id").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return ToolResult::error("Missing required parameter: room_id"),
            };

            let message = match _params.get("message").and_then(|v| v.as_str()) {
                Some(m) => m,
                None => return ToolResult::error("Missing required parameter: message"),
            };

            _ctx.emit_progress(&format!("sending to {}", room_id));

            let client = state.client.read().await;
            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(room_id) {
                Ok(room_id) => room_id.to_owned(),
                Err(e) => return ToolResult::error(format!("Invalid room ID: {e}")),
            };

            match client.send_chat(&room_id_parsed, message).await {
                Ok(()) => ToolResult::text(format!("Message sent to {}", room_id)),
                Err(e) => ToolResult::error(format!("Failed to send message: {e}")),
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════
//  matrix_read — Read recent messages from a Matrix room
// ════════════════════════════════════════════════════════════════════

pub struct MatrixReadTool {
    definition: ToolDefinition,
    #[cfg(feature = "matrix")]
    state: Option<Arc<MatrixState>>,
}

impl Default for MatrixReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixReadTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "matrix_read".to_string(),
                description: "Read recent messages from a Matrix room. Returns the last N \
                    messages including sender, timestamp, and content. Shows both human \
                    messages and clankers agent messages."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": {
                            "type": "string",
                            "description": "Matrix room ID or alias"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of messages to return (default: 20, max: 100)",
                            "default": 20
                        }
                    },
                    "required": ["room_id"]
                }),
            },
            #[cfg(feature = "matrix")]
            state: None,
        }
    }

    #[cfg(feature = "matrix")]
    pub fn with_state(mut self, state: Arc<MatrixState>) -> Self {
        self.state = Some(state);
        self
    }
}

#[async_trait]
impl Tool for MatrixReadTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        #[cfg(not(feature = "matrix"))]
        {
            return ToolResult::error("Matrix support not enabled. Rebuild with `--features matrix`.");
        }

        #[cfg(feature = "matrix")]
        {
            let state = match &self.state {
                Some(s) => s,
                None => return ToolResult::error("Matrix not connected. Use /matrix login first."),
            };

            let room_id = match _params.get("room_id").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return ToolResult::error("Missing required parameter: room_id"),
            };

            let limit = _params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20).min(100) as usize;

            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(room_id) {
                Ok(id) => id.to_owned(),
                Err(e) => return ToolResult::error(format!("Invalid room ID: {e}")),
            };

            let client = state.client.read().await;
            match client.message_history(&room_id_parsed, limit).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        return ToolResult::text(format!(
                            "No messages found in {}. The room may be empty or history is not accessible.",
                            room_id
                        ));
                    }

                    let formatted: Vec<String> = messages
                        .iter()
                        .rev() // reverse to chronological (oldest first)
                        .filter_map(|msg| {
                            // Skip raw clankers protocol messages — show human-readable ones
                            if matches!(msg.msg_type, clankers_matrix::client::HistoryMessageType::Clankers) {
                                return None;
                            }
                            let ts = msg.timestamp.format("%H:%M:%S");
                            Some(format!("[{}] {}: {}", ts, msg.sender, msg.body))
                        })
                        .collect();

                    if formatted.is_empty() {
                        return ToolResult::text(format!(
                            "No human-readable messages in {}. Only clankers protocol traffic found.",
                            room_id
                        ));
                    }

                    ToolResult::text(format!(
                        "Messages in {} ({} shown):\n\n{}",
                        room_id,
                        formatted.len(),
                        formatted.join("\n")
                    ))
                }
                Err(e) => ToolResult::error(format!("Failed to fetch messages: {e}")),
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════
//  matrix_rooms — List joined Matrix rooms
// ════════════════════════════════════════════════════════════════════

pub struct MatrixRoomsTool {
    definition: ToolDefinition,
    #[cfg(feature = "matrix")]
    state: Option<Arc<MatrixState>>,
}

impl Default for MatrixRoomsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixRoomsTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "matrix_rooms".to_string(),
                description: "List all Matrix rooms this clankers instance has joined. Shows room \
                    name, ID, member count, and topic."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            },
            #[cfg(feature = "matrix")]
            state: None,
        }
    }

    #[cfg(feature = "matrix")]
    pub fn with_state(mut self, state: Arc<MatrixState>) -> Self {
        self.state = Some(state);
        self
    }
}

#[async_trait]
impl Tool for MatrixRoomsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        #[cfg(not(feature = "matrix"))]
        {
            return ToolResult::error("Matrix support not enabled. Rebuild with `--features matrix`.");
        }

        #[cfg(feature = "matrix")]
        {
            let state = match &self.state {
                Some(s) => s,
                None => return ToolResult::error("Matrix not connected. Use /matrix login first."),
            };

            let client = state.client.read().await;
            let rooms = client.joined_rooms();

            if rooms.is_empty() {
                return ToolResult::text("No rooms joined. Use /matrix join <room> to join a room.");
            }

            let formatted: Vec<String> = rooms
                .iter()
                .map(|r| {
                    format!(
                        "• {} ({})\n  Members: {} | Topic: {}",
                        r.name,
                        r.room_id,
                        r.member_count,
                        if r.topic.is_empty() { "(none)" } else { &r.topic },
                    )
                })
                .collect();

            ToolResult::text(format!("Joined rooms:\n\n{}", formatted.join("\n\n")))
        }
    }
}

// ════════════════════════════════════════════════════════════════════
//  matrix_peers — List clankers instances discovered via Matrix
// ════════════════════════════════════════════════════════════════════

pub struct MatrixPeersTool {
    definition: ToolDefinition,
    #[cfg(feature = "matrix")]
    state: Option<Arc<MatrixState>>,
}

impl Default for MatrixPeersTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixPeersTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "matrix_peers".to_string(),
                description: "List clankers instances discovered via Matrix rooms. Shows their \
                    instance name, capabilities, available agents, and online status. These \
                    peers can receive prompts and RPC requests."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            },
            #[cfg(feature = "matrix")]
            state: None,
        }
    }

    #[cfg(feature = "matrix")]
    pub fn with_state(mut self, state: Arc<MatrixState>) -> Self {
        self.state = Some(state);
        self
    }
}

#[async_trait]
impl Tool for MatrixPeersTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        #[cfg(not(feature = "matrix"))]
        {
            return ToolResult::error("Matrix support not enabled. Rebuild with `--features matrix`.");
        }

        #[cfg(feature = "matrix")]
        {
            let state = match &self.state {
                Some(s) => s,
                None => return ToolResult::error("Matrix not connected. Use /matrix login first."),
            };

            let peers = state.bridge.peers().await;

            if peers.is_empty() {
                return ToolResult::text(
                    "No clankers peers discovered yet. Other clankers instances in shared Matrix rooms \
                     will appear here when they announce themselves.",
                );
            }

            let formatted: Vec<String> = peers
                .iter()
                .map(|p| {
                    let mut info = format!(
                        "• {} ({})\n  Version: {} | Prompts: {}",
                        p.instance_name,
                        p.user_id,
                        p.version,
                        if p.accepts_prompts { "yes" } else { "no" },
                    );
                    if !p.tags.is_empty() {
                        info.push_str(&format!("\n  Tags: {}", p.tags.join(", ")));
                    }
                    if !p.agents.is_empty() {
                        info.push_str(&format!("\n  Agents: {}", p.agents.join(", ")));
                    }
                    if let Some(ref model) = p.model {
                        info.push_str(&format!("\n  Model: {}", model));
                    }
                    info
                })
                .collect();

            ToolResult::text(format!("Matrix peers:\n\n{}", formatted.join("\n\n")))
        }
    }
}

// ════════════════════════════════════════════════════════════════════
//  matrix_join — Join a Matrix room
// ════════════════════════════════════════════════════════════════════

pub struct MatrixJoinTool {
    definition: ToolDefinition,
    #[cfg(feature = "matrix")]
    state: Option<Arc<MatrixState>>,
}

impl Default for MatrixJoinTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixJoinTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "matrix_join".to_string(),
                description: "Join a Matrix room by ID or alias. Once joined, the agent can \
                    send and receive messages in the room and discover other clankers instances."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room": {
                            "type": "string",
                            "description": "Room ID (e.g. '!abc123:matrix.org') or alias (e.g. '#clankers-collab:matrix.org')"
                        }
                    },
                    "required": ["room"]
                }),
            },
            #[cfg(feature = "matrix")]
            state: None,
        }
    }

    #[cfg(feature = "matrix")]
    pub fn with_state(mut self, state: Arc<MatrixState>) -> Self {
        self.state = Some(state);
        self
    }
}

#[async_trait]
impl Tool for MatrixJoinTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        #[cfg(not(feature = "matrix"))]
        {
            return ToolResult::error("Matrix support not enabled. Rebuild with `--features matrix`.");
        }

        #[cfg(feature = "matrix")]
        {
            let state = match &self.state {
                Some(s) => s,
                None => return ToolResult::error("Matrix not connected. Use /matrix login first."),
            };

            let room = match _params.get("room").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return ToolResult::error("Missing required parameter: room"),
            };

            let client = state.client.read().await;
            match client.join_room(room).await {
                Ok(room_id) => {
                    ToolResult::text(format!("Joined room {}. You can now send messages with matrix_send.", room_id))
                }
                Err(e) => ToolResult::error(format!("Failed to join room: {e}")),
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════
//  matrix_rpc — Send an RPC request to a clankers peer via Matrix
// ════════════════════════════════════════════════════════════════════

pub struct MatrixRpcTool {
    definition: ToolDefinition,
    #[cfg(feature = "matrix")]
    state: Option<Arc<MatrixState>>,
}

impl Default for MatrixRpcTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixRpcTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "matrix_rpc".to_string(),
                description: "Send an RPC request to another clankers instance via Matrix. \
                    Supports the same methods as the direct iroh RPC: ping, version, status, \
                    prompt. The target clankers must be in the same Matrix room."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": {
                            "type": "string",
                            "description": "Matrix room ID where the target clankers is"
                        },
                        "target": {
                            "type": "string",
                            "description": "Target clankers user ID (e.g. '@clankers-bot:matrix.org'). If omitted, broadcasts to all clankers in the room."
                        },
                        "method": {
                            "type": "string",
                            "enum": ["ping", "version", "status", "prompt"],
                            "description": "RPC method to call"
                        },
                        "params": {
                            "type": "object",
                            "description": "Method parameters (e.g. {\"text\": \"...\"} for prompt)"
                        }
                    },
                    "required": ["room_id", "method"]
                }),
            },
            #[cfg(feature = "matrix")]
            state: None,
        }
    }

    #[cfg(feature = "matrix")]
    pub fn with_state(mut self, state: Arc<MatrixState>) -> Self {
        self.state = Some(state);
        self
    }
}

#[async_trait]
impl Tool for MatrixRpcTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        #[cfg(not(feature = "matrix"))]
        {
            return ToolResult::error("Matrix support not enabled. Rebuild with `--features matrix`.");
        }

        #[cfg(feature = "matrix")]
        {
            let state = match &self.state {
                Some(s) => s,
                None => return ToolResult::error("Matrix not connected. Use /matrix login first."),
            };

            let room_id = match _params.get("room_id").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return ToolResult::error("Missing required parameter: room_id"),
            };

            let method = match _params.get("method").and_then(|v| v.as_str()) {
                Some(m) => m,
                None => return ToolResult::error("Missing required parameter: method"),
            };

            let rpc_params = _params.get("params").cloned().unwrap_or(json!({}));

            let mut request = clankers_matrix::protocol::RpcRequest::new(method, rpc_params);

            if let Some(target) = _params.get("target").and_then(|v| v.as_str()) {
                request = request.to(target);
            }

            let client = state.client.read().await;
            let timeout = std::time::Duration::from_secs(30);

            match state.bridge.send_rpc(&client, room_id, &request, timeout).await {
                Ok(response) => {
                    if let Some(result) = response.result {
                        ToolResult::text(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
                    } else if let Some(err) = response.error {
                        ToolResult::error(format!("RPC error ({}): {}", err.code, err.message))
                    } else {
                        ToolResult::text("RPC completed with no result")
                    }
                }
                Err(e) => ToolResult::error(format!("RPC failed: {e}")),
            }
        }
    }
}
