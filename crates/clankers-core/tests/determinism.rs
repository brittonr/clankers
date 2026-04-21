use clankers_core::ActiveLoopState;
use clankers_core::CompletionStatus;
use clankers_core::CoreEffectId;
use clankers_core::CoreFailure;
use clankers_core::CoreInput;
use clankers_core::CoreState;
use clankers_core::CoreThinkingLevel;
use clankers_core::CoreThinkingLevelInput;
use clankers_core::DisabledToolsUpdate;
use clankers_core::FollowUpSource;
use clankers_core::LoopFollowUpCompleted;
use clankers_core::LoopRequest;
use clankers_core::PendingFollowUpState;
use clankers_core::PendingPromptState;
use clankers_core::PendingToolFilterState;
use clankers_core::PostPromptEvaluation;
use clankers_core::PromptCompleted;
use clankers_core::PromptRequest;
use clankers_core::ToolFilterApplied;
use clankers_core::reduce;

const FIRST_EFFECT_ID: u64 = 1;
const SECOND_EFFECT_ID: u64 = 2;
const LOOP_ITERATION: u32 = 1;
const LOOP_ITERATION_LIMIT: u32 = 3;

fn loop_state() -> ActiveLoopState {
    ActiveLoopState {
        loop_id: "loop-1".to_string(),
        prompt_text: "continue loop".to_string(),
        current_iteration: LOOP_ITERATION,
        max_iterations: LOOP_ITERATION_LIMIT,
        break_condition: None,
    }
}

fn prompt_pending_state() -> CoreState {
    CoreState {
        busy: true,
        pending_prompt: Some(PendingPromptState {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            prompt_text: "hello".to_string(),
            image_count: 0,
        }),
        next_effect_id: CoreEffectId(FIRST_EFFECT_ID),
        ..CoreState::default()
    }
}

fn prompt_failure_state() -> CoreState {
    CoreState {
        busy: true,
        active_loop_state: Some(loop_state()),
        pending_prompt: Some(PendingPromptState {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            prompt_text: "hello".to_string(),
            image_count: 0,
        }),
        pending_follow_up_state: Some(PendingFollowUpState {
            effect_id: CoreEffectId(SECOND_EFFECT_ID),
            prompt_text: "continue loop".to_string(),
            source: FollowUpSource::LoopContinuation,
        }),
        next_effect_id: CoreEffectId(SECOND_EFFECT_ID),
        ..CoreState::default()
    }
}

fn tool_filter_pending_state() -> CoreState {
    CoreState {
        disabled_tools: vec!["bash".to_string()],
        pending_tool_filter: Some(PendingToolFilterState {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            requested_disabled_tools: vec!["bash".to_string()],
        }),
        next_effect_id: CoreEffectId(FIRST_EFFECT_ID),
        ..CoreState::default()
    }
}

fn loop_follow_up_pending_state() -> CoreState {
    CoreState {
        active_loop_state: Some(loop_state()),
        pending_follow_up_state: Some(PendingFollowUpState {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            prompt_text: "continue loop".to_string(),
            source: FollowUpSource::LoopContinuation,
        }),
        next_effect_id: CoreEffectId(FIRST_EFFECT_ID),
        ..CoreState::default()
    }
}

#[test]
fn reducer_replays_identical_state_and_input_pairs_deterministically() {
    let cases = vec![
        (
            CoreState::default(),
            CoreInput::PromptRequested(PromptRequest {
                text: "hello".to_string(),
                image_count: 0,
            }),
        ),
        (
            prompt_pending_state(),
            CoreInput::PromptCompleted(PromptCompleted {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                completion_status: CompletionStatus::Succeeded,
            }),
        ),
        (
            prompt_failure_state(),
            CoreInput::PromptCompleted(PromptCompleted {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                completion_status: CompletionStatus::Failed(CoreFailure::Message("boom".to_string())),
            }),
        ),
        (
            CoreState::default(),
            CoreInput::StartLoop(LoopRequest {
                loop_id: "loop-1".to_string(),
                prompt_text: "continue loop".to_string(),
                max_iterations: LOOP_ITERATION_LIMIT,
                break_condition: None,
            }),
        ),
        (
            CoreState {
                active_loop_state: Some(loop_state()),
                ..CoreState::default()
            },
            CoreInput::StopLoop,
        ),
        (
            CoreState::default(),
            CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
                active_loop_state: Some(loop_state()),
                pending_follow_up_state: None,
                auto_test_enabled: true,
                auto_test_command: Some("cargo test".to_string()),
                auto_test_in_progress: false,
            }),
        ),
        (CoreState::default(), CoreInput::SetThinkingLevel {
            requested: CoreThinkingLevelInput::Level(CoreThinkingLevel::High),
        }),
        (CoreState::default(), CoreInput::CycleThinkingLevel),
        (
            CoreState::default(),
            CoreInput::SetDisabledTools(DisabledToolsUpdate {
                requested_disabled_tools: vec!["bash".to_string(), "read".to_string()],
            }),
        ),
        (
            tool_filter_pending_state(),
            CoreInput::ToolFilterApplied(ToolFilterApplied {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                applied_disabled_tool_set: vec!["bash".to_string()],
            }),
        ),
        (
            loop_follow_up_pending_state(),
            CoreInput::LoopFollowUpCompleted(LoopFollowUpCompleted {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                completion_status: CompletionStatus::Succeeded,
            }),
        ),
        (
            CoreState {
                active_loop_state: Some(loop_state()),
                pending_follow_up_state: Some(PendingFollowUpState {
                    effect_id: CoreEffectId(FIRST_EFFECT_ID),
                    prompt_text: "continue loop".to_string(),
                    source: FollowUpSource::LoopContinuation,
                }),
                next_effect_id: CoreEffectId(FIRST_EFFECT_ID),
                ..CoreState::default()
            },
            CoreInput::StopLoop,
        ),
        (
            CoreState {
                active_loop_state: Some(loop_state()),
                ..CoreState::default()
            },
            CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
                active_loop_state: None,
                pending_follow_up_state: None,
                auto_test_enabled: true,
                auto_test_command: Some("cargo test".to_string()),
                auto_test_in_progress: false,
            }),
        ),
    ];

    for (state, input) in cases {
        let first = reduce(&state, &input);
        let second = reduce(&state, &input);
        assert_eq!(first, second, "state={state:?} input={input:?}");
    }
}
