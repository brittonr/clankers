//! Room management — create, invite, and configure clankers collaboration rooms.

use serde::Deserialize;
use serde::Serialize;

use crate::client::MatrixClient;
use crate::client::MatrixError;

/// Create a new Matrix room for clankers collaboration.
///
/// Sets up a private room with E2E encryption enabled and a descriptive
/// topic indicating it's used for clankers inter-agent communication.
#[allow(clippy::unused_async)] // Will use await once room creation is implemented.
pub async fn create_clankers_room(
    _client: &MatrixClient,
    _name: &str,
    _topic: Option<&str>,
) -> Result<String, MatrixError> {
    // For now, delegate to the raw client SDK via send_text to a
    // known room. Full room creation requires the matrix-sdk room
    // creation API which we'll wire up once the base client is solid.
    //
    // TODO: Use client.create_room() with proper settings:
    // - visibility: private
    // - preset: private_chat
    // - name: provided name
    // - topic: "clankers collaboration room" or custom
    // - initial_state: enable encryption
    Err(MatrixError::Config(
        "Room creation not yet implemented — join an existing room with `matrix_join`".to_string(),
    ))
}

/// Information about a room and its clankers participants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClankersRoom {
    /// Matrix room ID
    pub room_id: String,
    /// Room display name
    pub name: String,
    /// Room topic
    pub topic: String,
    /// Total member count
    pub member_count: u64,
    /// Known clankers instances in this room
    pub clankers_members: Vec<ClankersMember>,
}

/// A clankers instance that's a member of a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClankersMember {
    /// Matrix user ID
    pub user_id: String,
    /// Instance name (from announce)
    pub instance_name: String,
    /// Whether currently online
    pub online: bool,
    /// Capability tags
    pub tags: Vec<String>,
}
