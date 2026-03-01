//! Matrix client — session management, login, sync, and message send/receive.
//!
//! Wraps `matrix-sdk` with clankers-specific conveniences: automatic session
//! restore, structured message sending, and an event stream that yields
//! [`ClankersEvent`]s.

use std::path::Path;

use matrix_sdk::Client;
use matrix_sdk::Room;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::ruma::RoomId;
use matrix_sdk::ruma::UInt;
use matrix_sdk::ruma::events::AnySyncTimelineEvent;
use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::serde::Raw;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::config::MatrixConfig;
use crate::protocol::Announce;
use crate::protocol::ChatMessage;
use crate::protocol::EVENT_ANNOUNCE;
use crate::protocol::EVENT_CHAT;
use crate::protocol::EVENT_RPC_REQUEST;
use crate::protocol::EVENT_RPC_RESPONSE;
use crate::protocol::ClankersEvent;
use crate::protocol::RpcRequest;
use crate::protocol::RpcResponse;

/// Errors from the Matrix client.
#[derive(Debug, thiserror::Error)]
pub enum MatrixError {
    #[error("Matrix SDK error: {0}")]
    Sdk(String),

    #[error("Not logged in")]
    NotLoggedIn,

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("Login failed: {0}")]
    LoginFailed(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serde(String),
}

impl From<matrix_sdk::Error> for MatrixError {
    fn from(e: matrix_sdk::Error) -> Self {
        MatrixError::Sdk(e.to_string())
    }
}

/// State of the Matrix connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Syncing,
    Error,
}

/// The clankers Matrix client.
///
/// Manages a single Matrix session. Create via [`MatrixClient::new`],
/// login with [`MatrixClient::login`] or [`MatrixClient::restore_session`],
/// then call [`MatrixClient::start_sync`] to begin receiving events.
pub struct MatrixClient {
    /// The underlying matrix-sdk client.
    client: Option<Client>,

    /// Current configuration.
    config: MatrixConfig,

    /// Connection state.
    state: ConnectionState,

    /// Channel for outgoing clankers events (received from Matrix).
    event_tx: broadcast::Sender<ClankersEvent>,

    /// Our user ID once logged in.
    user_id: Option<OwnedUserId>,

    /// Instance name for announcements.
    instance_name: String,
}

impl MatrixClient {
    /// Create a new Matrix client from configuration.
    ///
    /// Does **not** connect or login — call [`login`] or [`restore_session`] next.
    pub fn new(config: MatrixConfig, instance_name: impl Into<String>) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            client: None,
            config,
            state: ConnectionState::Disconnected,
            event_tx,
            user_id: None,
            instance_name: instance_name.into(),
        }
    }

    /// Subscribe to incoming clankers events from Matrix rooms.
    pub fn subscribe(&self) -> broadcast::Receiver<ClankersEvent> {
        self.event_tx.subscribe()
    }

    /// Current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Our Matrix user ID (if logged in).
    pub fn user_id(&self) -> Option<&OwnedUserId> {
        self.user_id.as_ref()
    }

    /// Build the underlying matrix-sdk Client.
    async fn build_client(&mut self, store_path: &Path) -> Result<Client, MatrixError> {
        let homeserver = self
            .config
            .homeserver
            .parse::<url::Url>()
            .map_err(|e| MatrixError::Config(format!("Invalid homeserver URL: {e}")))?;

        std::fs::create_dir_all(store_path)
            .map_err(|e| MatrixError::Config(format!("Cannot create store dir: {e}")))?;

        let client = Client::builder()
            .homeserver_url(&homeserver)
            .sqlite_store(store_path, None)
            .build()
            .await
            .map_err(|e| MatrixError::Sdk(format!("Failed to build client: {e}")))?;

        Ok(client)
    }

    /// Login with username and password.
    ///
    /// Stores the access token and device ID in the config for future
    /// session restore.
    pub async fn login(&mut self, username: &str, password: &str, store_path: &Path) -> Result<(), MatrixError> {
        self.state = ConnectionState::Connecting;

        let client = self.build_client(store_path).await?;

        let response = client
            .matrix_auth()
            .login_username(username, password)
            .initial_device_display_name(&self.config.device_name)
            .send()
            .await
            .map_err(|e| MatrixError::LoginFailed(e.to_string()))?;

        info!(
            user_id = %response.user_id,
            device_id = %response.device_id,
            "Matrix login successful"
        );

        self.user_id = Some(response.user_id.clone());
        self.config.user_id = response.user_id.to_string();
        self.config.access_token = Some(response.access_token.clone());
        self.config.device_id = Some(response.device_id.to_string());
        self.client = Some(client);
        self.state = ConnectionState::Connected;

        Ok(())
    }

    /// Restore a previous session from saved credentials.
    pub async fn restore_session(&mut self, store_path: &Path) -> Result<(), MatrixError> {
        if !self.config.has_session() {
            return Err(MatrixError::NotLoggedIn);
        }

        self.state = ConnectionState::Connecting;
        let client = self.build_client(store_path).await?;

        // The SDK's sqlite store handles session persistence automatically
        // after the initial login. We just need to verify the session is valid.
        let user_id = OwnedUserId::try_from(self.config.user_id.as_str())
            .map_err(|e| MatrixError::Config(format!("Invalid user_id: {e}")))?;

        self.user_id = Some(user_id);
        self.client = Some(client);
        self.state = ConnectionState::Connected;

        info!("Matrix session restored for {}", self.config.user_id);
        Ok(())
    }

    /// Start the background sync loop. Spawns a tokio task that processes
    /// incoming events and forwards them as [`ClankersEvent`]s.
    pub async fn start_sync(&mut self) -> Result<(), MatrixError> {
        let client = self.client.as_ref().ok_or(MatrixError::NotLoggedIn)?;
        self.state = ConnectionState::Syncing;

        let event_tx = self.event_tx.clone();
        let client_clone = client.clone();
        let user_id = self.user_id.clone();

        // Register the event handler for room messages
        client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, _room: Room| {
            let tx = event_tx.clone();
            let my_user_id = user_id.clone();
            async move {
                // Skip our own messages
                if my_user_id.as_ref().is_some_and(|id| *id == ev.sender) {
                    return;
                }

                let clankers_event = parse_room_message(&ev);
                if let Some(event) = clankers_event {
                    if tx.send(event).is_err() {
                        debug!("No subscribers for Matrix event");
                    }
                }
            }
        });

        // Spawn the sync loop
        tokio::spawn(async move {
            info!("Starting Matrix sync loop");
            let settings = SyncSettings::default();
            client_clone.sync(settings).await.unwrap_or_else(|e| {
                error!("Matrix sync failed: {e}");
            });
        });

        // Auto-join configured rooms
        for room_id_str in &self.config.auto_join_rooms {
            if let Ok(room_id) = RoomId::parse(room_id_str.as_str()) {
                match client.join_room_by_id(&room_id).await {
                    Ok(_) => info!("Auto-joined room {}", room_id),
                    Err(e) => warn!("Failed to auto-join {}: {}", room_id, e),
                }
            }
        }

        // Send initial announcement if configured
        if self.config.announce_on_join {
            let announce = Announce::new(&self.instance_name, self.config.user_id.as_str());
            for room_id_str in &self.config.auto_join_rooms {
                if let Ok(room_id) = RoomId::parse(room_id_str.as_str()) {
                    if let Err(e) = self.send_announce(&room_id, &announce).await {
                        warn!("Failed to announce in {}: {}", room_id, e);
                    }
                }
            }
        }

        Ok(())
    }

    // ── Message sending ────────────────────────────────────────────

    /// Send a text message to a room.
    pub async fn send_text(&self, room_id: &RoomId, text: &str) -> Result<(), MatrixError> {
        let client = self.client.as_ref().ok_or(MatrixError::NotLoggedIn)?;
        let room = client.get_room(room_id).ok_or_else(|| MatrixError::RoomNotFound(room_id.to_string()))?;

        let content = RoomMessageEventContent::text_plain(text);
        room.send(content).await.map_err(MatrixError::from)?;
        Ok(())
    }

    /// Send a clankers announcement to a room.
    pub async fn send_announce(&self, room_id: &RoomId, announce: &Announce) -> Result<(), MatrixError> {
        self.send_custom_event(room_id, EVENT_ANNOUNCE, announce).await
    }

    /// Send an RPC request to a room.
    pub async fn send_rpc_request(&self, room_id: &RoomId, request: &RpcRequest) -> Result<(), MatrixError> {
        self.send_custom_event(room_id, EVENT_RPC_REQUEST, request).await
    }

    /// Send an RPC response to a room.
    pub async fn send_rpc_response(&self, room_id: &RoomId, response: &RpcResponse) -> Result<(), MatrixError> {
        self.send_custom_event(room_id, EVENT_RPC_RESPONSE, response).await
    }

    /// Send a clankers chat message to a room.
    pub async fn send_chat(&self, room_id: &RoomId, body: &str) -> Result<(), MatrixError> {
        let user_id = self.config.user_id.clone();
        let msg = ChatMessage::new(body, &self.instance_name, &user_id);
        self.send_custom_event(room_id, EVENT_CHAT, &msg).await?;

        // Also send as regular text so human Matrix clients can see it
        self.send_text(room_id, &format!("[{}] {}", self.instance_name, body)).await?;
        Ok(())
    }

    /// Send a custom clankers event to a room.
    ///
    /// Falls back to sending as a formatted text message with a JSON code
    /// block, since custom event types require special room configuration.
    /// In practice we embed the clankers payload in the `m.text` body with a
    /// prefix tag so both humans and clankers instances can read it.
    async fn send_custom_event<T: serde::Serialize>(
        &self,
        room_id: &RoomId,
        event_type: &str,
        payload: &T,
    ) -> Result<(), MatrixError> {
        let json = serde_json::to_string(payload).map_err(|e| MatrixError::Serde(e.to_string()))?;

        // Encode as a text message with a machine-parseable prefix.
        // Format: `[clankers:<event_type>] <json>`
        // This ensures compatibility with all Matrix rooms without custom
        // event type registration.
        let text = format!("[clankers:{}] {}", event_type, json);
        self.send_text(room_id, &text).await
    }

    // ── Room queries ───────────────────────────────────────────────

    /// List joined rooms.
    pub fn joined_rooms(&self) -> Vec<RoomInfo> {
        let client = match &self.client {
            Some(c) => c,
            None => return Vec::new(),
        };

        client
            .joined_rooms()
            .into_iter()
            .map(|room| RoomInfo {
                room_id: room.room_id().to_string(),
                name: room.name().unwrap_or_default(),
                member_count: room.joined_members_count(),
                topic: room.topic().unwrap_or_default(),
            })
            .collect()
    }

    /// Join a room by ID or alias.
    pub async fn join_room(&self, room_id_or_alias: &str) -> Result<String, MatrixError> {
        let client = self.client.as_ref().ok_or(MatrixError::NotLoggedIn)?;

        // Try as room ID first
        if let Ok(room_id) = RoomId::parse(room_id_or_alias) {
            let room = client.join_room_by_id(&room_id).await?;
            return Ok(room.room_id().to_string());
        }

        // Try as room alias
        let response = client
            .join_room_by_id_or_alias(
                room_id_or_alias
                    .try_into()
                    .map_err(|e: ruma::IdParseError| MatrixError::Config(format!("Invalid room ID or alias: {e}")))?,
                &[],
            )
            .await?;

        Ok(response.room_id().to_string())
    }

    /// Leave a room.
    pub async fn leave_room(&self, room_id: &str) -> Result<(), MatrixError> {
        let client = self.client.as_ref().ok_or(MatrixError::NotLoggedIn)?;
        let room_id = RoomId::parse(room_id).map_err(|e| MatrixError::Config(format!("Invalid room ID: {e}")))?;
        let room = client.get_room(&room_id).ok_or_else(|| MatrixError::RoomNotFound(room_id.to_string()))?;
        room.leave().await?;
        Ok(())
    }

    /// Fetch recent message history from a room.
    ///
    /// Returns up to `limit` messages in reverse chronological order
    /// (newest first). Messages are decrypted if possible.
    pub async fn message_history(&self, room_id: &RoomId, limit: usize) -> Result<Vec<HistoryMessage>, MatrixError> {
        let client = self.client.as_ref().ok_or(MatrixError::NotLoggedIn)?;
        let room = client.get_room(room_id).ok_or_else(|| MatrixError::RoomNotFound(room_id.to_string()))?;

        let limit_uint = UInt::new(limit as u64).unwrap_or(UInt::new(20).unwrap());
        let mut options = MessagesOptions::backward();
        options.limit = limit_uint;

        let response = room
            .messages(options)
            .await
            .map_err(|e| MatrixError::Sdk(format!("Failed to fetch messages: {e}")))?;

        let mut messages = Vec::with_capacity(response.chunk.len());

        for timeline_event in &response.chunk {
            let raw: &Raw<AnySyncTimelineEvent> = timeline_event.raw();

            // Deserialize to get sender, timestamp, and body
            if let Ok(event) = raw.deserialize() {
                if let AnySyncTimelineEvent::MessageLike(msg_event) = event {
                    use matrix_sdk::ruma::events::AnySyncMessageLikeEvent;
                    if let AnySyncMessageLikeEvent::RoomMessage(
                        matrix_sdk::ruma::events::SyncMessageLikeEvent::Original(original),
                    ) = msg_event
                    {
                        let body = match &original.content.msgtype {
                            MessageType::Text(text) => text.body.clone(),
                            MessageType::Notice(notice) => notice.body.clone(),
                            MessageType::Emote(emote) => format!("* {}", emote.body),
                            _ => continue,
                        };

                        let sender = original.sender.to_string();
                        let timestamp = chrono::DateTime::from_timestamp(original.origin_server_ts.as_secs().into(), 0)
                            .unwrap_or_else(chrono::Utc::now);

                        // Classify: is this a clankers protocol message or regular?
                        let msg_type = if body.starts_with("[clankers:") {
                            HistoryMessageType::Clankers
                        } else {
                            HistoryMessageType::Text
                        };

                        messages.push(HistoryMessage {
                            sender,
                            body,
                            timestamp,
                            msg_type,
                        });
                    }
                }
            }
        }

        Ok(messages)
    }

    /// Get the current config (for saving after login).
    pub fn config(&self) -> &MatrixConfig {
        &self.config
    }

    /// Mutable access to config.
    pub fn config_mut(&mut self) -> &mut MatrixConfig {
        &mut self.config
    }
}

// ── Event parsing ──────────────────────────────────────────────────

/// Parse a Matrix room message into a ClankersEvent.
fn parse_room_message(ev: &OriginalSyncRoomMessageEvent) -> Option<ClankersEvent> {
    let body = match &ev.content.msgtype {
        MessageType::Text(text) => &text.body,
        _ => return None,
    };

    let sender = ev.sender.to_string();
    let timestamp =
        chrono::DateTime::from_timestamp(ev.origin_server_ts.as_secs().into(), 0).unwrap_or_else(|| chrono::Utc::now());

    // Try to parse as a clankers-tagged message: [clankers:<type>] <json>
    if let Some(rest) = body.strip_prefix("[clankers:") {
        if let Some(bracket_end) = rest.find(']') {
            let event_type = &rest[..bracket_end];
            let json_str = rest[bracket_end + 1..].trim();

            return match event_type {
                EVENT_ANNOUNCE => serde_json::from_str::<Announce>(json_str).ok().map(ClankersEvent::Announce),
                EVENT_RPC_REQUEST => serde_json::from_str::<RpcRequest>(json_str).ok().map(ClankersEvent::RpcRequest),
                EVENT_RPC_RESPONSE => serde_json::from_str::<RpcResponse>(json_str).ok().map(ClankersEvent::RpcResponse),
                EVENT_CHAT => serde_json::from_str::<ChatMessage>(json_str).ok().map(ClankersEvent::Chat),
                _ => {
                    debug!("Unknown clankers event type: {}", event_type);
                    None
                }
            };
        }
    }

    // Regular text message from a human or non-clankers client
    Some(ClankersEvent::Text {
        sender,
        body: body.clone(),
        timestamp,
    })
}

/// Summary info about a joined room.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoomInfo {
    pub room_id: String,
    pub name: String,
    pub member_count: u64,
    pub topic: String,
}

/// A message from room history.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryMessage {
    /// Matrix user ID of the sender.
    pub sender: String,
    /// Message body text.
    pub body: String,
    /// When the message was sent.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether this is a regular text message or a clankers protocol message.
    pub msg_type: HistoryMessageType,
}

/// Classification of a history message.
#[derive(Debug, Clone, serde::Serialize)]
pub enum HistoryMessageType {
    /// Regular text from a human or bot.
    Text,
    /// A clankers protocol message (announce, RPC, chat).
    Clankers,
}
