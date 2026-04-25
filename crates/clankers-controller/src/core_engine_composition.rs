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
    use clankers_core::CoreInput;
    use clankers_core::PromptRequest;
    use clankers_engine::EngineEffect;
    use clankers_engine::EngineMessageRole;
    use clankers_engine::EngineState;

    use super::*;

    const TEST_EFFECT_ID: u64 = 42;
    const TEST_IMAGE_COUNT: u32 = 0;
    const TEST_PROMPT: &str = "summarize core ownership";
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
}
