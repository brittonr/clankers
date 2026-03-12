//! Session persistence — write messages to the session on turn boundaries.

use clankers_agent::events::AgentEvent;
use clankers_session::SessionManager;
use tracing::warn;

use crate::SessionController;

impl SessionController {
    /// Persist agent messages on AgentEnd events.
    ///
    /// Called from `process_agent_event` for each event. Only AgentEnd
    /// carries the messages that need persisting.
    pub(crate) fn persist_event(&mut self, event: &AgentEvent) {
        let Some(ref mut sm) = self.session_manager else {
            return;
        };

        if let AgentEvent::AgentEnd { messages } = event {
            persist_messages(sm, messages);
        }
    }
}

/// Persist a batch of agent messages to the session manager.
fn persist_messages(sm: &mut SessionManager, messages: &[clankers_provider::message::AgentMessage]) {
    for msg in messages {
        // Each message appended with the session manager's current active
        // leaf as parent. The session manager tracks the head internally.
        let parent = sm.active_leaf_id().cloned();
        if let Err(e) = sm.append_message(msg.clone(), parent) {
            warn!("failed to persist message: {e}");
        }
    }
}
