//! Iroh QUIC RPC for clankers-router daemon communication
//!
//! The router runs as a long-lived daemon that holds credentials and talks
//! to LLM APIs. Clients (clankers) connect over iroh QUIC to send completion
//! requests and receive streaming responses.
//!
//! ## Discovery
//!
//! The daemon writes its node ID and PID to `~/.config/clankers-router/daemon.json`.
//! Clients read this file to find the daemon. mDNS is also enabled for
//! automatic LAN discovery.
//!
//! ## Wire protocol
//!
//! ALPN: `b"clankers/router/1"`
//!
//! Each bidirectional QUIC stream carries one request/response exchange.
//! Frames are length-prefixed JSON: `[4-byte BE length][JSON payload]`.
//!
//! Simple methods (models.list, status):
//!   Client → Server: Request frame
//!   Server → Client: Response frame
//!
//! Streaming methods (complete):
//!   Client → Server: Request frame
//!   Server → Client: N × Notification frames (StreamEvents)
//!   Server → Client: 1 × Response frame (final result)

pub mod client;
pub mod daemon;
pub mod protocol;
pub mod server;

pub const ALPN: &[u8] = b"clankers/router/1";
pub const MDNS_SERVICE: &str = "_clankers-router._udp.local.";
