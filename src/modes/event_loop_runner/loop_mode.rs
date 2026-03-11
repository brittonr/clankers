//! Loop mode management — iteration tracking and break condition evaluation.
//!
//! Extracted from the main event loop runner. The `LoopEngine` from
//! `clankers-loop` is the single source of truth; `app.loop_status` is
//! a display-only projection updated after each iteration.

use clankers_loop::BreakCondition;
use clankers_loop::LoopDef;
use clankers_loop::LoopId;

use super::EventLoopRunner;
use crate::modes::interactive::AgentCommand;

impl<'a> EventLoopRunner<'a> {
    /// Lazily register the loop with the engine on the first iteration.
    ///
    /// The `/loop` slash command writes to `app.loop_status` (display
    /// state). This method translates that into a `LoopDef` and
    /// registers it with the `LoopEngine`.
    pub(super) fn ensure_loop_registered(&mut self) -> Option<LoopId> {
        if let Some(ref id) = self.active_loop_id {
            return Some(id.clone());
        }

        let ls = self.app.loop_status.as_ref()?;

        let break_condition = match &ls.break_text {
            Some(text) => clankers_loop::parse_break_condition(text),
            None => BreakCondition::Never,
        };

        let action = serde_json::json!({"prompt": ls.prompt.as_deref().unwrap_or("")});

        let def = if matches!(break_condition, BreakCondition::Never) {
            LoopDef::fixed(&ls.name, ls.max_iterations, action)
        } else {
            LoopDef::until(&ls.name, break_condition, action).with_max_iterations(ls.max_iterations)
        };

        let Some(id) = self.loop_engine.register(def) else {
            tracing::warn!("loop registration failed: too many active loops");
            return None;
        };
        self.loop_engine.start(&id);
        self.active_loop_id = Some(id.clone());
        Some(id)
    }

    /// After a successful turn, check whether to continue the loop.
    pub(super) fn maybe_continue_loop(&mut self) {
        if self.app.loop_status.is_none() {
            // `/loop stop` clears display state — clean up engine.
            if let Some(ref id) = self.active_loop_id {
                self.loop_engine.stop(id);
                self.loop_engine.remove(id);
            }
            self.active_loop_id = None;
            self.loop_turn_output.clear();
            return;
        }

        let Some(loop_id) = self.ensure_loop_registered() else {
            return;
        };

        // Feed accumulated output to the engine for break condition checks.
        let output = std::mem::take(&mut self.loop_turn_output);
        let should_continue = self.loop_engine.record_iteration(&loop_id, output, None);

        // Sync engine state back to display state for TUI.
        if let Some(state) = self.loop_engine.get(&loop_id)
            && let Some(ref mut ls) = self.app.loop_status
        {
            ls.iteration = state.current_iteration;
        }

        if !should_continue {
            let reason = self.loop_engine.get(&loop_id).map_or("finished", |s| match s.status {
                clankers_loop::LoopStatus::Completed => "completed",
                clankers_loop::LoopStatus::Stopped => "max iterations reached",
                clankers_loop::LoopStatus::Failed => "failed",
                _ => "finished",
            });
            self.finish_loop(reason);
            return;
        }

        // Paused — don't re-send, leave state as-is.
        if !self.app.loop_status.as_ref().is_some_and(|ls| ls.active) {
            return;
        }

        // Continue — re-send the prompt.
        let prompt = self.app.loop_status.as_ref().and_then(|ls| ls.prompt.clone());
        if let Some(prompt) = prompt {
            let _ = self.cmd_tx.send(AgentCommand::ResetCancel);
            let _ = self.cmd_tx.send(AgentCommand::Prompt(prompt));
        } else {
            self.finish_loop("no prompt captured");
        }
    }

    /// Clean up loop state and notify the user.
    pub(super) fn finish_loop(&mut self, reason: &str) {
        let summary = self.app.loop_status.as_ref().map(|ls| {
            format!(
                "Loop '{}' {} after {} iteration(s).",
                ls.name, reason, ls.iteration,
            )
        });

        if let Some(ref id) = self.active_loop_id {
            self.loop_engine.remove(id);
        }
        self.active_loop_id = None;
        self.app.loop_status = None;
        self.loop_turn_output.clear();

        // Tiger Style: post-condition — loop state fully cleaned up
        debug_assert!(self.active_loop_id.is_none());
        debug_assert!(self.app.loop_status.is_none());
        debug_assert!(self.loop_turn_output.is_empty());

        if let Some(msg) = summary {
            self.app.push_system(msg, false);
        }
    }
}
