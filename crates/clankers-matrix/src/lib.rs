//! # clankers-matrix
//!
//! Matrix protocol bridge for clankers — enables communication between clankers
//! instances over Matrix rooms.
//!
//! ## Architecture
//!
//! Each clankers instance connects to a Matrix homeserver as a regular user.
//! Instances join a shared room and exchange structured JSON messages
//! using a custom `m.clankers.*` message type namespace:
//!
//! - `m.clankers.announce` — Periodic capability advertisement
//! - `m.clankers.rpc.request` — JSON-RPC request (prompt, ping, status, etc.)
//! - `m.clankers.rpc.response` — JSON-RPC response
//! - `m.clankers.chat` — Free-form text messages between agents
//!
//! Regular Matrix text messages (`m.text`) in the room are also visible
//! to the agent via the `matrix_read` tool, allowing human-to-agent
//! interaction through any Matrix client.
//!
//! ## Modules
//!
//! - [`client`] — Matrix SDK wrapper and session management
//! - [`config`] — Credentials and homeserver configuration
//! - [`protocol`] — Wire format for clankers-to-clankers messages
//! - [`bridge`] — Translates between Matrix events and clankers RPC
//! - [`room`] — Room management (join, create, invite)

pub mod bridge;
pub mod client;
pub mod config;
pub mod protocol;
pub mod room;

pub use client::MatrixClient;
pub use config::MatrixConfig;
// Re-export ruma for consumers that need room ID types etc.
pub use matrix_sdk::ruma;
