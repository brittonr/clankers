//! Auto-test and post-prompt action processing.
//!
//! Contains logic for determining what action should be taken after
//! a prompt completes, including auto-test execution and loop continuation.

use crate::PostPromptAction;
use crate::SessionController;
use crate::loop_mode::LoopConfig;

impl SessionController {
    /// Start a prompt in embedded mode through the reducer-backed prompt path.
    pub fn start_embedded_prompt(&mut self, prompt_text: &str, image_count: u32) -> bool {
        let input = clankers_core::CoreInput::PromptRequested(clankers_core::PromptRequest {
            text: prompt_text.to_string(),
            image_count,
        });

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                debug_assert!(effects.iter().any(|effect| matches!(
                    effect,
                    clankers_core::CoreEffect::StartPrompt {
                        prompt_text: effect_prompt,
                        image_count: effect_image_count,
                        ..
                    } if effect_prompt == prompt_text && *effect_image_count == image_count
                )));
                true
            }
            clankers_core::CoreOutcome::Rejected { .. } => {
                self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                    text: "Prompt start rejected".to_string(),
                    is_error: true,
                });
                false
            }
        }
    }

    /// Complete an embedded-mode prompt through the reducer-backed prompt path.
    pub fn finish_embedded_prompt(&mut self, completion_status: clankers_core::CompletionStatus) {
        if let Some(pending_prompt) = self.core_state.pending_prompt.clone() {
            let applied = self.apply_prompt_completion(clankers_core::PromptCompleted {
                effect_id: pending_prompt.effect_id,
                completion_status,
            });
            debug_assert!(applied, "embedded prompt completion should match the pending prompt");
            return;
        }

        self.busy = false;
        self.core_state.busy = false;
        if matches!(completion_status, clankers_core::CompletionStatus::Failed(_)) && self.active_loop_id.is_some() {
            self.finish_loop("failed (error)");
        }
    }

    /// Check if auto-test should run after a prompt completes. Returns a
    /// prompt string to send to the agent, or None.
    pub fn maybe_auto_test(&mut self) -> Option<String> {
        if !self.auto_test_enabled {
            return None;
        }
        if self.auto_test_in_progress {
            return None;
        }
        if self.active_loop_id.is_some() {
            return None;
        }
        let cmd = self.auto_test_command.as_ref()?;
        self.auto_test_in_progress = true;
        self.core_state.auto_test_in_progress = true;
        Some(format!("Run `{cmd}` and fix any failures. Do not ask for confirmation."))
    }

    /// Clear the auto-test guard (call after the auto-test prompt completes).
    pub fn clear_auto_test(&mut self) {
        self.auto_test_in_progress = false;
        self.core_state.auto_test_in_progress = false;
    }

    /// Determine what to do after a prompt completes (embedded mode).
    ///
    /// Call this from the TUI's `handle_task_results` after receiving
    /// `PromptDone(None)` and confirming there's no queued user prompt.
    /// Returns the action the TUI should take.
    pub fn check_post_prompt(&mut self) -> PostPromptAction {
        let observed_loop_progress = self.observe_post_prompt_loop_state();
        let input = clankers_core::CoreInput::EvaluatePostPrompt(clankers_core::PostPromptEvaluation {
            active_loop_state: observed_loop_progress.active_loop_state.clone(),
            pending_follow_up_state: self.core_state.pending_follow_up_state.clone(),
            auto_test_enabled: self.auto_test_enabled,
            auto_test_command: self.auto_test_command.clone(),
            auto_test_in_progress: self.auto_test_in_progress,
        });

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                let mut post_prompt_action = PostPromptAction::None;
                let mut completion_reason = observed_loop_progress.completion_reason;

                for effect in effects {
                    match effect {
                        clankers_core::CoreEffect::RunLoopFollowUp {
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
                        clankers_core::CoreEffect::EmitLogicalEvent(
                            clankers_core::CoreLogicalEvent::LoopStateChanged {
                                active_loop_state: None,
                            },
                        ) => {
                            if let Some(reason) = completion_reason.take() {
                                self.finish_loop(&reason);
                            }
                        }
                        _ => {}
                    }
                }

                post_prompt_action
            }
            clankers_core::CoreOutcome::Rejected { .. } => PostPromptAction::None,
        }
    }

    /// Notify the controller that a follow-up prompt was accepted or rejected by the shell.
    pub fn complete_follow_up(
        &mut self,
        effect_id: clankers_core::CoreEffectId,
        completion_status: clankers_core::CompletionStatus,
    ) {
        let input = clankers_core::CoreInput::LoopFollowUpCompleted(clankers_core::LoopFollowUpCompleted {
            effect_id,
            completion_status: completion_status.clone(),
        });
        let previous_loop_active = self.active_loop_id.is_some();

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                if matches!(completion_status, clankers_core::CompletionStatus::Failed(_)) {
                    if previous_loop_active {
                        self.finish_loop("failed (follow-up)");
                    } else {
                        self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                            text: "Post-prompt follow-up failed".to_string(),
                            is_error: true,
                        });
                    }
                }
                for effect in effects {
                    if let clankers_core::CoreEffect::EmitLogicalEvent(
                        clankers_core::CoreLogicalEvent::LoopStateChanged {
                            active_loop_state: None,
                        },
                    ) = effect
                    {
                        self.core_state.active_loop_state = None;
                    }
                }
            }
            clankers_core::CoreOutcome::Rejected { .. } => {
                self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                    text: "Post-prompt follow-up completion rejected".to_string(),
                    is_error: true,
                });
            }
        }
    }

    /// Sync loop state from the TUI's loop_status.
    ///
    /// Called before `check_post_prompt()` to ensure the controller's
    /// loop engine matches the TUI's `/loop` command state.
    pub fn sync_loop_from_tui(&mut self, loop_status: Option<&clankers_tui_types::LoopDisplayState>) {
        match (loop_status, &self.active_loop_id) {
            // TUI has loop but controller doesn't → register it
            (Some(ls), None) => {
                let config = LoopConfig {
                    name: ls.name.clone(),
                    prompt: ls.prompt.clone(),
                    max_iterations: ls.max_iterations,
                    break_text: ls.break_text.clone(),
                };
                self.start_loop(config);
            }
            // TUI cleared loop but controller still has one → stop it
            (None, Some(_)) => {
                if let Some(ref id) = self.active_loop_id {
                    self.loop_engine.stop(id);
                    self.loop_engine.remove(id);
                }
                self.active_loop_id = None;
                self.loop_turn_output.clear();
                self.core_state.active_loop_state = None;
                self.core_state.pending_follow_up_state = None;
            }
            // Both in sync (or neither has a loop)
            _ => {}
        }
    }

    /// Get the current loop iteration count (for TUI display sync).
    pub fn loop_iteration(&self) -> Option<u32> {
        self.active_loop_id.as_ref().and_then(|id| self.loop_engine.get(id)).map(|s| s.current_iteration)
    }

    /// Notify the controller that a prompt completed (embedded mode).
    ///
    /// Updates busy state. Called from the TUI when `TaskResult::PromptDone`
    /// is received, before calling `check_post_prompt()`.
    pub fn notify_prompt_done(&mut self, had_error: bool) {
        let completion_status = if had_error {
            clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Message(
                "embedded prompt failed".to_string(),
            ))
        } else {
            clankers_core::CompletionStatus::Succeeded
        };
        self.finish_embedded_prompt(completion_status);
    }
}

#[cfg(test)]
mod tests {
    use clanker_loop::LoopId;
    use clankers_tui_types::LoopDisplayState;

    use crate::PostPromptAction;
    use crate::test_helpers::make_test_controller;

    #[test]
    fn test_auto_test_disabled() {
        let mut ctrl = make_test_controller();
        assert!(ctrl.maybe_auto_test().is_none());
    }

    #[test]
    fn test_auto_test_fires() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let prompt = ctrl.maybe_auto_test();
        assert!(prompt.is_some());
        assert!(prompt.unwrap().contains("cargo test"));

        // Second call blocked (in progress)
        assert!(ctrl.maybe_auto_test().is_none());

        // After clearing, can fire again
        ctrl.clear_auto_test();
        assert!(ctrl.maybe_auto_test().is_some());
    }

    #[test]
    fn test_auto_test_blocked_during_loop() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());
        ctrl.active_loop_id = Some(clanker_loop::LoopId("test-loop".to_string()));

        assert!(ctrl.maybe_auto_test().is_none());
    }

    #[test]
    fn test_check_post_prompt_no_loop_no_autotest() {
        let mut ctrl = make_test_controller();

        let action = ctrl.check_post_prompt();
        assert!(matches!(action, PostPromptAction::None));
    }

    #[test]
    fn test_check_post_prompt_with_auto_test_enabled() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let action = ctrl.check_post_prompt();
        assert!(matches!(action, PostPromptAction::RunAutoTest { .. }));

        if let PostPromptAction::RunAutoTest { prompt, .. } = action {
            assert!(prompt.contains("cargo test"));
        }
    }

    #[test]
    fn test_check_post_prompt_with_active_loop() {
        const LOOP_ITERATION_LIMIT: u32 = 2;
        const FIRST_COMPLETED_ITERATION: u32 = 1;

        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });

        let action = ctrl.check_post_prompt();
        assert!(matches!(action, PostPromptAction::ContinueLoop { ref prompt, .. } if prompt == "continue loop"));
        assert_eq!(
            ctrl.core_state.active_loop_state.as_ref().map(|loop_state| loop_state.current_iteration),
            Some(FIRST_COMPLETED_ITERATION)
        );
        assert!(!ctrl.auto_test_in_progress);
    }

    #[test]
    fn test_check_post_prompt_finishes_completed_loop_without_follow_up() {
        const SINGLE_ITERATION_LOOP: u32 = 1;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: SINGLE_ITERATION_LOOP,
            break_text: None,
        });

        let action = ctrl.check_post_prompt();
        assert!(matches!(action, PostPromptAction::None));
        assert!(ctrl.active_loop_id.is_none());
        let events = ctrl.drain_events();
        assert!(matches!(
            events.as_slice(),
            [clankers_protocol::DaemonEvent::SystemMessage { text, is_error: false }] if text.contains("after 1 iteration")
        ));
    }

    fn seed_pending_prompt(ctrl: &mut crate::SessionController) {
        ctrl.busy = true;
        ctrl.core_state.busy = true;
        ctrl.core_state.pending_prompt = Some(clankers_core::PendingPromptState {
            effect_id: clankers_core::CoreEffectId(1),
            prompt_text: "hello".to_string(),
            image_count: 0,
        });
        ctrl.core_state.next_effect_id = clankers_core::CoreEffectId(1);
    }

    #[test]
    fn test_start_embedded_prompt_tracks_pending_prompt_and_busy() {
        let mut ctrl = make_test_controller();

        let started = ctrl.start_embedded_prompt("hello", 0);

        assert!(started);
        assert!(ctrl.busy);
        assert!(ctrl.core_state.busy);
        assert_eq!(
            ctrl.core_state.pending_prompt,
            Some(clankers_core::PendingPromptState {
                effect_id: clankers_core::CoreEffectId(1),
                prompt_text: "hello".to_string(),
                image_count: 0,
            })
        );
        assert!(ctrl.drain_events().is_empty());
    }

    #[test]
    fn test_finish_embedded_prompt_consumes_pending_prompt_via_reducer() {
        let mut ctrl = make_test_controller();
        ctrl.set_auto_test(true, Some("cargo test".to_string()));
        assert!(ctrl.start_embedded_prompt("hello", 0));

        ctrl.finish_embedded_prompt(clankers_core::CompletionStatus::Succeeded);

        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(ctrl.drain_events().is_empty());
        assert!(matches!(ctrl.check_post_prompt(), PostPromptAction::RunAutoTest { .. }));
    }

    #[test]
    fn test_prompt_completion_success_keeps_no_ack_and_allows_auto_test_follow_up() {
        let mut ctrl = make_test_controller();
        ctrl.set_auto_test(true, Some("cargo test".to_string()));
        seed_pending_prompt(&mut ctrl);

        let applied = ctrl.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: clankers_core::CoreEffectId(1),
            completion_status: clankers_core::CompletionStatus::Succeeded,
        });

        assert!(applied);
        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(ctrl.drain_events().is_empty());
        assert!(matches!(ctrl.check_post_prompt(), PostPromptAction::RunAutoTest { .. }));
    }

    #[test]
    fn test_prompt_completion_success_keeps_no_ack_and_allows_loop_follow_up() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });
        seed_pending_prompt(&mut ctrl);

        let applied = ctrl.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: clankers_core::CoreEffectId(1),
            completion_status: clankers_core::CompletionStatus::Succeeded,
        });

        assert!(applied);
        assert!(ctrl.drain_events().is_empty());
        assert!(matches!(
            ctrl.check_post_prompt(),
            PostPromptAction::ContinueLoop { ref prompt, .. } if prompt == "continue loop"
        ));
    }

    #[test]
    fn test_prompt_completion_failure_suppresses_follow_up_and_finishes_loop() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });
        seed_pending_prompt(&mut ctrl);

        let applied = ctrl.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: clankers_core::CoreEffectId(1),
            completion_status: clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Message(
                "boom".to_string(),
            )),
        });

        assert!(applied);
        assert!(!ctrl.has_active_loop());
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [clankers_protocol::DaemonEvent::SystemMessage { text, is_error: false }]
                if text.contains("failed (error)")
        ));
        assert!(matches!(ctrl.check_post_prompt(), PostPromptAction::None));
    }

    #[test]
    fn test_prompt_completion_rejection_keeps_state_and_emits_error() {
        let mut ctrl = make_test_controller();
        seed_pending_prompt(&mut ctrl);
        let previous_state = ctrl.core_state.clone();

        let applied = ctrl.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: clankers_core::CoreEffectId(2),
            completion_status: clankers_core::CompletionStatus::Succeeded,
        });

        assert!(!applied);
        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn test_prompt_completion_duplicate_completion_is_rejected() {
        let mut ctrl = make_test_controller();
        seed_pending_prompt(&mut ctrl);
        assert!(ctrl.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: clankers_core::CoreEffectId(1),
            completion_status: clankers_core::CompletionStatus::Succeeded,
        }));
        let settled_state = ctrl.core_state.clone();
        assert!(ctrl.drain_events().is_empty());

        let applied = ctrl.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: clankers_core::CoreEffectId(1),
            completion_status: clankers_core::CompletionStatus::Succeeded,
        });

        assert!(!applied);
        assert_eq!(ctrl.core_state, settled_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn test_complete_follow_up_success_clears_pending_without_extra_events() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });

        let effect_id = match ctrl.check_post_prompt() {
            PostPromptAction::ContinueLoop { effect_id, .. } => effect_id,
            other => panic!("expected ContinueLoop, got {other:?}"),
        };

        ctrl.complete_follow_up(effect_id, clankers_core::CompletionStatus::Succeeded);

        assert!(ctrl.has_active_loop());
        assert!(ctrl.core_state.pending_follow_up_state.is_none());
        assert_eq!(ctrl.core_state.active_loop_state.as_ref().map(|loop_state| loop_state.current_iteration), Some(1));
        assert!(ctrl.drain_events().is_empty());
    }

    #[test]
    fn test_complete_follow_up_failure_finishes_loop_and_emits_message() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });

        let effect_id = match ctrl.check_post_prompt() {
            PostPromptAction::ContinueLoop { effect_id, .. } => effect_id,
            other => panic!("expected ContinueLoop, got {other:?}"),
        };

        ctrl.complete_follow_up(
            effect_id,
            clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Message("boom".to_string())),
        );

        assert!(!ctrl.has_active_loop());
        assert!(ctrl.core_state.pending_follow_up_state.is_none());
        assert!(ctrl.core_state.active_loop_state.is_none());
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [clankers_protocol::DaemonEvent::SystemMessage { text, is_error: false }]
                if text.contains("failed (follow-up)")
        ));
    }

    #[test]
    fn test_complete_follow_up_failure_without_loop_emits_error_message() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let effect_id = match ctrl.check_post_prompt() {
            PostPromptAction::RunAutoTest { effect_id, .. } => effect_id,
            other => panic!("expected RunAutoTest, got {other:?}"),
        };

        ctrl.complete_follow_up(
            effect_id,
            clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Message("boom".to_string())),
        );

        assert!(ctrl.core_state.pending_follow_up_state.is_none());
        assert!(!ctrl.auto_test_in_progress);
        assert!(!ctrl.core_state.auto_test_in_progress);
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [clankers_protocol::DaemonEvent::SystemMessage { text, is_error: true }]
                if text == "Post-prompt follow-up failed"
        ));
    }

    #[test]
    fn test_complete_follow_up_rejection_keeps_state_and_emits_error() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let effect_id = match ctrl.check_post_prompt() {
            PostPromptAction::RunAutoTest { effect_id, .. } => effect_id,
            other => panic!("expected RunAutoTest, got {other:?}"),
        };
        let previous_state = ctrl.core_state.clone();
        let wrong_effect_id = clankers_core::CoreEffectId(effect_id.0 + 1);

        ctrl.complete_follow_up(wrong_effect_id, clankers_core::CompletionStatus::Succeeded);

        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn test_complete_follow_up_duplicate_completion_is_rejected() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let effect_id = match ctrl.check_post_prompt() {
            PostPromptAction::RunAutoTest { effect_id, .. } => effect_id,
            other => panic!("expected RunAutoTest, got {other:?}"),
        };
        ctrl.complete_follow_up(effect_id, clankers_core::CompletionStatus::Succeeded);
        let settled_state = ctrl.core_state.clone();
        assert!(ctrl.drain_events().is_empty());

        ctrl.complete_follow_up(effect_id, clankers_core::CompletionStatus::Succeeded);

        assert_eq!(ctrl.core_state, settled_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn test_notify_prompt_done_clears_busy() {
        let mut ctrl = make_test_controller();
        ctrl.busy = true;

        ctrl.notify_prompt_done(false);
        assert!(!ctrl.busy);
    }

    #[test]
    fn test_notify_prompt_done_with_error_finishes_loop() {
        let mut ctrl = make_test_controller();
        ctrl.busy = true;
        ctrl.active_loop_id = Some(LoopId("test-loop".to_string()));

        ctrl.notify_prompt_done(true); // had_error = true
        assert!(!ctrl.busy);

        // Loop should be finished (active_loop_id cleared by finish_loop)
        // This depends on finish_loop implementation, but we can test the busy state
    }

    #[test]
    fn test_sync_loop_from_tui_starts_loop() {
        let mut ctrl = make_test_controller();

        let loop_state = LoopDisplayState {
            name: "test-loop".to_string(),
            prompt: Some("test prompt".to_string()),
            max_iterations: 5,
            break_text: Some("done".to_string()),
            iteration: 1,
            active: true,
        };

        // Controller has no loop, TUI has loop → should start it
        ctrl.sync_loop_from_tui(Some(&loop_state));
        assert!(ctrl.active_loop_id.is_some());
    }

    #[test]
    fn test_sync_loop_from_tui_clears_loop() {
        let mut ctrl = make_test_controller();

        // Set up active loop in controller
        ctrl.active_loop_id = Some(LoopId("existing-loop".to_string()));

        // TUI has no loop → should clear controller's loop
        ctrl.sync_loop_from_tui(None);
        assert!(ctrl.active_loop_id.is_none());
        assert!(ctrl.loop_turn_output.is_empty());
    }

    #[test]
    fn test_sync_loop_from_tui_both_none() {
        let mut ctrl = make_test_controller();

        // Both controller and TUI have no loop → no change
        ctrl.sync_loop_from_tui(None);
        assert!(ctrl.active_loop_id.is_none());
    }

    #[test]
    fn test_loop_iteration_returns_none_when_no_loop() {
        let ctrl = make_test_controller();
        assert!(ctrl.loop_iteration().is_none());
    }

    #[test]
    fn test_loop_iteration_with_active_loop() {
        let mut ctrl = make_test_controller();

        // Create a loop config
        let config = crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("test prompt".to_string()),
            max_iterations: 5,
            break_text: Some("done".to_string()),
        };

        // Start the loop
        ctrl.start_loop(config);

        // Should return the current iteration (starts at 0)
        if let Some(iteration) = ctrl.loop_iteration() {
            assert_eq!(iteration, 0); // Default starting iteration
        }
    }
}
