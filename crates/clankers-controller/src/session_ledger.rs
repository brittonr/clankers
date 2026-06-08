//! Controller-owned session ledger port.
//!
//! Concrete session formats stay at the host edge. The controller only needs a
//! tiny ledger contract for appending transcript messages and recording
//! compaction summaries.

use std::any::Any;

use clanker_message::transcript::AgentMessage;
use clanker_message::transcript::MessageId;

pub trait ControllerSessionLedger: Send {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn session_id(&self) -> &str;

    fn is_persisted(&self, id: &MessageId) -> bool;

    fn append_message_to_active_leaf(&mut self, message: AgentMessage) -> Result<(), String>;

    fn record_compaction_summary(&mut self, summary: String) -> Result<(), String>;
}
