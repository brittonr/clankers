//! Process monitoring display type reexports.
//!
//! The neutral process observation contracts live in `clanker-message`; the TUI
//! crate reexports them so display-edge callers keep the same import path.

pub use clanker_message::ProcessDataSource;
pub use clanker_message::ProcessDisplayState;
pub use clanker_message::ProcessSnapshot;
