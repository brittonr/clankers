//! Message types for LLM agent conversations
//!
//! Defines all message types used in the agent conversation loop:
//! user messages, assistant responses, tool results, and various
//! metadata messages (branching, compaction, custom).
//!
//! Also provides the richer streaming event types that wrap
//! `clankers-router`'s generic streaming with typed [`Content`] blocks.

pub mod message;
pub mod result_streaming;
pub mod streaming;
pub mod tool_result;

// Re-export core types at crate root for convenience
// Re-export Usage from clankers-router (used by AssistantMessage)
pub use clankers_router::Usage;
pub use message::generate_id;
pub use message::*;
// Re-export result streaming types at crate root
pub use result_streaming::ResultChunk;
pub use result_streaming::ToolResultAccumulator;
pub use result_streaming::TruncationConfig;
pub use streaming::ContentDelta;
pub use streaming::StreamDelta;
pub use streaming::StreamEvent;
// Re-export tool result types at crate root
pub use tool_result::ToolResult;
pub use tool_result::ToolResultContent;
