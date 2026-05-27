//! Host-facing embedding facade for Clankers.
//!
//! This crate is intentionally transport-neutral. Public types model sessions,
//! prompts, tools, confirmation decisions, prompt assembly, and runtime-owned
//! services without exposing daemon frames, TUI state, CLI arguments, ACP/MCP
//! envelopes, or Matrix adapter types.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::numeric_units,
        tigerstyle::explicit_defaults,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::bool_naming,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::too_many_parameters,
        tigerstyle::ambient_clock,
        tigerstyle::usize_in_public_api,
        tigerstyle::no_unwrap,
        reason = "runtime facade preserves embedded SDK DTO/API compatibility; behavior is covered by runtime parity tests"
    )
)]

#[cfg(test)]
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
#[cfg(test)]
use serde_json::json;
use thiserror::Error;

pub mod adapters;
mod boundary;
pub mod confirmation;
pub mod dynamic_runtime;
pub mod effects;
mod event_summary;
pub mod events;
pub mod ledger;
pub mod process_jobs;
pub mod prompt;
pub mod runtime;
pub mod services;
pub mod session;
pub mod steel_mutation;
pub mod steel_orchestration;
pub mod steel_orchestration_mutation;
pub mod steel_repo_evolution;
pub mod steel_runtime;
pub mod steel_tool_substrate;
pub mod tools;

pub use adapters::NoopRuntimeCancellationAdapter;
pub use adapters::NoopRuntimeEventObserver;
pub use adapters::NoopRuntimeRetryAdapter;
pub use adapters::NoopRuntimeUsageAdapter;
pub use adapters::RuntimeCancellationAdapter;
pub use adapters::RuntimeEventObserver;
pub use adapters::RuntimeRetryAdapter;
pub use adapters::RuntimeRetryRequest;
pub use adapters::RuntimeToolAdapter;
pub use adapters::RuntimeToolRequest;
pub use adapters::RuntimeToolResponse;
pub use adapters::RuntimeToolStatus;
pub use adapters::RuntimeUsageAdapter;
pub use adapters::RuntimeUsageObservation;
pub use adapters::RuntimeUsageObservationKind;
pub use adapters::UnavailableRuntimeToolAdapter;
#[cfg(test)]
use boundary::public_type_names;
pub use confirmation::ConfirmationAction;
pub use confirmation::ConfirmationBroker;
pub use confirmation::ConfirmationDecision;
pub use confirmation::ConfirmationFuture;
pub use confirmation::ConfirmationRequest;
pub use confirmation::FailClosedConfirmationBroker;
pub use confirmation::request_confirmation_fail_closed;
pub use dynamic_runtime::CrossLayerFixtureReceipt;
pub use dynamic_runtime::DYNAMIC_RUNTIME_ACTION_SCHEMA;
pub use dynamic_runtime::DYNAMIC_RUNTIME_RECEIPT_SCHEMA;
pub use dynamic_runtime::DynamicRuntimeActionEnvelope;
pub use dynamic_runtime::DynamicRuntimeActionKind;
pub use dynamic_runtime::DynamicRuntimeActionReason;
pub use dynamic_runtime::DynamicRuntimeActionReceipt;
pub use dynamic_runtime::DynamicRuntimeActionStatus;
pub use dynamic_runtime::DynamicRuntimeAuthorizationContext;
pub use dynamic_runtime::DynamicRuntimeKind;
pub use dynamic_runtime::DynamicRuntimeRedactionClass;
pub use dynamic_runtime::FakeSteelOrchestrationProfile;
pub use dynamic_runtime::FakeSteelOrchestrationReceipt;
pub use dynamic_runtime::FakeSteelOrchestrationRequest;
pub use dynamic_runtime::SteelAmbientAccessKind;
pub use dynamic_runtime::WasmToolExecutionProfile;
pub use dynamic_runtime::WasmToolExecutionReceipt;
pub use dynamic_runtime::WasmToolExecutionRequest;
pub use dynamic_runtime::WasmToolExecutionStatus;
pub use dynamic_runtime::authorize_dynamic_runtime_action;
pub use dynamic_runtime::run_cross_layer_fixture;
pub use dynamic_runtime::run_fake_steel_orchestration;
pub use dynamic_runtime::run_fake_wasm_tool_execution;
pub use dynamic_runtime::steel_ambient_access_negative_fixtures;
pub use effects::EffectAbilityClass;
pub use effects::EffectCorrelationId;
pub use effects::EffectGate;
pub use effects::EffectHandler;
pub use effects::EffectHandlerMode;
pub use effects::EffectRequest;
pub use effects::EffectRequestRef;
pub use effects::EffectResult;
pub use effects::EffectResultStatus;
pub use effects::REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION;
pub use effects::RemoteArtifactEnvelope;
pub use effects::RemoteDependencyFailure;
pub use effects::RemoteDependencyFailureKind;
pub use effects::RemoteDependencySyncReport;
pub use effects::RemoteExecutionArtifactKind;
pub use effects::RemoteExecutionDependency;
pub use effects::RemoteExecutionRequest;
pub use effects::RemoteExecutionTarget;
pub use effects::StaticEffectHandler;
pub use effects::UcanAuthorizationMetadata;
pub use effects::evaluate_remote_dependency_sync;
pub use effects::initial_effect_handler_subset;
pub use effects::run_effect_fail_closed;
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
pub use ledger::SessionLedgerEntry;
pub use ledger::SessionLedgerMessage;
pub use ledger::SessionLedgerReceipt;
pub use ledger::SessionLedgerRecord;
pub use ledger::SessionLedgerReplay;
pub use ledger::SessionLedgerReplayMetadata;
pub use ledger::SessionLedgerRole;
pub use ledger::SessionLedgerSummary;
pub use ledger::SessionLedgerUnsupported;
pub use ledger::SessionLedgerUsage;
pub use ledger::ledger_entries_from_engine_messages;
pub use ledger::ledger_messages_from_engine_messages;
pub use ledger::replay_ledger_entries;
pub use prompt::AssembledPrompt;
pub use prompt::ContextReferenceKind;
pub use prompt::ContextReferenceRequest;
pub use prompt::DisabledPromptSourceService;
pub use prompt::EchoModelAdapter;
pub use prompt::HostContext;
pub use prompt::ModelAdapter;
pub use prompt::ModelFailure;
pub use prompt::ModelRequest;
pub use prompt::ModelRequestMetadata;
pub use prompt::ModelResponse;
pub use prompt::PromptAssembler;
pub use prompt::PromptAssemblyPolicy;
pub use prompt::PromptId;
pub use prompt::PromptInput;
pub use prompt::PromptProvenance;
pub use prompt::PromptReceipt;
pub use prompt::PromptSection;
pub use prompt::PromptSourceKind;
pub use prompt::PromptSourceRequest;
pub use prompt::PromptSourceService;
pub use prompt::PromptSources;
pub use prompt::SkillSnippet;
pub use prompt::StaticPromptSourceService;
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
pub use services::DisabledSessionStore;
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
pub use services::ProviderMessage;
pub use services::ProviderMessageRole;
pub use services::ProviderModelFailure;
pub use services::ProviderModelRequest;
pub use services::ProviderModelResponse;
pub use services::ProviderModelStatus;
pub use services::ProviderRouterService;
pub use services::ProviderStreamEvent;
pub use services::ResolvedSkillSnippet;
pub use services::RuntimeServices;
pub use services::SessionRecord;
pub use services::SessionStore;
pub use services::SettingsService;
pub use services::SkillResolution;
pub use services::SkillResolutionRequest;
pub use services::SkillStore;
pub use session::SessionHandle;
pub use session::SessionId;
pub use session::SessionOptions;
pub use steel_orchestration::DEFAULT_TURN_EXECUTION_SEAM;
pub use steel_orchestration::DEFAULT_TURN_EXECUTION_SOURCE;
pub use steel_orchestration::DEFAULT_TURN_PLANNING_SEAM;
pub use steel_orchestration::OrchestrationCandidate;
pub use steel_orchestration::OrchestrationDecision;
pub use steel_orchestration::OrchestrationFallbackMode;
pub use steel_orchestration::OrchestrationIssueCode;
pub use steel_orchestration::OrchestrationPlan;
pub use steel_orchestration::OrchestrationPlanReceipt;
pub use steel_orchestration::OrchestrationPlanStatus;
pub use steel_orchestration::OrchestrationPlannerKind;
pub use steel_orchestration::OrchestrationRolloutStage;
pub use steel_orchestration::RustNativeFallbackStatus;
pub use steel_orchestration::STEEL_ORCHESTRATION_PLAN_SCHEMA;
pub use steel_orchestration::STEEL_ORCHESTRATION_RECEIPT_SCHEMA;
pub use steel_orchestration::STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA;
pub use steel_orchestration::STEEL_TURN_EXECUTION_RECEIPT_SCHEMA;
pub use steel_orchestration::SteelOrchestrationProfile;
pub use steel_orchestration::SteelTurnExecutionHostCallPayload;
pub use steel_orchestration::SteelTurnExecutionHostCallReceipt;
pub use steel_orchestration::SteelTurnExecutionInput;
pub use steel_orchestration::SteelTurnExecutionReceipt;
pub use steel_orchestration::SteelTurnExecutionStatus;
pub use steel_orchestration::SteelTurnPlanHostCallPayload;
pub use steel_orchestration::SteelTurnPlanningAuthorityGrant;
pub use steel_orchestration::SteelTurnPlanningAuthorityReason;
pub use steel_orchestration::SteelTurnPlanningAuthorityReceipt;
pub use steel_orchestration::SteelTurnPlanningAuthorityStatus;
pub use steel_orchestration::TurnPlanningInput;
pub use steel_orchestration::authorize_steel_turn_execution;
pub use steel_orchestration::plan_turn_with_steel_or_fallback;
pub use steel_orchestration::rust_native_turn_plan;
pub use steel_repo_evolution::SteelRepoEvolutionActivationReason;
pub use steel_repo_evolution::SteelRepoEvolutionActivationStatus;
pub use steel_repo_evolution::load_repo_evolution_pack;
pub use steel_runtime::STEEL_RUNTIME_RECEIPT_SCHEMA;
pub use steel_runtime::STEEL_RUNTIME_STATUS_SCHEMA;
pub use steel_runtime::SteelHostCallOutcome;
pub use steel_runtime::SteelHostCallReceipt;
pub use steel_runtime::SteelHostFunctionRegistration;
pub use steel_runtime::SteelRuntimeProfile;
pub use steel_runtime::SteelRuntimeReasonCode;
pub use steel_runtime::SteelRuntimeReceipt;
pub use steel_runtime::SteelRuntimeRequest;
pub use steel_runtime::SteelRuntimeStatus;
pub use steel_runtime::SteelRuntimeStatusCode;
pub use steel_runtime::evaluate_steel_request;
pub use steel_runtime::steel_runtime_status;
pub use steel_tool_substrate::DEFAULT_TOOL_SUBSTRATE_CALL_SEAM;
pub use steel_tool_substrate::DEFAULT_TOOL_SUBSTRATE_LIST_SEAM;
pub use steel_tool_substrate::STEEL_TOOL_SUBSTRATE_PLAN_SCHEMA;
pub use steel_tool_substrate::STEEL_TOOL_SUBSTRATE_RECEIPT_SCHEMA;
pub use steel_tool_substrate::SteelToolExecutorKind;
pub use steel_tool_substrate::SteelToolInvocationInput;
pub use steel_tool_substrate::SteelToolInvocationPlan;
pub use steel_tool_substrate::SteelToolInvocationReceipt;
pub use steel_tool_substrate::SteelToolSubstrateFallbackMode;
pub use steel_tool_substrate::SteelToolSubstrateIssue;
pub use steel_tool_substrate::SteelToolSubstrateProfile;
pub use steel_tool_substrate::SteelToolSubstrateRolloutStage;
pub use steel_tool_substrate::SteelToolSubstrateStatus;
pub use steel_tool_substrate::plan_tool_invocation_with_steel_or_fallback;
pub use steel_tool_substrate::steel_tool_plan_payload;
pub use tools::CapabilityPack;
pub use tools::SideEffectLevel;
pub use tools::ToolCatalog;
pub use tools::ToolCatalogBuilder;
pub use tools::ToolCatalogOmission;
pub use tools::ToolCollisionPolicy;
pub use tools::ToolDescriptor;
pub use tools::ToolEffectReceipt;

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
    #[error("session missing: {0}")]
    SessionMissing(String),
    #[error("session unsupported: {0}")]
    SessionUnsupported(String),
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
            Self::SessionMissing(_) | Self::SessionUnsupported(_) => ErrorClass::Session,
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
                engine_content: Vec::new(),
                usage: None,
                stop_reason: None,
                failure: None,
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
    async fn runtime_events_project_to_shared_semantic_stream_in_order() {
        let runtime = RuntimeBuilder::new().model_adapter(Arc::new(ScriptedModel)).build().unwrap();
        let session = runtime
            .create_session(SessionOptions {
                session_id: Some(SessionId::from_host("semantic-session")),
                model: None,
            })
            .await
            .unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("semantic prompt")).await.unwrap();

        let mut kinds = Vec::new();
        for _ in 0..4 {
            let event = events.recv().await.unwrap();
            let semantic = event.to_semantic_event();
            assert_eq!(semantic.metadata().session_id.as_deref(), Some("semantic-session"));
            assert!(!semantic.metadata().contains_secret_markers());
            kinds.push(semantic.kind().to_string());
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

        let provider_error = extensions.provider_router.complete(provider_request()).unwrap_err();
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
    fn provider_model_contract_literal_fixtures_cover_request_stream_failures_and_usage() {
        let request = ProviderModelRequest {
            provider: "openai-codex".to_string(),
            model: Some("openai-codex/gpt-5.3-codex".to_string()),
            account_label: Some("chatgpt-work".to_string()),
            route_source: "embedded-test".to_string(),
            session_id: Some("session-provider-contract".to_string()),
            system_prompt: Some("Be precise".to_string()),
            messages: vec![
                ProviderMessage::user_text("hello"),
                ProviderMessage::assistant(
                    vec![clanker_message::Content::Text {
                        text: "previous answer".to_string(),
                    }],
                    Some("openai-codex/gpt-5.3-codex".to_string()),
                ),
                ProviderMessage::tool_result(
                    "call_1:item_1",
                    "read_file",
                    vec![clanker_message::Content::Text {
                        text: "file contents".to_string(),
                    }],
                    false,
                ),
            ],
            tools: vec![clanker_message::ToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({"type":"object"}),
            }],
            thinking: Some(clanker_message::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(1024),
            }),
            max_tokens: Some(256),
            temperature: Some(0.1),
            no_cache: true,
            cache_ttl: Some("1h".to_string()),
            metadata: EventMetadata::empty().with("request_kind", "contract-fixture"),
        };

        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "provider": "openai-codex",
                "model": "openai-codex/gpt-5.3-codex",
                "account_label": "chatgpt-work",
                "route_source": "embedded-test",
                "session_id": "session-provider-contract",
                "system_prompt": "Be precise",
                "messages": [
                    {
                        "role": "user",
                        "content": [{"type": "Text", "text": "hello"}],
                        "id": null,
                        "model": null,
                        "call_id": null,
                        "tool_name": null,
                        "is_error": false
                    },
                    {
                        "role": "assistant",
                        "content": [{"type": "Text", "text": "previous answer"}],
                        "id": null,
                        "model": "openai-codex/gpt-5.3-codex",
                        "call_id": null,
                        "tool_name": null,
                        "is_error": false
                    },
                    {
                        "role": "tool",
                        "content": [{"type": "Text", "text": "file contents"}],
                        "id": null,
                        "model": null,
                        "call_id": "call_1:item_1",
                        "tool_name": "read_file",
                        "is_error": false
                    }
                ],
                "tools": [{"name": "read_file", "description": "Read a file", "input_schema": {"type":"object"}}],
                "thinking": {"enabled": true, "budget_tokens": 1024},
                "max_tokens": 256,
                "temperature": 0.1,
                "no_cache": true,
                "cache_ttl": "1h",
                "metadata": {"session_id": null, "fields": {"request_kind": "contract-fixture"}}
            })
        );

        let response = ProviderModelResponse::completed(
            vec![
                ProviderStreamEvent::TextDelta {
                    index: 0,
                    text: "hi".to_string(),
                },
                ProviderStreamEvent::Usage {
                    stop_reason: Some(clanker_message::StopReason::Stop),
                    usage: clanker_message::Usage {
                        input_tokens: 7,
                        output_tokens: 11,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 3,
                    },
                },
            ],
            vec![clanker_message::Content::Text { text: "hi".to_string() }],
            Some(clanker_message::Usage {
                input_tokens: 7,
                output_tokens: 11,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 3,
            }),
            Some(clanker_message::StopReason::Stop),
            ExtensionReceipt::new("provider", "complete", ExtensionStatus::Succeeded)
                .with_metadata("provider", "openai-codex"),
        );
        assert_eq!(serde_json::to_value(&response).unwrap()["status"], json!("completed"));
        assert_eq!(serde_json::to_value(&response).unwrap()["stream_events"][0]["type"], json!("text_delta"));
        assert_eq!(serde_json::to_value(&response).unwrap()["usage"]["output_tokens"], json!(11));

        let retry = ProviderModelResponse::failure(
            ProviderModelStatus::RetryableFailure,
            ProviderModelFailure::retryable("rate limited", Some(429)),
            ExtensionReceipt::new("provider", "complete", ExtensionStatus::Failed),
        );
        assert_eq!(serde_json::to_value(&retry).unwrap()["status"], json!("retryable_failure"));
        assert_eq!(serde_json::to_value(&retry).unwrap()["failure"]["retryable"], json!(true));

        let terminal = ProviderModelResponse::failure(
            ProviderModelStatus::TerminalFailure,
            ProviderModelFailure::terminal("bad request", Some(400)),
            ExtensionReceipt::new("provider", "complete", ExtensionStatus::Failed),
        );
        assert_eq!(serde_json::to_value(&terminal).unwrap()["status"], json!("terminal_failure"));
        assert_eq!(serde_json::to_value(&terminal).unwrap()["failure"]["retryable"], json!(false));
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

        fn complete(&self, request: ProviderModelRequest) -> Result<ProviderModelResponse, RuntimeError> {
            let receipt = ExtensionReceipt::new("host_provider_router", "complete", ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source);
            Ok(ProviderModelResponse::completed(Vec::new(), Vec::new(), None, None, receipt))
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

    struct ToolThenDoneModel {
        calls: std::sync::Mutex<usize>,
    }

    impl ToolThenDoneModel {
        fn new() -> Self {
            Self {
                calls: std::sync::Mutex::new(0),
            }
        }
    }

    impl ModelAdapter for ToolThenDoneModel {
        fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            let mut calls = self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            *calls += 1;
            if *calls == 1 {
                return Ok(ModelResponse {
                    events: Vec::new(),
                    engine_content: vec![clanker_message::Content::ToolUse {
                        id: "runtime-tool-call".to_string(),
                        name: "read".to_string(),
                        input: json!({"path": "README.md"}),
                    }],
                    usage: None,
                    stop_reason: Some(clanker_message::StopReason::ToolUse),
                    failure: None,
                });
            }
            Ok(ModelResponse {
                events: vec![SessionEvent::AssistantDelta {
                    prompt_id: request.prompt_id,
                    text: "continued after tool feedback".to_string(),
                    metadata: EventMetadata::empty().with("source", "tool_then_done"),
                }],
                engine_content: Vec::new(),
                usage: None,
                stop_reason: None,
                failure: None,
            })
        }
    }

    struct CountingRuntimeToolAdapter {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl CountingRuntimeToolAdapter {
        fn new() -> Self {
            Self {
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl RuntimeToolAdapter for CountingRuntimeToolAdapter {
        fn execute_tool(&self, request: RuntimeToolRequest) -> Result<RuntimeToolResponse, RuntimeError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(request.tool_name, "read");
            Ok(RuntimeToolResponse::succeeded(
                vec![clanker_message::Content::Text {
                    text: "tool adapter result".to_string(),
                }],
                json!({"source": "counting_tool_adapter"}),
            ))
        }
    }

    #[tokio::test]
    async fn runtime_facade_tool_feedback_uses_engine_host_turn_loop() {
        let tool_adapter = Arc::new(CountingRuntimeToolAdapter::new());
        let runtime = RuntimeBuilder::new()
            .model_adapter(Arc::new(ToolThenDoneModel::new()))
            .tool_adapter(tool_adapter.clone())
            .build()
            .unwrap();
        let session = runtime
            .create_session(SessionOptions {
                session_id: Some(SessionId::from_host("runtime-engine-host-session")),
                model: Some("runtime-engine-host-model".to_string()),
            })
            .await
            .unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("exercise tool loop")).await.unwrap();

        let mut kinds = Vec::new();
        for _ in 0..5 {
            kinds.push(safe_event_summary(&events.recv().await.unwrap())["type"].as_str().unwrap().to_string());
        }
        assert_eq!(kinds, vec![
            "prompt_accepted",
            "tool_started",
            "tool_finished",
            "assistant_delta",
            "completed",
        ]);
        assert_eq!(tool_adapter.calls(), 1);
    }

    struct CountingRuntimeEventObserver {
        events: std::sync::atomic::AtomicUsize,
    }

    impl CountingRuntimeEventObserver {
        fn new() -> Self {
            Self {
                events: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn events(&self) -> usize {
            self.events.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl RuntimeEventObserver for CountingRuntimeEventObserver {
        fn observe_engine_event(&self, _event: &clankers_engine::EngineEvent) -> Result<(), RuntimeError> {
            self.events.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    struct CountingRuntimeUsageAdapter {
        observations: std::sync::atomic::AtomicUsize,
    }

    impl CountingRuntimeUsageAdapter {
        fn new() -> Self {
            Self {
                observations: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn observations(&self) -> usize {
            self.observations.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl RuntimeUsageAdapter for CountingRuntimeUsageAdapter {
        fn observe_usage(&self, _observation: RuntimeUsageObservation) -> Result<(), RuntimeError> {
            self.observations.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn runtime_facade_invokes_event_and_usage_adapter_slots() {
        let event_observer = Arc::new(CountingRuntimeEventObserver::new());
        let usage_adapter = Arc::new(CountingRuntimeUsageAdapter::new());
        let runtime = RuntimeBuilder::new()
            .event_observer(event_observer.clone())
            .usage_adapter(usage_adapter.clone())
            .build()
            .unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        session.submit_prompt(PromptInput::new("adapter slot counters")).await.unwrap();

        assert!(event_observer.events() >= 1);
        assert_eq!(usage_adapter.observations(), 1);
    }

    struct FailingModel;

    impl ModelAdapter for FailingModel {
        fn complete(&self, _request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            Err(RuntimeError::Model("provider unavailable".to_string()))
        }
    }

    #[tokio::test]
    async fn runtime_facade_projects_model_failure_to_error_event() {
        let runtime = RuntimeBuilder::new().model_adapter(Arc::new(FailingModel)).build().unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        let mut events = session.take_events().await.unwrap();
        let error = session.submit_prompt(PromptInput::new("fail please")).await.unwrap_err();
        assert_eq!(error.class(), ErrorClass::Model);

        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "prompt_accepted");
        let error_summary = safe_event_summary(&events.recv().await.unwrap());
        assert_eq!(error_summary["type"], "error");
        assert_eq!(error_summary["class"], "Model");
    }

    struct RetryThenDoneModel {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl RetryThenDoneModel {
        fn new() -> Self {
            Self {
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl ModelAdapter for RetryThenDoneModel {
        fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if call == 0 {
                return Ok(ModelResponse {
                    failure: Some(ModelFailure::retryable("rate limited", Some(429))),
                    ..ModelResponse::default()
                });
            }
            Ok(ModelResponse {
                events: vec![SessionEvent::AssistantDelta {
                    prompt_id: request.prompt_id,
                    text: "recovered after retry".to_string(),
                    metadata: EventMetadata::empty().with("source", "retry_then_done"),
                }],
                ..ModelResponse::default()
            })
        }
    }

    struct CountingRuntimeRetryAdapter {
        sleeps: std::sync::atomic::AtomicUsize,
    }

    impl CountingRuntimeRetryAdapter {
        fn new() -> Self {
            Self {
                sleeps: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn sleeps(&self) -> usize {
            self.sleeps.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl RuntimeRetryAdapter for CountingRuntimeRetryAdapter {
        fn sleep_for_retry(&self, request: RuntimeRetryRequest) -> Result<(), RuntimeError> {
            assert_eq!(request.delay_ms, 1000);
            self.sleeps.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn runtime_facade_retryable_model_failure_uses_retry_adapter() {
        let retry = Arc::new(CountingRuntimeRetryAdapter::new());
        let runtime = RuntimeBuilder::new()
            .model_adapter(Arc::new(RetryThenDoneModel::new()))
            .retry_adapter(retry.clone())
            .build()
            .unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("retry once")).await.unwrap();

        assert_eq!(retry.sleeps(), 1);
        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "prompt_accepted");
        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "assistant_delta");
        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "completed");
    }

    struct PanicModel;

    impl ModelAdapter for PanicModel {
        fn complete(&self, _request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            panic!("cancelled runtime must not invoke model adapter");
        }
    }

    struct AlwaysCancelled;

    impl RuntimeCancellationAdapter for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }

        fn cancellation_reason(&self) -> String {
            "host cancelled before model".to_string()
        }
    }

    #[tokio::test]
    async fn runtime_facade_cancellation_adapter_finishes_without_model_call() {
        let runtime = RuntimeBuilder::new()
            .model_adapter(Arc::new(PanicModel))
            .cancellation_adapter(Arc::new(AlwaysCancelled))
            .build()
            .unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("cancel before model")).await.unwrap();

        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "prompt_accepted");
        let complete = safe_event_summary(&events.recv().await.unwrap());
        assert_eq!(complete["type"], "completed");
        assert_eq!(complete["stop_reason"], "Cancelled");
    }

    #[tokio::test]
    async fn runtime_facade_missing_tool_adapter_fails_closed_before_side_effects() {
        let runtime = RuntimeBuilder::new().model_adapter(Arc::new(ToolThenDoneModel::new())).build().unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("missing tool adapter")).await.unwrap();

        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "prompt_accepted");
        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "tool_started");
        let tool_done = safe_event_summary(&events.recv().await.unwrap());
        assert_eq!(tool_done["type"], "tool_finished");
        assert_eq!(tool_done["status"], "Failed");
        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "assistant_delta");
        assert_eq!(safe_event_summary(&events.recv().await.unwrap())["type"], "completed");
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
    fn tool_dispatch_route_preserves_public_name_and_records_handler_status() {
        let bash = ToolDescriptor::new("bash", "shell", SideEffectLevel::Dangerous);
        let handler = StaticEffectHandler::new(EffectAbilityClass::Shell, EffectHandlerMode::Simulate {
            summary: "dry-run shell".to_owned(),
        });

        let receipt = bash.route_through_effect_handler(EffectCorrelationId::from_static("tool-call-1"), &handler);

        assert_eq!(receipt.tool_name, "bash");
        assert_eq!(receipt.effect_class, EffectAbilityClass::Shell);
        assert_eq!(receipt.handler_status, EffectResultStatus::Simulated);
        assert_eq!(receipt.safe_summary, "dry-run shell");
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
            extensions.provider_router.complete(provider_request()),
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

    fn provider_request() -> ProviderModelRequest {
        let mut request = ProviderModelRequest::user_prompt("anthropic", Some("test".to_string()), "hello");
        request.account_label = Some("primary".to_string());
        request.route_source = "embedded".to_string();
        request.session_id = Some("session".to_string());
        request.max_tokens = Some(8);
        request
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
        fn complete(&self, request: ProviderModelRequest) -> Result<ProviderModelResponse, RuntimeError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let receipt = ExtensionReceipt::new("injected_provider_router", "complete", ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source);
            Ok(ProviderModelResponse::completed(Vec::new(), Vec::new(), None, None, receipt))
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
        let response = extensions.provider_router.complete(provider_request()).unwrap();
        assert_eq!(response.status, ProviderModelStatus::Completed);
        assert_eq!(response.receipt.status, ExtensionStatus::Succeeded);
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
    fn config_prompt_skill_service_fixtures_cover_host_desktop_missing_and_redaction() {
        let host_sources = PromptSources {
            system_prompt: Some("system safe".to_string()),
            host_context: vec![HostContext {
                label: "app".to_string(),
                content: "host context".to_string(),
            }],
            skill_snippets: vec![SkillSnippet {
                name: "review".to_string(),
                content: "skill content".to_string(),
                source: "injected".to_string(),
            }],
            ..PromptSources::default()
        };
        let host =
            PromptAssembler::assemble(&PromptAssemblyPolicy::host_context_only(), &host_sources, "hello".to_string())
                .expect("host-only prompt assembles");
        assert_eq!(host.sections.iter().map(|section| section.label.as_str()).collect::<Vec<_>>(), vec![
            "app",
            "skill:review",
            "system"
        ]);
        assert!(host.provenance.iter().any(|item| item.source == PromptSourceKind::Skill));

        let mut filesystem_sources = host_sources.clone();
        filesystem_sources.filesystem_context_requested = true;
        filesystem_sources.filesystem_context = vec![HostContext {
            label: "file:README.md".to_string(),
            content: "desktop file context".to_string(),
        }];
        let disabled = PromptAssembler::assemble(
            &PromptAssemblyPolicy::host_context_only(),
            &filesystem_sources,
            "hello".to_string(),
        )
        .unwrap_err();
        assert_eq!(disabled, RuntimeError::FilesystemDiscoveryDisabled);

        let desktop = PromptAssembler::assemble(
            &PromptAssemblyPolicy::desktop_default(),
            &filesystem_sources,
            "hello".to_string(),
        )
        .expect("desktop-enabled context assembles");
        assert!(desktop.provenance.iter().any(|item| item.source == PromptSourceKind::Filesystem));

        let mut secret_sources = host_sources.clone();
        secret_sources.skill_snippets[0].content = "api_key secret-token".to_string();
        let redacted =
            PromptAssembler::assemble(&PromptAssemblyPolicy::host_context_only(), &secret_sources, "hello".to_string())
                .expect("redacted skill prompt assembles");
        assert!(redacted.sections.iter().any(|section| section.content == "[REDACTED]"));
        assert!(redacted.provenance.iter().all(|item| !contains_secret_marker(&item.safe_summary)));

        let missing_skill = RuntimeServices::stateless()
            .skills
            .resolve(SkillResolutionRequest {
                requested: vec!["review".to_string()],
            })
            .unwrap_err();
        assert_eq!(missing_skill, RuntimeError::ExtensionUnavailable("skill service unavailable".to_string()));
    }

    #[test]
    fn prompt_source_service_injection_is_used_by_runtime_assembly() {
        let sources = PromptSources {
            host_context: vec![HostContext {
                label: "service".to_string(),
                content: "resolved by service".to_string(),
            }],
            ..PromptSources::default()
        };
        let service = Arc::new(StaticPromptSourceService::new(sources).with_capability("test_prompt_service"));
        let runtime = RuntimeBuilder::new().prompt_source_service(service).build().unwrap();

        let assembled = runtime.assemble_prompt("hello").unwrap();

        assert_eq!(assembled.sections[0].label, "service");
        assert_eq!(assembled.sections[0].content, "resolved by service");
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
    fn prompt_assembly_repeated_prompts_are_pure_and_do_not_suppress_follow_ups() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            host_context: vec![HostContext {
                label: "app".to_string(),
                content: "safe app context".to_string(),
            }],
            ..PromptSources::default()
        };

        let first = PromptAssembler::assemble(&policy, &sources, "first".to_string()).unwrap();
        let second = PromptAssembler::assemble(&policy, &sources, "second".to_string()).unwrap();
        let first_again = PromptAssembler::assemble(&policy, &sources, "first".to_string()).unwrap();

        assert_eq!(first.sections, first_again.sections);
        assert_eq!(first.user_prompt, "first");
        assert_eq!(second.user_prompt, "second");
        assert_eq!(second.sections, first.sections);
        assert!(second.provenance.iter().all(|item| !contains_secret_marker(&item.safe_summary)));
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

    #[derive(Default)]
    struct RecordingHistoryModel {
        requests: std::sync::Mutex<Vec<Vec<String>>>,
    }

    impl ModelAdapter for RecordingHistoryModel {
        fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(history_lines(&request));
            Ok(ModelResponse {
                events: vec![SessionEvent::AssistantDelta {
                    prompt_id: request.prompt_id,
                    text: "resumed".to_string(),
                    metadata: EventMetadata::empty().with("source", "recording_history"),
                }],
                engine_content: Vec::new(),
                usage: Some(clanker_message::Usage {
                    input_tokens: 4,
                    output_tokens: 2,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                }),
                stop_reason: None,
                failure: None,
            })
        }
    }

    #[derive(Default)]
    struct ProductOwnedSessionStore {
        records: std::sync::Mutex<std::collections::BTreeMap<SessionId, SessionRecord>>,
    }

    impl SessionStore for ProductOwnedSessionStore {
        fn capability(&self) -> &'static str {
            "product_owned"
        }

        fn save(&self, record: SessionRecord) -> Result<(), RuntimeError> {
            self.records
                .lock()
                .map_err(|_| RuntimeError::StoreUnavailable("product sessions".to_string()))?
                .insert(record.session_id.clone(), record);
            Ok(())
        }

        fn load(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, RuntimeError> {
            Ok(self
                .records
                .lock()
                .map_err(|_| RuntimeError::StoreUnavailable("product sessions".to_string()))?
                .get(session_id)
                .cloned())
        }
    }

    fn history_lines(request: &ModelRequest) -> Vec<String> {
        request
            .history
            .iter()
            .map(|message| format!("{}: {}", ledger_role_name(message.role), message.text_summary()))
            .collect()
    }

    fn ledger_role_name(role: SessionLedgerRole) -> &'static str {
        match role {
            SessionLedgerRole::User => "user",
            SessionLedgerRole::Assistant => "assistant",
            SessionLedgerRole::Tool => "tool",
        }
    }

    fn seed_resume_record(session_id: SessionId) -> SessionRecord {
        let mut record = SessionRecord::new(session_id.clone());
        record.last_prompt = Some(PromptId::from_host("prompt-seed"));
        record.ledger_entries = vec![
            SessionLedgerEntry::summary("The user previously supplied launch-code context."),
            SessionLedgerEntry::message(SessionLedgerMessage::text(
                SessionLedgerRole::User,
                "Remember the launch code name is Orchard.",
            )),
            SessionLedgerEntry::message(SessionLedgerMessage::text(
                SessionLedgerRole::Assistant,
                "Stored: launch code name Orchard.",
            )),
            SessionLedgerEntry::message(SessionLedgerMessage::text(SessionLedgerRole::Tool, "lookup: Orchard")),
            SessionLedgerEntry::receipt(
                PromptId::from_host("prompt-seed"),
                "completed",
                EventMetadata::new(session_id).with("store", "seed"),
            ),
        ];
        record
    }

    async fn run_resume_backend_fixture(store: Arc<dyn SessionStore>) -> Vec<String> {
        let session_id = SessionId::from_host("resume-ledger-session");
        store.save(seed_resume_record(session_id.clone())).unwrap();
        let services = RuntimeServices {
            sessions: store,
            ..RuntimeServices::in_memory()
        };
        let model = Arc::new(RecordingHistoryModel::default());
        let model_adapter: Arc<dyn ModelAdapter> = model.clone();
        let runtime = RuntimeBuilder::new().services(services).model_adapter(model_adapter).build().unwrap();
        let session = runtime
            .resume_session(session_id, SessionOptions {
                session_id: None,
                model: Some("resume-model".to_string()),
            })
            .await
            .unwrap();

        session.submit_prompt(PromptInput::new("What launch code name did I give you?")).await.unwrap();

        model.locked_requests().pop().unwrap()
    }

    impl RecordingHistoryModel {
        fn locked_requests(&self) -> std::sync::MutexGuard<'_, Vec<Vec<String>>> {
            self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        }
    }

    #[tokio::test]
    async fn session_resume_two_backends_restore_ordered_ledger_context() {
        let expected = vec![
            "user: Session summary:\nThe user previously supplied launch-code context.".to_string(),
            "user: Remember the launch code name is Orchard.".to_string(),
            "assistant: Stored: launch code name Orchard.".to_string(),
            "tool: lookup: Orchard".to_string(),
            "user: What launch code name did I give you?".to_string(),
        ];

        let in_memory = run_resume_backend_fixture(Arc::new(InMemorySessionStore::default())).await;
        let product_owned = run_resume_backend_fixture(Arc::new(ProductOwnedSessionStore::default())).await;

        assert_eq!(in_memory, expected);
        assert_eq!(product_owned, expected);
    }

    #[tokio::test]
    async fn session_resume_missing_or_unsupported_store_fails_before_model() {
        let missing_model = Arc::new(RecordingHistoryModel::default());
        let missing_model_adapter: Arc<dyn ModelAdapter> = missing_model.clone();
        let missing_runtime = RuntimeBuilder::new().model_adapter(missing_model_adapter).build().unwrap();
        let missing_error = match missing_runtime
            .resume_session(SessionId::from_host("missing-session"), SessionOptions::default())
            .await
        {
            Ok(_) => panic!("missing session unexpectedly resumed"),
            Err(error) => error,
        };
        assert_eq!(missing_error, RuntimeError::SessionMissing("missing-session".to_string()));
        assert!(missing_model.locked_requests().is_empty());

        let unsupported_model = Arc::new(RecordingHistoryModel::default());
        let unsupported_model_adapter: Arc<dyn ModelAdapter> = unsupported_model.clone();
        let unsupported_runtime = RuntimeBuilder::new()
            .services(RuntimeServices::stateless())
            .model_adapter(unsupported_model_adapter)
            .build()
            .unwrap();
        let unsupported_error = match unsupported_runtime
            .resume_session(SessionId::from_host("unsupported-session"), SessionOptions::default())
            .await
        {
            Ok(_) => panic!("unsupported session store unexpectedly resumed"),
            Err(error) => error,
        };
        assert_eq!(unsupported_error, RuntimeError::SessionUnsupported("session store disabled".to_string()));
        assert!(unsupported_model.locked_requests().is_empty());
    }

    #[tokio::test]
    async fn stateless_runtime_with_disabled_store_can_run_without_resume() {
        let model = Arc::new(RecordingHistoryModel::default());
        let model_adapter: Arc<dyn ModelAdapter> = model.clone();
        let runtime = RuntimeBuilder::new()
            .services(RuntimeServices::stateless())
            .model_adapter(model_adapter)
            .build()
            .unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        session.submit_prompt(PromptInput::new("stateless prompt")).await.unwrap();
        assert_eq!(model.locked_requests().len(), 1);
        assert_eq!(model.locked_requests()[0], vec!["user: stateless prompt".to_string()]);
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
