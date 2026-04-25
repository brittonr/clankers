use clankers_core::CompletionStatus;
use clankers_core::CoreEffect;
use clankers_core::CoreEffectId;
use clankers_core::CoreLogicalEvent;
use clankers_core::CoreOutcome;
use clankers_core::CoreThinkingLevel;
use clankers_core::ToolFilterApplied;
use clankers_protocol::DaemonEvent;

use crate::PendingWorkId;
use crate::PostPromptAction;
use crate::SessionController;
use crate::core_engine_composition::AcceptedPromptKind;
use crate::core_engine_composition::AcceptedPromptStart;
use crate::loop_mode::LoopConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThinkingEffectExecution {
    pub previous: CoreThinkingLevel,
    pub current: CoreThinkingLevel,
}

const FOLLOW_UP_IMAGE_COUNT: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoreEffectGateRejection {
    CoreRejected(clankers_core::CoreError),
    MissingEnginePromptEffect,
    ReplayQueuedPromptNeedsFreshCorePrompt,
    UnexpectedCoreEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AcceptedEnginePrompt {
    UserPrompt(AcceptedPromptStart),
    FollowUp(AcceptedPromptStart),
}

impl AcceptedEnginePrompt {
    #[allow(dead_code)] // Handoff wiring lands after the prompt gate helper.
    pub(crate) fn prompt_start(&self) -> &AcceptedPromptStart {
        match self {
            Self::UserPrompt(prompt_start) | Self::FollowUp(prompt_start) => prompt_start,
        }
    }
}

pub(crate) fn accepted_engine_prompt_from_core_outcome(
    core_outcome: &CoreOutcome,
) -> Result<AcceptedEnginePrompt, CoreEffectGateRejection> {
    let effects = match core_outcome {
        CoreOutcome::Transitioned { effects, .. } => effects,
        CoreOutcome::Rejected { error, .. } => {
            return Err(CoreEffectGateRejection::CoreRejected(error.clone()));
        }
    };

    let mut replay_queued_prompt = false;
    let mut accepted_prompt = None;
    for effect in effects {
        let candidate = match effect {
            CoreEffect::StartPrompt {
                effect_id,
                prompt_text,
                image_count,
            } => Some(AcceptedEnginePrompt::UserPrompt(AcceptedPromptStart {
                core_effect_id: *effect_id,
                kind: AcceptedPromptKind::UserPrompt,
                prompt_text: prompt_text.clone(),
                image_count: *image_count,
            })),
            CoreEffect::RunLoopFollowUp {
                effect_id,
                prompt_text,
                source,
            } => Some(AcceptedEnginePrompt::FollowUp(AcceptedPromptStart {
                core_effect_id: *effect_id,
                kind: AcceptedPromptKind::FollowUp(*source),
                prompt_text: prompt_text.clone(),
                image_count: FOLLOW_UP_IMAGE_COUNT,
            })),
            CoreEffect::ReplayQueuedPrompt => {
                replay_queued_prompt = true;
                None
            }
            CoreEffect::EmitLogicalEvent(_)
            | CoreEffect::ApplyThinkingLevel { .. }
            | CoreEffect::ApplyToolFilter { .. } => None,
        };

        if let Some(candidate) = candidate {
            if accepted_prompt.is_some() {
                return Err(CoreEffectGateRejection::UnexpectedCoreEffect);
            }
            accepted_prompt = Some(candidate);
        }
    }

    match accepted_prompt {
        Some(prompt) => Ok(prompt),
        None if replay_queued_prompt => Err(CoreEffectGateRejection::ReplayQueuedPromptNeedsFreshCorePrompt),
        None => Err(CoreEffectGateRejection::MissingEnginePromptEffect),
    }
}

impl SessionController {
    pub(crate) fn execute_prompt_request_effects(
        &mut self,
        effects: Vec<CoreEffect>,
        requested_text: &str,
        requested_image_count: u32,
    ) -> CoreEffectId {
        let mut prompt_effect_id = None;
        let mut saw_busy_change = false;

        for effect in effects {
            match effect {
                CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: true }) => {
                    saw_busy_change = true;
                }
                CoreEffect::StartPrompt {
                    effect_id,
                    prompt_text,
                    image_count,
                } => {
                    debug_assert_eq!(prompt_text, requested_text);
                    debug_assert_eq!(image_count, requested_image_count);
                    prompt_effect_id = Some(effect_id);
                }
                _ => {}
            }
        }

        debug_assert!(saw_busy_change, "prompt request must emit a busy logical event");
        prompt_effect_id.expect("prompt request must yield a start effect")
    }

    pub(crate) fn execute_prompt_completion_effects(&mut self, effects: Vec<CoreEffect>) {
        let mut saw_busy_change = false;

        for effect in effects {
            match effect {
                CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: false }) => {
                    saw_busy_change = true;
                }
                CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                    active_loop_state: None,
                }) if self.active_loop_id.is_some() => {
                    self.finish_loop("failed (error)");
                }
                _ => {}
            }
        }

        debug_assert!(saw_busy_change, "prompt completion must emit a busy logical event");
    }

    pub(crate) fn execute_thinking_effects(&mut self, effects: Vec<CoreEffect>) -> ThinkingEffectExecution {
        let mut thinking_change = None;

        for effect in effects {
            match effect {
                CoreEffect::ApplyThinkingLevel { level } => {
                    if let Some(agent) = self.agent.as_mut() {
                        agent.apply_controller_thinking_level(Self::provider_thinking_level(level));
                    }
                }
                CoreEffect::EmitLogicalEvent(CoreLogicalEvent::ThinkingLevelChanged { previous, current }) => {
                    thinking_change = Some(ThinkingEffectExecution { previous, current });
                }
                _ => {}
            }
        }

        thinking_change.expect("thinking level change must emit a logical event")
    }

    pub(crate) fn execute_tool_filter_request_effects(&mut self, effects: Vec<CoreEffect>) -> bool {
        let mut saw_apply_tool_filter = false;
        let mut all_feedback_applied = true;

        for effect in effects {
            if let CoreEffect::ApplyToolFilter {
                effect_id,
                disabled_tools,
            } = effect
            {
                saw_apply_tool_filter = true;
                if let Some(rebuilder) = self.tool_rebuilder.as_ref() {
                    let filtered = rebuilder.rebuild_filtered(&disabled_tools);
                    if let Some(agent) = self.agent.as_mut() {
                        agent.apply_core_filtered_tools(filtered);
                    }
                }

                let applied = self.apply_tool_filter_feedback(ToolFilterApplied {
                    effect_id,
                    applied_disabled_tool_set: disabled_tools,
                });
                all_feedback_applied &= applied;
            }
        }

        debug_assert!(saw_apply_tool_filter, "disabled-tools transition must emit ApplyToolFilter");
        all_feedback_applied
    }

    pub(crate) fn execute_tool_filter_feedback_effects(&mut self, effects: Vec<CoreEffect>) {
        for effect in effects {
            if let CoreEffect::EmitLogicalEvent(CoreLogicalEvent::DisabledToolsChanged { disabled_tools }) = effect {
                self.emit(DaemonEvent::DisabledToolsChanged { tools: disabled_tools });
            }
        }
    }

    pub(crate) fn execute_start_loop_effects(&mut self, effects: Vec<CoreEffect>) {
        let mut saw_loop_state_change = false;

        for effect in effects {
            if let CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                active_loop_state: Some(active_loop_state),
            }) = effect
            {
                saw_loop_state_change = true;
                let config = LoopConfig {
                    name: active_loop_state.loop_id,
                    prompt: Some(active_loop_state.prompt_text),
                    max_iterations: active_loop_state.max_iterations,
                    break_text: active_loop_state.break_condition,
                };
                if self.start_loop(config).is_none() {
                    self.core_state.active_loop_state = None;
                }
            }
        }

        debug_assert!(saw_loop_state_change, "start-loop transition must emit LoopStateChanged");
    }

    pub(crate) fn execute_stop_loop_effects(&mut self, effects: Vec<CoreEffect>) {
        let mut saw_loop_state_change = false;

        for effect in effects {
            if let CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                active_loop_state: None,
            }) = effect
            {
                saw_loop_state_change = true;
                self.stop_loop();
            }
        }

        debug_assert!(saw_loop_state_change, "stop-loop transition must emit LoopStateChanged");
    }

    pub(crate) fn execute_post_prompt_effects(
        &mut self,
        effects: Vec<CoreEffect>,
        mut completion_reason: Option<String>,
    ) -> PostPromptAction {
        let mut post_prompt_action = PostPromptAction::None;

        for effect in effects {
            match effect {
                CoreEffect::ReplayQueuedPrompt => {
                    post_prompt_action = PostPromptAction::ReplayQueuedPrompt;
                }
                CoreEffect::RunLoopFollowUp {
                    effect_id,
                    prompt_text,
                    source,
                } => {
                    post_prompt_action = match source {
                        clankers_core::FollowUpSource::LoopContinuation => PostPromptAction::ContinueLoop {
                            pending_work_id: PendingWorkId::from_core(effect_id),
                            prompt: prompt_text,
                        },
                        clankers_core::FollowUpSource::AutoTest => PostPromptAction::RunAutoTest {
                            pending_work_id: PendingWorkId::from_core(effect_id),
                            prompt: prompt_text,
                        },
                    };
                }
                CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                    active_loop_state: None,
                }) => {
                    if let Some(reason) = completion_reason.take() {
                        self.finish_loop(&reason);
                    }
                }
                _ => {}
            }
        }

        post_prompt_action
    }

    pub(crate) fn execute_follow_up_dispatch_effects(
        &mut self,
        effects: Vec<CoreEffect>,
        dispatch_status: &clankers_core::FollowUpDispatchStatus,
    ) {
        let completion_status = match dispatch_status {
            clankers_core::FollowUpDispatchStatus::Accepted => return,
            clankers_core::FollowUpDispatchStatus::Rejected(failure) => CompletionStatus::Failed(failure.clone()),
        };
        self.execute_follow_up_failure_effects(effects, &completion_status);
    }

    pub(crate) fn execute_follow_up_completion_effects(
        &mut self,
        effects: Vec<CoreEffect>,
        completion_status: &CompletionStatus,
    ) {
        self.execute_follow_up_failure_effects(effects, completion_status);
    }

    fn execute_follow_up_failure_effects(&mut self, effects: Vec<CoreEffect>, completion_status: &CompletionStatus) {
        let mut loop_finished = false;

        for effect in effects {
            if let CoreEffect::EmitLogicalEvent(CoreLogicalEvent::LoopStateChanged {
                active_loop_state: None,
            }) = effect
            {
                loop_finished = true;
            }
        }

        if loop_finished {
            self.finish_loop("failed (follow-up)");
            return;
        }

        if matches!(completion_status, CompletionStatus::Failed(_)) {
            self.emit(DaemonEvent::SystemMessage {
                text: "Post-prompt follow-up failed".to_string(),
                is_error: true,
            });
        }
    }
}

#[cfg(test)]
mod accepted_engine_prompt_tests {
    use clankers_core::CoreError;
    use clankers_core::CoreInput;
    use clankers_core::CoreState;
    use clankers_core::FollowUpSource;
    use clankers_core::PostPromptEvaluation;
    use clankers_core::PromptRequest;

    use super::*;

    const TEST_IMAGE_COUNT: u32 = 2;
    const TEST_PROMPT: &str = "compose accepted prompt";
    const FOLLOW_UP_PROMPT: &str = "continue loop";

    fn accepted_prompt_outcome() -> CoreOutcome {
        clankers_core::reduce(
            &CoreState::default(),
            &CoreInput::PromptRequested(PromptRequest {
                text: TEST_PROMPT.to_owned(),
                image_count: TEST_IMAGE_COUNT,
                originating_follow_up_effect_id: None,
            }),
        )
    }

    #[test]
    fn accepted_engine_prompt_normalizes_start_prompt() {
        let accepted = accepted_engine_prompt_from_core_outcome(&accepted_prompt_outcome())
            .expect("prompt request must normalize to engine prompt");

        match accepted {
            AcceptedEnginePrompt::UserPrompt(prompt_start) => {
                assert_eq!(prompt_start.kind, AcceptedPromptKind::UserPrompt);
                assert_eq!(prompt_start.prompt_text, TEST_PROMPT);
                assert_eq!(prompt_start.image_count, TEST_IMAGE_COUNT);
            }
            AcceptedEnginePrompt::FollowUp(_) => panic!("expected user prompt normalization"),
        }
    }

    #[test]
    fn accepted_engine_prompt_normalizes_loop_follow_up() {
        let outcome = CoreOutcome::Transitioned {
            next_state: CoreState::default(),
            effects: vec![CoreEffect::RunLoopFollowUp {
                effect_id: CoreEffectId(1),
                prompt_text: FOLLOW_UP_PROMPT.to_owned(),
                source: FollowUpSource::LoopContinuation,
            }],
        };

        let accepted =
            accepted_engine_prompt_from_core_outcome(&outcome).expect("loop follow-up must normalize to engine prompt");

        match accepted {
            AcceptedEnginePrompt::FollowUp(prompt_start) => {
                assert_eq!(prompt_start.kind, AcceptedPromptKind::FollowUp(FollowUpSource::LoopContinuation));
                assert_eq!(prompt_start.prompt_text, FOLLOW_UP_PROMPT);
                assert_eq!(prompt_start.image_count, FOLLOW_UP_IMAGE_COUNT);
            }
            AcceptedEnginePrompt::UserPrompt(_) => panic!("expected follow-up normalization"),
        }
    }

    #[test]
    fn accepted_engine_prompt_rejects_core_rejection() {
        let mut busy = CoreState::default();
        busy.busy = true;
        let outcome = clankers_core::reduce(
            &busy,
            &CoreInput::PromptRequested(PromptRequest {
                text: TEST_PROMPT.to_owned(),
                image_count: TEST_IMAGE_COUNT,
                originating_follow_up_effect_id: None,
            }),
        );

        let rejection = accepted_engine_prompt_from_core_outcome(&outcome)
            .expect_err("busy core rejection must not create engine input");

        assert_eq!(rejection, CoreEffectGateRejection::CoreRejected(CoreError::Busy));
    }

    #[test]
    fn accepted_engine_prompt_rejects_replay_without_fresh_core_prompt() {
        let outcome = CoreOutcome::Transitioned {
            next_state: CoreState::default(),
            effects: vec![CoreEffect::ReplayQueuedPrompt],
        };

        let rejection = accepted_engine_prompt_from_core_outcome(&outcome)
            .expect_err("queued replay must be resubmitted through core prompt request");

        assert_eq!(rejection, CoreEffectGateRejection::ReplayQueuedPromptNeedsFreshCorePrompt);
    }

    #[test]
    fn accepted_engine_prompt_rejects_missing_prompt_effect() {
        let outcome = clankers_core::reduce(
            &CoreState::default(),
            &CoreInput::EvaluatePostPrompt(PostPromptEvaluation {
                active_loop_state: None,
                pending_follow_up_state: None,
                auto_test_enabled: false,
                auto_test_command: None,
                auto_test_in_progress: false,
                queued_prompt_present: false,
            }),
        );

        let rejection = accepted_engine_prompt_from_core_outcome(&outcome)
            .expect_err("non-prompt core transition must not create engine input");

        assert_eq!(rejection, CoreEffectGateRejection::MissingEnginePromptEffect);
    }

    #[test]
    fn accepted_engine_prompt_rejects_multiple_submittable_effects() {
        let outcome = CoreOutcome::Transitioned {
            next_state: CoreState::default(),
            effects: vec![
                CoreEffect::StartPrompt {
                    effect_id: CoreEffectId(1),
                    prompt_text: TEST_PROMPT.to_owned(),
                    image_count: TEST_IMAGE_COUNT,
                },
                CoreEffect::RunLoopFollowUp {
                    effect_id: CoreEffectId(2),
                    prompt_text: FOLLOW_UP_PROMPT.to_owned(),
                    source: FollowUpSource::AutoTest,
                },
            ],
        };

        let rejection = accepted_engine_prompt_from_core_outcome(&outcome)
            .expect_err("multiple prompt effects must be impossible at the gate");

        assert_eq!(rejection, CoreEffectGateRejection::UnexpectedCoreEffect);
    }
}
