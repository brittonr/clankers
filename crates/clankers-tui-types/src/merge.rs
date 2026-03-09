//! Types for the interactive merge view.

/// A message entry as seen by the merge interactive view.
///
/// This is a simplified view of `MessageEntry` + `AgentMessage` — just the
/// fields needed for the merge overlay's toggle-list rendering.
#[derive(Debug, Clone)]
pub struct MergeMessageView {
    /// Unique message identifier (string form).
    pub id: String,
    /// Human-readable preview of the message content.
    pub preview: String,
    /// Short label for the message variant (e.g. "User", "Assistant", "Tool").
    pub variant_label: &'static str,
}
