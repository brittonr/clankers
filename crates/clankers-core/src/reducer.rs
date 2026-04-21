use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::types::ActiveLoopState;
use crate::types::CompletionStatus;
use crate::types::CoreEffect;
use crate::types::CoreEffectId;
use crate::types::CoreError;
use crate::types::CoreInput;
use crate::types::CoreLogicalEvent;
use crate::types::CoreOutcome;
use crate::types::CoreState;
use crate::types::DisabledToolsUpdate;
use crate::types::FollowUpSource;
use crate::types::LoopFollowUpCompleted;
use crate::types::LoopRequest;
use crate::types::PendingFollowUpState;
use crate::types::PendingPromptState;
use crate::types::PendingToolFilterState;
use crate::types::PostPromptEvaluation;
use crate::types::PromptCompleted;
use crate::types::PromptRequest;
use crate::types::ToolFilterApplied;

const AUTO_TEST_PROMPT_PREFIX: &str = "Run `";
const AUTO_TEST_PROMPT_SUFFIX: &str = "` and fix any failures. Do not ask for confirmation.";

pub fn reduce(state: &CoreState, input: &CoreInput) -> CoreOutcome {
    match input {
        CoreInput::PromptRequested(request) => reduce_prompt_requested(state, request),
        CoreInput::PromptCompleted(completed) => reduce_prompt_completed(state, completed),
        CoreInput::EvaluatePostPrompt(evaluation) => reduce_post_prompt_evaluation(state, evaluation),
        CoreInput::SetThinkingLevel { requested } => reduce_set_thinking_level(state, requested),
        CoreInput::CycleThinkingLevel => reduce_cycle_thinking_level(state),
        CoreInput::SetDisabledTools(update) => reduce_set_disabled_tools(state, update),
        CoreInput::ToolFilterApplied(applied) => reduce_tool_filter_applied(state, applied),
        CoreInput::StartLoop(request) => reduce_start_loop(state, request),
        CoreInput::StopLoop => reduce_stop_loop(state),
        CoreInput::LoopFollowUpCompleted(completed) => reduce_loop_follow_up_completed(state, completed),
    }
}

fn reduce_prompt_requested(state: &CoreState, request: &PromptRequest) -> CoreOutcome {
    if state.busy {
        return rejection(state, CoreError::Busy);
    }

    let mut next_state = state.clone();
    let effect_id = allocate_effect_id(&mut next_state);
    next_state.busy = true;
    next_state.pending_prompt = Some(PendingPromptState {
        effect_id,
        prompt_text: request.text.clone(),
        image_count: request.image_count,
    });

    transitioned(next_state, vec![
        CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: true }),
        CoreEffect::StartPrompt {
            effect_id,
            prompt_text: request.text.clone(),
            image_count: request.image_count,
        },
    ])
}

fn reduce_prompt_completed(state: &CoreState, completed: &PromptCompleted) -> CoreOutcome {
    let Some(pending_prompt) = state.pending_prompt.as_ref() else {
        return if has_other_pending_work(state) {
            rejection(state, CoreError::OutOfOrderRuntimeResult)
        } else {
            rejection(state, CoreError::PromptCompletionMismatch {
                effect_id: completed.effect_id,
            })
        };
    };

    if pending_prompt.effect_id != completed.effect_id {
        return rejection(state, CoreError::PromptCompletionMismatch {
            effect_id: completed.effect_id,
        });
    }

    let mut next_state = state.clone();
    next_state.busy = false;
    next_state.pending_prompt = None;

    let mut effects = vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged {
        busy: false,
    })];

    if matches!(completed.completion_status, CompletionStatus::Failed(_)) && next_state.active_loop_state.is_some() {
        next_state.active_loop_state = None;
        next_state.pending_follow_up_state = None;
        effects.push(CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
            active_loop_state: None,
        }));
    }

    transitioned(next_state, effects)
}

fn reduce_post_prompt_evaluation(state: &CoreState, evaluation: &PostPromptEvaluation) -> CoreOutcome {
    let mut next_state = state.clone();
    next_state.active_loop_state = evaluation.active_loop_state.clone();
    next_state.pending_follow_up_state = evaluation.pending_follow_up_state.clone();
    next_state.auto_test_enabled = evaluation.auto_test_enabled;
    next_state.auto_test_command = evaluation.auto_test_command.clone();
    next_state.auto_test_in_progress = evaluation.auto_test_in_progress;

    if next_state.pending_follow_up_state.is_some() {
        return transitioned(next_state, Vec::new());
    }

    if let Some(active_loop_state) = next_state.active_loop_state.as_ref() {
        let prompt_text = active_loop_state.prompt_text.clone();
        let effect_id = allocate_effect_id(&mut next_state);
        next_state.pending_follow_up_state = Some(PendingFollowUpState {
            effect_id,
            prompt_text: prompt_text.clone(),
            source: FollowUpSource::LoopContinuation,
        });
        return transitioned(next_state, vec![CoreEffect::RunLoopFollowUp {
            effect_id,
            prompt_text,
            source: FollowUpSource::LoopContinuation,
        }]);
    }

    if next_state.auto_test_enabled
        && !next_state.auto_test_in_progress
        && let Some(command) = next_state.auto_test_command.clone()
    {
        let effect_id = allocate_effect_id(&mut next_state);
        let prompt_text = build_auto_test_prompt(&command);
        next_state.auto_test_in_progress = true;
        next_state.pending_follow_up_state = Some(PendingFollowUpState {
            effect_id,
            prompt_text: prompt_text.clone(),
            source: FollowUpSource::AutoTest,
        });
        return transitioned(next_state, vec![CoreEffect::RunLoopFollowUp {
            effect_id,
            prompt_text,
            source: FollowUpSource::AutoTest,
        }]);
    }

    next_state.auto_test_in_progress = false;
    transitioned(next_state, Vec::new())
}

fn reduce_set_thinking_level(state: &CoreState, requested: &crate::types::CoreThinkingLevelInput) -> CoreOutcome {
    let crate::types::CoreThinkingLevelInput::Level(level) = requested else {
        let crate::types::CoreThinkingLevelInput::Invalid(raw) = requested else {
            unreachable!("thinking input must be level or invalid")
        };
        return rejection(state, CoreError::InvalidThinkingLevel { raw: raw.clone() });
    };

    let mut next_state = state.clone();
    let previous = next_state.thinking_level;
    next_state.thinking_level = *level;

    transitioned(next_state, vec![
        CoreEffect::ApplyThinkingLevel { level: *level },
        CoreEffect::EmitLogicalEvent(CoreLogicalEvent::ThinkingLevelChanged {
            previous,
            current: *level,
        }),
    ])
}

fn reduce_cycle_thinking_level(state: &CoreState) -> CoreOutcome {
    let next_level = state.thinking_level.next();
    reduce_set_thinking_level(state, &crate::types::CoreThinkingLevelInput::Level(next_level))
}

fn reduce_set_disabled_tools(state: &CoreState, update: &DisabledToolsUpdate) -> CoreOutcome {
    if state.pending_tool_filter.is_some() {
        return rejection(state, CoreError::ToolFilterStillPending);
    }

    let mut next_state = state.clone();
    let effect_id = allocate_effect_id(&mut next_state);
    next_state.disabled_tools = update.requested_disabled_tools.clone();
    next_state.pending_tool_filter = Some(PendingToolFilterState {
        effect_id,
        requested_disabled_tools: update.requested_disabled_tools.clone(),
    });

    transitioned(next_state, vec![CoreEffect::ApplyToolFilter {
        effect_id,
        disabled_tools: update.requested_disabled_tools.clone(),
    }])
}

fn reduce_tool_filter_applied(state: &CoreState, applied: &ToolFilterApplied) -> CoreOutcome {
    let Some(pending_tool_filter) = state.pending_tool_filter.as_ref() else {
        return if has_pending_work_other_than_tool_filter(state) {
            rejection(state, CoreError::OutOfOrderRuntimeResult)
        } else {
            rejection(state, CoreError::ToolFilterMismatch {
                effect_id: applied.effect_id,
            })
        };
    };

    if pending_tool_filter.effect_id != applied.effect_id
        || pending_tool_filter.requested_disabled_tools != applied.applied_disabled_tool_set
    {
        return rejection(state, CoreError::ToolFilterMismatch {
            effect_id: applied.effect_id,
        });
    }

    let mut next_state = state.clone();
    next_state.pending_tool_filter = None;

    transitioned(next_state, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::DisabledToolsChanged {
        disabled_tools: applied.applied_disabled_tool_set.clone(),
    })])
}

fn reduce_start_loop(state: &CoreState, request: &LoopRequest) -> CoreOutcome {
    if state.pending_follow_up_state.is_some() {
        return rejection(state, CoreError::LoopFollowUpStillPending);
    }
    if state.active_loop_state.is_some() {
        return rejection(state, CoreError::LoopAlreadyActive);
    }

    let active_loop_state = ActiveLoopState {
        loop_id: request.loop_id.clone(),
        prompt_text: request.prompt_text.clone(),
        current_iteration: 0,
        max_iterations: request.max_iterations,
        break_condition: request.break_condition.clone(),
    };
    let mut next_state = state.clone();
    next_state.active_loop_state = Some(active_loop_state.clone());

    transitioned(next_state, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
        active_loop_state: Some(active_loop_state),
    })])
}

fn reduce_stop_loop(state: &CoreState) -> CoreOutcome {
    if state.pending_follow_up_state.is_some() {
        return rejection(state, CoreError::LoopFollowUpStillPending);
    }
    if state.active_loop_state.is_none() {
        return rejection(state, CoreError::LoopNotActive);
    }

    let mut next_state = state.clone();
    next_state.active_loop_state = None;

    transitioned(next_state, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
        active_loop_state: None,
    })])
}

fn reduce_loop_follow_up_completed(state: &CoreState, completed: &LoopFollowUpCompleted) -> CoreOutcome {
    let Some(pending_follow_up_state) = state.pending_follow_up_state.as_ref() else {
        return if has_pending_work_other_than_follow_up(state) {
            rejection(state, CoreError::OutOfOrderRuntimeResult)
        } else {
            rejection(state, CoreError::LoopFollowUpMismatch {
                effect_id: completed.effect_id,
            })
        };
    };

    if pending_follow_up_state.effect_id != completed.effect_id {
        return rejection(state, CoreError::LoopFollowUpMismatch {
            effect_id: completed.effect_id,
        });
    }

    let mut next_state = state.clone();
    let source = pending_follow_up_state.source;
    next_state.pending_follow_up_state = None;

    let mut effects = Vec::new();
    if matches!(completed.completion_status, CompletionStatus::Failed(_)) {
        if matches!(source, FollowUpSource::AutoTest) {
            next_state.auto_test_in_progress = false;
        }
        if matches!(source, FollowUpSource::LoopContinuation) && next_state.active_loop_state.is_some() {
            next_state.active_loop_state = None;
            effects.push(CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                active_loop_state: None,
            }));
        }
    }

    transitioned(next_state, effects)
}

fn build_auto_test_prompt(command: &str) -> String {
    format!("{AUTO_TEST_PROMPT_PREFIX}{command}{AUTO_TEST_PROMPT_SUFFIX}")
}

fn allocate_effect_id(state: &mut CoreState) -> CoreEffectId {
    let next_id = CoreEffectId(state.next_effect_id.0 + 1);
    state.next_effect_id = next_id;
    next_id
}

fn has_other_pending_work(state: &CoreState) -> bool {
    state.pending_tool_filter.is_some() || state.pending_follow_up_state.is_some()
}

fn has_pending_work_other_than_tool_filter(state: &CoreState) -> bool {
    state.pending_prompt.is_some() || state.pending_follow_up_state.is_some()
}

fn has_pending_work_other_than_follow_up(state: &CoreState) -> bool {
    state.pending_prompt.is_some() || state.pending_tool_filter.is_some()
}

fn transitioned(next_state: CoreState, effects: Vec<CoreEffect>) -> CoreOutcome {
    CoreOutcome::Transitioned { next_state, effects }
}

fn rejection(state: &CoreState, error: CoreError) -> CoreOutcome {
    CoreOutcome::Rejected {
        unchanged_state: state.clone(),
        error,
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;
    use crate::types::CoreFailure;
    use crate::types::CoreInput;
    use crate::types::CoreThinkingLevel;
    use crate::types::CoreThinkingLevelInput;

    const FIRST_EFFECT_ID: u64 = 1;
    const SECOND_EFFECT_ID: u64 = 2;
    const LOOP_ITERATION_LIMIT: u32 = 3;

    fn loop_request() -> LoopRequest {
        LoopRequest {
            loop_id: "loop-1".to_string(),
            prompt_text: "continue loop".to_string(),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_condition: None,
        }
    }

    fn started_loop_state() -> ActiveLoopState {
        ActiveLoopState {
            loop_id: "loop-1".to_string(),
            prompt_text: "continue loop".to_string(),
            current_iteration: 0,
            max_iterations: LOOP_ITERATION_LIMIT,
            break_condition: None,
        }
    }

    fn loop_state() -> ActiveLoopState {
        ActiveLoopState {
            loop_id: "loop-1".to_string(),
            prompt_text: "continue loop".to_string(),
            current_iteration: 1,
            max_iterations: LOOP_ITERATION_LIMIT,
            break_condition: None,
        }
    }

    fn prompt_started_state() -> CoreState {
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

    fn pending_loop_follow_up_state() -> CoreState {
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

    fn pending_tool_filter_state() -> CoreState {
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

    fn expect_transition(outcome: CoreOutcome) -> (CoreState, Vec<CoreEffect>) {
        match outcome {
            CoreOutcome::Transitioned { next_state, effects } => (next_state, effects),
            other => panic!("expected transition, got {other:?}"),
        }
    }

    fn expect_rejection(outcome: CoreOutcome) -> (CoreState, CoreError) {
        match outcome {
            CoreOutcome::Rejected { unchanged_state, error } => (unchanged_state, error),
            other => panic!("expected rejection, got {other:?}"),
        }
    }

    #[test]
    fn prompt_request_marks_busy_and_emits_start_effect() {
        let request = CoreInput::PromptRequested(PromptRequest {
            text: "hello".to_string(),
            image_count: 0,
        });

        let (next_state, effects) = expect_transition(reduce(&CoreState::default(), &request));

        assert!(next_state.busy);
        assert_eq!(
            next_state.pending_prompt,
            Some(PendingPromptState {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                prompt_text: "hello".to_string(),
                image_count: 0,
            })
        );
        assert_eq!(effects, vec![
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: true }),
            CoreEffect::StartPrompt {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                prompt_text: "hello".to_string(),
                image_count: 0,
            },
        ]);
    }

    #[test]
    fn repeated_prompt_request_is_rejected_while_busy() {
        let state = prompt_started_state();
        let request = CoreInput::PromptRequested(PromptRequest {
            text: "again".to_string(),
            image_count: 0,
        });

        let (unchanged_state, error) = expect_rejection(reduce(&state, &request));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::Busy);
    }

    #[test]
    fn prompt_completion_success_clears_busy_without_loop_mutation() {
        let state = prompt_started_state();
        let input = CoreInput::PromptCompleted(PromptCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });

        let (next_state, effects) = expect_transition(reduce(&state, &input));

        assert!(!next_state.busy);
        assert!(next_state.pending_prompt.is_none());
        assert!(next_state.active_loop_state.is_none());
        assert_eq!(effects, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged {
            busy: false
        })]);
    }

    #[test]
    fn prompt_completion_failure_clears_active_loop_and_pending_follow_up() {
        let mut state = prompt_started_state();
        state.active_loop_state = Some(loop_state());
        state.pending_follow_up_state = Some(PendingFollowUpState {
            effect_id: CoreEffectId(SECOND_EFFECT_ID),
            prompt_text: "continue loop".to_string(),
            source: FollowUpSource::LoopContinuation,
        });
        state.next_effect_id = CoreEffectId(SECOND_EFFECT_ID);

        let input = CoreInput::PromptCompleted(PromptCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Failed(CoreFailure::Cancelled),
        });

        let (next_state, effects) = expect_transition(reduce(&state, &input));

        assert!(!next_state.busy);
        assert!(next_state.pending_prompt.is_none());
        assert!(next_state.pending_follow_up_state.is_none());
        assert!(next_state.active_loop_state.is_none());
        assert_eq!(effects, vec![
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: false }),
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                active_loop_state: None,
            }),
        ]);
    }

    #[test]
    fn post_prompt_evaluation_prefers_loop_follow_up() {
        let input = CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
            active_loop_state: Some(loop_state()),
            pending_follow_up_state: None,
            auto_test_enabled: true,
            auto_test_command: Some("cargo test".to_string()),
            auto_test_in_progress: false,
        });

        let (next_state, effects) = expect_transition(reduce(&CoreState::default(), &input));

        assert_eq!(
            next_state.pending_follow_up_state,
            Some(PendingFollowUpState {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                prompt_text: "continue loop".to_string(),
                source: FollowUpSource::LoopContinuation,
            })
        );
        assert!(!next_state.auto_test_in_progress);
        assert_eq!(effects, vec![CoreEffect::RunLoopFollowUp {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            prompt_text: "continue loop".to_string(),
            source: FollowUpSource::LoopContinuation,
        }]);
    }

    #[test]
    fn post_prompt_evaluation_schedules_auto_test_when_loop_inactive() {
        let input = CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
            active_loop_state: None,
            pending_follow_up_state: None,
            auto_test_enabled: true,
            auto_test_command: Some("cargo test".to_string()),
            auto_test_in_progress: false,
        });

        let (next_state, effects) = expect_transition(reduce(&CoreState::default(), &input));

        assert!(next_state.auto_test_in_progress);
        assert_eq!(
            next_state.pending_follow_up_state,
            Some(PendingFollowUpState {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                prompt_text: "Run `cargo test` and fix any failures. Do not ask for confirmation.".to_string(),
                source: FollowUpSource::AutoTest,
            })
        );
        assert_eq!(effects, vec![CoreEffect::RunLoopFollowUp {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            prompt_text: "Run `cargo test` and fix any failures. Do not ask for confirmation.".to_string(),
            source: FollowUpSource::AutoTest,
        }]);
    }

    #[test]
    fn post_prompt_evaluation_clears_auto_test_guard_when_no_follow_up_runs() {
        let state = CoreState {
            auto_test_in_progress: true,
            ..CoreState::default()
        };
        let input = CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
            active_loop_state: None,
            pending_follow_up_state: None,
            auto_test_enabled: false,
            auto_test_command: None,
            auto_test_in_progress: true,
        });

        let (next_state, effects) = expect_transition(reduce(&state, &input));

        assert!(!next_state.auto_test_in_progress);
        assert!(effects.is_empty());
    }

    #[test]
    fn valid_thinking_level_updates_state_and_effects() {
        let input = CoreInput::SetThinkingLevel {
            requested: CoreThinkingLevelInput::Level(CoreThinkingLevel::High),
        };

        let (next_state, effects) = expect_transition(reduce(&CoreState::default(), &input));

        assert_eq!(next_state.thinking_level, CoreThinkingLevel::High);
        assert_eq!(effects, vec![
            CoreEffect::ApplyThinkingLevel {
                level: CoreThinkingLevel::High,
            },
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::ThinkingLevelChanged {
                previous: CoreThinkingLevel::Off,
                current: CoreThinkingLevel::High,
            }),
        ]);
    }

    #[test]
    fn cycle_thinking_level_uses_shared_transition_order() {
        let input = CoreInput::CycleThinkingLevel;

        let (next_state, effects) = expect_transition(reduce(&CoreState::default(), &input));

        assert_eq!(next_state.thinking_level, CoreThinkingLevel::Low);
        assert_eq!(effects, vec![
            CoreEffect::ApplyThinkingLevel {
                level: CoreThinkingLevel::Low,
            },
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::ThinkingLevelChanged {
                previous: CoreThinkingLevel::Off,
                current: CoreThinkingLevel::Low,
            }),
        ]);
    }

    #[test]
    fn invalid_thinking_level_is_rejected_without_state_change() {
        let state = CoreState {
            thinking_level: CoreThinkingLevel::Medium,
            ..CoreState::default()
        };
        let input = CoreInput::SetThinkingLevel {
            requested: CoreThinkingLevelInput::Invalid("bogus".to_string()),
        };

        let (unchanged_state, error) = expect_rejection(reduce(&state, &input));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::InvalidThinkingLevel {
            raw: "bogus".to_string(),
        });
    }

    #[test]
    fn disabled_tools_request_sets_pending_filter() {
        let input = CoreInput::SetDisabledTools(DisabledToolsUpdate {
            requested_disabled_tools: vec!["bash".to_string(), "read".to_string()],
        });

        let (next_state, effects) = expect_transition(reduce(&CoreState::default(), &input));

        assert_eq!(next_state.disabled_tools, vec!["bash".to_string(), "read".to_string()]);
        assert_eq!(
            next_state.pending_tool_filter,
            Some(PendingToolFilterState {
                effect_id: CoreEffectId(FIRST_EFFECT_ID),
                requested_disabled_tools: vec!["bash".to_string(), "read".to_string()],
            })
        );
        assert_eq!(effects, vec![CoreEffect::ApplyToolFilter {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            disabled_tools: vec!["bash".to_string(), "read".to_string()],
        }]);
    }

    #[test]
    fn disabled_tools_request_is_rejected_while_filter_pending() {
        let state = pending_tool_filter_state();
        let input = CoreInput::SetDisabledTools(DisabledToolsUpdate {
            requested_disabled_tools: vec!["read".to_string()],
        });

        let (unchanged_state, error) = expect_rejection(reduce(&state, &input));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::ToolFilterStillPending);
    }

    #[test]
    fn tool_filter_feedback_accepts_matching_applied_set() {
        let state = pending_tool_filter_state();
        let input = CoreInput::ToolFilterApplied(ToolFilterApplied {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            applied_disabled_tool_set: vec!["bash".to_string()],
        });

        let (next_state, effects) = expect_transition(reduce(&state, &input));

        assert!(next_state.pending_tool_filter.is_none());
        assert_eq!(effects, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::DisabledToolsChanged {
            disabled_tools: vec!["bash".to_string()],
        })]);
    }

    #[test]
    fn start_loop_rejects_when_already_active() {
        let state = CoreState {
            active_loop_state: Some(loop_state()),
            ..CoreState::default()
        };

        let (unchanged_state, error) = expect_rejection(reduce(&state, &CoreInput::StartLoop(loop_request())));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::LoopAlreadyActive);
    }

    #[test]
    fn start_loop_rejects_when_follow_up_pending() {
        let state = pending_loop_follow_up_state();

        let (unchanged_state, error) = expect_rejection(reduce(&state, &CoreInput::StartLoop(loop_request())));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::LoopFollowUpStillPending);
    }

    #[test]
    fn start_loop_transitions_to_active_loop_state() {
        let (next_state, effects) =
            expect_transition(reduce(&CoreState::default(), &CoreInput::StartLoop(loop_request())));

        assert_eq!(next_state.active_loop_state, Some(started_loop_state().clone()));
        assert_eq!(effects, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
            active_loop_state: Some(started_loop_state()),
        })]);
    }

    #[test]
    fn stop_loop_requires_active_loop() {
        let (unchanged_state, error) = expect_rejection(reduce(&CoreState::default(), &CoreInput::StopLoop));

        assert_eq!(unchanged_state, CoreState::default());
        assert_eq!(error, CoreError::LoopNotActive);
    }

    #[test]
    fn stop_loop_rejects_when_follow_up_pending() {
        let state = pending_loop_follow_up_state();

        let (unchanged_state, error) = expect_rejection(reduce(&state, &CoreInput::StopLoop));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::LoopFollowUpStillPending);
    }

    #[test]
    fn stop_loop_clears_active_loop() {
        let state = CoreState {
            active_loop_state: Some(loop_state()),
            ..CoreState::default()
        };

        let (next_state, effects) = expect_transition(reduce(&state, &CoreInput::StopLoop));

        assert!(next_state.active_loop_state.is_none());
        assert_eq!(effects, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
            active_loop_state: None,
        })]);
    }

    #[test]
    fn loop_follow_up_completion_failure_clears_pending_and_ends_loop() {
        let state = pending_loop_follow_up_state();
        let input = CoreInput::LoopFollowUpCompleted(LoopFollowUpCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Failed(CoreFailure::Message("boom".to_string())),
        });

        let (next_state, effects) = expect_transition(reduce(&state, &input));

        assert!(next_state.pending_follow_up_state.is_none());
        assert!(next_state.active_loop_state.is_none());
        assert_eq!(effects, vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
            active_loop_state: None,
        })]);
    }

    #[test]
    fn prompt_completion_empty_slot_is_rejected_with_prompt_mismatch() {
        let input = CoreInput::PromptCompleted(PromptCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });

        let (unchanged_state, error) = expect_rejection(reduce(&CoreState::default(), &input));

        assert_eq!(unchanged_state, CoreState::default());
        assert_eq!(error, CoreError::PromptCompletionMismatch {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
        });
    }

    #[test]
    fn out_of_order_runtime_results_are_rejected() {
        let state = pending_tool_filter_state();
        let input = CoreInput::PromptCompleted(PromptCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });

        let (unchanged_state, error) = expect_rejection(reduce(&state, &input));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::OutOfOrderRuntimeResult);
    }

    #[test]
    fn tool_filter_feedback_with_mismatched_applied_set_is_rejected() {
        let state = pending_tool_filter_state();
        let input = CoreInput::ToolFilterApplied(ToolFilterApplied {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            applied_disabled_tool_set: vec!["read".to_string()],
        });

        let (unchanged_state, error) = expect_rejection(reduce(&state, &input));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::ToolFilterMismatch {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
        });
    }

    #[test]
    fn duplicate_prompt_completion_is_rejected_after_slot_clears() {
        let state = prompt_started_state();
        let completion = CoreInput::PromptCompleted(PromptCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });
        let (next_state, _) = expect_transition(reduce(&state, &completion));

        let (unchanged_state, error) = expect_rejection(reduce(&next_state, &completion));

        assert_eq!(unchanged_state, next_state);
        assert_eq!(error, CoreError::PromptCompletionMismatch {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
        });
    }

    #[test]
    fn loop_follow_up_completion_empty_slot_is_rejected_with_follow_up_mismatch() {
        let input = CoreInput::LoopFollowUpCompleted(LoopFollowUpCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });

        let (unchanged_state, error) = expect_rejection(reduce(&CoreState::default(), &input));

        assert_eq!(unchanged_state, CoreState::default());
        assert_eq!(error, CoreError::LoopFollowUpMismatch {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
        });
    }

    #[test]
    fn mismatched_feedback_tokens_are_rejected_without_state_change() {
        let state = pending_loop_follow_up_state();
        let input = CoreInput::LoopFollowUpCompleted(LoopFollowUpCompleted {
            effect_id: CoreEffectId(SECOND_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });

        let (unchanged_state, error) = expect_rejection(reduce(&state, &input));

        assert_eq!(unchanged_state, state);
        assert_eq!(error, CoreError::LoopFollowUpMismatch {
            effect_id: CoreEffectId(SECOND_EFFECT_ID),
        });
    }

    #[test]
    fn duplicate_loop_follow_up_completion_is_rejected_after_slot_clears() {
        let state = pending_loop_follow_up_state();
        let completion = CoreInput::LoopFollowUpCompleted(LoopFollowUpCompleted {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
            completion_status: CompletionStatus::Succeeded,
        });
        let (next_state, _) = expect_transition(reduce(&state, &completion));

        let (unchanged_state, error) = expect_rejection(reduce(&next_state, &completion));

        assert_eq!(unchanged_state, next_state);
        assert_eq!(error, CoreError::LoopFollowUpMismatch {
            effect_id: CoreEffectId(FIRST_EFFECT_ID),
        });
    }
}
