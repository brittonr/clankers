//! Peer registry — persistent store of known clankers peers
//!
//! Stored at `~/.clankers/agent/peers.json`. Each peer has:
//! - A public key (EndpointId)
//! - An optional human-readable name
//! - Capabilities (what tools/agents it has, whether it accepts prompts)
//! - Last-seen timestamp
//! - Online/offline status (probed on demand)

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

/// A known peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Human-readable name for this peer
    pub name: String,
    /// Hex-encoded public key
    pub node_id: String,
    /// What this peer can do
    #[serde(default)]
    pub capabilities: PeerCapabilities,
    /// Last time we successfully contacted this peer
    #[serde(default)]
    pub last_seen: Option<DateTime<Utc>>,
    /// When this peer was added
    pub added: DateTime<Utc>,
}

/// What a peer advertises it can do
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerCapabilities {
    /// Accepts remote prompt execution
    #[serde(default)]
    pub accepts_prompts: bool,
    /// Available agent definitions
    #[serde(default)]
    pub agents: Vec<String>,
    /// Available tools
    #[serde(default)]
    pub tools: Vec<String>,
    /// Free-form tags for routing (e.g. "gpu", "code-review", "testing")
    #[serde(default)]
    pub tags: Vec<String>,
    /// clankers version
    #[serde(default)]
    pub version: Option<String>,
}

/// The peer registry
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerRegistry {
    /// Peers keyed by node_id (hex public key)
    pub peers: HashMap<String, PeerInfo>,
}

impl PeerRegistry {
    /// Load from disk, or return empty registry
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Save to disk
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Add or update a peer
    pub fn add(&mut self, node_id: &str, name: &str) {
        let entry = self.peers.entry(node_id.to_string()).or_insert_with(|| PeerInfo {
            name: name.to_string(),
            node_id: node_id.to_string(),
            capabilities: PeerCapabilities::default(),
            last_seen: None,
            added: Utc::now(),
        });
        entry.name = name.to_string();
    }

    /// Remove a peer
    pub fn remove(&mut self, node_id: &str) -> bool {
        self.peers.remove(node_id).is_some()
    }

    /// Update capabilities for a peer (e.g. after a status probe)
    pub fn update_capabilities(&mut self, node_id: &str, caps: PeerCapabilities) {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.capabilities = caps;
            peer.last_seen = Some(Utc::now());
        }
    }

    /// Mark a peer as seen now
    pub fn touch(&mut self, node_id: &str) {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.last_seen = Some(Utc::now());
        }
    }

    /// Find peers that match a tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<&PeerInfo> {
        self.peers.values().filter(|p| p.capabilities.tags.iter().any(|t| t == tag)).collect()
    }

    /// Find peers that have a specific agent definition
    pub fn find_by_agent(&self, agent_name: &str) -> Vec<&PeerInfo> {
        self.peers.values().filter(|p| p.capabilities.agents.iter().any(|a| a == agent_name)).collect()
    }

    /// Get all peers sorted by last_seen (most recent first)
    pub fn list(&self) -> Vec<&PeerInfo> {
        let mut peers: Vec<_> = self.peers.values().collect();
        peers.sort_by_key(|p| Reverse(p.last_seen));
        peers
    }
}

/// Default path for the peer registry
pub fn registry_path(paths: &crate::config::ClankersPaths) -> PathBuf {
    paths.global_config_dir.join("peers.json")
}
