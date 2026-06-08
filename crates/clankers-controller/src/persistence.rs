//! Session persistence — write messages to the session ledger on turn boundaries.

use clanker_message::transcript::AgentMessage;
use clankers_agent::events::AgentEvent;
use tracing::warn;

use crate::ControllerSessionLedger;
use crate::SessionController;

impl SessionController {
    /// Persist agent messages on AgentEnd events.
    ///
    /// Called from `process_agent_event` for each event. Only AgentEnd
    /// carries the messages that need persisting.
    pub(crate) fn persist_event(&mut self, event: &AgentEvent) {
        let Some(ref mut ledger) = self.session_ledger else {
            return;
        };

        if let AgentEvent::AgentEnd { messages } = event {
            let session_id = ledger.session_id().to_string();
            persist_messages(ledger.as_mut(), messages);
            if let Some(service) = &self.persistence_service {
                service.index_messages(&session_id, messages);
            }
            return;
        }

        if let AgentEvent::SessionCompactionSummary { summary } = event {
            let session_id = ledger.session_id().to_string();
            if let Err(error) = ledger.record_compaction_summary(summary.clone()) {
                warn!("failed to persist compaction summary: {error}");
            }
            if let Some(service) = &self.persistence_service {
                service.store_compaction_summary_tool_result(&session_id, summary);
            }
        }
    }

    pub(crate) fn flush_agent_messages_on_shutdown(&mut self) -> usize {
        let Some(ref mut agent) = self.agent else {
            return 0;
        };
        let Some(ref mut ledger) = self.session_ledger else {
            return 0;
        };
        flush_unpersisted_messages(ledger.as_mut(), &agent.messages().to_vec())
    }
}

/// Persist a batch of agent messages to the session ledger.
fn persist_messages(ledger: &mut dyn ControllerSessionLedger, messages: &[AgentMessage]) {
    for msg in messages {
        if let Err(error) = ledger.append_message_to_active_leaf(msg.clone()) {
            warn!("failed to persist message: {error}");
        }
    }
}

fn flush_unpersisted_messages(ledger: &mut dyn ControllerSessionLedger, messages: &[AgentMessage]) -> usize {
    let mut flushed = 0;
    for msg in messages {
        if ledger.is_persisted(msg.id()) {
            continue;
        }
        if let Err(error) = ledger.append_message_to_active_leaf(msg.clone()) {
            warn!("shutdown flush failed: {error}");
        } else {
            flushed += 1;
        }
    }
    flushed
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::Mutex;

    use chrono::Utc;
    use clanker_message::Content;
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
        fn index_messages(&self, session_id: &str, messages: &[AgentMessage]) {
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

    #[derive(Default)]
    struct RecordingSessionLedger {
        session_id: String,
        messages: Vec<MessageId>,
        persisted: HashSet<MessageId>,
        summaries: Vec<String>,
    }

    impl RecordingSessionLedger {
        fn new(session_id: &str) -> Self {
            Self {
                session_id: session_id.to_string(),
                ..Self::default()
            }
        }
    }

    impl crate::ControllerSessionLedger for RecordingSessionLedger {
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn session_id(&self) -> &str {
            &self.session_id
        }

        fn is_persisted(&self, id: &MessageId) -> bool {
            self.persisted.contains(id)
        }

        fn append_message_to_active_leaf(&mut self, message: AgentMessage) -> Result<(), String> {
            let id = message.id().clone();
            self.persisted.insert(id.clone());
            self.messages.push(id);
            Ok(())
        }

        fn record_compaction_summary(&mut self, summary: String) -> Result<(), String> {
            self.summaries.push(summary);
            Ok(())
        }
    }

    fn make_controller_with_persistence() -> (SessionController, TempDir, Arc<RecordingPersistenceService>) {
        let tmp = TempDir::new().expect("tempdir should exist");
        let session_id = "session".to_string();
        let persistence_service = Arc::new(RecordingPersistenceService::default());
        let agent = clankers_agent::Agent::new_with_agent_settings(
            model_service(Arc::new(MockProvider)),
            vec![],
            clankers_agent::AgentSettings::default(),
            "test-model".to_string(),
            "test system prompt".to_string(),
        );
        let controller = SessionController::new(agent, ControllerConfig {
            session_id: session_id.clone(),
            model: "test-model".to_string(),
            session_ledger: Some(Box::new(RecordingSessionLedger::new(&session_id))),
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

        let recorded = persistence_service.summaries.lock().expect("summaries lock");
        assert_eq!(recorded.as_slice(), &[(controller.session_id().to_string(), summary)]);
    }
}
