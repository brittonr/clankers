//! Structured progress and result streaming for tools.
//!
//! Progress types come from the neutral agent tool boundary so built-in tool
//! policy does not import display DTOs. Result streaming types are re-exported
//! from `clanker-message`.

pub use clanker_message::ResultChunk;
pub use clanker_message::ToolResultAccumulator;
pub use clanker_message::TruncationConfig;
pub use clankers_agent::tool::progress::ProgressKind;
pub use clankers_agent::tool::progress::ToolProgress;
