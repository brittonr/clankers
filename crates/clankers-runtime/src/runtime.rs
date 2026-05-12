//! Runtime construction and host-facing runtime handle.

use std::sync::Arc;

use crate::AssembledPrompt;
use crate::ConfirmationBroker;
use crate::ConfirmationDecision;
use crate::ConfirmationRequest;
use crate::EchoModelAdapter;
use crate::FailClosedConfirmationBroker;
use crate::ModelAdapter;
use crate::PromptAssembler;
use crate::PromptAssemblyPolicy;
use crate::PromptSources;
use crate::RuntimeError;
use crate::RuntimeServices;
use crate::SessionHandle;
use crate::SessionOptions;
use crate::ToolCatalog;
use crate::boundary::validate_public_runtime_boundary;
use crate::request_confirmation_fail_closed;

/// Runtime construction entrypoint for embedded hosts.
pub struct RuntimeBuilder {
    model: Arc<dyn ModelAdapter>,
    pub(crate) services: RuntimeServices,
    pub(crate) prompt_policy: PromptAssemblyPolicy,
    pub(crate) prompt_sources: PromptSources,
    pub(crate) tool_catalog: ToolCatalog,
    pub(crate) confirmation_broker: Arc<dyn ConfirmationBroker>,
    pub(crate) event_buffer: usize,
}

impl RuntimeBuilder {
    /// Create a builder with safe in-memory defaults and an echo model adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: Arc::new(EchoModelAdapter),
            services: RuntimeServices::in_memory(),
            prompt_policy: PromptAssemblyPolicy::host_context_only(),
            prompt_sources: PromptSources::default(),
            tool_catalog: ToolCatalog::embedding_safe(),
            confirmation_broker: Arc::new(FailClosedConfirmationBroker),
            event_buffer: 128,
        }
    }

    /// Use a host-supplied model adapter.
    #[must_use]
    pub fn model_adapter(mut self, model: Arc<dyn ModelAdapter>) -> Self {
        self.model = model;
        self
    }

    /// Use explicit runtime service implementations.
    #[must_use]
    pub fn services(mut self, services: RuntimeServices) -> Self {
        self.services = services;
        self
    }

    /// Use explicit prompt assembly inputs.
    #[must_use]
    pub fn prompt_assembly(mut self, policy: PromptAssemblyPolicy, sources: PromptSources) -> Self {
        self.prompt_policy = policy;
        self.prompt_sources = sources;
        self
    }

    /// Use a host-defined tool catalog.
    #[must_use]
    pub fn tool_catalog(mut self, catalog: ToolCatalog) -> Self {
        self.tool_catalog = catalog;
        self
    }

    /// Use a host-supplied confirmation broker.
    #[must_use]
    pub fn confirmation_broker(mut self, broker: Arc<dyn ConfirmationBroker>) -> Self {
        self.confirmation_broker = broker;
        self
    }

    /// Set the per-session event channel capacity.
    #[must_use]
    pub fn event_buffer(mut self, event_buffer: usize) -> Self {
        self.event_buffer = event_buffer.max(1);
        self
    }

    /// Build a runtime.
    pub fn build(self) -> Result<Runtime, RuntimeError> {
        validate_public_runtime_boundary()?;
        Ok(Runtime {
            inner: Arc::new(RuntimeInner {
                model: self.model,
                services: self.services,
                prompt_policy: self.prompt_policy,
                prompt_sources: self.prompt_sources,
                tool_catalog: self.tool_catalog,
                confirmation_broker: self.confirmation_broker,
                event_buffer: self.event_buffer,
            }),
        })
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Built runtime handle. Cloneable and cheap.
#[derive(Clone)]
pub struct Runtime {
    pub(crate) inner: Arc<RuntimeInner>,
}

pub(crate) struct RuntimeInner {
    pub(crate) model: Arc<dyn ModelAdapter>,
    pub(crate) services: RuntimeServices,
    pub(crate) prompt_policy: PromptAssemblyPolicy,
    pub(crate) prompt_sources: PromptSources,
    pub(crate) tool_catalog: ToolCatalog,
    pub(crate) confirmation_broker: Arc<dyn ConfirmationBroker>,
    pub(crate) event_buffer: usize,
}

impl Runtime {
    /// Create a new host-facing session.
    pub async fn create_session(&self, options: SessionOptions) -> Result<SessionHandle, RuntimeError> {
        SessionHandle::new(Arc::clone(&self.inner), options)
    }

    /// Return the catalog published to embedded hosts.
    #[must_use]
    pub fn tool_catalog(&self) -> &ToolCatalog {
        &self.inner.tool_catalog
    }

    /// Assemble a prompt with the runtime policy.
    pub fn assemble_prompt(&self, user_prompt: impl Into<String>) -> Result<AssembledPrompt, RuntimeError> {
        PromptAssembler::assemble(&self.inner.prompt_policy, &self.inner.prompt_sources, user_prompt.into())
    }

    /// Ask the confirmation broker through the same fail-closed substrate used by sessions.
    pub async fn request_confirmation(
        &self,
        request: ConfirmationRequest,
    ) -> Result<ConfirmationDecision, RuntimeError> {
        request_confirmation_fail_closed(self.inner.confirmation_broker.as_ref(), request).await
    }

    /// Execute a host action only after the broker approves the typed request.
    pub async fn run_confirmed_action<T>(
        &self,
        request: ConfirmationRequest,
        action: impl FnOnce() -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let decision = self.request_confirmation(request).await?;
        if !decision.approved {
            return Err(RuntimeError::ConfirmationDenied(decision.reason));
        }
        action()
    }
}
