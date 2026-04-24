//! Structured progress and result streaming for tools
//!
//! Progress types re-exported from `clanker-tui-types`.
//! Result streaming types re-exported from `clanker-message`.

// ProgressKind and ToolProgress — canonical definitions in clanker-tui-types.
pub use clanker_tui_types::ProgressKind;
pub use clanker_tui_types::ToolProgress;

// ResultChunk, TruncationConfig, ToolResultAccumulator — canonical definitions in clanker-message.
pub use clanker_message::ResultChunk;
pub use clanker_message::ToolResultAccumulator;
pub use clanker_message::TruncationConfig;
