//! Inter-pane communication (pipe, messages)

use serde::Deserialize;
use serde::Serialize;

use super::commands;
use crate::agent::events::AgentEvent;

/// Agent status communicated to other panes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Busy { tool: Option<String> },
    Waiting,
    Done,
    Error { message: String },
}

/// Message sent between panes via zellij pipes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneMessage {
    pub from: String,
    pub to: Option<String>, // None = broadcast
    pub payload: PanePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PanePayload {
    StatusUpdate(AgentStatus),
    TaskComplete { result: String },
    MergeRequest { branch: String },
    Custom { kind: String, data: serde_json::Value },
}

/// Send a status update to the status bar plugin.
/// Best-effort: silently ignores errors (e.g. when not inside Zellij).
/// Disables itself after the first failure to avoid spawning a process
/// on every agent event when the target plugin isn't loaded.
pub fn send_status(status: &AgentStatus) {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    static PIPE_DISABLED: AtomicBool = AtomicBool::new(false);

    if !super::is_inside_zellij() || PIPE_DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let json = serde_json::to_string(status).unwrap_or_default();
    if commands::pipe_message("clankers-status-bar", "status", &json).is_err() {
        // Target plugin not available — stop trying
        PIPE_DISABLED.store(true, Ordering::Relaxed);
    }
}

/// Map an agent lifecycle event to a Zellij IPC status update.
/// Called from the interactive event loop on every agent event.
/// No-op when not inside a Zellij session.
///
/// The actual pipe command runs on a background thread so it can
/// never block the TUI event loop.
pub fn broadcast_agent_event(event: &AgentEvent) {
    if !super::is_inside_zellij() {
        return;
    }
    let status = match event_to_status(event) {
        Some(s) => s,
        None => return,
    };
    // Fire-and-forget on a background thread
    std::thread::spawn(move || {
        send_status(&status);
    });
}

/// Map an agent event to a status, returning None for events that don't
/// produce a status change. Exposed for testing.
pub fn event_to_status(event: &AgentEvent) -> Option<AgentStatus> {
    match event {
        AgentEvent::AgentStart | AgentEvent::TurnStart { .. } => Some(AgentStatus::Busy { tool: None }),
        AgentEvent::ToolExecutionStart { tool_name, .. } => Some(AgentStatus::Busy {
            tool: Some(tool_name.clone()),
        }),
        AgentEvent::ToolExecutionEnd { .. } => Some(AgentStatus::Busy { tool: None }),
        AgentEvent::AgentEnd { .. } | AgentEvent::TurnEnd { .. } => Some(AgentStatus::Idle),
        AgentEvent::UserCancel => Some(AgentStatus::Idle),
        AgentEvent::SessionShutdown { .. } => Some(AgentStatus::Done),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus::Busy {
            tool: Some("bash".to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Busy"));
        assert!(json.contains("bash"));

        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
        match deserialized {
            AgentStatus::Busy { tool } => assert_eq!(tool, Some("bash".to_string())),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_agent_status_idle_serialization() {
        let status = AgentStatus::Idle;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, AgentStatus::Idle));
    }

    #[test]
    fn test_pane_message_serialization() {
        let msg = PaneMessage {
            from: "main".to_string(),
            to: Some("worker-1".to_string()),
            payload: PanePayload::StatusUpdate(AgentStatus::Busy { tool: None }),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: PaneMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from, "main");
        assert_eq!(deserialized.to, Some("worker-1".to_string()));
    }

    #[test]
    fn test_pane_payload_variants() {
        let payloads = vec![
            PanePayload::StatusUpdate(AgentStatus::Done),
            PanePayload::TaskComplete {
                result: "ok".to_string(),
            },
            PanePayload::MergeRequest {
                branch: "feature-x".to_string(),
            },
            PanePayload::Custom {
                kind: "test".to_string(),
                data: serde_json::json!({"key": "value"}),
            },
        ];
        for payload in payloads {
            let json = serde_json::to_string(&payload).unwrap();
            let _: PanePayload = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_event_to_status_agent_start() {
        let status = event_to_status(&AgentEvent::AgentStart);
        assert!(matches!(status, Some(AgentStatus::Busy { tool: None })));
    }

    #[test]
    fn test_event_to_status_tool_execution() {
        let status = event_to_status(&AgentEvent::ToolExecutionStart {
            call_id: "123".to_string(),
            tool_name: "read".to_string(),
        });
        match status {
            Some(AgentStatus::Busy { tool }) => assert_eq!(tool, Some("read".to_string())),
            _ => panic!("Expected Busy with tool"),
        }
    }

    #[test]
    fn test_event_to_status_agent_end() {
        let status = event_to_status(&AgentEvent::AgentEnd { messages: vec![] });
        assert!(matches!(status, Some(AgentStatus::Idle)));
    }

    #[test]
    fn test_event_to_status_user_cancel() {
        let status = event_to_status(&AgentEvent::UserCancel);
        assert!(matches!(status, Some(AgentStatus::Idle)));
    }

    #[test]
    fn test_event_to_status_streaming_delta_returns_none() {
        let status = event_to_status(&AgentEvent::ContentBlockStop { index: 0 });
        assert!(status.is_none());
    }

    #[test]
    fn test_event_to_status_session_shutdown() {
        let status = event_to_status(&AgentEvent::SessionShutdown {
            session_id: "abc".to_string(),
        });
        assert!(matches!(status, Some(AgentStatus::Done)));
    }
}
