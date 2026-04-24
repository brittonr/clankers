//! Session persistence — write messages to the session on turn boundaries.

use clankers_agent::events::AgentEvent;
use clankers_db::tool_results::StoredToolResult;
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
            return;
        }

        if let AgentEvent::SessionCompactionSummary { summary } = event {
            if let Err(error) = sm.record_compaction_summary(summary.clone()) {
                warn!("failed to persist compaction summary: {error}");
            }
            if let Some(db) = self.agent.as_ref().and_then(|agent| agent.db())
                && let Err(error) = persist_compaction_summary_tool_result(db, sm.session_id(), summary)
            {
                warn!("failed to persist compaction summary tool result: {error}");
            }
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

pub(crate) fn persist_compaction_summary_tool_result(
    db: &clankers_db::Db,
    session_id: &str,
    summary: &str,
) -> clankers_db::error::Result<()> {
    let line_count = summary.lines().count();
    let byte_count = summary.len();
    let entry = StoredToolResult {
        session_id: session_id.to_string(),
        call_id: "compaction-summary".to_string(),
        tool_name: "compaction-summary".to_string(),
        content_text: summary.to_string(),
        has_image: false,
        is_error: false,
        byte_count,
        line_count,
    };
    db.tool_results().store(&entry)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::config::ControllerConfig;
    use crate::test_helpers::MockProvider;

    fn make_controller_with_persistence() -> (SessionController, TempDir, clankers_db::Db) {
        let tmp = TempDir::new().expect("tempdir should exist");
        let db = clankers_db::Db::in_memory().expect("db should exist");
        let cwd = tmp.path().to_string_lossy().to_string();
        let session_manager =
            clankers_session::SessionManager::create(tmp.path(), &cwd, "test-model", None, None, None)
                .expect("session manager should create");
        let agent = clankers_agent::Agent::new(
            Arc::new(MockProvider),
            vec![],
            clankers_config::settings::Settings::default(),
            "test-model".to_string(),
            "test system prompt".to_string(),
        )
        .with_db(db.clone());
        let controller = SessionController::new(agent, ControllerConfig {
            session_id: session_manager.session_id().to_string(),
            model: "test-model".to_string(),
            session_manager: Some(session_manager),
            ..Default::default()
        });
        (controller, tmp, db)
    }

    #[test]
    fn persist_event_stores_compaction_summary_in_session_and_db() {
        let (mut controller, _tmp, db) = make_controller_with_persistence();
        let summary = "## Active Task\n- continue".to_string();

        controller.persist_event(&AgentEvent::SessionCompactionSummary {
            summary: summary.clone(),
        });

        let session_summary = controller.session_manager().expect("session manager").latest_compaction_summary();
        assert_eq!(session_summary, Some(summary.as_str()));

        let db_entry = db
            .tool_results()
            .get(controller.session_id(), "compaction-summary")
            .expect("db lookup should work")
            .expect("db entry should exist");
        assert_eq!(db_entry.tool_name, "compaction-summary");
        assert_eq!(db_entry.content_text, summary);
    }
}
