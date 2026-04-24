//! Structured progress and result streaming for tools
//!
//! Progress types re-exported from `clanker-tui-types`.
//! Result streaming types re-exported from `clankers-message`.

// ProgressKind and ToolProgress — canonical definitions in clanker-tui-types.
pub use clanker_tui_types::ProgressKind;
pub use clanker_tui_types::ToolProgress;

// ResultChunk, TruncationConfig, ToolResultAccumulator — canonical definitions in clankers-message.
pub use clankers_message::ResultChunk;
pub use clankers_message::ToolResultAccumulator;
pub use clankers_message::TruncationConfig;
