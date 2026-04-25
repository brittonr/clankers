//! Pure composition seam between `clankers-core` prompt acceptance and
//! `clankers-engine` model/tool turn submission.
//!
//! This module does not execute model requests, tools, daemon I/O, storage, or
//! timers. It only normalizes accepted core prompt work into engine-native data
//! and preserves controller-owned correlation for later shell feedback.

use clanker_message::ThinkingConfig;
use clanker_message::ToolDefinition;
use clankers_core::CoreEffectId;
use clankers_core::FollowUpSource;
use clankers_engine::EngineInput;
use clankers_engine::EngineMessage;
use clankers_engine::EnginePromptSubmission;

/// Prompt accepted by `clankers-core` and ready to enter engine-owned policy.
#[allow(dead_code)] // Follow-up wiring lands after the pure seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AcceptedPromptKind {
    UserPrompt,
    FollowUp(FollowUpSource),
}

/// Controller-held prompt acceptance data correlated to a core effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedPromptStart {
    pub(crate) core_effect_id: CoreEffectId,
    pub(crate) kind: AcceptedPromptKind,
    pub(crate) prompt_text: String,
    pub(crate) image_count: u32,
}

/// Shell policy that parameterizes an engine submission without giving the
/// composition seam access to runtime objects.
#[derive(Debug, Clone)]
pub(crate) struct EngineSubmissionPolicy {
    pub(crate) model: String,
    pub(crate) system_prompt: String,
    pub(crate) max_tokens: Option<usize>,
    pub(crate) temperature: Option<f64>,
    pub(crate) thinking: Option<ThinkingConfig>,
    pub(crate) tools: Vec<ToolDefinition>,
    pub(crate) no_cache: bool,
    pub(crate) cache_ttl: Option<String>,
    pub(crate) session_id: String,
    pub(crate) model_request_slot_budget: u32,
}

/// Engine submission paired with core correlation retained by controller code.
#[derive(Debug, Clone)]
pub(crate) struct EngineSubmissionPlan {
    pub(crate) core_effect_id: CoreEffectId,
    pub(crate) prompt_kind: AcceptedPromptKind,
    pub(crate) engine_input: EngineInput,
}

pub(crate) fn engine_submission_from_prompt_start(
    prompt_start: &AcceptedPromptStart,
    prior_messages: Vec<EngineMessage>,
    policy: EngineSubmissionPolicy,
) -> EngineSubmissionPlan {
    let submission = EnginePromptSubmission {
        messages: prior_messages,
        model: policy.model,
        system_prompt: policy.system_prompt,
        max_tokens: policy.max_tokens,
        temperature: policy.temperature,
        thinking: policy.thinking,
        tools: policy.tools,
        no_cache: policy.no_cache,
        cache_ttl: policy.cache_ttl,
        session_id: policy.session_id,
        model_request_slot_budget: policy.model_request_slot_budget,
    };

    EngineSubmissionPlan {
        core_effect_id: prompt_start.core_effect_id,
        prompt_kind: prompt_start.kind.clone(),
        engine_input: EngineInput::SubmitUserPrompt { submission },
    }
}

#[cfg(test)]
#[allow(dead_code)] // Negative tests construct one target per routing case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositionReducer {
    CoreLifecycle,
    EngineTurn,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) enum CompositionFeedback {
    Core(clankers_core::CoreInput),
    Engine(EngineInput),
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompositionRejection {
    LifecycleFeedbackSentToEngine,
    TurnFeedbackSentToCore,
}

#[cfg(test)]
#[allow(dead_code)] // Tests inspect only the reducer side relevant to each case.
#[derive(Debug, Clone)]
pub(crate) enum CompositionStep {
    Core(clankers_core::CoreOutcome),
    Engine(clankers_engine::EngineOutcome),
}

#[cfg(test)]
pub(crate) fn apply_composition_feedback_for_tests(
    target: CompositionReducer,
    core_state: &clankers_core::CoreState,
    engine_state: &clankers_engine::EngineState,
    feedback: CompositionFeedback,
) -> Result<CompositionStep, CompositionRejection> {
    match (target, feedback) {
        (CompositionReducer::CoreLifecycle, CompositionFeedback::Core(input)) => {
            Ok(CompositionStep::Core(clankers_core::reduce(core_state, &input)))
        }
        (CompositionReducer::EngineTurn, CompositionFeedback::Engine(input)) => {
            Ok(CompositionStep::Engine(clankers_engine::reduce(engine_state, &input)))
        }
        (CompositionReducer::CoreLifecycle, CompositionFeedback::Engine(_)) => {
            Err(CompositionRejection::TurnFeedbackSentToCore)
        }
        (CompositionReducer::EngineTurn, CompositionFeedback::Core(_)) => {
            Err(CompositionRejection::LifecycleFeedbackSentToEngine)
        }
    }
}

#[cfg(test)]
mod tests {
    use clanker_message::Content;
    use clankers_core::ActiveLoopState;
    use clankers_core::CompletionStatus;
    use clankers_core::CoreEffectId;
    use clankers_core::CoreInput;
    use clankers_core::CoreOutcome;
    use clankers_core::CoreState;
    use clankers_core::FollowUpDispatchAcknowledged;
    use clankers_core::FollowUpDispatchStatus;
    use clankers_core::FollowUpSource;
    use clankers_core::LoopFollowUpCompleted;
    use clankers_core::PostPromptEvaluation;
    use clankers_core::PromptCompleted;
    use clankers_core::PromptRequest;
    use clankers_engine::EngineEffect;
    use clankers_engine::EngineMessageRole;
    use clankers_engine::EngineState;

    use super::*;
    use crate::core_effects::AcceptedEnginePrompt;
    use crate::core_effects::accepted_engine_prompt_from_core_outcome;

    const TEST_EFFECT_ID: u64 = 42;
    const TEST_IMAGE_COUNT: u32 = 0;
    const TEST_PROMPT: &str = "summarize core ownership";
    const FOLLOW_UP_PROMPT: &str = "continue loop";
    const TEST_SESSION_ID: &str = "session-1";
    const TEST_MODEL: &str = "test-model";
    const TEST_SYSTEM_PROMPT: &str = "you are precise";
    const TEST_MAX_MODEL_REQUESTS: u32 = 3;
    const EXPECTED_ENGINE_NOTICE_COUNT: usize = 3;

    fn test_prompt_start() -> AcceptedPromptStart {
        AcceptedPromptStart {
            core_effect_id: CoreEffectId(TEST_EFFECT_ID),
            kind: AcceptedPromptKind::UserPrompt,
            prompt_text: TEST_PROMPT.to_owned(),
            image_count: TEST_IMAGE_COUNT,
        }
    }

    fn test_policy() -> EngineSubmissionPolicy {
        EngineSubmissionPolicy {
            model: TEST_MODEL.to_owned(),
            system_prompt: TEST_SYSTEM_PROMPT.to_owned(),
            max_tokens: None,
            temperature: None,
            thinking: None,
            tools: Vec::new(),
            no_cache: false,
            cache_ttl: None,
            session_id: TEST_SESSION_ID.to_owned(),
            model_request_slot_budget: TEST_MAX_MODEL_REQUESTS,
        }
    }

    fn prior_messages() -> Vec<EngineMessage> {
        vec![EngineMessage {
            role: EngineMessageRole::User,
            content: vec![Content::Text {
                text: TEST_PROMPT.to_owned(),
            }],
        }]
    }

    fn text_from_first_message(messages: &[EngineMessage]) -> Option<&str> {
        let first_message = messages.first()?;
        let first_content = first_message.content.first()?;
        match first_content {
            Content::Text { text } => Some(text.as_str()),
            Content::Image { .. } | Content::Thinking { .. } | Content::ToolUse { .. } | Content::ToolResult { .. } => {
                None
            }
        }
    }

    #[test]
    fn engine_submission_preserves_prompt_identity_and_policy() {
        let plan = engine_submission_from_prompt_start(&test_prompt_start(), prior_messages(), test_policy());

        assert_eq!(plan.core_effect_id, CoreEffectId(TEST_EFFECT_ID));
        assert_eq!(plan.prompt_kind, AcceptedPromptKind::UserPrompt);
        match plan.engine_input {
            EngineInput::SubmitUserPrompt { submission } => {
                assert_eq!(submission.model, TEST_MODEL);
                assert_eq!(submission.system_prompt, TEST_SYSTEM_PROMPT);
                assert_eq!(submission.session_id, TEST_SESSION_ID);
                assert_eq!(submission.model_request_slot_budget, TEST_MAX_MODEL_REQUESTS);
                assert_eq!(text_from_first_message(&submission.messages), Some(TEST_PROMPT));
            }
            other => panic!("expected submit prompt input, got {other:?}"),
        }
    }

    #[test]
    fn apply_composition_feedback_routes_engine_submission_to_engine_reducer() {
        let plan = engine_submission_from_prompt_start(&test_prompt_start(), prior_messages(), test_policy());
        let outcome = apply_composition_feedback_for_tests(
            CompositionReducer::EngineTurn,
            &clankers_core::CoreState::default(),
            &EngineState::new(),
            CompositionFeedback::Engine(plan.engine_input),
        )
        .expect("engine feedback must route to engine reducer");

        match outcome {
            CompositionStep::Engine(outcome) => {
                assert!(outcome.rejection.is_none());
                assert_eq!(outcome.effects.len(), EXPECTED_ENGINE_NOTICE_COUNT);
                let request = outcome.effects.iter().find_map(|effect| match effect {
                    EngineEffect::RequestModel(request) => Some(request),
                    EngineEffect::ExecuteTool(_) | EngineEffect::ScheduleRetry { .. } | EngineEffect::EmitEvent(_) => {
                        None
                    }
                });
                let request = request.expect("submit prompt must emit model request");
                assert_eq!(request.session_id, TEST_SESSION_ID);
                assert_eq!(request.model, TEST_MODEL);
                assert_eq!(text_from_first_message(&request.messages), Some(TEST_PROMPT));
            }
            CompositionStep::Core(_) => panic!("expected engine reducer result"),
        }
    }

    #[test]
    fn apply_composition_feedback_rejects_cross_reducer_feedback() {
        let prompt_input = CoreInput::PromptRequested(PromptRequest {
            text: TEST_PROMPT.to_owned(),
            image_count: TEST_IMAGE_COUNT,
            originating_follow_up_effect_id: None,
        });
        let rejection = apply_composition_feedback_for_tests(
            CompositionReducer::EngineTurn,
            &clankers_core::CoreState::default(),
            &EngineState::new(),
            CompositionFeedback::Core(prompt_input),
        )
        .expect_err("core lifecycle feedback must not route to engine reducer");

        assert_eq!(rejection, CompositionRejection::LifecycleFeedbackSentToEngine);
    }

    #[test]
    fn composition_positive_prompt_sequencing_runs_core_engine_core_in_order() {
        let initial_core_state = clankers_core::CoreState::default();
        let prompt_outcome = clankers_core::reduce(
            &initial_core_state,
            &CoreInput::PromptRequested(PromptRequest {
                text: TEST_PROMPT.to_owned(),
                image_count: TEST_IMAGE_COUNT,
                originating_follow_up_effect_id: None,
            }),
        );
        let accepted_prompt = accepted_engine_prompt_from_core_outcome(&prompt_outcome)
            .expect("accepted core prompt must create engine prompt gate");
        let AcceptedEnginePrompt::UserPrompt(prompt_start) = accepted_prompt else {
            panic!("plain prompt must normalize as user prompt");
        };
        let CoreOutcome::Transitioned {
            next_state: prompt_pending_core_state,
            ..
        } = prompt_outcome
        else {
            panic!("prompt request must transition core state");
        };
        assert!(prompt_pending_core_state.busy);
        assert!(prompt_pending_core_state.pending_prompt.is_some());

        let plan = engine_submission_from_prompt_start(&prompt_start, prior_messages(), test_policy());
        assert_eq!(plan.core_effect_id, prompt_start.core_effect_id);
        let engine_step = apply_composition_feedback_for_tests(
            CompositionReducer::EngineTurn,
            &prompt_pending_core_state,
            &EngineState::new(),
            CompositionFeedback::Engine(plan.engine_input),
        )
        .expect("adapter plan must reduce through engine");
        let CompositionStep::Engine(engine_outcome) = engine_step else {
            panic!("engine prompt plan must target engine reducer");
        };
        assert!(engine_outcome.rejection.is_none());
        assert!(engine_outcome.next_state.pending_model_request.is_some());

        let prompt_completion = clankers_core::reduce(
            &prompt_pending_core_state,
            &CoreInput::PromptCompleted(PromptCompleted {
                effect_id: prompt_start.core_effect_id,
                completion_status: CompletionStatus::Succeeded,
            }),
        );
        let CoreOutcome::Transitioned {
            next_state: completed_core_state,
            effects: completion_effects,
        } = prompt_completion
        else {
            panic!("prompt completion must transition core state");
        };
        assert!(!completed_core_state.busy);
        assert!(completed_core_state.pending_prompt.is_none());
        assert!(!completion_effects.is_empty());

        let post_prompt = clankers_core::reduce(
            &completed_core_state,
            &CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
                active_loop_state: None,
                pending_follow_up_state: None,
                auto_test_enabled: false,
                auto_test_command: None,
                auto_test_in_progress: false,
                queued_prompt_present: false,
            }),
        );
        let CoreOutcome::Transitioned {
            next_state: post_prompt_state,
            effects: post_prompt_effects,
        } = post_prompt
        else {
            panic!("post-prompt evaluation must transition core state");
        };
        assert_eq!(post_prompt_state, completed_core_state);
        assert!(post_prompt_effects.is_empty());
    }

    fn evaluate_for_follow_up(source: FollowUpSource) -> (CoreState, AcceptedPromptStart) {
        const ACTIVE_LOOP_ITERATION: u32 = 1;
        const FOLLOW_UP_ITERATION_LIMIT: u32 = 3;
        let active_loop_state = ActiveLoopState {
            loop_id: "loop-1".to_owned(),
            prompt_text: FOLLOW_UP_PROMPT.to_owned(),
            current_iteration: ACTIVE_LOOP_ITERATION,
            max_iterations: FOLLOW_UP_ITERATION_LIMIT,
            break_condition: None,
        };
        let evaluation = match source {
            FollowUpSource::LoopContinuation => PostPromptEvaluation {
                active_loop_state: Some(active_loop_state),
                pending_follow_up_state: None,
                auto_test_enabled: false,
                auto_test_command: None,
                auto_test_in_progress: false,
                queued_prompt_present: false,
            },
            FollowUpSource::AutoTest => PostPromptEvaluation {
                active_loop_state: None,
                pending_follow_up_state: None,
                auto_test_enabled: true,
                auto_test_command: Some("cargo test".to_owned()),
                auto_test_in_progress: false,
                queued_prompt_present: false,
            },
        };
        let outcome = clankers_core::reduce(&CoreState::default(), &CoreInput::EvaluatePostPrompt(evaluation));
        let accepted = accepted_engine_prompt_from_core_outcome(&outcome)
            .expect("follow-up evaluation must normalize to accepted engine prompt");
        let AcceptedEnginePrompt::FollowUp(prompt_start) = accepted else {
            panic!("expected follow-up prompt start");
        };
        assert_eq!(prompt_start.kind, AcceptedPromptKind::FollowUp(source));
        let CoreOutcome::Transitioned { next_state, .. } = outcome else {
            panic!("follow-up evaluation must transition core state");
        };
        (next_state, prompt_start)
    }

    #[test]
    fn composition_positive_queued_prompt_replay_requires_fresh_core_prompt() {
        let replay_outcome = clankers_core::reduce(
            &CoreState::default(),
            &CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
                active_loop_state: None,
                pending_follow_up_state: None,
                auto_test_enabled: false,
                auto_test_command: None,
                auto_test_in_progress: false,
                queued_prompt_present: true,
            }),
        );
        let replay_rejection = accepted_engine_prompt_from_core_outcome(&replay_outcome)
            .expect_err("queued replay must not bypass fresh core prompt request");
        assert_eq!(
            replay_rejection,
            crate::core_effects::CoreEffectGateRejection::ReplayQueuedPromptNeedsFreshCorePrompt
        );

        let fresh_prompt_outcome = clankers_core::reduce(
            &CoreState::default(),
            &CoreInput::PromptRequested(PromptRequest {
                text: TEST_PROMPT.to_owned(),
                image_count: TEST_IMAGE_COUNT,
                originating_follow_up_effect_id: None,
            }),
        );
        let fresh_prompt = accepted_engine_prompt_from_core_outcome(&fresh_prompt_outcome)
            .expect("fresh queued prompt replay must pass through normal prompt gate");
        assert!(matches!(fresh_prompt, AcceptedEnginePrompt::UserPrompt(_)));
    }

    #[test]
    fn composition_positive_follow_up_sequence_acknowledges_dispatch_before_engine_submission() {
        for source in [FollowUpSource::LoopContinuation, FollowUpSource::AutoTest] {
            let (pending_dispatch_state, prompt_start) = evaluate_for_follow_up(source);
            let ack_outcome = clankers_core::reduce(
                &pending_dispatch_state,
                &CoreInput::FollowUpDispatchAcknowledged(FollowUpDispatchAcknowledged {
                    effect_id: prompt_start.core_effect_id,
                    dispatch_status: FollowUpDispatchStatus::Accepted,
                }),
            );
            let CoreOutcome::Transitioned {
                next_state: pending_completion_state,
                ..
            } = ack_outcome
            else {
                panic!("accepted follow-up dispatch must transition core state");
            };

            let plan = engine_submission_from_prompt_start(&prompt_start, prior_messages(), test_policy());
            let engine_step = apply_composition_feedback_for_tests(
                CompositionReducer::EngineTurn,
                &pending_completion_state,
                &EngineState::new(),
                CompositionFeedback::Engine(plan.engine_input),
            )
            .expect("accepted follow-up must submit to engine after dispatch ack");
            assert!(matches!(engine_step, CompositionStep::Engine(_)));

            let completion_outcome = clankers_core::reduce(
                &pending_completion_state,
                &CoreInput::LoopFollowUpCompleted(LoopFollowUpCompleted {
                    effect_id: prompt_start.core_effect_id,
                    completion_status: CompletionStatus::Succeeded,
                }),
            );
            let CoreOutcome::Transitioned {
                next_state: completed_follow_up_state,
                ..
            } = completion_outcome
            else {
                panic!("accepted follow-up completion must use LoopFollowUpCompleted");
            };
            assert!(completed_follow_up_state.pending_follow_up_state.is_none());
        }
    }
}
