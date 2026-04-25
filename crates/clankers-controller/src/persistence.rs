//! Session persistence — write messages to the session on turn boundaries.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
            if let Some(search_index) = &self.search_index {
                index_messages_for_search(search_index, sm.session_id(), messages);
            }
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
        let parent = sm.active_leaf_id().cloned();
        if let Err(e) = sm.append_message(msg.clone(), parent) {
            warn!("failed to persist message: {e}");
        }
    }
}

/// Index persisted messages into the full-text search index.
pub(crate) fn index_messages_for_search(
    search_index: &clankers_db::search_index::SearchIndex,
    session_id: &str,
    messages: &[clankers_provider::message::AgentMessage],
) {
    let mut batch: Vec<(&str, String, &str, String, i64)> = Vec::new();

    for msg in messages {
        let id = msg.id().to_string();
        let role = msg.role();
        let timestamp = msg.timestamp().timestamp();

        let text = match msg {
            clanker_message::AgentMessage::User(m) => extract_text(&m.content),
            clanker_message::AgentMessage::Assistant(m) => extract_text(&m.content),
            clanker_message::AgentMessage::ToolResult(m) => extract_text(&m.content),
            clanker_message::AgentMessage::BashExecution(m) => {
                format!("{} {} {}", m.command, m.stdout, m.stderr)
            }
            _ => continue,
        };

        if !text.trim().is_empty() {
            batch.push((session_id, id, role, text, timestamp));
        }
    }

    if batch.is_empty() {
        return;
    }

    let refs: Vec<(&str, &str, &str, &str, i64)> = batch
        .iter()
        .map(|(sid, id, role, text, ts)| (*sid, id.as_str(), *role, text.as_str(), *ts))
        .collect();

    if let Err(e) = search_index.index_messages_batch(&refs) {
        warn!("failed to index messages for search: {e}");
    }
}

fn extract_text(content: &[clanker_message::Content]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            clanker_message::Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
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
