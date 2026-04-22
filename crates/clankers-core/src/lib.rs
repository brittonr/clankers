#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod reducer;
mod types;

pub use crate::reducer::reduce;
pub use crate::types::ActiveLoopState;
pub use crate::types::CompletionStatus;
pub use crate::types::CoreEffect;
pub use crate::types::CoreEffectId;
pub use crate::types::CoreError;
pub use crate::types::CoreFailure;
pub use crate::types::CoreInput;
pub use crate::types::CoreLogicalEvent;
pub use crate::types::CoreOutcome;
pub use crate::types::CoreState;
pub use crate::types::CoreThinkingLevel;
pub use crate::types::CoreThinkingLevelInput;
pub use crate::types::DisabledToolsUpdate;
pub use crate::types::FollowUpDispatchAcknowledged;
pub use crate::types::FollowUpDispatchStatus;
pub use crate::types::FollowUpSource;
pub use crate::types::LoopFollowUpCompleted;
pub use crate::types::LoopRequest;
pub use crate::types::PendingFollowUpStage;
pub use crate::types::PendingFollowUpState;
pub use crate::types::PendingPromptState;
pub use crate::types::PendingToolFilterState;
pub use crate::types::PostPromptEvaluation;
pub use crate::types::PromptCompleted;
pub use crate::types::PromptRequest;
pub use crate::types::ToolFilterApplied;
