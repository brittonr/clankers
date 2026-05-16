//! Host-facing embedding facade for Clankers.
//!
//! This crate is intentionally transport-neutral. Public types model sessions,
//! prompts, tools, confirmation decisions, prompt assembly, and runtime-owned
//! services without exposing daemon frames, TUI state, CLI arguments, ACP/MCP
//! envelopes, or Matrix adapter types.

#[cfg(test)]
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
#[cfg(test)]
use serde_json::json;
use thiserror::Error;

mod boundary;
pub mod confirmation;
pub mod effects;
mod event_summary;
pub mod events;
pub mod prompt;
pub mod runtime;
pub mod services;
pub mod session;
pub mod tools;

#[cfg(test)]
use boundary::public_type_names;
pub use confirmation::ConfirmationAction;
pub use confirmation::ConfirmationBroker;
pub use confirmation::ConfirmationDecision;
pub use confirmation::ConfirmationFuture;
pub use confirmation::ConfirmationRequest;
pub use confirmation::FailClosedConfirmationBroker;
pub use confirmation::request_confirmation_fail_closed;
pub use effects::EffectAbilityClass;
pub use effects::EffectCorrelationId;
pub use effects::EffectHandler;
pub use effects::EffectRequest;
pub use effects::EffectRequestRef;
pub use effects::EffectResult;
pub use effects::EffectResultStatus;
pub use event_summary::headless_prompt_parity_fixture;
pub use event_summary::safe_event_summary;
pub use events::ErrorClass;
pub use events::EventMetadata;
pub use events::SessionEvent;
pub use events::StopReason;
pub use events::ToolStatus;
#[cfg(test)]
use events::contains_secret_marker;
use events::sanitize_metadata_value;
pub use prompt::AssembledPrompt;
pub use prompt::ContextReferenceKind;
pub use prompt::ContextReferenceRequest;
pub use prompt::EchoModelAdapter;
pub use prompt::HostContext;
pub use prompt::ModelAdapter;
pub use prompt::ModelRequest;
pub use prompt::ModelResponse;
pub use prompt::PromptAssembler;
pub use prompt::PromptAssemblyPolicy;
pub use prompt::PromptId;
pub use prompt::PromptInput;
pub use prompt::PromptProvenance;
pub use prompt::PromptReceipt;
pub use prompt::PromptSection;
pub use prompt::PromptSourceKind;
pub use prompt::PromptSources;
pub use prompt::UnsupportedContextReference;
pub use runtime::Runtime;
pub use runtime::RuntimeBuilder;
pub use services::AuthService;
pub use services::AuthStoreAccessRequest;
pub use services::AuthStoreOperation;
pub use services::CacheStore;
pub use services::CheckpointStore;
pub use services::CredentialPoolPolicyService;
pub use services::CredentialPoolRequest;
pub use services::DesktopRuntimeServices;
pub use services::DisabledExtensionService;
pub use services::ExtensionAuthStoreService;
pub use services::ExtensionReceipt;
pub use services::ExtensionRuntimeKind;
pub use services::ExtensionRuntimeRequest;
pub use services::ExtensionRuntimeService;
pub use services::ExtensionServices;
pub use services::ExtensionStatus;
pub use services::ExtensionToolDescriptor;
pub use services::InMemorySessionStore;
pub use services::NoopService;
pub use services::PluginStore;
pub use services::ProjectContextService;
pub use services::PromptReplayEntry;
pub use services::ProviderExecutionRequest;
pub use services::ProviderRouterService;
pub use services::RuntimeServices;
pub use services::SessionRecord;
pub use services::SessionStore;
pub use services::SettingsService;
pub use services::SkillStore;
pub use session::SessionHandle;
pub use session::SessionId;
pub use session::SessionOptions;
pub use tools::CapabilityPack;
pub use tools::SideEffectLevel;
pub use tools::ToolCatalog;
pub use tools::ToolCatalogBuilder;
pub use tools::ToolCatalogOmission;
pub use tools::ToolCollisionPolicy;
pub use tools::ToolDescriptor;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeError {
    #[error("invalid prompt: {0}")]
    InvalidPrompt(String),
    #[error("event stream has already been taken")]
    EventStreamAlreadyTaken,
    #[error("event stream is closed")]
    EventStreamClosed,
    #[error("session is shut down")]
    SessionShutdown,
    #[error("filesystem discovery is disabled")]
    FilesystemDiscoveryDisabled,
    #[error("invalid tool: {0}")]
    InvalidTool(String),
    #[error("tool name collision: {0}")]
    ToolNameCollision(String),
    #[error("store unavailable: {0}")]
    StoreUnavailable(String),
    #[error("confirmation unavailable: {0}")]
    ConfirmationUnavailable(String),
    #[error("confirmation timed out")]
    ConfirmationTimedOut,
    #[error("confirmation cancelled")]
    ConfirmationCancelled,
    #[error("confirmation denied: {0}")]
    ConfirmationDenied(String),
    #[error("extension unavailable: {0}")]
    ExtensionUnavailable(String),
    #[error("public runtime boundary leaked adapter type: {0}")]
    PublicBoundaryLeak(String),
    #[error("model failed: {0}")]
    Model(String),
}

impl RuntimeError {
    #[must_use]
    pub fn safe_message(&self) -> String {
        sanitize_metadata_value(self.to_string())
    }

    #[must_use]
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::InvalidPrompt(_) => ErrorClass::InvalidInput,
            Self::EventStreamAlreadyTaken | Self::EventStreamClosed | Self::SessionShutdown => ErrorClass::Session,
            Self::FilesystemDiscoveryDisabled => ErrorClass::Policy,
            Self::InvalidTool(_) | Self::ToolNameCollision(_) => ErrorClass::Tooling,
            Self::StoreUnavailable(_) => ErrorClass::Storage,
            Self::ConfirmationUnavailable(_)
            | Self::ConfirmationTimedOut
            | Self::ConfirmationCancelled
            | Self::ConfirmationDenied(_) => ErrorClass::Confirmation,
            Self::ExtensionUnavailable(_) => ErrorClass::Extension,
            Self::PublicBoundaryLeak(_) => ErrorClass::Boundary,
            Self::Model(_) => ErrorClass::Model,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::validate_public_runtime_boundary;

    struct ScriptedModel;

    impl ModelAdapter for ScriptedModel {
        fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            Ok(ModelResponse {
                events: vec![
                    SessionEvent::ThinkingDelta {
                        prompt_id: request.prompt_id.clone(),
                        text: "thinking".to_string(),
                        metadata: EventMetadata::empty().with("source", "scripted"),
                    },
                    SessionEvent::AssistantDelta {
                        prompt_id: request.prompt_id.clone(),
                        text: "done".to_string(),
                        metadata: EventMetadata::empty().with("source", "scripted"),
                    },
                ],
            })
        }
    }

    #[tokio::test]
    async fn runtime_facade_streams_host_events_in_order() {
        let runtime = RuntimeBuilder::new().model_adapter(Arc::new(ScriptedModel)).build().unwrap();
        let session = runtime
            .create_session(SessionOptions {
                session_id: Some(SessionId::from_host("host-session")),
                model: None,
            })
            .await
            .unwrap();
        let mut events = session.take_events().await.unwrap();
        let receipt = session.submit_prompt(PromptInput::new("hello host")).await.unwrap();
        assert!(receipt.prompt_id.as_str().starts_with("prompt_"));

        let mut kinds = Vec::new();
        for _ in 0..4 {
            let event = events.recv().await.unwrap();
            kinds.push(safe_event_summary(&event)["type"].as_str().unwrap().to_string());
            match event {
                SessionEvent::PromptAccepted { metadata, .. }
                | SessionEvent::ThinkingDelta { metadata, .. }
                | SessionEvent::AssistantDelta { metadata, .. }
                | SessionEvent::Completed { metadata, .. } => {
                    assert_eq!(metadata.session_id.as_ref().unwrap().as_str(), "host-session");
                    assert!(!metadata.contains_secret_markers());
                }
                _ => {}
            }
        }
        assert_eq!(kinds, vec!["prompt_accepted", "thinking_delta", "assistant_delta", "completed"]);
    }

    #[tokio::test]
    async fn default_runtime_does_not_need_ambient_paths() {
        let runtime = RuntimeBuilder::new().build().unwrap();
        let metadata = runtime.inner.services.capability_metadata();
        assert_eq!(metadata.fields.get("settings").unwrap(), "noop");
        assert_eq!(metadata.fields.get("auth").unwrap(), "noop");
        assert_eq!(metadata.fields.get("sessions").unwrap(), "in_memory");
        assert_eq!(metadata.fields.get("provider_router").unwrap(), "disabled");
        assert_eq!(metadata.fields.get("extension_auth_store").unwrap(), "disabled");
        assert_eq!(metadata.fields.get("credential_pool").unwrap(), "disabled");
        assert_eq!(metadata.fields.get("extension_runtime").unwrap(), "disabled");
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        session.submit_prompt(PromptInput::new("no ambient path access")).await.unwrap();
    }

    #[test]
    fn disabled_extension_services_fail_closed_without_startup_side_effects() {
        let extensions = ExtensionServices::disabled();
        assert_eq!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Plugin).unwrap(), Vec::new());
        assert_eq!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Mcp).unwrap(), Vec::new());
        assert_eq!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Gateway).unwrap(), Vec::new());

        let provider_error = extensions
            .provider_router
            .execute(ProviderExecutionRequest {
                provider: "openai-codex".to_string(),
                model: Some("gpt-5.3-codex".to_string()),
                account_label: Some("desktop".to_string()),
                route_source: "embedded".to_string(),
                prompt: Some("hello".to_string()),
                system_prompt: None,
                max_tokens: Some(8),
                session_id: Some("session-runtime-test".to_string()),
            })
            .unwrap_err();
        assert_eq!(provider_error, RuntimeError::ExtensionUnavailable("provider router disabled".to_string()));

        let auth_error = extensions
            .auth_store
            .access(AuthStoreAccessRequest {
                provider: "anthropic".to_string(),
                account_label: Some("default".to_string()),
                operation: AuthStoreOperation::PendingLoginVerifier,
            })
            .unwrap_err();
        assert_eq!(auth_error, RuntimeError::ExtensionUnavailable("extension auth store disabled".to_string()));

        let pool_error = extensions
            .credential_pool
            .select(CredentialPoolRequest {
                provider: "anthropic".to_string(),
                strategy: "fill_first".to_string(),
                account_label: None,
            })
            .unwrap_err();
        assert_eq!(pool_error, RuntimeError::ExtensionUnavailable("credential pool disabled".to_string()));

        let runtime_error = extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("plugin_secret_token_runtime".to_string()),
                visible_tool_name: Some("plugin_secret_token_tool".to_string()),
                original_tool_name: Some("raw".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: json!({"secret_token": "abc123"}),
            })
            .unwrap_err();
        assert_eq!(runtime_error, RuntimeError::ExtensionUnavailable("extension runtime disabled".to_string()));
    }

    #[test]
    fn extension_receipts_and_descriptors_redact_secret_like_metadata() {
        let receipt =
            ExtensionReceipt::new("bearer token provider", "authorization header call", ExtensionStatus::Failed)
                .with_metadata("api_key", "abc123")
                .with_metadata("provider", "anthropic")
                .with_error_class(ErrorClass::Extension);
        assert_eq!(receipt.source, "[REDACTED]");
        assert_eq!(receipt.action, "[REDACTED]");
        assert_eq!(receipt.metadata.fields.get("api_key").unwrap(), "[REDACTED]");
        assert_eq!(receipt.metadata.fields.get("provider").unwrap(), "anthropic");
        assert!(!receipt.contains_secret_markers());

        let descriptor = ExtensionToolDescriptor::new(
            ExtensionRuntimeKind::Mcp,
            "mcp_authorization_header_tool",
            Some("plugin token payload".to_string()),
            SideEffectLevel::ExternalIo,
        );
        assert_eq!(descriptor.visible_tool_name, "[REDACTED]");
        assert_eq!(descriptor.original_tool_name.as_deref(), Some("[REDACTED]"));
        assert!(!descriptor.metadata.contains_secret_markers());
    }

    struct StaticExtensionService;

    impl ProviderRouterService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_provider_router"
        }

        fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(ExtensionReceipt::new("host_provider_router", "execute", ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source))
        }
    }

    impl ExtensionAuthStoreService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_auth_store"
        }

        fn access(&self, request: AuthStoreAccessRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(
                ExtensionReceipt::new(
                    "host_auth_store",
                    format!("{:?}", request.operation),
                    ExtensionStatus::Succeeded,
                )
                .with_metadata("provider", request.provider),
            )
        }
    }

    impl CredentialPoolPolicyService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_credential_pool"
        }

        fn select(&self, request: CredentialPoolRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(ExtensionReceipt::new("host_credential_pool", request.strategy, ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider))
        }
    }

    impl ExtensionRuntimeService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_extension_runtime"
        }

        fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
            Ok(vec![ExtensionToolDescriptor::new(
                kind,
                "host_visible_tool",
                Some("original_tool".to_string()),
                SideEffectLevel::ExternalIo,
            )])
        }

        fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(ExtensionReceipt::new("host_extension_runtime", request.action, ExtensionStatus::Succeeded))
        }
    }

    #[test]
    fn host_supplied_extension_services_are_explicit_capabilities() {
        let service = Arc::new(StaticExtensionService);
        let extensions = ExtensionServices {
            provider_router: service.clone(),
            auth_store: service.clone(),
            credential_pool: service.clone(),
            runtime: service,
        };
        let metadata = extensions.capability_metadata();
        assert_eq!(metadata.fields.get("provider_router").unwrap(), "host_provider_router");
        assert_eq!(metadata.fields.get("auth_store").unwrap(), "host_auth_store");
        assert_eq!(metadata.fields.get("credential_pool").unwrap(), "host_credential_pool");
        assert_eq!(metadata.fields.get("runtime").unwrap(), "host_extension_runtime");

        let tools = extensions.runtime.publishable_tools(ExtensionRuntimeKind::Mcp).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].visible_tool_name, "host_visible_tool");
    }

    #[test]
    fn public_api_boundary_rejects_transport_type_leakage() {
        validate_public_runtime_boundary().unwrap();
        let names = public_type_names().join("\n");
        for denied in ["DaemonEvent", "SessionCommand", "Tui", "Acp", "Mcp", "Cli"] {
            assert!(!names.contains(denied), "public API leaked {denied}");
        }
    }

    #[tokio::test]
    async fn fake_provider_prompt_matches_headless_parity_fixture() {
        let runtime = RuntimeBuilder::new().build().unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("parity")).await.unwrap();
        let mut kinds = Vec::new();
        for _ in 0..4 {
            kinds.push(safe_event_summary(&events.recv().await.unwrap())["type"].as_str().unwrap().to_string());
        }
        assert_eq!(kinds, headless_prompt_parity_fixture("parity"));
    }

    #[test]
    fn tool_catalog_embedding_safe_excludes_dangerous_packs() {
        let catalog = ToolCatalog::embedding_safe();
        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("bash"));
        assert!(!catalog.packs().contains(&CapabilityPack::ShellCommands));
    }

    #[test]
    fn tool_catalog_supports_custom_tool_collision_policy() {
        let descriptor = ToolDescriptor::new("host_search", "host search", SideEffectLevel::ReadOnly);
        let builder = ToolCatalog::builder().pack(CapabilityPack::ReadOnly).custom_tool(descriptor.clone()).unwrap();
        let err = builder.custom_tool(descriptor).unwrap_err();
        assert_eq!(err, RuntimeError::ToolNameCollision("host_search".to_string()));
    }

    #[test]
    fn tool_catalog_filters_disabled_tools_from_host_metadata() {
        let custom = ToolDescriptor::new("host_search", "host search", SideEffectLevel::ReadOnly);
        let catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .pack(CapabilityPack::ShellCommands)
            .disabled_tools(["search", "bash", "host_search"])
            .custom_tool(custom)
            .unwrap()
            .build()
            .unwrap();

        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("search"));
        assert!(!catalog.contains_tool("bash"));
        assert!(!catalog.contains_tool("host_search"));
        assert!(catalog.tools().all(|tool| !matches!(tool.name.as_str(), "search" | "bash" | "host_search")));
    }

    #[derive(Default)]
    struct CountingExtensionRuntimeService {
        publish_calls: std::sync::atomic::AtomicUsize,
        execute_calls: std::sync::atomic::AtomicUsize,
    }

    impl ExtensionRuntimeService for CountingExtensionRuntimeService {
        fn capability(&self) -> &'static str {
            "counting_extension_runtime"
        }

        fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
            self.publish_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![ExtensionToolDescriptor::new(
                kind,
                "plugin_echo",
                Some("echo".to_string()),
                SideEffectLevel::ExternalIo,
            )])
        }

        fn execute(&self, _request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
            self.execute_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ExtensionReceipt::new("counting_extension_runtime", "execute", ExtensionStatus::Succeeded))
        }
    }

    #[test]
    fn tool_catalog_capability_pack_matrix_does_not_expand_dangerous_packs() {
        let cases = [
            (vec![CapabilityPack::ReadOnly], vec!["read", "search"], vec!["write", "bash", "web", "process"]),
            (
                vec![CapabilityPack::ReadOnly, CapabilityPack::WorkspaceMutation],
                vec!["read", "write", "patch"],
                vec!["bash", "web", "process"],
            ),
            (vec![CapabilityPack::ShellCommands], vec!["bash"], vec!["web", "process"]),
            (vec![CapabilityPack::Network], vec!["web"], vec!["bash", "process"]),
            (vec![CapabilityPack::ExternalProcesses], vec!["process"], vec!["bash", "web"]),
        ];
        for (packs, included, excluded) in cases {
            let mut builder = ToolCatalog::builder();
            for pack in packs {
                builder = builder.pack(pack);
            }
            let catalog = builder.build().unwrap();
            for name in included {
                assert!(catalog.contains_tool(name), "expected tool {name}");
            }
            for name in excluded {
                assert!(!catalog.contains_tool(name), "unexpected tool {name}");
            }
            for descriptor in catalog.tools() {
                assert_eq!(descriptor.requires_confirmation, descriptor.side_effect.requires_confirmation());
            }
        }
    }

    #[test]
    fn capability_packs_map_to_only_their_effect_classes() {
        let cases = [
            (CapabilityPack::ReadOnly, vec![EffectAbilityClass::Filesystem]),
            (CapabilityPack::WorkspaceMutation, vec![EffectAbilityClass::Filesystem]),
            (CapabilityPack::ShellCommands, vec![EffectAbilityClass::Shell]),
            (CapabilityPack::Network, vec![EffectAbilityClass::Network]),
            (CapabilityPack::ExternalProcesses, vec![EffectAbilityClass::Shell]),
        ];

        for (pack, expected) in cases {
            let actual = pack.effect_classes().into_iter().collect::<Vec<_>>();
            assert_eq!(actual, expected, "pack {pack:?} expanded effect classes");
        }
    }

    #[test]
    fn tool_descriptor_maps_known_dangerous_tools_to_specific_effect_classes() {
        let bash = ToolDescriptor::new("bash", "shell", SideEffectLevel::Dangerous);
        let process = ToolDescriptor::new("process", "process", SideEffectLevel::Dangerous);
        let custom = ToolDescriptor::new("custom-danger", "custom", SideEffectLevel::Dangerous);

        assert_eq!(bash.effect_class(), EffectAbilityClass::Shell);
        assert_eq!(process.effect_class(), EffectAbilityClass::Shell);
        assert_eq!(custom.effect_class(), EffectAbilityClass::Tool);
    }

    #[test]
    fn tool_catalog_disabled_filter_overrides_packs_with_safe_omissions() {
        let catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .pack(CapabilityPack::ShellCommands)
            .disabled_tools(["search", "bash"])
            .build()
            .unwrap();
        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("search"));
        assert!(!catalog.contains_tool("bash"));
        assert!(
            catalog
                .omissions()
                .iter()
                .any(|item| item.name == "search" && item.reason == "disabled_by_host_filter")
        );
        assert!(
            catalog
                .omissions()
                .iter()
                .any(|item| item.name == "bash" && item.reason == "disabled_by_host_filter")
        );
        let serialized = serde_json::to_string(catalog.omissions()).unwrap();
        assert!(!serialized.contains("TOKEN"));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn tool_catalog_custom_tools_apply_collision_policy_matrix() {
        let host_search =
            ToolDescriptor::new("host_search", "host search", SideEffectLevel::ReadOnly).with_source("host");
        let catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .custom_tool(host_search)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(catalog.tools().find(|tool| tool.name == "host_search").unwrap().source, "host");

        let collision =
            ToolDescriptor::new("read", "host read", SideEffectLevel::WorkspaceMutation).with_source("host");
        let err = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .collision_policy(ToolCollisionPolicy::Reject)
            .custom_tool(collision.clone())
            .unwrap_err();
        assert_eq!(err, RuntimeError::ToolNameCollision("read".to_string()));

        let keep = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .collision_policy(ToolCollisionPolicy::KeepExisting)
            .custom_tool(collision.clone())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(keep.tools().find(|tool| tool.name == "read").unwrap().source, "clankers");
        assert!(keep.omissions().iter().any(|item| item.reason == "name_collision_existing_kept"));

        let override_catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .collision_policy(ToolCollisionPolicy::HostOverrides)
            .custom_tool(collision)
            .unwrap()
            .build()
            .unwrap();
        let read = override_catalog.tools().find(|tool| tool.name == "read").unwrap();
        assert_eq!(read.source, "host");
        assert_eq!(read.side_effect, SideEffectLevel::WorkspaceMutation);
    }

    #[test]
    fn tool_catalog_extension_descriptors_require_runtime_availability_without_execute() {
        let disabled = ExtensionServices::disabled();
        let disabled_catalog = ToolCatalog::builder()
            .extension_runtime_tools(ExtensionRuntimeKind::Plugin, disabled.runtime.as_ref())
            .unwrap()
            .build()
            .unwrap();
        assert!(!disabled_catalog.contains_tool("plugin_echo"));

        let runtime = CountingExtensionRuntimeService::default();
        let catalog = ToolCatalog::builder()
            .extension_runtime_tools(ExtensionRuntimeKind::Plugin, &runtime)
            .unwrap()
            .build()
            .unwrap();
        let tool = catalog.tools().find(|tool| tool.name == "plugin_echo").unwrap();
        assert_eq!(tool.source, "extension:plugin");
        assert_eq!(tool.side_effect, SideEffectLevel::ExternalIo);
        assert_eq!(runtime.publish_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tool_catalog_metadata_query_does_not_start_extension_runtimes() {
        let runtime = CountingExtensionRuntimeService::default();
        let catalog = ToolCatalog::builder()
            .extension_runtime_tools(ExtensionRuntimeKind::Plugin, &runtime)
            .unwrap()
            .build()
            .unwrap();
        let _ = catalog.tools().collect::<Vec<_>>();
        assert!(catalog.contains_tool("plugin_echo"));
        assert!(catalog.omissions().is_empty());
        assert_eq!(runtime.publish_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn runtime_extension_service_matrix_default_safe_fails_closed_independently() {
        let extensions = ExtensionServices::disabled();
        assert!(matches!(
            extensions.provider_router.execute(provider_request()),
            Err(RuntimeError::ExtensionUnavailable(_))
        ));
        assert!(matches!(extensions.auth_store.access(auth_request()), Err(RuntimeError::ExtensionUnavailable(_))));
        assert!(matches!(
            extensions.credential_pool.select(pool_request()),
            Err(RuntimeError::ExtensionUnavailable(_))
        ));
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Plugin).unwrap().is_empty());
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Mcp).unwrap().is_empty());
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Gateway).unwrap().is_empty());
        assert!(matches!(extensions.runtime.execute(runtime_request()), Err(RuntimeError::ExtensionUnavailable(_))));
    }

    fn provider_request() -> ProviderExecutionRequest {
        ProviderExecutionRequest {
            provider: "anthropic".to_string(),
            model: Some("test".to_string()),
            account_label: Some("primary".to_string()),
            route_source: "embedded".to_string(),
            prompt: Some("hello".to_string()),
            system_prompt: None,
            max_tokens: Some(8),
            session_id: Some("session".to_string()),
        }
    }

    fn auth_request() -> AuthStoreAccessRequest {
        AuthStoreAccessRequest {
            provider: "anthropic".to_string(),
            account_label: Some("primary".to_string()),
            operation: AuthStoreOperation::Lookup,
        }
    }

    fn pool_request() -> CredentialPoolRequest {
        CredentialPoolRequest {
            provider: "anthropic".to_string(),
            strategy: "fill_first".to_string(),
            account_label: Some("primary".to_string()),
        }
    }

    fn runtime_request() -> ExtensionRuntimeRequest {
        ExtensionRuntimeRequest {
            kind: ExtensionRuntimeKind::Plugin,
            action: "execute".to_string(),
            extension_name: Some("plugin".to_string()),
            visible_tool_name: Some("tool".to_string()),
            original_tool_name: Some("tool".to_string()),
            runtime_entrypoint: Some("main".to_string()),
            arguments: json!({}),
        }
    }

    #[derive(Default)]
    struct CountingProviderRouterService(std::sync::atomic::AtomicUsize);

    impl ProviderRouterService for CountingProviderRouterService {
        fn capability(&self) -> &'static str {
            "injected_provider_router"
        }
        fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ExtensionReceipt::new("injected_provider_router", "execute", ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source))
        }
    }

    #[test]
    fn runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback() {
        let provider = Arc::new(CountingProviderRouterService::default());
        let extensions = ExtensionServices {
            provider_router: provider.clone(),
            auth_store: Arc::new(DisabledExtensionService),
            credential_pool: Arc::new(DisabledExtensionService),
            runtime: Arc::new(DisabledExtensionService),
        };
        let receipt = extensions.provider_router.execute(provider_request()).unwrap();
        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert!(matches!(extensions.auth_store.access(auth_request()), Err(RuntimeError::ExtensionUnavailable(_))));
        assert!(matches!(
            extensions.credential_pool.select(pool_request()),
            Err(RuntimeError::ExtensionUnavailable(_))
        ));
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Plugin).unwrap().is_empty());
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn runtime_extension_service_matrix_injected_error_receipts_are_redacted() {
        let receipt = ExtensionReceipt::new("provider_router", "execute", ExtensionStatus::Failed)
            .with_metadata("api_key", "secret-token")
            .with_metadata("prompt_bytes", "123")
            .with_error_class(ErrorClass::Extension);
        let serialized = serde_json::to_string(&receipt).unwrap();
        assert!(!receipt.contains_secret_markers());
        assert!(!serialized.contains("secret-token"));
        assert!(serialized.contains("prompt_bytes"));
    }

    #[test]
    fn runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error() {
        for status in [
            ExtensionStatus::Succeeded,
            ExtensionStatus::Unavailable,
            ExtensionStatus::Failed,
        ] {
            let receipt = ExtensionReceipt::new("provider", "execute", status)
                .with_metadata("provider", "anthropic")
                .with_metadata("output_bytes", "42")
                .with_metadata("bearer_token", "secret");
            let serialized = serde_json::to_string(&receipt).unwrap();
            assert!(!receipt.contains_secret_markers());
            assert!(serialized.contains("anthropic"));
            assert!(serialized.contains("42"));
            assert!(!serialized.contains("secret"));
        }
    }

    #[test]
    fn prompt_assembly_host_context_only_redacts_provenance_content() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            system_prompt: Some("system token secret".to_string()),
            host_context: vec![HostContext {
                label: "app".to_string(),
                content: "safe app context".to_string(),
            }],
            filesystem_context_requested: false,
            ..PromptSources::default()
        };
        let assembled = PromptAssembler::assemble(&policy, &sources, "hello".to_string()).unwrap();
        assert_eq!(assembled.sections[0].content, "safe app context");
        assert_eq!(assembled.sections[1].content, "[REDACTED]");
        assert!(assembled.provenance.iter().all(|item| !contains_secret_marker(&item.safe_summary)));
    }

    #[test]
    fn prompt_assembly_rejects_filesystem_discovery_when_disabled() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            filesystem_context_requested: true,
            ..PromptSources::default()
        };
        assert_eq!(
            PromptAssembler::assemble(&policy, &sources, "hello".to_string()).unwrap_err(),
            RuntimeError::FilesystemDiscoveryDisabled
        );
    }

    #[test]
    fn prompt_assembly_reports_disabled_context_references_without_content() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            context_references: vec![ContextReferenceRequest::new(
                "src/secret-token.rs",
                ContextReferenceKind::File,
            )],
            ..PromptSources::default()
        };
        let assembled = PromptAssembler::assemble(&policy, &sources, "hello".to_string()).unwrap();

        assert!(!assembled.context_references_enabled);
        assert_eq!(assembled.unsupported_context_references.len(), 1);
        let unsupported = &assembled.unsupported_context_references[0];
        assert_eq!(unsupported.label, "[REDACTED]");
        assert_eq!(unsupported.kind, ContextReferenceKind::File);
        assert!(unsupported.reason.contains("disabled"));
        assert!(assembled.sections.is_empty());
    }

    struct ErrorBroker(RuntimeError);

    impl ConfirmationBroker for ErrorBroker {
        fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
            let error = self.0.clone();
            Box::pin(async move { Err(error) })
        }
    }

    struct StaticBroker(ConfirmationDecision);

    impl ConfirmationBroker for StaticBroker {
        fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
            let decision = self.0.clone();
            Box::pin(async move { Ok(decision) })
        }
    }

    #[tokio::test]
    async fn confirmation_broker_fail_closed_for_absent_timeout_cancelled() {
        for error in [
            RuntimeError::ConfirmationUnavailable("missing".to_string()),
            RuntimeError::ConfirmationTimedOut,
            RuntimeError::ConfirmationCancelled,
        ] {
            let broker = ErrorBroker(error);
            let decision = request_confirmation_fail_closed(
                &broker,
                ConfirmationRequest::new(ConfirmationAction::RunCommand, "run command"),
            )
            .await
            .unwrap();
            assert!(!decision.approved);
        }
    }

    #[test]
    fn confirmation_request_metadata_redacts_secret_markers() {
        let request =
            ConfirmationRequest::new(ConfirmationAction::Custom("deploy".to_string()), "use bearer token abc123");
        assert_eq!(request.summary, "[REDACTED]");
    }

    #[tokio::test]
    async fn confirmed_action_does_not_execute_before_approval() {
        let denied_runtime = RuntimeBuilder::new()
            .confirmation_broker(Arc::new(StaticBroker(ConfirmationDecision::deny("no"))))
            .build()
            .unwrap();
        let mut executed = false;
        let err = denied_runtime
            .run_confirmed_action(ConfirmationRequest::new(ConfirmationAction::RunCommand, "run command"), || {
                executed = true;
                Ok(())
            })
            .await
            .unwrap_err();
        assert!(!executed);
        assert_eq!(err, RuntimeError::ConfirmationDenied("no".to_string()));

        let approved_runtime = RuntimeBuilder::new()
            .confirmation_broker(Arc::new(StaticBroker(ConfirmationDecision::approve("yes"))))
            .build()
            .unwrap();
        let mut approved_executed = false;
        approved_runtime
            .run_confirmed_action(ConfirmationRequest::new(ConfirmationAction::RunCommand, "run command"), || {
                approved_executed = true;
                Ok(())
            })
            .await
            .unwrap();
        assert!(approved_executed);
    }

    #[tokio::test]
    async fn in_memory_session_replay_records_last_prompt() {
        let store = Arc::new(InMemorySessionStore::default());
        let services = RuntimeServices {
            sessions: store.clone(),
            ..RuntimeServices::in_memory()
        };
        let runtime = RuntimeBuilder::new().services(services).build().unwrap();
        let session_id = SessionId::from_host("replay-session");
        let session = runtime
            .create_session(SessionOptions {
                session_id: Some(session_id.clone()),
                model: None,
            })
            .await
            .unwrap();
        session.submit_prompt(PromptInput::new("persist me")).await.unwrap();
        session.submit_prompt(PromptInput::new("persist me too")).await.unwrap();
        let record = store.load(&session_id).unwrap().unwrap();
        assert!(record.last_prompt.is_some());
        let replay = record.replay_context();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].user_prompt, "persist me");
        assert_eq!(replay[1].user_prompt, "persist me too");
        assert_eq!(replay[0].assembled_prompt.user_prompt, "persist me");
    }
}
