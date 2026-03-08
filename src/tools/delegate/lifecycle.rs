//! Worker lifecycle management and state tracking

/// State tracking for an active worker
pub struct WorkerState {
    pub(super) _cwd: String,
    pub(super) _agent: Option<String>,
    /// If delegated to a remote peer, its node_id
    pub(super) _remote_peer: Option<String>,
}

impl WorkerState {
    pub fn new(cwd: String, agent: Option<String>, remote_peer: Option<String>) -> Self {
        Self {
            _cwd: cwd,
            _agent: agent,
            _remote_peer: remote_peer,
        }
    }
}
