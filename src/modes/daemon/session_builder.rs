//! Socketless daemon session construction plans.
//!
//! The control socket and chat transports are imperative shells: they accept
//! requests, open sockets, and send frames. This module owns the deterministic
//! session-construction decisions that can be tested without binding a Unix
//! socket or spawning an actor.

use std::path::Path;
use std::path::PathBuf;

use clankers_controller::transport::SessionHandle;
use clankers_controller::transport::session_socket_path;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SerializedMessage;
use clankers_protocol::SessionCommand;
use clankers_protocol::SessionKey;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use super::session_store::SessionCatalogEntry;
use super::session_store::SessionLifecycle;

/// User/control-plane inputs for a local daemon session create request.
pub(crate) struct CreateSessionPlanRequest {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub resume_id: Option<String>,
    pub continue_last: bool,
    pub cwd: Option<String>,
    pub thinking_level: Option<String>,
}

/// Inputs passed to `agent_process::spawn_agent_process`.
pub(crate) struct AgentSpawnPlan {
    pub session_id: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub capabilities: Option<Vec<clankers_ucan::Capability>>,
    pub public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
}

/// Socketless construction result for a daemon session.
pub(crate) struct SessionBuildPlan {
    pub session_id: String,
    pub resolved_model: String,
    pub socket_path: PathBuf,
    pub spawn: AgentSpawnPlan,
    pub seed_messages: Vec<SerializedMessage>,
    pub thinking_level: Option<String>,
    pub key: Option<SessionKey>,
}

/// Builder that owns session-construction policy while callers own IO/spawn.
pub(crate) struct SessionBuilder {
    default_model: String,
    sessions_dir: PathBuf,
}

impl SessionBuilder {
    pub(crate) fn from_global_paths(default_model: impl Into<String>) -> Self {
        Self {
            default_model: default_model.into(),
            sessions_dir: clankers_config::ClankersPaths::get().global_sessions_dir.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_sessions_dir(default_model: impl Into<String>, sessions_dir: PathBuf) -> Self {
        Self {
            default_model: default_model.into(),
            sessions_dir,
        }
    }

    pub(crate) fn plan_create_session(&self, request: CreateSessionPlanRequest) -> SessionBuildPlan {
        let effective_cwd = request.cwd.as_deref().unwrap_or(".");
        let (session_id, seed_messages) = resolve_session_resume_in_dir(
            &self.sessions_dir,
            request.resume_id.as_deref(),
            request.continue_last,
            effective_cwd,
        );
        let resolved_model = request.model.clone().unwrap_or_else(|| self.default_model.clone());
        let socket_path = session_socket_path(&session_id);
        let spawn = AgentSpawnPlan {
            session_id: session_id.clone(),
            model: request.model,
            system_prompt: request.system_prompt,
            capabilities: None,
            public_auth: None,
        };

        SessionBuildPlan {
            session_id,
            resolved_model,
            socket_path,
            spawn,
            seed_messages,
            thinking_level: request.thinking_level.filter(|level| !level.trim().is_empty()),
            key: None,
        }
    }

    pub(crate) fn plan_new_keyed_session(
        &self,
        key: &SessionKey,
        capabilities: Option<Vec<clankers_ucan::Capability>>,
        public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
    ) -> SessionBuildPlan {
        let session_id = clanker_message::generate_id();
        let socket_path = session_socket_path(&session_id);
        let spawn = AgentSpawnPlan {
            session_id: session_id.clone(),
            model: None,
            system_prompt: None,
            capabilities,
            public_auth,
        };

        SessionBuildPlan {
            session_id,
            resolved_model: self.default_model.clone(),
            socket_path,
            spawn,
            seed_messages: Vec::new(),
            thinking_level: None,
            key: Some(key.clone()),
        }
    }

    pub(crate) fn plan_recovered_catalog_session(
        &self,
        entry: &SessionCatalogEntry,
        public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
    ) -> SessionBuildPlan {
        let seed_messages = load_recovery_seed_messages(entry);
        let socket_path = session_socket_path(&entry.session_id);
        let spawn = AgentSpawnPlan {
            session_id: entry.session_id.clone(),
            model: Some(entry.model.clone()),
            system_prompt: None,
            capabilities: None,
            public_auth,
        };

        SessionBuildPlan {
            session_id: entry.session_id.clone(),
            resolved_model: entry.model.clone(),
            socket_path,
            spawn,
            seed_messages,
            thinking_level: None,
            key: None,
        }
    }

    pub(crate) fn plan_recovered_keyed_session(
        &self,
        key: &SessionKey,
        entry: &SessionCatalogEntry,
        public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
    ) -> SessionBuildPlan {
        let mut plan = self.plan_recovered_catalog_session(entry, public_auth);
        plan.key = Some(key.clone());
        plan
    }
}

impl SessionBuildPlan {
    pub(crate) fn session_handle(
        &self,
        cmd_tx: mpsc::UnboundedSender<SessionCommand>,
        event_tx: broadcast::Sender<DaemonEvent>,
    ) -> SessionHandle {
        SessionHandle {
            session_id: self.session_id.clone(),
            model: self.resolved_model.clone(),
            turn_count: 0,
            last_active: chrono::Utc::now().to_rfc3339(),
            client_count: 0,
            cmd_tx: Some(cmd_tx),
            event_tx: Some(event_tx),
            socket_path: self.socket_path.clone(),
            state: "active".to_string(),
        }
    }

    pub(crate) fn catalog_entry(&self, automerge_path: PathBuf, now: String) -> SessionCatalogEntry {
        SessionCatalogEntry {
            session_id: self.session_id.clone(),
            automerge_path,
            model: self.resolved_model.clone(),
            created_at: now.clone(),
            last_active: now,
            turn_count: 0,
            state: SessionLifecycle::Active,
        }
    }

    pub(crate) fn thinking_command(&self) -> Option<SessionCommand> {
        self.thinking_level.as_ref().map(|level| SessionCommand::SetThinkingLevel { level: level.clone() })
    }

    pub(crate) fn seed_command(&self) -> Option<SessionCommand> {
        if self.seed_messages.is_empty() {
            None
        } else {
            Some(SessionCommand::SeedMessages {
                messages: self.seed_messages.clone(),
            })
        }
    }
}

pub(crate) fn load_recovery_seed_messages(entry: &SessionCatalogEntry) -> Vec<SerializedMessage> {
    if !entry.automerge_path.exists() {
        tracing::warn!("recovery automerge file missing at {:?} — starting fresh", entry.automerge_path);
        return Vec::new();
    }

    match clankers_session::SessionManager::open(entry.automerge_path.clone()) {
        Ok(manager) => match manager.build_context() {
            Ok(messages) => {
                let serialized = serialize_seed_messages(&messages);
                tracing::info!("loaded {} recovery messages from {:?}", serialized.len(), entry.automerge_path);
                serialized
            }
            Err(error) => {
                tracing::warn!(
                    "failed to build recovery context from {:?}: {error} — starting fresh",
                    entry.automerge_path
                );
                Vec::new()
            }
        },
        Err(error) => {
            tracing::warn!("failed to open recovery automerge at {:?}: {error} — starting fresh", entry.automerge_path);
            Vec::new()
        }
    }
}

pub(crate) fn serialize_seed_messages(messages: &[clanker_message::AgentMessage]) -> Vec<SerializedMessage> {
    messages
        .iter()
        .filter_map(|message| {
            let (role, content, model) = match message {
                clanker_message::AgentMessage::User(user) => ("user", text_content(&user.content), None),
                clanker_message::AgentMessage::Assistant(assistant) => {
                    ("assistant", text_content(&assistant.content), Some(assistant.model.clone()))
                }
                _ => return None,
            };
            if content.is_empty() {
                None
            } else {
                Some(SerializedMessage {
                    role: role.to_string(),
                    content,
                    model,
                    timestamp: None,
                })
            }
        })
        .collect()
}

fn text_content(content: &[clanker_message::Content]) -> String {
    content
        .iter()
        .filter_map(|part| match part {
            clanker_message::Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn resolve_session_resume_in_dir(
    sessions_dir: &Path,
    resume_id: Option<&str>,
    continue_last: bool,
    cwd: &str,
) -> (String, Vec<SerializedMessage>) {
    let try_open = |file: PathBuf| -> Option<(String, Vec<SerializedMessage>)> {
        let manager = clankers_session::SessionManager::open(file).ok()?;
        let messages = manager.build_context().ok()?;
        Some((manager.session_id().to_string(), serialize_seed_messages(&messages)))
    };

    if let Some(id) = resume_id {
        let files = clankers_session::store::list_sessions(sessions_dir, cwd);
        if let Some(file) = files
            .into_iter()
            .find(|file| file.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.contains(id)))
            && let Some(result) = try_open(file)
        {
            return result;
        }
    }

    if continue_last {
        let files = clankers_session::store::list_sessions(sessions_dir, cwd);
        if let Some(file) = files.into_iter().next()
            && let Some(result) = try_open(file)
        {
            return result;
        }
    }

    (clanker_message::generate_id(), Vec::new())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;
    use clanker_message::AssistantMessage;
    use clanker_message::Content;
    use clanker_message::MessageId;
    use clanker_message::StopReason;
    use clanker_message::Usage;
    use clanker_message::UserMessage;
    use tempfile::tempdir;

    use super::*;

    fn append_resume_fixture(session: &mut clankers_session::SessionManager) {
        let user_id = MessageId::new("user-1");
        session
            .append_message(
                clanker_message::AgentMessage::User(UserMessage {
                    id: user_id.clone(),
                    content: vec![Content::Text {
                        text: "hello from resume".to_string(),
                    }],
                    timestamp: Utc::now(),
                }),
                None,
            )
            .unwrap();
        session
            .append_message(
                clanker_message::AgentMessage::Assistant(AssistantMessage {
                    id: MessageId::new("assistant-1"),
                    content: vec![Content::Text {
                        text: "hello back".to_string(),
                    }],
                    model: "fixture-model".to_string(),
                    usage: Usage::default(),
                    stop_reason: StopReason::Stop,
                    timestamp: Utc::now(),
                }),
                Some(user_id),
            )
            .unwrap();
    }

    #[test]
    fn create_plan_for_new_session_has_spawn_and_handle_data_without_socket() {
        let dir = tempdir().unwrap();
        let builder = SessionBuilder::for_sessions_dir("default-model", dir.path().join("sessions"));

        let plan = builder.plan_create_session(CreateSessionPlanRequest {
            model: None,
            system_prompt: Some("system".to_string()),
            resume_id: None,
            continue_last: false,
            cwd: Some("/tmp/project".to_string()),
            thinking_level: Some("max".to_string()),
        });

        assert_eq!(plan.resolved_model, "default-model");
        assert_eq!(plan.spawn.session_id, plan.session_id);
        assert_eq!(plan.spawn.model, None);
        assert_eq!(plan.spawn.system_prompt.as_deref(), Some("system"));
        let expected_socket_name = format!("session-{}.sock", plan.session_id);
        assert_eq!(plan.socket_path.file_name().and_then(|name| name.to_str()), Some(expected_socket_name.as_str()));
        assert!(matches!(plan.thinking_command(), Some(SessionCommand::SetThinkingLevel { level }) if level == "max"));
        assert!(plan.seed_command().is_none());
    }

    #[test]
    fn create_plan_resolves_resume_messages_without_socket() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        let cwd = dir.path().join("project");
        let cwd_text = cwd.to_string_lossy().to_string();
        let mut session =
            clankers_session::SessionManager::create(&sessions_dir, &cwd_text, "fixture-model", None, None, None)
                .unwrap();
        append_resume_fixture(&mut session);
        let resume_id = session.session_id().to_string();

        let builder = SessionBuilder::for_sessions_dir("default-model", sessions_dir);
        let plan = builder.plan_create_session(CreateSessionPlanRequest {
            model: Some("requested-model".to_string()),
            system_prompt: None,
            resume_id: Some(resume_id.clone()),
            continue_last: false,
            cwd: Some(cwd_text),
            thinking_level: None,
        });

        assert_eq!(plan.session_id, resume_id);
        assert_eq!(plan.resolved_model, "requested-model");
        assert_eq!(plan.seed_messages.len(), 2);
        assert_eq!(plan.seed_messages[0].role, "user");
        assert_eq!(plan.seed_messages[0].content, "hello from resume");
        assert_eq!(plan.seed_messages[1].role, "assistant");
        assert_eq!(plan.seed_messages[1].model.as_deref(), Some("fixture-model"));
        assert!(matches!(plan.seed_command(), Some(SessionCommand::SeedMessages { messages }) if messages.len() == 2));
    }

    #[test]
    fn keyed_plans_prepare_new_and_recovered_actor_inputs_without_socket() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        let cwd = dir.path().join("project");
        let cwd_text = cwd.to_string_lossy().to_string();
        let mut session =
            clankers_session::SessionManager::create(&sessions_dir, &cwd_text, "catalog-model", None, None, None)
                .unwrap();
        append_resume_fixture(&mut session);
        let entry = SessionCatalogEntry {
            session_id: session.session_id().to_string(),
            automerge_path: PathBuf::from(session.file_path()),
            model: "catalog-model".to_string(),
            created_at: "now".to_string(),
            last_active: "now".to_string(),
            turn_count: 7,
            state: SessionLifecycle::Suspended,
        };
        let key = SessionKey::Matrix {
            user_id: "@user:example".to_string(),
            room_id: "!room:example".to_string(),
        };
        let builder = SessionBuilder::for_sessions_dir("default-model", sessions_dir);

        let new_plan = builder.plan_new_keyed_session(&key, None, None);
        assert_eq!(new_plan.resolved_model, "default-model");
        assert_eq!(new_plan.key.as_ref(), Some(&key));
        assert!(new_plan.spawn.model.is_none());
        assert!(new_plan.seed_messages.is_empty());

        let recovered = builder.plan_recovered_keyed_session(&key, &entry, None);
        assert_eq!(recovered.session_id, entry.session_id);
        assert_eq!(recovered.resolved_model, "catalog-model");
        assert_eq!(recovered.key.as_ref(), Some(&key));
        assert_eq!(recovered.spawn.model.as_deref(), Some("catalog-model"));
        assert_eq!(recovered.seed_messages.len(), 2);
    }
}
