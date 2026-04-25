//! Peer data abstractions for the TUI.

/// A peer's info as seen by the TUI.
#[derive(Debug, Clone)]
pub struct PeerInfoView {
    pub name: String,
    pub node_id: String,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub added: chrono::DateTime<chrono::Utc>,
    pub capabilities: PeerCapabilitiesView,
}

/// Peer capabilities as seen by the TUI.
#[derive(Debug, Clone, Default)]
pub struct PeerCapabilitiesView {
    pub accepts_prompts: bool,
    pub agents: Vec<String>,
    pub tools: Vec<String>,
    pub tags: Vec<String>,
    pub version: Option<String>,
}
