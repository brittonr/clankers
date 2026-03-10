//! Message types for LLM agent conversations
//!
//! Defines all message types used in the agent conversation loop:
//! user messages, assistant responses, tool results, and various
//! metadata messages (branching, compaction, custom).
//!
//! Also provides the richer streaming event types that wrap
//! `clankers-router`'s generic streaming with typed [`Content`] blocks.

pub mod message;
pub mod streaming;

// Re-export core types at crate root for convenience
pub use message::generate_id;
pub use message::*;
pub use streaming::ContentDelta;
pub use streaming::StreamDelta;
pub use streaming::StreamEvent;

// Re-export Usage from clankers-router (used by AssistantMessage)
pub use clankers_router::Usage;
