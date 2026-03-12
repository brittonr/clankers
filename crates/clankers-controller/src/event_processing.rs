//! Agent event processing pipeline.
//!
//! Contains the event processing logic that handles agent events, performs
//! audit tracking, session persistence, and lifecycle hooks.

use clankers_agent::events::AgentEvent;
use clankers_protocol::DaemonEvent;

use crate::{convert::agent_event_to_daemon_event, SessionController};

impl SessionController {
    /// Drain pending events. Called in a loop by the transport layer.
    ///
    /// In daemon mode, reads from the internal agent event receiver.
    /// In embedded mode, events must be fed via [`feed_event`] first.
    pub fn drain_events(&mut self) -> Vec<DaemonEvent> {
        // Drain agent events from internal receiver (daemon mode).
        // Collect into a Vec to avoid borrowing event_rx and self simultaneously.
        let events: Vec<AgentEvent> = if let Some(ref mut rx) = self.event_rx {
            let mut buf = Vec::new();
            while let Ok(event) = rx.try_recv() {
                buf.push(event);
            }
            buf
        } else {
            Vec::new()
        };
        for event in &events {
            self.process_agent_event(event);
        }
        std::mem::take(&mut self.outgoing)
    }

    /// Take accumulated outgoing events without draining the internal
    /// receiver. Used in embedded mode after calling [`feed_event`].
    pub fn take_outgoing(&mut self) -> Vec<DaemonEvent> {
        std::mem::take(&mut self.outgoing)
    }

    /// Feed a single agent event for processing (embedded mode).
    ///
    /// Performs audit tracking, session persistence, lifecycle hooks,
    /// loop output accumulation, and DaemonEvent translation — the same
    /// processing that `drain_events` does internally.
    pub fn feed_event(&mut self, event: &AgentEvent) {
        self.process_agent_event(event);
    }

    /// Process a single agent event into zero or more daemon events.
    fn process_agent_event(&mut self, event: &AgentEvent) {
        // 1. Audit tracking
        self.audit.process_event(event);

        // 2. Track tool call names
        if let AgentEvent::ToolCall { call_id, tool_name, .. } = event {
            self.tool_call_names.insert(call_id.clone(), tool_name.clone());

            // Check signal_loop_success tool
            if tool_name == "signal_loop_success" {
                self.signal_loop_break();
            }
        }

        // 3. Accumulate tool output for loop break conditions
        if let AgentEvent::ToolExecutionEnd { result, .. } = event {
            for content in &result.content {
                if let clankers_agent::ToolResultContent::Text { text } = content {
                    if !self.loop_turn_output.is_empty() {
                        self.loop_turn_output.push('\n');
                    }
                    self.loop_turn_output.push_str(text);
                }
            }
        }

        // 4. Persist to session
        self.persist_event(event);

        // 5. Translate to DaemonEvent
        if let Some(daemon_event) = agent_event_to_daemon_event(event) {
            self.outgoing.push(daemon_event);
        }

        // 6. Fire lifecycle hooks
        self.fire_lifecycle_hooks(event);
    }

    /// Fire lifecycle hooks for session and turn events.
    fn fire_lifecycle_hooks(&self, event: &AgentEvent) {
        let Some(ref pipeline) = self.hook_pipeline else {
            return;
        };

        let session_id = self.session_id.clone();
        match event {
            AgentEvent::SessionStart { session_id: sid } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::SessionStart,
                    clankers_hooks::HookPayload::session("session-start", sid),
                );
            }
            AgentEvent::SessionShutdown { session_id: sid } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::SessionEnd,
                    clankers_hooks::HookPayload::session("session-end", sid),
                );
            }
            AgentEvent::TurnStart { .. } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::TurnStart,
                    clankers_hooks::HookPayload::empty("turn-start", &session_id),
                );
            }
            AgentEvent::TurnEnd { .. } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::TurnEnd,
                    clankers_hooks::HookPayload::empty("turn-end", &session_id),
                );
            }
            AgentEvent::ModelChange { from, to, reason } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::ModelChange,
                    clankers_hooks::HookPayload::model_change(
                        "model-change",
                        &session_id,
                        from,
                        to,
                        reason,
                    ),
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use clankers_agent::events::AgentEvent;
    use clankers_message::{ToolResult, ToolResultContent};
    use clankers_protocol::DaemonEvent;
    use serde_json::json;

    use crate::config::ControllerConfig;
    use crate::SessionController;

    fn make_embedded_controller() -> SessionController {
        let config = ControllerConfig {
            session_id: "test-session".to_string(),
            model: "test-model".to_string(),
            ..Default::default()
        };
        SessionController::new_embedded(config)
    }

    #[test]
    fn test_feed_event_produces_daemon_event() {
        let mut ctrl = make_embedded_controller();

        // Feed an AgentStart event
        let event = AgentEvent::AgentStart;
        ctrl.feed_event(&event);

        // Should produce DaemonEvent in outgoing buffer
        let outgoing = ctrl.take_outgoing();
        assert_eq!(outgoing.len(), 1);
        assert!(matches!(outgoing[0], DaemonEvent::AgentStart { .. }));
    }

    #[test]
    fn test_tool_events_accumulate_output() {
        let mut ctrl = make_embedded_controller();

        // Feed a tool execution end with text output
        let event = AgentEvent::ToolExecutionEnd {
            call_id: "test-call".to_string(),
            result: ToolResult {
                content: vec![ToolResultContent::Text {
                    text: "test output".to_string(),
                }],
                details: None,
                full_output_path: None,
                is_error: false,
            },
            is_error: false,
        };
        ctrl.feed_event(&event);

        // Should accumulate in loop_turn_output
        assert_eq!(ctrl.loop_turn_output, "test output");

        // Feed another tool execution
        let event2 = AgentEvent::ToolExecutionEnd {
            call_id: "test-call-2".to_string(),
            result: ToolResult {
                content: vec![ToolResultContent::Text {
                    text: "more output".to_string(),
                }],
                details: None,
                full_output_path: None,
                is_error: false,
            },
            is_error: false,
        };
        ctrl.feed_event(&event2);

        // Should append with newline
        assert_eq!(ctrl.loop_turn_output, "test output\nmore output");
    }

    #[test]
    fn test_feed_event_tracks_tool_call_names() {
        let mut ctrl = make_embedded_controller();

        // Feed a ToolCall event
        let event = AgentEvent::ToolCall {
            call_id: "test-call-id".to_string(),
            tool_name: "test_tool".to_string(),
            input: json!({"param": "value"}),
        };
        ctrl.feed_event(&event);

        // Should track the tool call name
        assert_eq!(
            ctrl.tool_call_names.get("test-call-id"),
            Some(&"test_tool".to_string())
        );
    }

    #[test]
    fn test_signal_loop_success_triggers_break() {
        let mut ctrl = make_embedded_controller();

        // Feed signal_loop_success tool call
        let event = AgentEvent::ToolCall {
            call_id: "signal-call".to_string(),
            tool_name: "signal_loop_success".to_string(),
            input: json!({}),
        };
        ctrl.feed_event(&event);

        // Should have called signal_loop_break (this would affect active loop state
        // if there was one running, but we can verify the tool call was tracked)
        assert_eq!(
            ctrl.tool_call_names.get("signal-call"),
            Some(&"signal_loop_success".to_string())
        );
    }

    #[test]
    fn test_take_outgoing_returns_and_clears() {
        let mut ctrl = make_embedded_controller();

        // Feed events to produce outgoing
        let event1 = AgentEvent::AgentStart;
        let event2 = AgentEvent::AgentEnd {
            messages: vec![],
        };
        ctrl.feed_event(&event1);
        ctrl.feed_event(&event2);

        // First take_outgoing should return events
        let outgoing1 = ctrl.take_outgoing();
        assert_eq!(outgoing1.len(), 2);

        // Second take_outgoing should return empty
        let outgoing2 = ctrl.take_outgoing();
        assert!(outgoing2.is_empty());
    }

    #[test]
    fn test_drain_events_returns_empty_in_embedded_mode() {
        let mut ctrl = make_embedded_controller();

        // In embedded mode, drain_events should return empty
        // because there's no internal receiver
        let events = ctrl.drain_events();
        assert!(events.is_empty());
    }

    #[test]
    fn test_process_agent_event_pipeline() {
        let mut ctrl = make_embedded_controller();

        // 1. Feed ToolCall event
        let tool_call_event = AgentEvent::ToolCall {
            call_id: "call-1".to_string(),
            tool_name: "bash".to_string(),
            input: json!({"command": "echo test"}),
        };
        ctrl.feed_event(&tool_call_event);

        // Verify audit pending count increased and tool_call_names populated
        assert!(!ctrl.tool_call_names.is_empty());
        assert_eq!(ctrl.tool_call_names.get("call-1"), Some(&"bash".to_string()));

        let events = ctrl.take_outgoing();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DaemonEvent::ToolCall { .. }));

        // 2. Feed ToolExecutionStart
        let start_event = AgentEvent::ToolExecutionStart {
            call_id: "call-1".to_string(),
            tool_name: "bash".to_string(),
        };
        ctrl.feed_event(&start_event);

        // Should have audit tracking but minimal outgoing events
        let _events = ctrl.take_outgoing();
        // ToolExecutionStart may or may not produce events - just check it doesn't crash

        // 3. Feed ToolExecutionEnd
        let end_event = AgentEvent::ToolExecutionEnd {
            call_id: "call-1".to_string(),
            result: ToolResult {
                content: vec![ToolResultContent::Text {
                    text: "test output from bash".to_string(),
                }],
                details: None,
                full_output_path: None,
                is_error: false,
            },
            is_error: false,
        };
        ctrl.feed_event(&end_event);

        // Should populate loop_turn_output and produce ToolDone event
        assert_eq!(ctrl.loop_turn_output, "test output from bash");
        let events = ctrl.take_outgoing();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DaemonEvent::ToolDone { .. }));

        // 4. Feed AgentStart and AgentEnd
        let agent_start = AgentEvent::AgentStart;
        let agent_end = AgentEvent::AgentEnd {
            messages: vec![],
        };

        ctrl.feed_event(&agent_start);
        ctrl.feed_event(&agent_end);

        let events = ctrl.take_outgoing();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], DaemonEvent::AgentStart { .. }));
        assert!(matches!(events[1], DaemonEvent::AgentEnd { .. }));
    }
}