use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CoreEffectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoreThinkingLevel {
    #[default]
    Off,
    Low,
    Medium,
    High,
    Max,
}

impl CoreThinkingLevel {
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreThinkingLevelInput {
    Level(CoreThinkingLevel),
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRequest {
    pub text: String,
    pub image_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptCompleted {
    pub effect_id: CoreEffectId,
    pub completion_status: CompletionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisabledToolsUpdate {
    pub requested_disabled_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFilterApplied {
    pub effect_id: CoreEffectId,
    pub applied_disabled_tool_set: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRequest {
    pub loop_id: String,
    pub prompt_text: String,
    pub max_iterations: u32,
    pub break_condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopFollowUpCompleted {
    pub effect_id: CoreEffectId,
    pub completion_status: CompletionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostPromptEvaluation {
    pub active_loop_state: Option<ActiveLoopState>,
    pub pending_follow_up_state: Option<PendingFollowUpState>,
    pub auto_test_enabled: bool,
    pub auto_test_command: Option<String>,
    pub auto_test_in_progress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreInput {
    PromptRequested(PromptRequest),
    PromptCompleted(PromptCompleted),
    EvaluatePostPrompt(PostPromptEvaluation),
    SetThinkingLevel { requested: CoreThinkingLevelInput },
    CycleThinkingLevel,
    SetDisabledTools(DisabledToolsUpdate),
    ToolFilterApplied(ToolFilterApplied),
    StartLoop(LoopRequest),
    StopLoop,
    LoopFollowUpCompleted(LoopFollowUpCompleted),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLoopState {
    pub loop_id: String,
    pub prompt_text: String,
    pub current_iteration: u32,
    pub max_iterations: u32,
    pub break_condition: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowUpSource {
    LoopContinuation,
    AutoTest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPromptState {
    pub effect_id: CoreEffectId,
    pub prompt_text: String,
    pub image_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingToolFilterState {
    pub effect_id: CoreEffectId,
    pub requested_disabled_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFollowUpState {
    pub effect_id: CoreEffectId,
    pub prompt_text: String,
    pub source: FollowUpSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoreState {
    pub busy: bool,
    pub thinking_level: CoreThinkingLevel,
    pub disabled_tools: Vec<String>,
    pub next_effect_id: CoreEffectId,
    pub pending_prompt: Option<PendingPromptState>,
    pub pending_tool_filter: Option<PendingToolFilterState>,
    pub pending_follow_up_state: Option<PendingFollowUpState>,
    pub active_loop_state: Option<ActiveLoopState>,
    pub auto_test_enabled: bool,
    pub auto_test_command: Option<String>,
    pub auto_test_in_progress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreLogicalEvent {
    BusyChanged {
        busy: bool,
    },
    ThinkingLevelChanged {
        previous: CoreThinkingLevel,
        current: CoreThinkingLevel,
    },
    DisabledToolsChanged {
        disabled_tools: Vec<String>,
    },
    LoopStateChanged {
        active_loop_state: Option<ActiveLoopState>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEffect {
    StartPrompt {
        effect_id: CoreEffectId,
        prompt_text: String,
        image_count: u32,
    },
    ApplyThinkingLevel {
        level: CoreThinkingLevel,
    },
    ApplyToolFilter {
        effect_id: CoreEffectId,
        disabled_tools: Vec<String>,
    },
    EmitLogicalEvent(CoreLogicalEvent),
    RunLoopFollowUp {
        effect_id: CoreEffectId,
        prompt_text: String,
        source: FollowUpSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreFailure {
    Cancelled,
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionStatus {
    Succeeded,
    Failed(CoreFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Busy,
    InvalidThinkingLevel { raw: String },
    PromptCompletionMismatch { effect_id: CoreEffectId },
    LoopFollowUpMismatch { effect_id: CoreEffectId },
    ToolFilterMismatch { effect_id: CoreEffectId },
    OutOfOrderRuntimeResult,
    LoopAlreadyActive,
    LoopNotActive,
    LoopFollowUpStillPending,
    ToolFilterStillPending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreOutcome {
    Transitioned {
        next_state: CoreState,
        effects: Vec<CoreEffect>,
    },
    Rejected {
        unchanged_state: CoreState,
        error: CoreError,
    },
}
