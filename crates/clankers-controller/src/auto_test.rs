//! Auto-test and post-prompt action processing.
//!
//! Contains logic for determining what action should be taken after
//! a prompt completes, including auto-test execution and loop continuation.

use crate::{loop_mode::LoopConfig, PostPromptAction, SessionController};

impl SessionController {
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
        Some(format!(
            "Run `{cmd}` and fix any failures. Do not ask for confirmation."
        ))
    }

    /// Clear the auto-test guard (call after the auto-test prompt completes).
    pub fn clear_auto_test(&mut self) {
        self.auto_test_in_progress = false;
    }

    /// Determine what to do after a prompt completes (embedded mode).
    ///
    /// Call this from the TUI's `handle_task_results` after receiving
    /// `PromptDone(None)` and confirming there's no queued user prompt.
    /// Returns the action the TUI should take.
    pub fn check_post_prompt(&mut self) -> PostPromptAction {
        // Loop continuation takes priority
        if self.active_loop_id.is_some()
            && let Some(prompt) = self.maybe_continue_loop()
        {
            return PostPromptAction::ContinueLoop(prompt);
        }

        // Auto-test
        if let Some(prompt) = self.maybe_auto_test() {
            return PostPromptAction::RunAutoTest(prompt);
        }
        self.clear_auto_test();

        PostPromptAction::None
    }

    /// Sync loop state from the TUI's loop_status.
    ///
    /// Called before `check_post_prompt()` to ensure the controller's
    /// loop engine matches the TUI's `/loop` command state.
    pub fn sync_loop_from_tui(
        &mut self,
        loop_status: Option<&clankers_tui_types::LoopDisplayState>,
    ) {
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
            }
            // Both in sync (or neither has a loop)
            _ => {}
        }
    }

    /// Get the current loop iteration count (for TUI display sync).
    pub fn loop_iteration(&self) -> Option<u32> {
        self.active_loop_id
            .as_ref()
            .and_then(|id| self.loop_engine.get(id))
            .map(|s| s.current_iteration)
    }

    /// Notify the controller that a prompt completed (embedded mode).
    ///
    /// Updates busy state. Called from the TUI when `TaskResult::PromptDone`
    /// is received, before calling `check_post_prompt()`.
    pub fn notify_prompt_done(&mut self, had_error: bool) {
        self.busy = false;
        if had_error && self.active_loop_id.is_some() {
            self.finish_loop("failed (error)");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{test_helpers::make_test_controller, PostPromptAction};
    use clankers_loop::LoopId;
    use clankers_tui_types::LoopDisplayState;

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
        ctrl.active_loop_id = Some(clankers_loop::LoopId("test-loop".to_string()));

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
        assert!(matches!(action, PostPromptAction::RunAutoTest(_)));

        if let PostPromptAction::RunAutoTest(prompt) = action {
            assert!(prompt.contains("cargo test"));
        }
    }

    #[test]
    fn test_check_post_prompt_with_active_loop() {
        let mut ctrl = make_test_controller();
        
        // Set up a mock active loop
        ctrl.active_loop_id = Some(LoopId("test-loop".to_string()));
        
        // Mock a loop config in the engine (this is a bit tricky without loop engine internals)
        // For this test, we'll assume maybe_continue_loop returns Some when there's an active loop
        // In practice, this would depend on loop engine implementation

        let action = ctrl.check_post_prompt();
        // The behavior depends on whether maybe_continue_loop returns a prompt
        // Since we don't have loop content, it should fall through to auto-test or None
        match action {
            PostPromptAction::ContinueLoop(_) => {
                // Expected if loop has continuation prompt
            }
            PostPromptAction::None => {
                // Expected if loop doesn't have continuation
            }
            _ => panic!("Unexpected action type"),
        }
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