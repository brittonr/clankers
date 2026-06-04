//! Runtime compatibility reexports for the green session ledger core.
//!
//! Pure ledger DTOs and replay logic live in `clankers-engine-host`; this
//! module preserves the runtime facade names while projecting neutral ledger
//! errors into `RuntimeError` at the app-edge.

use clankers_engine::EngineMessage;
pub use clankers_engine_host::SessionLedgerError;
pub use clankers_engine_host::SessionLedgerReplay;
pub use clankers_engine_host::SessionLedgerReplayMetadata;
pub use clankers_engine_host::SessionLedgerRole;
pub use clankers_engine_host::SessionLedgerUnsupported;

use crate::EventMetadata;
use crate::PromptId;
use crate::RuntimeError;
use crate::SessionId;
pub type SessionLedgerEntry = clankers_engine_host::SessionLedgerEntry<PromptId, EventMetadata>;
pub type SessionLedgerMessage = clankers_engine_host::SessionLedgerMessage<PromptId>;
pub type SessionLedgerReceipt = clankers_engine_host::SessionLedgerReceipt<PromptId, EventMetadata>;
pub type SessionLedgerRecord = clankers_engine_host::SessionLedgerRecord<SessionId, PromptId, EventMetadata>;
pub type SessionLedgerSummary = clankers_engine_host::SessionLedgerSummary<PromptId>;
pub type SessionLedgerUsage = clankers_engine_host::SessionLedgerUsage<PromptId>;

pub fn replay_ledger_entries(entries: &[SessionLedgerEntry]) -> Result<SessionLedgerReplay, RuntimeError> {
    clankers_engine_host::replay_ledger_entries(entries)
        .map_err(|error| RuntimeError::SessionUnsupported(error.safe_message()))
}

#[must_use]
pub fn ledger_messages_from_engine_messages(messages: &[EngineMessage]) -> Vec<SessionLedgerMessage> {
    clankers_engine_host::ledger_messages_from_engine_messages(messages)
}

#[must_use]
pub fn ledger_entries_from_engine_messages(messages: &[EngineMessage]) -> Vec<SessionLedgerEntry> {
    clankers_engine_host::ledger_entries_from_engine_messages(messages)
}
