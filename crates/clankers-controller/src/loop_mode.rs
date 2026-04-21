//! Loop mode management — iteration tracking and break condition evaluation.
//!
//! Mirrors the loop logic from EventLoopRunner, but emits DaemonEvents
//! instead of mutating App state.

use clanker_loop::BreakCondition;
use clanker_loop::LoopDef;
use clanker_loop::LoopId;
use clanker_loop::LoopStatus;
use clankers_core::ActiveLoopState;
use clankers_protocol::DaemonEvent;
use tracing::warn;

use crate::SessionController;

/// Loop configuration sent by a client.
#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub name: String,
    pub prompt: Option<String>,
    pub max_iterations: u32,
    pub break_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ObservedLoopProgress {
    pub active_loop_state: Option<ActiveLoopState>,
    pub completion_reason: Option<String>,
}

impl SessionController {
    /// Register and start a loop from a client-provided config.
    pub fn start_loop(&mut self, config: LoopConfig) -> Option<LoopId> {
        let break_condition = match &config.break_text {
            Some(text) => clanker_loop::parse_break_condition(text),
            None => BreakCondition::Never,
        };

        let action = serde_json::json!({"prompt": config.prompt.as_deref().unwrap_or("")});

        let def = if matches!(break_condition, BreakCondition::Never) {
            LoopDef::fixed(&config.name, config.max_iterations, action)
        } else {
            LoopDef::until(&config.name, break_condition, action).with_max_iterations(config.max_iterations)
        };

        let Some(id) = self.loop_engine.register(def) else {
            warn!("loop registration failed: too many active loops");
            self.emit(DaemonEvent::SystemMessage {
                text: "Loop registration failed: too many active loops".to_string(),
                is_error: true,
            });
            return None;
        };
        self.loop_engine.start(&id);
        self.active_loop_id = Some(id.clone());
        self.core_state.active_loop_state = Some(ActiveLoopState {
            loop_id: id.0.clone(),
            prompt_text: config.prompt.unwrap_or_default(),
            current_iteration: 0,
            max_iterations: config.max_iterations,
            break_condition: config.break_text,
        });
        Some(id)
    }

    /// Observe loop progress after a prompt finishes without mutating core state.
    pub(crate) fn observe_post_prompt_loop_state(&mut self) -> ObservedLoopProgress {
        let Some(loop_id) = self.active_loop_id.as_ref().cloned() else {
            return ObservedLoopProgress::default();
        };

        // Feed accumulated output to the engine for break condition checks.
        let output = std::mem::take(&mut self.loop_turn_output);
        let should_continue = self.loop_engine.record_iteration(&loop_id, output, None);

        if !should_continue {
            let reason = self.loop_engine.get(&loop_id).map_or("finished", |state| match state.status {
                LoopStatus::Completed => "completed",
                LoopStatus::Stopped => "max iterations reached",
                LoopStatus::Failed => "failed",
                _ => "finished",
            });
            return ObservedLoopProgress {
                active_loop_state: None,
                completion_reason: Some(reason.to_string()),
            };
        }

        let previous_loop_state = self.core_state.active_loop_state.as_ref();
        let next_loop_state = self.loop_engine.get(&loop_id).map(|state| ActiveLoopState {
            loop_id: loop_id.0.clone(),
            prompt_text: state
                .def
                .action
                .get("prompt")
                .and_then(|value| value.as_str())
                .map(String::from)
                .unwrap_or_default(),
            current_iteration: state.current_iteration,
            max_iterations: previous_loop_state.map_or(0, |loop_state| loop_state.max_iterations),
            break_condition: previous_loop_state.and_then(|loop_state| loop_state.break_condition.clone()),
        });

        ObservedLoopProgress {
            active_loop_state: next_loop_state,
            completion_reason: None,
        }
    }

    /// After a prompt completes, check whether to continue the loop.
    /// Returns Some(prompt) if the loop should continue with another iteration.
    pub fn maybe_continue_loop(&mut self) -> Option<String> {
        let observed = self.observe_post_prompt_loop_state();
        if let Some(reason) = observed.completion_reason {
            self.finish_loop(&reason);
            return None;
        }

        self.core_state.active_loop_state = observed.active_loop_state.clone();
        observed.active_loop_state.map(|state| state.prompt_text)
    }

    /// Stop the active loop.
    pub fn stop_loop(&mut self) {
        if let Some(ref id) = self.active_loop_id {
            self.loop_engine.stop(id);
        }
        self.finish_loop("stopped by user");
    }

    /// Signal the loop break condition (from signal_loop_success tool).
    pub fn signal_loop_break(&mut self) {
        if let Some(ref id) = self.active_loop_id {
            self.loop_engine.signal_break(id);
        }
    }

    /// Whether a loop is currently active.
    pub fn has_active_loop(&self) -> bool {
        self.active_loop_id.is_some()
    }

    /// Clean up loop state and notify clients.
    pub(crate) fn finish_loop(&mut self, reason: &str) {
        let iteration = self
            .active_loop_id
            .as_ref()
            .and_then(|id| self.loop_engine.get(id))
            .map_or(0, |s| s.current_iteration);

        if let Some(ref id) = self.active_loop_id {
            self.loop_engine.remove(id);
        }
        self.active_loop_id = None;
        self.loop_turn_output.clear();
        self.core_state.active_loop_state = None;
        self.core_state.pending_follow_up_state = None;

        self.emit(DaemonEvent::SystemMessage {
            text: format!("Loop {reason} after {iteration} iteration(s)."),
            is_error: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctrl() -> SessionController {
        // reuse the test helper from the parent module
        let provider = std::sync::Arc::new(MockProvider);
        let agent = clankers_agent::Agent::new(
            provider,
            vec![],
            clankers_config::settings::Settings::default(),
            "test-model".to_string(),
            "test".to_string(),
        );
        SessionController::new(agent, crate::config::ControllerConfig {
            session_id: "test".to_string(),
            model: "test".to_string(),
            ..Default::default()
        })
    }

    struct MockProvider;
    #[async_trait::async_trait]
    impl clankers_provider::Provider for MockProvider {
        async fn complete(
            &self,
            _: clankers_provider::CompletionRequest,
            _: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            Ok(())
        }
        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_start_loop() {
        let mut ctrl = make_ctrl();
        let config = LoopConfig {
            name: "test-loop".to_string(),
            prompt: Some("check status".to_string()),
            max_iterations: 3,
            break_text: None,
        };
        let id = ctrl.start_loop(config);
        assert!(id.is_some());
        assert!(ctrl.has_active_loop());
    }

    #[test]
    fn test_stop_loop() {
        let mut ctrl = make_ctrl();
        ctrl.start_loop(LoopConfig {
            name: "test".to_string(),
            prompt: Some("go".to_string()),
            max_iterations: 5,
            break_text: None,
        });
        assert!(ctrl.has_active_loop());

        ctrl.stop_loop();
        assert!(!ctrl.has_active_loop());

        let events = ctrl.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DaemonEvent::SystemMessage { text, .. } if text.contains("stopped")))
        );
    }

    #[test]
    fn test_loop_max_iterations() {
        let mut ctrl = make_ctrl();
        ctrl.start_loop(LoopConfig {
            name: "fixed".to_string(),
            prompt: Some("go".to_string()),
            max_iterations: 2,
            break_text: None,
        });

        // Simulate 2 iterations
        let result1 = ctrl.maybe_continue_loop();
        assert!(result1.is_some()); // Should continue

        let result2 = ctrl.maybe_continue_loop();
        assert!(result2.is_none()); // Max reached
        assert!(!ctrl.has_active_loop());
    }
}
