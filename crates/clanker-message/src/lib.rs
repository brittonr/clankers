#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
//! Message types for LLM agent conversations
//!
//! Defines stable content/tool/usage/streaming contracts for reusable SDK
//! boundaries, plus explicitly separated Clankers transcript compatibility
//! records for desktop/session adapters.
//!
//! Also provides router/provider-neutral streaming event types with typed
//! [`Content`] blocks.

pub mod content;
pub mod contracts;
pub mod cost;
#[cfg(feature = "transcript-compat")]
pub mod message;
pub mod process;
pub mod result_streaming;
pub mod semantic_event;
pub mod streaming;
pub mod token;
pub mod tool_result;
#[cfg(feature = "transcript-compat")]
pub mod transcript;

// Re-export core types at crate root for convenience
pub use content::Content;
pub use content::ImageSource;
pub use content::StopReason;
pub use contracts::ThinkingConfig;
pub use contracts::ThinkingLevel;
pub use contracts::ToolDefinition;
pub use contracts::Usage;
pub use cost::BudgetEvent;
pub use cost::BudgetStatus;
pub use cost::CostProvider;
pub use cost::CostSummary;
pub use cost::ModelCostBreakdown;
pub use process::ProcessDataSource;
pub use process::ProcessDisplayState;
pub use process::ProcessEvent;
pub use process::ProcessMeta;
pub use process::ProcessSnapshot;
// Re-export result streaming types at crate root
pub use result_streaming::ResultChunk;
pub use result_streaming::ToolResultAccumulator;
pub use result_streaming::TruncationConfig;
pub use semantic_event::SemanticConfirmationRequest;
pub use semantic_event::SemanticErrorClass;
pub use semantic_event::SemanticEvent;
pub use semantic_event::SemanticEventMetadata;
pub use semantic_event::SemanticImage;
pub use semantic_event::SemanticStopReason;
pub use semantic_event::SemanticToolStatus;
pub use streaming::ContentDelta;
pub use streaming::MessageMetadata;
pub use streaming::StreamDelta;
pub use streaming::StreamEvent;
pub use token::estimate_tokens;
pub use token::estimate_tokens_for_messages;
// Re-export tool result types at crate root
pub use tool_result::ToolResult;
pub use tool_result::ToolResultContent;
