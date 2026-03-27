//! Iroh P2P communication
//!
//! Peer-to-peer agent communication over iroh QUIC.
//! ALPN: b"clankers/rpc/1"
//!
//! ## Wire protocol
//!
//! Each bidirectional QUIC stream carries one request/response exchange.
//! All frames are length-prefixed JSON: `[4-byte big-endian length][JSON payload]`.
//!
//! Request:  `{ "method": "ping", "params": { ... } }`
//! Response: `{ "ok": <value> }` or `{ "error": "message" }`
//!
//! For streaming methods (prompt), intermediate notification frames
//! (no `ok`/`error` key) are sent before the final response.
//!
//! For file transfer, raw bytes follow the framed request/response.
//!
//! ## Auth
//!
//! The server maintains an allowlist of peer public keys. Connections from
//! unknown peers are rejected at the stream level. Use `--allow-all` to
//! disable the check, or `clankers rpc allow <node-id>` to add peers.
//!
//! ## Discovery
//!
//! The endpoint is configured with mDNS (LAN auto-discovery) and DNS pkarr
//! (WAN discovery via relay servers). Peers on the same LAN can find each
//! other without manual `peers add`. Use `clankers rpc discover --mdns` to scan
//! the local network for clankers instances.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use iroh::Endpoint;
use iroh::PublicKey;
use iroh::SecretKey;

use crate::provider::Provider;
use crate::tools::Tool;

pub mod client;
pub mod protocol;
pub mod server;

// Re-export public API
pub use client::discover_mdns_peers;
pub use client::recv_file;
pub use client::run_heartbeat;
pub use client::send_file;
pub use client::send_rpc;
pub use client::send_rpc_streaming;
pub use protocol::read_frame;
pub use protocol::write_frame;
pub use server::handle_prompt_streaming_pub;
pub use server::serve_rpc;

pub const ALPN: &[u8] = b"clankers/rpc/1";

/// mDNS service name for clankers auto-discovery on LAN
const MDNS_SERVICE_NAME: &str = "_clankers._udp.local.";

// ── Server types ────────────────────────────────────────────────────────────

/// Metadata about this node, always available.
pub struct NodeMeta {
    pub tags: Vec<String>,
    pub agent_names: Vec<String>,
}

/// Context for handling RPC requests that need agent capabilities.
pub struct RpcContext {
    pub provider: Arc<dyn Provider>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub settings: crate::config::settings::Settings,
    pub model: String,
    pub system_prompt: String,
}

/// Combined server state.
pub struct ServerState {
    pub meta: NodeMeta,
    pub agent: Option<RpcContext>,
    pub acl: AccessControl,
    /// Directory where received files are stored
    pub receive_dir: Option<PathBuf>,
}

/// Access control for incoming connections.
pub struct AccessControl {
    /// If true, accept all peers (no allowlist check).
    pub allow_all: bool,
    /// Set of hex-encoded public keys that are allowed to connect.
    pub allowed: HashSet<String>,
}

impl AccessControl {
    pub fn open() -> Self {
        Self {
            allow_all: true,
            allowed: HashSet::new(),
        }
    }

    pub fn from_allowlist(allowed: HashSet<String>) -> Self {
        Self {
            allow_all: false,
            allowed,
        }
    }

    pub fn is_allowed(&self, peer: &PublicKey) -> bool {
        self.allow_all || self.allowed.contains(&peer.to_string())
    }
}

// ── Allowlist persistence ───────────────────────────────────────────────────

pub fn allowlist_path(paths: &crate::config::ClankersPaths) -> PathBuf {
    paths.global_config_dir.join("allowed_peers.json")
}

/// Load the allowlist from disk.
pub fn load_allowlist(path: &Path) -> HashSet<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

/// Save the allowlist to disk.
pub fn save_allowlist(path: &Path, allowed: &HashSet<String>) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let list: Vec<&String> = {
        let mut v: Vec<_> = allowed.iter().collect();
        v.sort();
        v
    };
    let json = serde_json::to_string_pretty(&list).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

// ── Identity ────────────────────────────────────────────────────────────────

/// Persistent identity for this node.
pub struct Identity {
    pub secret_key: SecretKey,
    pub path: PathBuf,
}

impl Identity {
    pub fn load_or_generate(path: &Path) -> Self {
        let secret_key = if path.exists() {
            let bytes = std::fs::read(path).unwrap_or_default();
            if bytes.len() == 32 {
                let mut key_bytes = [0u8; 32];
                key_bytes.copy_from_slice(&bytes);
                SecretKey::from_bytes(&key_bytes)
            } else {
                let key = SecretKey::generate(&mut rand::rng());
                std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).ok();
                std::fs::write(path, key.to_bytes()).ok();
                key
            }
        } else {
            let key = SecretKey::generate(&mut rand::rng());
            std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).ok();
            std::fs::write(path, key.to_bytes()).ok();
            key
        };
        Self {
            secret_key,
            path: path.to_path_buf(),
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.secret_key.public()
    }
}

pub fn identity_path(paths: &crate::config::ClankersPaths) -> PathBuf {
    paths.global_config_dir.join("identity.key")
}

// ── Endpoint (with mDNS + DNS discovery) ────────────────────────────────────

/// Start an iroh endpoint with mDNS (LAN) and default DNS discovery.
///
/// The endpoint uses a shared QUIC socket that can both accept incoming
/// connections (server) and initiate outgoing connections (client), enabling
/// bidirectional communication through a single endpoint.
pub async fn start_endpoint(identity: &Identity) -> Result<Endpoint, crate::error::Error> {
    let no_mdns = std::env::var("CLANKERS_NO_MDNS").unwrap_or_default() == "1";

    // Default builder includes DNS pkarr discovery for WAN.
    // Only add mDNS (LAN auto-discovery) if not disabled.
    let mut builder = Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![ALPN.to_vec()]);

    if !no_mdns {
        let mdns = iroh::address_lookup::MdnsAddressLookup::builder().service_name(MDNS_SERVICE_NAME);
        builder = builder.address_lookup(mdns);
    }

    let endpoint = builder
        .bind()
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to bind iroh endpoint: {}", e),
        })?;
    Ok(endpoint)
}

/// Start an endpoint without mDNS (for tests or minimal usage).
pub async fn start_endpoint_no_mdns(identity: &Identity) -> Result<Endpoint, crate::error::Error> {
    let endpoint = Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .clear_address_lookup()
        .bind()
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to bind iroh endpoint: {}", e),
        })?;
    Ok(endpoint)
}
