use clankers_core::CompletionStatus;
use clankers_core::CoreEffect;
use clankers_core::CoreEffectId;
use clankers_core::CoreLogicalEvent;
use clankers_core::CoreThinkingLevel;
use clankers_core::ToolFilterApplied;
use clankers_protocol::DaemonEvent;

use crate::PostPromptAction;
use crate::SessionController;
use crate::loop_mode::LoopConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThinkingEffectExecution {
    pub previous: CoreThinkingLevel,
    pub current: CoreThinkingLevel,
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
                self.emit(DaemonEvent::DisabledToolsChanged {
                    tools: disabled_tools,
                });
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
                CoreEffect::RunLoopFollowUp {
                    effect_id,
                    prompt_text,
                    source,
                } => {
                    post_prompt_action = match source {
                        clankers_core::FollowUpSource::LoopContinuation => PostPromptAction::ContinueLoop {
                            effect_id,
                            prompt: prompt_text,
                        },
                        clankers_core::FollowUpSource::AutoTest => PostPromptAction::RunAutoTest {
                            effect_id,
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

    pub(crate) fn execute_follow_up_completion_effects(
        &mut self,
        effects: Vec<CoreEffect>,
        completion_status: &CompletionStatus,
    ) {
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
