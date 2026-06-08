//! Controller-owned persistence side-effect port.
//!
//! Session ledger mutation remains handled by `ControllerSessionLedger`; optional
//! search indexing and companion tool-result storage are host-injected side effects.

use clanker_message::transcript::AgentMessage;

pub trait ControllerPersistenceService: Send + Sync {
    fn index_messages(&self, session_id: &str, messages: &[AgentMessage]);

    fn store_compaction_summary_tool_result(&self, session_id: &str, summary: &str);
}
