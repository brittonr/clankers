//! Host-facing session identifiers, options, and handles.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::EventMetadata;
use crate::ModelRequest;
use crate::PromptAssembler;
use crate::PromptId;
use crate::PromptInput;
use crate::PromptReceipt;
use crate::PromptReplayEntry;
use crate::RuntimeError;
use crate::SessionEvent;
use crate::SessionRecord;
use crate::StopReason;
use crate::runtime::RuntimeInner;

/// Stable identifier for a host-facing runtime session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Generate a fresh session id for an embedded host.
    #[must_use]
    pub fn new() -> Self {
        Self(format!("session_{}", Uuid::new_v4()))
    }

    /// Build a session id from host-owned storage.
    #[must_use]
    pub fn from_host(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the stable id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Options used when creating an embedded session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionOptions {
    pub session_id: Option<SessionId>,
    pub model: Option<String>,
}

/// Host-facing session handle.
#[derive(Clone)]
pub struct SessionHandle {
    runtime: Arc<RuntimeInner>,
    state: Arc<Mutex<SessionState>>,
    events: Arc<Mutex<Option<mpsc::Receiver<SessionEvent>>>>,
    tx: mpsc::Sender<SessionEvent>,
}

#[derive(Debug, Clone)]
struct SessionState {
    session_id: SessionId,
    model: Option<String>,
    disabled_tools: BTreeSet<String>,
    is_shutdown: bool,
}

impl SessionHandle {
    pub(crate) fn new(runtime: Arc<RuntimeInner>, options: SessionOptions) -> Result<Self, RuntimeError> {
        let session_id = options.session_id.unwrap_or_default();
        let (tx, rx) = mpsc::channel(runtime.event_buffer);
        let state = SessionState {
            session_id: session_id.clone(),
            model: options.model,
            disabled_tools: BTreeSet::new(),
            is_shutdown: false,
        };
        runtime.services.sessions.save(SessionRecord {
            session_id: session_id.clone(),
            created_at: Utc::now(),
            last_prompt: None,
            prompts: Vec::new(),
        })?;
        Ok(Self {
            runtime,
            state: Arc::new(Mutex::new(state)),
            events: Arc::new(Mutex::new(Some(rx))),
            tx,
        })
    }

    /// Return the session id without exposing daemon/session protocol frames.
    pub async fn session_id(&self) -> SessionId {
        self.state.lock().await.session_id.clone()
    }

    /// Take the semantic event receiver. A session exposes one ordered event stream.
    pub async fn take_events(&self) -> Result<mpsc::Receiver<SessionEvent>, RuntimeError> {
        self.events.lock().await.take().ok_or(RuntimeError::EventStreamAlreadyTaken)
    }

    /// Submit one prompt and emit typed semantic events in causal order.
    pub async fn submit_prompt(&self, input: PromptInput) -> Result<PromptReceipt, RuntimeError> {
        let (session_id, model, disabled_tools) = {
            let state = self.state.lock().await;
            if state.is_shutdown {
                return Err(RuntimeError::SessionShutdown);
            }
            (state.session_id.clone(), state.model.clone(), state.disabled_tools.clone())
        };

        let assembled =
            PromptAssembler::assemble(&self.runtime.prompt_policy, &self.runtime.prompt_sources, input.text)?;
        let prompt_id = PromptId::new();
        let safe_metadata = EventMetadata::new(session_id.clone())
            .with("prompt_id", prompt_id.as_str())
            .with("model", model.clone().unwrap_or_else(|| "default".to_string()))
            .with("prompt_chars", assembled.user_prompt.chars().count().to_string())
            .with("disabled_tool_count", disabled_tools.len().to_string());

        self.emit(SessionEvent::PromptAccepted {
            prompt_id: prompt_id.clone(),
            metadata: safe_metadata.clone(),
        })
        .await?;

        let request = ModelRequest {
            session_id: session_id.clone(),
            prompt_id: prompt_id.clone(),
            model,
            prompt: assembled.clone(),
            disabled_tools,
        };
        match self.runtime.model.complete(request) {
            Ok(response) => {
                for event in response.events {
                    self.emit(event.with_session_metadata(session_id.clone(), prompt_id.clone())).await?;
                }
                let mut record = self
                    .runtime
                    .services
                    .sessions
                    .load(&session_id)?
                    .unwrap_or_else(|| SessionRecord::new(session_id.clone()));
                record.last_prompt = Some(prompt_id.clone());
                record.prompts.push(PromptReplayEntry {
                    prompt_id: prompt_id.clone(),
                    user_prompt: assembled.user_prompt.clone(),
                    assembled_prompt: assembled.clone(),
                    completed_at: Utc::now(),
                });
                self.runtime.services.sessions.save(SessionRecord {
                    session_id: session_id.clone(),
                    ..record
                })?;
                self.emit(SessionEvent::Completed {
                    prompt_id: prompt_id.clone(),
                    stop_reason: StopReason::Complete,
                    metadata: EventMetadata::new(session_id).with("prompt_id", prompt_id.as_str()),
                })
                .await?;
            }
            Err(error) => {
                self.emit(SessionEvent::Error {
                    prompt_id: Some(prompt_id.clone()),
                    message: error.safe_message(),
                    error_class: error.class(),
                    metadata: EventMetadata::new(session_id).with("prompt_id", prompt_id.as_str()),
                })
                .await?;
                return Err(error);
            }
        }
        Ok(PromptReceipt { prompt_id })
    }

    /// Request cancellation/interrupt. The first slice emits a terminal semantic event.
    pub async fn interrupt(&self) -> Result<(), RuntimeError> {
        let session_id = self.session_id().await;
        self.emit(SessionEvent::Completed {
            prompt_id: PromptId::from_host("interrupt"),
            stop_reason: StopReason::Interrupted,
            metadata: EventMetadata::new(session_id),
        })
        .await
    }

    /// Update the preferred model for later prompts.
    pub async fn set_model(&self, model: impl Into<String>) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().await;
        if state.is_shutdown {
            return Err(RuntimeError::SessionShutdown);
        }
        state.model = Some(model.into());
        Ok(())
    }

    /// Replace the disabled tool set for later prompts.
    pub async fn set_disabled_tools(&self, tools: impl IntoIterator<Item = String>) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().await;
        if state.is_shutdown {
            return Err(RuntimeError::SessionShutdown);
        }
        state.disabled_tools = tools.into_iter().collect();
        Ok(())
    }

    /// Shut down the session and emit a final typed event.
    pub async fn shutdown(&self) -> Result<(), RuntimeError> {
        let session_id = {
            let mut state = self.state.lock().await;
            state.is_shutdown = true;
            state.session_id.clone()
        };
        self.emit(SessionEvent::Shutdown {
            metadata: EventMetadata::new(session_id),
        })
        .await
    }

    async fn emit(&self, event: SessionEvent) -> Result<(), RuntimeError> {
        self.tx.send(event).await.map_err(|_| RuntimeError::EventStreamClosed)
    }
}
