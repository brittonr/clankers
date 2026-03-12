//! Auto-test mode — run a test command after the agent finishes a turn.
//!
//! When `auto_test_enabled` is true and `auto_test_command` is set on the App,
//! the runner spawns the command after a successful agent turn and displays
//! the results. If tests fail, the output is shown so the user can ask the
//! agent to fix the failures.

use super::EventLoopRunner;
use crate::modes::interactive::AgentCommand;

impl<'a> EventLoopRunner<'a> {
    /// Run the auto-test command if enabled. Skips if already running an auto-test
    /// turn (prevents infinite recursion) or if a loop is active.
    pub(super) fn maybe_run_auto_test(&mut self) {
        // Guard: don't recurse if this turn was itself an auto-test
        if self.auto_test_in_progress {
            self.auto_test_in_progress = false;
            return;
        }

        // Guard: feature must be enabled with a command configured
        let Some(ref cmd) = self.app.auto_test_command else {
            return;
        };
        if !self.app.auto_test_enabled {
            return;
        }

        let cmd = cmd.clone();
        self.auto_test_in_progress = true;

        self.app.push_system(format!("🧪 Running auto-test: {}", cmd), false);

        // Send the test command as a prompt so the agent sees the results
        // and can act on failures.
        let prompt = format!(
            "Run the following test command and report the results. \
             If any tests fail, analyze the failures and fix the issues.\n\n\
             ```\n{}\n```",
            cmd,
        );
        let _ = self.cmd_tx.send(AgentCommand::ResetCancel);
        let _ = self.cmd_tx.send(AgentCommand::Prompt(prompt));
    }
}
