//! Compatibility module for legacy `clanker_message::message::*` imports.
//!
//! Stable SDK content contracts live in [`crate::content`]. Clankers desktop
//! transcript/session records live in [`crate::transcript`]. This module keeps
//! older imports working without advertising transcript records as the generic
//! SDK message boundary.

pub use crate::content::Content;
pub use crate::content::ImageSource;
pub use crate::content::StopReason;
pub use crate::transcript::AgentMessage;
pub use crate::transcript::AssistantMessage;
pub use crate::transcript::BashExecutionMessage;
pub use crate::transcript::BranchSummaryMessage;
pub use crate::transcript::CompactionSummaryMessage;
pub use crate::transcript::CustomMessage;
pub use crate::transcript::MessageId;
pub use crate::transcript::ToolResultMessage;
pub use crate::transcript::UserMessage;
pub use crate::transcript::generate_id;
