//! Pure protocol conversions for transport/client seams.
//!
//! Socket and stream files should own framing, I/O, and relay loops.
//! Wire-struct and response construction lives here so FCIS rails can keep
//! those seams deterministic.

use std::path::Path;

use clankers_protocol::AttachResponse;
use clankers_protocol::ControlResponse;
use clankers_protocol::DaemonEvent;
use clankers_protocol::DaemonStatus;
use clankers_protocol::PluginSummary;
use clankers_protocol::ProcessInfo;
use clankers_protocol::SessionSummary;
use clankers_protocol::types::Handshake;
use clankers_protocol::types::PROTOCOL_VERSION;

use crate::transport::DaemonState;
use crate::transport::SessionHandle;
use crate::transport::SessionSocketInfo;

pub fn client_handshake(client_name: &str, token: Option<String>, session_id: Option<String>) -> Handshake {
    Handshake {
        protocol_version: PROTOCOL_VERSION,
        client_name: client_name.to_string(),
        token,
        session_id,
    }
}

pub fn session_info_event(session_id: &str, info: &SessionSocketInfo) -> DaemonEvent {
    DaemonEvent::SessionInfo {
        session_id: session_id.to_string(),
        model: info.model.clone(),
        system_prompt_hash: String::new(),
        available_models: info.available_models.clone(),
        active_account: info.active_account.clone(),
        disabled_tools: info.disabled_tools.clone(),
        auto_test_command: info.auto_test_command.clone(),
    }
}

pub fn session_summary(handle: &SessionHandle) -> SessionSummary {
    SessionSummary {
        session_id: handle.session_id.clone(),
        model: handle.model.clone(),
        turn_count: handle.turn_count,
        last_active: handle.last_active.clone(),
        client_count: handle.client_count,
        socket_path: handle.socket_path.to_string_lossy().into_owned(),
        state: handle.state.clone(),
    }
}

pub fn daemon_status(state: &DaemonState) -> DaemonStatus {
    DaemonStatus {
        uptime_secs: state.started_at.elapsed().as_secs_f64(),
        session_count: state.sessions.len(),
        total_clients: state.sessions.values().map(|handle| handle.client_count).sum(),
        pid: std::process::id(),
    }
}

pub fn control_sessions(state: &DaemonState) -> ControlResponse {
    ControlResponse::Sessions(state.sessions.values().map(session_summary).collect())
}

pub fn control_created(session_id: &str, socket_path: &Path) -> ControlResponse {
    ControlResponse::Created {
        session_id: session_id.to_string(),
        socket_path: socket_path.to_string_lossy().into_owned(),
    }
}

pub fn control_attached(socket_path: &Path) -> ControlResponse {
    ControlResponse::Attached {
        socket_path: socket_path.to_string_lossy().into_owned(),
    }
}

pub fn control_tree(processes: Vec<ProcessInfo>) -> ControlResponse {
    ControlResponse::Tree(processes)
}

pub fn control_killed() -> ControlResponse {
    ControlResponse::Killed
}

pub fn control_shutting_down() -> ControlResponse {
    ControlResponse::ShuttingDown
}

pub fn control_status(state: &DaemonState) -> ControlResponse {
    ControlResponse::Status(daemon_status(state))
}

pub fn control_restarting() -> ControlResponse {
    ControlResponse::Restarting
}

pub fn control_plugins(summaries: Vec<PluginSummary>) -> ControlResponse {
    ControlResponse::Plugins(summaries)
}

pub fn control_error(message: impl Into<String>) -> ControlResponse {
    ControlResponse::Error {
        message: message.into(),
    }
}

pub fn attach_ok(session_id: &str) -> AttachResponse {
    AttachResponse::Ok {
        session_id: session_id.to_string(),
    }
}

pub fn attach_error(message: impl Into<String>) -> AttachResponse {
    AttachResponse::Error {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Instant;

    use tokio::sync::broadcast;
    use tokio::sync::mpsc;

    use super::*;
    use crate::transport::SessionHandle;

    fn session_handle() -> SessionHandle {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, _event_rx) = broadcast::channel(1);
        SessionHandle {
            session_id: "session-1".to_string(),
            model: "sonnet".to_string(),
            turn_count: 3,
            last_active: "2026-04-22T09:00:00Z".to_string(),
            client_count: 2,
            cmd_tx: Some(cmd_tx),
            event_tx: Some(event_tx),
            socket_path: PathBuf::from("/tmp/session-1.sock"),
            state: "active".to_string(),
        }
    }

    #[test]
    fn client_handshake_uses_protocol_defaults() {
        let handshake = client_handshake("client", Some("token".to_string()), Some("session-1".to_string()));

        assert_eq!(handshake.protocol_version, PROTOCOL_VERSION);
        assert_eq!(handshake.client_name, "client");
        assert_eq!(handshake.token.as_deref(), Some("token"));
        assert_eq!(handshake.session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn session_info_event_copies_session_socket_info() {
        let info = SessionSocketInfo {
            model: "sonnet".to_string(),
            available_models: vec!["sonnet".to_string(), "haiku".to_string()],
            active_account: "acct".to_string(),
            disabled_tools: vec!["bash".to_string()],
            auto_test_command: Some("cargo test".to_string()),
        };

        let event = session_info_event("session-1", &info);

        assert!(matches!(
            event,
            DaemonEvent::SessionInfo {
                session_id,
                model,
                available_models,
                active_account,
                disabled_tools,
                auto_test_command,
                ..
            } if session_id == "session-1"
                && model == "sonnet"
                && available_models == vec!["sonnet".to_string(), "haiku".to_string()]
                && active_account == "acct"
                && disabled_tools == vec!["bash".to_string()]
                && auto_test_command.as_deref() == Some("cargo test")
        ));
    }

    #[test]
    fn session_summary_projects_handle_fields() {
        let summary = session_summary(&session_handle());

        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.model, "sonnet");
        assert_eq!(summary.turn_count, 3);
        assert_eq!(summary.last_active, "2026-04-22T09:00:00Z");
        assert_eq!(summary.client_count, 2);
        assert!(summary.socket_path.ends_with("session-1.sock"));
        assert_eq!(summary.state, "active");
    }

    fn daemon_state() -> DaemonState {
        let mut sessions = HashMap::new();
        sessions.insert("session-1".to_string(), session_handle());
        DaemonState {
            sessions,
            key_index: HashMap::new(),
            started_at: Instant::now(),
        }
    }

    #[test]
    fn daemon_status_counts_sessions_and_clients() {
        let status = daemon_status(&daemon_state());

        assert_eq!(status.session_count, 1);
        assert_eq!(status.total_clients, 2);
        assert_eq!(status.pid, std::process::id());
        assert!(status.uptime_secs >= 0.0);
    }

    #[test]
    fn control_responses_project_state_and_socket_metadata() {
        let state = daemon_state();
        let socket_path = PathBuf::from("/tmp/session-1.sock");

        assert!(matches!(control_sessions(&state), ControlResponse::Sessions(sessions) if sessions.len() == 1));
        assert!(matches!(control_status(&state), ControlResponse::Status(status) if status.session_count == 1));
        assert_eq!(control_created("session-1", &socket_path), ControlResponse::Created {
            session_id: "session-1".to_string(),
            socket_path: "/tmp/session-1.sock".to_string(),
        });
        assert_eq!(control_attached(&socket_path), ControlResponse::Attached {
            socket_path: "/tmp/session-1.sock".to_string(),
        });
    }

    #[test]
    fn control_responses_cover_success_and_error_variants() {
        let plugin = PluginSummary {
            name: "plugin".to_string(),
            version: "0.1.0".to_string(),
            state: "Active".to_string(),
            tools: vec!["tool".to_string()],
            permissions: vec!["ui".to_string()],
            kind: Some("stdio".to_string()),
            last_error: None,
        };

        assert_eq!(control_tree(vec![]), ControlResponse::Tree(vec![]));
        assert_eq!(control_killed(), ControlResponse::Killed);
        assert_eq!(control_shutting_down(), ControlResponse::ShuttingDown);
        assert_eq!(control_restarting(), ControlResponse::Restarting);
        assert_eq!(control_plugins(vec![plugin.clone()]), ControlResponse::Plugins(vec![plugin]));
        assert_eq!(control_error("bad response"), ControlResponse::Error {
            message: "bad response".to_string(),
        });
    }

    #[test]
    fn attach_responses_copy_fields() {
        assert_eq!(attach_ok("session-1"), AttachResponse::Ok {
            session_id: "session-1".to_string(),
        });
        assert_eq!(attach_error("no session"), AttachResponse::Error {
            message: "no session".to_string(),
        });
    }
}
