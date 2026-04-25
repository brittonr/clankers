//! Auto-test and post-prompt action processing.
//!
//! Contains logic for determining what action should be taken after
//! a prompt completes, including auto-test execution and loop continuation.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use crate::PendingWorkId;
use crate::PostPromptAction;
use crate::SessionController;
use crate::ShellFollowUpDispatch;
use crate::ShellPromptCompletion;
use crate::loop_mode::LoopConfig;

impl SessionController {
    /// Start a prompt in embedded mode through the reducer-backed prompt path.
    pub fn start_embedded_prompt(&mut self, prompt_text: &str, image_count: u32) -> bool {
        self.start_embedded_prompt_with_follow_up(prompt_text, image_count, None)
    }

    pub fn start_embedded_prompt_with_follow_up(
        &mut self,
        prompt_text: &str,
        image_count: u32,
        originating_follow_up_effect_id: Option<PendingWorkId>,
    ) -> bool {
        let input = clankers_core::CoreInput::PromptRequested(clankers_core::PromptRequest {
            text: prompt_text.to_string(),
            image_count,
            originating_follow_up_effect_id: originating_follow_up_effect_id.map(PendingWorkId::into_core),
        });

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                let _accepted_prompt = self.execute_prompt_request_effects(effects, prompt_text, image_count);
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
    pub fn finish_embedded_prompt(&mut self, completion_status: ShellPromptCompletion) {
        let Some(pending_prompt) = self.core_state.pending_prompt.clone() else {
            self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                text: "Embedded prompt completion rejected: no pending prompt".to_string(),
                is_error: true,
            });
            return;
        };
        debug_assert!(
            pending_prompt.originating_follow_up_effect_id.is_none(),
            "follow-up prompt completion must use complete_dispatched_follow_up"
        );

        let applied = self.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: pending_prompt.effect_id,
            completion_status: completion_status.to_core(),
        });
        debug_assert!(applied, "embedded prompt completion should match the pending prompt");
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
    pub fn check_post_prompt(&mut self, queued_prompt_present: bool) -> PostPromptAction {
        let observed_loop_progress = self.observe_post_prompt_loop_state();
        let input = clankers_core::CoreInput::EvaluatePostPrompt(clankers_core::PostPromptEvaluation {
            active_loop_state: observed_loop_progress.active_loop_state.clone(),
            pending_follow_up_state: self.core_state.pending_follow_up_state.clone(),
            auto_test_enabled: self.auto_test_enabled,
            auto_test_command: self.auto_test_command.clone(),
            auto_test_in_progress: self.auto_test_in_progress,
            queued_prompt_present,
        });

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_post_prompt_effects(effects, observed_loop_progress.completion_reason)
            }
            clankers_core::CoreOutcome::Rejected { .. } => PostPromptAction::None,
        }
    }

    pub fn pending_dispatched_follow_up_id(&self) -> Option<PendingWorkId> {
        self.core_state
            .pending_prompt
            .as_ref()
            .and_then(|pending_prompt| pending_prompt.originating_follow_up_effect_id)
            .map(PendingWorkId::from_core)
    }

    /// Notify the controller whether follow-up prompt dispatch was accepted or rejected by the
    /// shell.
    pub fn ack_follow_up_dispatch(&mut self, pending_work_id: PendingWorkId, dispatch_status: ShellFollowUpDispatch) {
        let core_dispatch_status = dispatch_status.to_core();
        let input =
            clankers_core::CoreInput::FollowUpDispatchAcknowledged(clankers_core::FollowUpDispatchAcknowledged {
                effect_id: pending_work_id.into_core(),
                dispatch_status: core_dispatch_status.clone(),
            });

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_follow_up_dispatch_effects(effects, &core_dispatch_status);
            }
            clankers_core::CoreOutcome::Rejected { .. } => {
                self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                    text: "Post-prompt follow-up dispatch rejected".to_string(),
                    is_error: true,
                });
            }
        }
    }

    /// Notify the controller that a dispatched follow-up prompt finished.
    pub fn complete_dispatched_follow_up(
        &mut self,
        pending_work_id: PendingWorkId,
        completion_status: ShellPromptCompletion,
    ) {
        let core_effect_id = pending_work_id.into_core();
        let core_completion_status = completion_status.to_core();
        let input = clankers_core::CoreInput::LoopFollowUpCompleted(clankers_core::LoopFollowUpCompleted {
            effect_id: core_effect_id,
            completion_status: core_completion_status.clone(),
        });
        let preflight = clankers_core::reduce(&self.core_state, &input);
        let (follow_up_next_state, follow_up_effects) = match preflight {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => (next_state, effects),
            clankers_core::CoreOutcome::Rejected { .. } => {
                self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                    text: "Post-prompt follow-up completion rejected".to_string(),
                    is_error: true,
                });
                return;
            }
        };

        let Some(pending_prompt) = self.core_state.pending_prompt.clone() else {
            if matches!(
                core_completion_status,
                clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Cancelled)
            ) {
                self.apply_core_state(follow_up_next_state);
                self.execute_follow_up_completion_effects(follow_up_effects, &core_completion_status);
                return;
            }
            self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                text: "Post-prompt follow-up completion rejected".to_string(),
                is_error: true,
            });
            return;
        };
        if pending_prompt.originating_follow_up_effect_id != Some(core_effect_id) {
            self.emit(clankers_protocol::DaemonEvent::SystemMessage {
                text: "Post-prompt follow-up completion rejected".to_string(),
                is_error: true,
            });
            return;
        }

        let prompt_applied = self.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: pending_prompt.effect_id,
            completion_status: core_completion_status.clone(),
        });
        if !prompt_applied {
            return;
        }

        match clankers_core::reduce(&self.core_state, &input) {
            clankers_core::CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_follow_up_completion_effects(effects, &core_completion_status);
            }
            clankers_core::CoreOutcome::Rejected { .. } => {
                unreachable!("preflight already rejected invalid follow-up completion")
            }
        }
    }

    /// Sync loop state from the TUI's loop_status.
    ///
    /// Called before `check_post_prompt(false)` to ensure the controller's
    /// loop engine matches the TUI's `/loop` command state.
    pub fn sync_loop_from_tui(&mut self, loop_status: Option<&clanker_tui_types::LoopDisplayState>) {
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
    /// is received, before calling `check_post_prompt(false)`.
    pub fn notify_prompt_done(&mut self, had_error: bool) {
        let completion_status = if had_error {
            ShellPromptCompletion::failed("embedded prompt failed")
        } else {
            ShellPromptCompletion::Succeeded
        };
        self.finish_embedded_prompt(completion_status);
    }
}

#[cfg(test)]
mod tests {
    use clanker_loop::LoopId;
    use clanker_tui_types::LoopDisplayState;

    use crate::PendingWorkId;
    use crate::PostPromptAction;
    use crate::ShellFollowUpDispatch;
    use crate::ShellPromptCompletion;
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

        let action = ctrl.check_post_prompt(false);
        assert!(matches!(action, PostPromptAction::None));
    }

    #[test]
    fn test_check_post_prompt_with_auto_test_enabled() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let action = ctrl.check_post_prompt(false);
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

        let action = ctrl.check_post_prompt(false);
        assert!(matches!(action, PostPromptAction::ContinueLoop { ref prompt, .. } if prompt == "continue loop"));
        assert_eq!(
            ctrl.core_state.active_loop_state.as_ref().map(|loop_state| loop_state.current_iteration),
            Some(FIRST_COMPLETED_ITERATION)
        );
        assert!(!ctrl.auto_test_in_progress);
    }

    #[test]
    fn test_check_post_prompt_prefers_loop_over_auto_test_when_both_are_available() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: 2,
            break_text: None,
        });

        let action = ctrl.check_post_prompt(false);

        assert!(matches!(action, PostPromptAction::ContinueLoop { .. }));
        assert!(!matches!(action, PostPromptAction::RunAutoTest { .. }));
        assert!(!ctrl.auto_test_in_progress);
    }

    #[test]
    fn test_check_post_prompt_replays_queued_prompt_before_follow_up() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: 2,
            break_text: None,
        });

        let action = ctrl.check_post_prompt(true);

        assert!(matches!(action, PostPromptAction::ReplayQueuedPrompt));
        assert!(ctrl.core_state.pending_follow_up_state.is_none());
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

        let action = ctrl.check_post_prompt(false);
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
            originating_follow_up_effect_id: None,
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
                originating_follow_up_effect_id: None,
            })
        );
        assert!(ctrl.drain_events().is_empty());
    }

    #[test]
    fn test_finish_embedded_prompt_consumes_pending_prompt_via_reducer() {
        let mut ctrl = make_test_controller();
        ctrl.set_auto_test(true, Some("cargo test".to_string()));
        assert!(ctrl.start_embedded_prompt("hello", 0));

        ctrl.finish_embedded_prompt(ShellPromptCompletion::Succeeded);

        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(ctrl.drain_events().is_empty());
        assert!(matches!(ctrl.check_post_prompt(false), PostPromptAction::RunAutoTest { .. }));
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
        assert!(matches!(ctrl.check_post_prompt(false), PostPromptAction::RunAutoTest { .. }));
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
            ctrl.check_post_prompt(false),
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
        assert!(matches!(ctrl.check_post_prompt(false), PostPromptAction::None));
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
    fn test_ack_follow_up_dispatch_and_completion_success_clear_pending_without_extra_events() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });

        let (effect_id, prompt) = match ctrl.check_post_prompt(false) {
            PostPromptAction::ContinueLoop {
                pending_work_id,
                prompt,
            } => (pending_work_id, prompt),
            other => panic!("expected ContinueLoop, got {other:?}"),
        };

        ctrl.ack_follow_up_dispatch(effect_id, ShellFollowUpDispatch::Accepted);
        assert!(ctrl.start_embedded_prompt_with_follow_up(&prompt, 0, Some(effect_id)));
        ctrl.complete_dispatched_follow_up(effect_id, ShellPromptCompletion::Succeeded);

        assert!(ctrl.has_active_loop());
        assert!(ctrl.core_state.pending_follow_up_state.is_none());
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert_eq!(ctrl.core_state.active_loop_state.as_ref().map(|loop_state| loop_state.current_iteration), Some(1));
        assert!(ctrl.drain_events().is_empty());
    }

    #[test]
    fn test_follow_up_dispatch_rejection_finishes_loop_and_emits_message() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });

        let effect_id = match ctrl.check_post_prompt(false) {
            PostPromptAction::ContinueLoop { pending_work_id, .. } => pending_work_id,
            other => panic!("expected ContinueLoop, got {other:?}"),
        };

        ctrl.ack_follow_up_dispatch(effect_id, ShellFollowUpDispatch::rejected("boom"));

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
    fn test_dispatched_follow_up_failure_without_loop_emits_error_message() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let (effect_id, prompt) = match ctrl.check_post_prompt(false) {
            PostPromptAction::RunAutoTest {
                pending_work_id,
                prompt,
            } => (pending_work_id, prompt),
            other => panic!("expected RunAutoTest, got {other:?}"),
        };

        ctrl.ack_follow_up_dispatch(effect_id, ShellFollowUpDispatch::Accepted);
        assert!(ctrl.start_embedded_prompt_with_follow_up(&prompt, 0, Some(effect_id)));
        ctrl.complete_dispatched_follow_up(effect_id, ShellPromptCompletion::failed("boom"));

        assert!(ctrl.core_state.pending_follow_up_state.is_none());
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(!ctrl.auto_test_in_progress);
        assert!(!ctrl.core_state.auto_test_in_progress);
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [clankers_protocol::DaemonEvent::SystemMessage { text, is_error: true }]
                if text == "Post-prompt follow-up failed"
        ));
    }

    #[test]
    fn test_follow_up_dispatch_rejection_keeps_state_and_emits_error_on_wrong_id() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let effect_id = match ctrl.check_post_prompt(false) {
            PostPromptAction::RunAutoTest { pending_work_id, .. } => pending_work_id,
            other => panic!("expected RunAutoTest, got {other:?}"),
        };
        let previous_state = ctrl.core_state.clone();
        let wrong_effect_id = PendingWorkId::from_raw(effect_id.raw() + 1);

        ctrl.ack_follow_up_dispatch(wrong_effect_id, ShellFollowUpDispatch::Accepted);

        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn test_complete_dispatched_follow_up_before_dispatch_ack_is_rejected() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let (effect_id, prompt) = match ctrl.check_post_prompt(false) {
            PostPromptAction::RunAutoTest {
                pending_work_id,
                prompt,
            } => (pending_work_id, prompt),
            other => panic!("expected RunAutoTest, got {other:?}"),
        };
        assert!(ctrl.start_embedded_prompt_with_follow_up(&prompt, 0, Some(effect_id)));
        let previous_state = ctrl.core_state.clone();

        ctrl.complete_dispatched_follow_up(effect_id, ShellPromptCompletion::Succeeded);

        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn test_complete_dispatched_follow_up_duplicate_completion_is_rejected() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let (effect_id, prompt) = match ctrl.check_post_prompt(false) {
            PostPromptAction::RunAutoTest {
                pending_work_id,
                prompt,
            } => (pending_work_id, prompt),
            other => panic!("expected RunAutoTest, got {other:?}"),
        };
        ctrl.ack_follow_up_dispatch(effect_id, ShellFollowUpDispatch::Accepted);
        assert!(ctrl.start_embedded_prompt_with_follow_up(&prompt, 0, Some(effect_id)));
        ctrl.complete_dispatched_follow_up(effect_id, ShellPromptCompletion::Succeeded);
        let settled_state = ctrl.core_state.clone();
        assert!(ctrl.drain_events().is_empty());

        ctrl.complete_dispatched_follow_up(effect_id, ShellPromptCompletion::Succeeded);

        assert_eq!(ctrl.core_state, settled_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [clankers_protocol::DaemonEvent::SystemMessage {
            is_error: true,
            ..
        }]));
    }

    #[test]
    fn pre_engine_cancellation_embedded_prompt_uses_core_completion_not_engine_cancel() {
        let mut ctrl = make_test_controller();
        assert!(ctrl.start_embedded_prompt("hello", 0));

        ctrl.finish_embedded_prompt(ShellPromptCompletion::cancelled());

        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(ctrl.drain_events().is_empty());
    }

    #[test]
    fn pre_engine_cancellation_dispatched_follow_up_completes_without_prompt_task() {
        const LOOP_ITERATION_LIMIT: u32 = 2;

        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });
        let effect_id = match ctrl.check_post_prompt(false) {
            PostPromptAction::ContinueLoop { pending_work_id, .. } => pending_work_id,
            other => panic!("expected ContinueLoop, got {other:?}"),
        };
        ctrl.ack_follow_up_dispatch(effect_id, ShellFollowUpDispatch::Accepted);
        assert!(ctrl.core_state.pending_prompt.is_none());

        ctrl.complete_dispatched_follow_up(effect_id, ShellPromptCompletion::cancelled());

        assert!(ctrl.core_state.pending_follow_up_state.is_none());
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(ctrl.core_state.active_loop_state.is_none());
        assert!(!ctrl.has_active_loop());
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [clankers_protocol::DaemonEvent::SystemMessage { text, is_error: false }]
                if text.contains("failed (follow-up)")
        ));
    }

    #[test]
    fn pre_engine_cancellation_controller_paths_do_not_construct_engine_cancel_turn() {
        const FORBIDDEN_PREFIX: &str = "EngineInput";
        const FORBIDDEN_SUFFIX: &str = "::CancelTurn";
        let forbidden = [FORBIDDEN_PREFIX, FORBIDDEN_SUFFIX].concat();
        let controller_sources = [
            include_str!("auto_test.rs"),
            include_str!("command.rs"),
            include_str!("core_effects.rs"),
        ];

        for source in controller_sources {
            assert!(
                !source.contains(&forbidden),
                "controller pre-engine lifecycle paths must not construct engine cancellation feedback"
            );
        }
    }

    #[test]
    fn test_notify_prompt_done_clears_busy() {
        let mut ctrl = make_test_controller();
        assert!(ctrl.start_embedded_prompt("hello", 0));

        ctrl.notify_prompt_done(false);
        assert!(!ctrl.busy);
    }

    #[test]
    fn test_notify_prompt_done_with_error_finishes_loop() {
        let mut ctrl = make_test_controller();
        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("continue loop".to_string()),
            max_iterations: 2,
            break_text: None,
        });
        assert!(ctrl.start_embedded_prompt("hello", 0));

        ctrl.notify_prompt_done(true); // had_error = true
        assert!(!ctrl.busy);
        assert!(ctrl.active_loop_id.is_none());
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
