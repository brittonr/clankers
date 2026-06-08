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
            if let Some(service) = &self.persistence_service {
                service.index_messages(sm.session_id(), messages);
            }
            return;
        }

        if let AgentEvent::SessionCompactionSummary { summary } = event {
            if let Err(error) = sm.record_compaction_summary(summary.clone()) {
                warn!("failed to persist compaction summary: {error}");
            }
            if let Some(service) = &self.persistence_service {
                service.store_compaction_summary_tool_result(sm.session_id(), summary);
            }
        }
    }
}

/// Persist a batch of agent messages to the session manager.
fn persist_messages(sm: &mut SessionManager, messages: &[clanker_message::transcript::AgentMessage]) {
    for msg in messages {
        let parent = sm.active_leaf_id().cloned();
        if let Err(e) = sm.append_message(msg.clone(), parent) {
            warn!("failed to persist message: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use chrono::Utc;
    use clanker_message::Content;
    use clanker_message::transcript::AgentMessage;
    use clanker_message::transcript::MessageId;
    use clanker_message::transcript::UserMessage;
    use tempfile::TempDir;

    use super::*;
    use crate::config::ControllerConfig;
    use crate::test_helpers::MockProvider;
    use crate::test_helpers::model_service;

    #[derive(Default)]
    struct RecordingPersistenceService {
        summaries: Mutex<Vec<(String, String)>>,
        indexed: Mutex<Vec<(String, usize)>>,
    }

    impl crate::ControllerPersistenceService for RecordingPersistenceService {
        fn index_messages(&self, session_id: &str, messages: &[clanker_message::transcript::AgentMessage]) {
            self.indexed
                .lock()
                .expect("indexed lock")
                .push((session_id.to_string(), messages.len()));
        }

        fn store_compaction_summary_tool_result(&self, session_id: &str, summary: &str) {
            self.summaries
                .lock()
                .expect("summaries lock")
                .push((session_id.to_string(), summary.to_string()));
        }
    }

    fn make_controller_with_persistence() -> (SessionController, TempDir, Arc<RecordingPersistenceService>) {
        let tmp = TempDir::new().expect("tempdir should exist");
        let cwd = tmp.path().to_string_lossy().to_string();
        let session_manager =
            clankers_session::SessionManager::create(tmp.path(), &cwd, "test-model", None, None, None)
                .expect("session manager should create");
        let persistence_service = Arc::new(RecordingPersistenceService::default());
        let agent = clankers_agent::Agent::new_with_agent_settings(
            model_service(Arc::new(MockProvider)),
            vec![],
            clankers_agent::AgentSettings::default(),
            "test-model".to_string(),
            "test system prompt".to_string(),
        );
        let controller = SessionController::new(agent, ControllerConfig {
            session_id: session_manager.session_id().to_string(),
            model: "test-model".to_string(),
            session_manager: Some(session_manager),
            persistence_service: Some(persistence_service.clone()),
            ..Default::default()
        });
        (controller, tmp, persistence_service)
    }

    #[test]
    fn persist_event_indexes_agent_end_messages_through_service() {
        let (mut controller, _tmp, persistence_service) = make_controller_with_persistence();
        let message = AgentMessage::User(UserMessage {
            id: MessageId::new("u1"),
            content: vec![Content::Text {
                text: "hello from the user".to_string(),
            }],
            timestamp: Utc::now(),
        });

        controller.persist_event(&AgentEvent::AgentEnd { messages: vec![message] });

        let indexed = persistence_service.indexed.lock().expect("indexed lock");
        assert_eq!(indexed.as_slice(), &[(controller.session_id().to_string(), 1)]);
    }

    #[test]
    fn persist_event_stores_compaction_summary_in_session_and_service() {
        let (mut controller, _tmp, persistence_service) = make_controller_with_persistence();
        let summary = "## Active Task\n- continue".to_string();

        controller.persist_event(&AgentEvent::SessionCompactionSummary {
            summary: summary.clone(),
        });

        let session_summary = controller.session_manager().expect("session manager").latest_compaction_summary();
        assert_eq!(session_summary, Some(summary.as_str()));
        let recorded = persistence_service.summaries.lock().expect("summaries lock");
        assert_eq!(recorded.as_slice(), &[(controller.session_id().to_string(), summary)]);
    }
}
