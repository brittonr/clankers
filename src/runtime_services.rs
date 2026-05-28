//! Explicit desktop/default service adapters for the embeddable runtime.
//!
//! These adapters make the normal Clankers path layout an explicit host-owned choice instead of
//! letting `clankers-runtime` discover `~/.clankers` or project `.clankers` paths implicitly.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use clankers_runtime::AuthService;
use clankers_runtime::AuthStoreAccessRequest;
use clankers_runtime::AuthStoreOperation;
use clankers_runtime::CacheStore;
use clankers_runtime::CheckpointStore;
use clankers_runtime::CredentialPoolPolicyService;
use clankers_runtime::CredentialPoolRequest;
use clankers_runtime::ExtensionAuthStoreService;
use clankers_runtime::ExtensionReceipt;
use clankers_runtime::ExtensionRuntimeKind;
use clankers_runtime::ExtensionRuntimeRequest;
use clankers_runtime::ExtensionRuntimeService;
use clankers_runtime::ExtensionServices;
use clankers_runtime::ExtensionToolDescriptor;
use clankers_runtime::PluginStore;
use clankers_runtime::ProjectContextService;
use clankers_runtime::ProviderMessageRole;
use clankers_runtime::ProviderModelFailure;
use clankers_runtime::ProviderModelRequest;
use clankers_runtime::ProviderModelResponse;
use clankers_runtime::ProviderModelStatus;
use clankers_runtime::ProviderRouterService;
use clankers_runtime::ProviderStreamEvent;
use clankers_runtime::ResolvedSkillSnippet;
use clankers_runtime::RuntimeError;
use clankers_runtime::RuntimeServices;
use clankers_runtime::SessionId;
use clankers_runtime::SessionRecord;
use clankers_runtime::SessionStore;
use clankers_runtime::SettingsService;
use clankers_runtime::SideEffectLevel;
use clankers_runtime::SkillResolution;
use clankers_runtime::SkillResolutionRequest;
use clankers_runtime::SkillStore;

/// Explicit adapter bundle for the normal desktop Clankers path layout.
pub struct DesktopRuntimeServiceAdapters;

impl DesktopRuntimeServiceAdapters {
    #[must_use]
    pub fn from_paths(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_plugin_manager(paths, project_paths, None)
    }

    #[must_use]
    pub fn from_paths_with_plugin_manager(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        plugin_manager: Arc<std::sync::Mutex<crate::plugin::PluginManager>>,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_extensions(paths, project_paths, None, Some(plugin_manager), None)
    }

    #[must_use]
    pub fn from_paths_with_provider_router(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        provider_router: Arc<dyn clankers_provider::Provider>,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_extensions(paths, project_paths, Some(provider_router), None, None)
    }

    #[must_use]
    pub fn from_paths_with_auth_store(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        auth_store: Arc<std::sync::Mutex<clankers_provider::auth::AuthStore>>,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_extensions(paths, project_paths, None, None, Some(auth_store))
    }

    fn from_paths_with_optional_plugin_manager(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_extensions(paths, project_paths, None, plugin_manager, None)
    }

    fn from_paths_with_optional_extensions(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        provider_router: Option<Arc<dyn clankers_provider::Provider>>,
        plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
        auth_store: Option<Arc<std::sync::Mutex<clankers_provider::auth::AuthStore>>>,
    ) -> RuntimeServices {
        let settings = Arc::new(DesktopSettingsService {
            global_settings: paths.global_settings.clone(),
            project_settings: project_paths.settings.clone(),
        });
        let auth = Arc::new(DesktopAuthService {
            auth_file: paths.global_auth.clone(),
        });
        let sessions = Arc::new(DesktopSessionStore {
            sessions_dir: paths.global_sessions_dir.clone(),
            memory: clankers_runtime::InMemorySessionStore::default(),
        });
        let cache = Arc::new(DesktopCacheStore {
            cache_dir: paths.global_config_dir.join("cache"),
        });
        let project_context = Arc::new(DesktopProjectContextService {
            root: project_paths.root.clone(),
            config_dir: project_paths.config_dir.clone(),
        });
        let skills = Arc::new(DesktopSkillStore {
            global_skills_dir: paths.global_skills_dir.clone(),
            project_skills_dir: project_paths.skills_dir.clone(),
        });
        let plugins = Arc::new(DesktopPluginStore {
            global_plugins_dir: paths.global_plugins_dir.clone(),
            project_plugins_dir: project_paths.plugins_dir.clone(),
        });
        let checkpoints = Arc::new(DesktopCheckpointStore {
            checkpoints_dir: project_paths.config_dir.join("checkpoints"),
        });
        let extensions = ExtensionServices {
            provider_router: Arc::new(DesktopProviderRouterService { provider_router }),
            auth_store: Arc::new(DesktopExtensionAuthStoreService {
                auth_store: auth_store.clone(),
            }),
            credential_pool: Arc::new(DesktopCredentialPoolPolicyService { auth_store }),
            runtime: Arc::new(DesktopExtensionRuntimeService { plugin_manager }),
        };
        RuntimeServices {
            settings,
            auth,
            sessions,
            cache,
            project_context,
            skills,
            plugins,
            checkpoints,
            extensions,
        }
    }
}

struct DesktopSettingsService {
    global_settings: PathBuf,
    project_settings: PathBuf,
}
struct DesktopAuthService {
    auth_file: PathBuf,
}
struct DesktopSessionStore {
    sessions_dir: PathBuf,
    memory: clankers_runtime::InMemorySessionStore,
}
struct DesktopCacheStore {
    cache_dir: PathBuf,
}
struct DesktopProjectContextService {
    root: PathBuf,
    config_dir: PathBuf,
}
struct DesktopSkillStore {
    global_skills_dir: PathBuf,
    project_skills_dir: PathBuf,
}
struct DesktopPluginStore {
    global_plugins_dir: PathBuf,
    project_plugins_dir: PathBuf,
}
struct DesktopCheckpointStore {
    checkpoints_dir: PathBuf,
}

impl SettingsService for DesktopSettingsService {
    fn capability(&self) -> &'static str {
        let _ = (&self.global_settings, &self.project_settings);
        "desktop_settings"
    }
}
impl AuthService for DesktopAuthService {
    fn capability(&self) -> &'static str {
        let _ = &self.auth_file;
        "desktop_auth"
    }
}
impl SessionStore for DesktopSessionStore {
    fn capability(&self) -> &'static str {
        let _ = &self.sessions_dir;
        "desktop_sessions"
    }

    fn save(&self, record: SessionRecord) -> Result<(), RuntimeError> {
        self.memory.save(record)
    }

    fn load(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, RuntimeError> {
        self.memory.load(session_id)
    }
}
impl CacheStore for DesktopCacheStore {
    fn capability(&self) -> &'static str {
        let _ = &self.cache_dir;
        "desktop_cache"
    }
}
impl ProjectContextService for DesktopProjectContextService {
    fn capability(&self) -> &'static str {
        let _ = (&self.root, &self.config_dir);
        "desktop_project_context"
    }
}
impl SkillStore for DesktopSkillStore {
    fn capability(&self) -> &'static str {
        let _ = (&self.global_skills_dir, &self.project_skills_dir);
        "desktop_skills"
    }

    fn resolve(&self, request: SkillResolutionRequest) -> Result<SkillResolution, RuntimeError> {
        let skills = clankers_skills::discover_skills(&self.global_skills_dir, Some(&self.project_skills_dir));
        let snippets = skills
            .into_iter()
            .filter(|skill| request.requested.is_empty() || request.requested.iter().any(|name| name == &skill.name))
            .map(|skill| ResolvedSkillSnippet {
                name: skill.name,
                description: skill.description,
                content: skill.content,
                source: "desktop_skill_roots".to_string(),
            })
            .collect::<Vec<_>>();
        let receipt = ExtensionReceipt::new("desktop_skills", "resolve", clankers_runtime::ExtensionStatus::Succeeded)
            .with_metadata("global_root_configured", self.global_skills_dir.is_dir().to_string())
            .with_metadata("project_root_configured", self.project_skills_dir.is_dir().to_string())
            .with_metadata("snippet_count", snippets.len().to_string())
            .with_metadata("requested_count", request.requested.len().to_string());
        Ok(SkillResolution { snippets, receipt })
    }
}
impl PluginStore for DesktopPluginStore {
    fn capability(&self) -> &'static str {
        let _ = (&self.global_plugins_dir, &self.project_plugins_dir);
        "desktop_plugins"
    }
}
impl CheckpointStore for DesktopCheckpointStore {
    fn capability(&self) -> &'static str {
        let _ = &self.checkpoints_dir;
        "desktop_checkpoints"
    }
}

struct DesktopProviderRouterService {
    provider_router: Option<Arc<dyn clankers_provider::Provider>>,
}
struct DesktopExtensionAuthStoreService {
    auth_store: Option<Arc<std::sync::Mutex<clankers_provider::auth::AuthStore>>>,
}
struct DesktopCredentialPoolPolicyService {
    auth_store: Option<Arc<std::sync::Mutex<clankers_provider::auth::AuthStore>>>,
}
struct DesktopExtensionRuntimeService {
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
}

impl ProviderRouterService for DesktopProviderRouterService {
    fn capability(&self) -> &'static str {
        "desktop_provider_router"
    }

    fn complete(&self, request: ProviderModelRequest) -> Result<ProviderModelResponse, RuntimeError> {
        let Some(provider_router) = &self.provider_router else {
            return Err(RuntimeError::ExtensionUnavailable("desktop provider router not injected".to_string()));
        };
        execute_injected_provider_router(Arc::clone(provider_router), request)
    }
}

fn execute_injected_provider_router(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: ProviderModelRequest,
) -> Result<ProviderModelResponse, RuntimeError> {
    let provider_name = request.provider.clone();
    let route_source = request.route_source.clone();
    let model = request.model.clone().unwrap_or_else(|| {
        provider_router
            .models()
            .first()
            .map(|model| model.id.clone())
            .unwrap_or_else(|| provider_name.clone())
    });
    let session_id = request.session_id.clone();
    let request_model = model.clone();
    let provider_request = build_provider_completion_request(request, request_model)?;
    match block_on_provider_execution(provider_router, provider_request)? {
        Ok(stats) => {
            let mut receipt = ExtensionReceipt::new(
                "desktop_provider_router",
                "complete",
                clankers_runtime::ExtensionStatus::Succeeded,
            )
            .with_metadata("provider", provider_name)
            .with_metadata("model", model)
            .with_metadata("route_source", route_source)
            .with_metadata("stream_events", stats.stream_events.len().to_string())
            .with_metadata("text_delta_bytes", stats.text_delta_bytes.to_string())
            .with_metadata("thinking_delta_bytes", stats.thinking_delta_bytes.to_string());
            if let Some(session_id) = session_id {
                receipt = receipt.with_metadata("session_id", session_id);
            }
            Ok(ProviderModelResponse::completed(
                stats.stream_events,
                stats.content,
                stats.usage,
                stats.stop_reason,
                receipt,
            ))
        }
        Err(error) => Ok(provider_failure_response(provider_name, model, route_source, session_id, error)),
    }
}

fn provider_failure_response(
    provider_name: String,
    model: String,
    route_source: String,
    session_id: Option<String>,
    error: clankers_provider::error::ProviderError,
) -> ProviderModelResponse {
    let retryable = error.is_retryable();
    let failure = if retryable {
        ProviderModelFailure::retryable(error.to_string(), error.status_code())
    } else {
        ProviderModelFailure::terminal(error.to_string(), error.status_code())
    };
    let mut receipt =
        ExtensionReceipt::new("desktop_provider_router", "complete", clankers_runtime::ExtensionStatus::Failed)
            .with_error_class(clankers_runtime::ErrorClass::Model)
            .with_metadata("provider", provider_name)
            .with_metadata("model", model)
            .with_metadata("route_source", route_source)
            .with_metadata("retryable", retryable.to_string());
    if let Some(status) = error.status_code() {
        receipt = receipt.with_metadata("status", status.to_string());
    }
    if let Some(session_id) = session_id {
        receipt = receipt.with_metadata("session_id", session_id);
    }
    let status = if retryable {
        ProviderModelStatus::RetryableFailure
    } else {
        ProviderModelStatus::TerminalFailure
    };
    ProviderModelResponse::failure(status, failure, receipt)
}

fn build_provider_completion_request(
    request: ProviderModelRequest,
    model: String,
) -> Result<clankers_provider::CompletionRequest, RuntimeError> {
    if request.messages.is_empty() {
        return Err(RuntimeError::InvalidPrompt("provider model request missing messages".to_string()));
    }
    let mut extra_params = HashMap::new();
    if let Some(session_id) = request.session_id {
        extra_params.insert("_session_id".to_string(), serde_json::json!(session_id));
    }
    if let Some(account_label) = request.account_label {
        extra_params.insert("_account_label".to_string(), serde_json::json!(account_label));
    }
    Ok(clankers_provider::router_request_bridge::completion_request_from_bridge_input(
        clankers_provider::router_request_bridge::CompletionRequestBridgeInput {
            model,
            messages: request.messages.into_iter().map(runtime_message_to_bridge_message).collect(),
            system_prompt: request.system_prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: request.tools,
            thinking: request.thinking,
            no_cache: request.no_cache,
            cache_ttl: request.cache_ttl,
            extra_params,
        },
    ))
}

fn runtime_message_to_bridge_message(
    message: clankers_runtime::ProviderMessage,
) -> clankers_provider::router_request_bridge::CompletionRequestBridgeMessage {
    clankers_provider::router_request_bridge::CompletionRequestBridgeMessage {
        role: match message.role {
            ProviderMessageRole::User => {
                clankers_provider::router_request_bridge::CompletionRequestBridgeMessageRole::User
            }
            ProviderMessageRole::Assistant => {
                clankers_provider::router_request_bridge::CompletionRequestBridgeMessageRole::Assistant
            }
            ProviderMessageRole::Tool => {
                clankers_provider::router_request_bridge::CompletionRequestBridgeMessageRole::Tool
            }
            ProviderMessageRole::System => {
                clankers_provider::router_request_bridge::CompletionRequestBridgeMessageRole::System
            }
        },
        content: message.content,
        id: message.id,
        model: message.model,
        call_id: message.call_id,
        tool_name: message.tool_name,
        is_error: message.is_error,
    }
}

#[derive(Default)]
struct ProviderExecutionStats {
    stream_events: Vec<ProviderStreamEvent>,
    content: Vec<clanker_message::Content>,
    usage: Option<clanker_message::Usage>,
    stop_reason: Option<clanker_message::StopReason>,
    text_delta_bytes: usize,
    thinking_delta_bytes: usize,
}

fn block_on_provider_execution(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: clankers_provider::CompletionRequest,
) -> Result<Result<ProviderExecutionStats, clankers_provider::error::ProviderError>, RuntimeError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), tokio::runtime::RuntimeFlavor::MultiThread) => {
            Ok(tokio::task::block_in_place(|| handle.block_on(run_provider_completion(provider_router, request))))
        }
        Ok(_) => std::thread::spawn(move || run_provider_completion_on_new_runtime(provider_router, request))
            .join()
            .map_err(|_| RuntimeError::ExtensionUnavailable("provider runtime thread panicked".to_string()))?,
        Err(_) => run_provider_completion_on_new_runtime(provider_router, request),
    }
}

fn run_provider_completion_on_new_runtime(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: clankers_provider::CompletionRequest,
) -> Result<Result<ProviderExecutionStats, clankers_provider::error::ProviderError>, RuntimeError> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RuntimeError::ExtensionUnavailable(format!("provider runtime unavailable: {error}")))?
        .block_on(run_provider_completion(provider_router, request)))
}

async fn run_provider_completion(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: clankers_provider::CompletionRequest,
) -> Result<ProviderExecutionStats, clankers_provider::error::ProviderError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let mut completion = Box::pin(provider_router.complete(request, tx));
    let mut stats = ProviderExecutionStats::default();
    loop {
        tokio::select! {
            result = &mut completion => {
                result?;
                break;
            }
            event = rx.recv() => {
                if let Some(event) = event {
                    record_provider_stream_event(&mut stats, &event);
                }
            }
        }
    }
    while let Some(event) = rx.recv().await {
        record_provider_stream_event(&mut stats, &event);
    }
    Ok(stats)
}

fn record_provider_stream_event(stats: &mut ProviderExecutionStats, event: &clankers_provider::streaming::StreamEvent) {
    match event {
        clankers_provider::streaming::StreamEvent::MessageStart { message } => {
            stats.stream_events.push(ProviderStreamEvent::MessageStart {
                model: message.model.clone(),
                role: message.role.clone(),
            });
        }
        clankers_provider::streaming::StreamEvent::ContentBlockStart { index, content_block } => {
            stats.content.push(content_block.clone());
            stats.stream_events.push(ProviderStreamEvent::ContentBlockStart {
                index: *index,
                content: content_block.clone(),
            });
        }
        clankers_provider::streaming::StreamEvent::ContentBlockDelta { index, delta } => match delta {
            clankers_provider::streaming::ContentDelta::TextDelta { text } => {
                stats.text_delta_bytes += text.len();
                stats.stream_events.push(ProviderStreamEvent::TextDelta {
                    index: *index,
                    text: text.clone(),
                });
            }
            clankers_provider::streaming::ContentDelta::ThinkingDelta { thinking } => {
                stats.thinking_delta_bytes += thinking.len();
                stats.stream_events.push(ProviderStreamEvent::ThinkingDelta {
                    index: *index,
                    thinking: thinking.clone(),
                });
            }
            clankers_provider::streaming::ContentDelta::InputJsonDelta { partial_json } => {
                stats.stream_events.push(ProviderStreamEvent::ToolInputJsonDelta {
                    index: *index,
                    partial_json: partial_json.clone(),
                });
            }
            clankers_provider::streaming::ContentDelta::SignatureDelta { signature } => {
                stats.stream_events.push(ProviderStreamEvent::SignatureDelta {
                    index: *index,
                    signature: signature.clone(),
                });
            }
        },
        clankers_provider::streaming::StreamEvent::ContentBlockStop { index } => {
            stats.stream_events.push(ProviderStreamEvent::ContentBlockStop { index: *index });
        }
        clankers_provider::streaming::StreamEvent::MessageDelta { stop_reason, usage } => {
            let stop_reason = parse_provider_stop_reason(stop_reason.as_deref());
            stats.usage = Some(usage.clone());
            stats.stop_reason = stop_reason.clone();
            stats.stream_events.push(ProviderStreamEvent::Usage {
                stop_reason,
                usage: usage.clone(),
            });
        }
        clankers_provider::streaming::StreamEvent::MessageStop => {
            stats.stream_events.push(ProviderStreamEvent::MessageStop);
        }
        clankers_provider::streaming::StreamEvent::Error { error } => {
            stats.stream_events.push(ProviderStreamEvent::Error {
                message: clankers_runtime::EventMetadata::empty().with("error", error).fields["error"].clone(),
            });
        }
    }
}

fn parse_provider_stop_reason(stop_reason: Option<&str>) -> Option<clanker_message::StopReason> {
    match stop_reason {
        Some("tool_use") => Some(clanker_message::StopReason::ToolUse),
        Some("max_tokens") => Some(clanker_message::StopReason::MaxTokens),
        Some("stop") | Some("end_turn") => Some(clanker_message::StopReason::Stop),
        Some(_) | None => None,
    }
}

impl ExtensionAuthStoreService for DesktopExtensionAuthStoreService {
    fn capability(&self) -> &'static str {
        "desktop_extension_auth_store"
    }

    fn access(&self, request: AuthStoreAccessRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let Some(auth_store) = &self.auth_store else {
            return Err(RuntimeError::ExtensionUnavailable("desktop auth store not injected".to_string()));
        };
        match request.operation {
            AuthStoreOperation::Lookup => auth_lookup_receipt(auth_store, request),
            AuthStoreOperation::RefreshPersist => Err(RuntimeError::ExtensionUnavailable(
                "desktop runtime auth refresh persistence not injected".to_string(),
            )),
            AuthStoreOperation::PendingLoginVerifier => Err(RuntimeError::ExtensionUnavailable(
                "desktop runtime pending login verifier store not injected".to_string(),
            )),
        }
    }
}

fn auth_lookup_receipt(
    auth_store: &Arc<std::sync::Mutex<clankers_provider::auth::AuthStore>>,
    request: AuthStoreAccessRequest,
) -> Result<ExtensionReceipt, RuntimeError> {
    let store = auth_store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let credentials = store.all_credentials(&request.provider);
    let selected = select_credential_summary(&credentials, request.account_label.as_deref());
    let mut receipt = ExtensionReceipt::new(
        "desktop_extension_auth_store",
        "lookup",
        if selected.is_some() {
            clankers_runtime::ExtensionStatus::Succeeded
        } else {
            clankers_runtime::ExtensionStatus::Unavailable
        },
    )
    .with_metadata("provider", request.provider)
    .with_metadata("credential_count", credentials.len().to_string());
    if let Some(account_label) = request.account_label {
        receipt = receipt.with_metadata("requested_account", account_label);
    }
    if let Some(summary) = selected {
        receipt = receipt
            .with_metadata("selected_account", summary.account)
            .with_metadata("credential_kind", summary.kind);
    } else {
        receipt = receipt.with_error_class(clankers_runtime::ErrorClass::Extension);
    }
    Ok(receipt)
}

struct CredentialSummary {
    account: String,
    kind: &'static str,
}

fn select_credential_summary(
    credentials: &[(String, clankers_provider::auth::StoredCredential)],
    account_label: Option<&str>,
) -> Option<CredentialSummary> {
    let (account, credential) = account_label
        .and_then(|requested| credentials.iter().find(|(account, _)| account == requested))
        .or_else(|| credentials.first())?;
    Some(CredentialSummary {
        account: account.clone(),
        kind: credential_kind(credential),
    })
}

fn credential_kind(credential: &clankers_provider::auth::StoredCredential) -> &'static str {
    match credential {
        clankers_provider::auth::StoredCredential::ApiKey { .. } => "static",
        clankers_provider::auth::StoredCredential::OAuth { .. } => "oauth",
    }
}

impl CredentialPoolPolicyService for DesktopCredentialPoolPolicyService {
    fn capability(&self) -> &'static str {
        "desktop_credential_pool"
    }

    fn select(&self, request: CredentialPoolRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let Some(auth_store) = &self.auth_store else {
            return Err(RuntimeError::ExtensionUnavailable("desktop credential pool not injected".to_string()));
        };
        let store = auth_store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let credentials = store.all_credentials(&request.provider);
        let selected = select_credential_summary(&credentials, request.account_label.as_deref());
        let mut receipt = ExtensionReceipt::new(
            "desktop_credential_pool",
            "select",
            if selected.is_some() {
                clankers_runtime::ExtensionStatus::Succeeded
            } else {
                clankers_runtime::ExtensionStatus::Unavailable
            },
        )
        .with_metadata("provider", request.provider)
        .with_metadata("strategy", request.strategy)
        .with_metadata("available_credentials", credentials.len().to_string());
        if let Some(account_label) = request.account_label {
            receipt = receipt.with_metadata("requested_account", account_label);
        }
        if let Some(summary) = selected {
            receipt = receipt
                .with_metadata("selected_account", summary.account)
                .with_metadata("credential_kind", summary.kind);
        } else {
            receipt = receipt.with_error_class(clankers_runtime::ErrorClass::Extension);
        }
        Ok(receipt)
    }
}

impl ExtensionRuntimeService for DesktopExtensionRuntimeService {
    fn capability(&self) -> &'static str {
        "desktop_extension_runtime"
    }

    fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
        if kind != ExtensionRuntimeKind::Plugin {
            return Ok(Vec::new());
        }
        let Some(plugin_manager) = &self.plugin_manager else {
            return Ok(Vec::new());
        };
        let host = crate::plugin::PluginHostFacade::new(Arc::clone(plugin_manager));
        let mut descriptors = Vec::new();
        for plugin in host.active_plugins() {
            if !plugin.manifest.kind.uses_wasm_runtime() {
                continue;
            }
            if plugin.manifest.tool_definitions.is_empty() {
                for tool_name in &plugin.manifest.tools {
                    descriptors.push(
                        ExtensionToolDescriptor::new(
                            ExtensionRuntimeKind::Plugin,
                            tool_name.clone(),
                            Some(tool_name.clone()),
                            SideEffectLevel::ExternalIo,
                        )
                        .with_metadata("plugin", plugin.name.clone())
                        .with_metadata("runtime_entrypoint", "handle_tool_call"),
                    );
                }
            } else {
                for tool in &plugin.manifest.tool_definitions {
                    descriptors.push(
                        ExtensionToolDescriptor::new(
                            ExtensionRuntimeKind::Plugin,
                            tool.name.clone(),
                            Some(tool.name.clone()),
                            SideEffectLevel::ExternalIo,
                        )
                        .with_metadata("plugin", plugin.name.clone())
                        .with_metadata("runtime_entrypoint", tool.handler.clone()),
                    );
                }
            }
        }
        Ok(descriptors)
    }

    fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
        if request.kind != ExtensionRuntimeKind::Plugin {
            return Err(RuntimeError::ExtensionUnavailable("unsupported desktop extension runtime kind".to_string()));
        }
        let Some(plugin_manager) = &self.plugin_manager else {
            return Err(RuntimeError::ExtensionUnavailable("desktop plugin runtime not injected".to_string()));
        };
        let plugin_name = request
            .extension_name
            .clone()
            .or_else(|| (!request.action.trim().is_empty()).then_some(request.action.clone()))
            .ok_or_else(|| RuntimeError::InvalidTool("plugin runtime request missing extension name".to_string()))?;
        let visible_tool = request
            .visible_tool_name
            .clone()
            .or_else(|| request.original_tool_name.clone())
            .unwrap_or_else(|| "plugin_tool".to_string());
        let handler = request.runtime_entrypoint.clone().unwrap_or_else(|| "handle_tool_call".to_string());
        let envelope = serde_json::json!({
            "tool": visible_tool,
            "args": request.arguments,
        });
        let input = serde_json::to_string(&envelope)
            .map_err(|error| RuntimeError::InvalidTool(format!("plugin runtime request encoding failed: {error}")))?;
        let output = {
            let manager = plugin_manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            manager.call_plugin(&plugin_name, &handler, &input)
        };
        match output {
            Ok(output) => Ok(ExtensionReceipt::new(
                "desktop_plugin_runtime",
                "execute",
                clankers_runtime::ExtensionStatus::Succeeded,
            )
            .with_metadata("plugin", plugin_name)
            .with_metadata("visible_tool", visible_tool)
            .with_metadata("runtime_entrypoint", handler)
            .with_metadata("output_bytes", output.len().to_string())),
            Err(_error) => Ok(ExtensionReceipt::new(
                "desktop_plugin_runtime",
                "execute",
                clankers_runtime::ExtensionStatus::Failed,
            )
            .with_error_class(clankers_runtime::ErrorClass::Extension)
            .with_metadata("plugin", plugin_name)
            .with_metadata("visible_tool", visible_tool)
            .with_metadata("runtime_entrypoint", handler)
            .with_metadata("error", "plugin_call_failed")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use clankers_runtime::AuthStoreAccessRequest;
    use clankers_runtime::AuthStoreOperation;
    use clankers_runtime::CredentialPoolRequest;
    use clankers_runtime::ExtensionRuntimeKind;
    use clankers_runtime::ExtensionRuntimeRequest;
    use clankers_runtime::ExtensionStatus;
    use clankers_runtime::ProviderModelRequest;
    use clankers_runtime::ProviderModelStatus;
    use clankers_runtime::ProviderStreamEvent;
    use clankers_runtime::RuntimeError;

    use super::DesktopRuntimeServiceAdapters;

    #[derive(Default)]
    struct RecordingProvider {
        requests: Mutex<Vec<clankers_provider::CompletionRequest>>,
        models: Vec<clankers_provider::Model>,
    }

    struct FailingProvider {
        status: u16,
        message: &'static str,
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for RecordingProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            self.requests.lock().unwrap().push(request);
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStart {
                index: 0,
                content_block: clanker_message::Content::Text {
                    text: "model output that must not appear in receipt".to_string(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: clankers_provider::streaming::ContentDelta::TextDelta {
                    text: "model output that must not appear in receipt".to_string(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageDelta {
                stop_reason: Some("stop".to_string()),
                usage: clanker_message::Usage {
                    input_tokens: 3,
                    output_tokens: 5,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 1,
                },
            })
            .await
            .ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &self.models
        }

        fn name(&self) -> &str {
            "recording-provider"
        }
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for FailingProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            Err(clankers_provider::error::provider_err_with_status_for_provider(
                self.status,
                self.message,
                "failing-provider",
            ))
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "failing-provider"
        }
    }

    fn test_provider_execution_request() -> ProviderModelRequest {
        let mut request = ProviderModelRequest::user_prompt(
            "recording-provider",
            Some("recording-model".to_string()),
            "prompt secret text must not appear in receipt",
        );
        request.account_label = Some("desktop-account".to_string());
        request.route_source = "runtime-test".to_string();
        request.system_prompt = Some("system secret text must not appear in receipt".to_string());
        request.max_tokens = Some(32);
        request.session_id = Some("session-provider-runtime".to_string());
        request
    }

    fn provider_request_for(provider: &str, model: &str, account: &str) -> ProviderModelRequest {
        let mut request = ProviderModelRequest::user_prompt(provider, Some(model.to_string()), "hello");
        request.account_label = Some(account.to_string());
        request.route_source = "prefix-parity".to_string();
        request.session_id = Some(format!("session-{account}"));
        request
    }

    fn injected_auth_store() -> Arc<Mutex<clankers_provider::auth::AuthStore>> {
        let mut store = clankers_provider::auth::AuthStore::default();
        store.set_credential("openrouter", "primary", clankers_provider::auth::StoredCredential::ApiKey {
            api_key: "sk-secret-runtime-auth-value".to_string(),
            label: Some("primary label must not leak".to_string()),
        });
        store.set_credential("openrouter", "backup", clankers_provider::auth::StoredCredential::OAuth {
            access_token: "oauth-access-secret-runtime-auth-value".to_string(),
            refresh_token: "oauth-refresh-secret-runtime-auth-value".to_string(),
            expires_at_ms: i64::MAX,
            label: None,
        });
        Arc::new(Mutex::new(store))
    }

    fn lookup_request(account_label: Option<&str>) -> AuthStoreAccessRequest {
        AuthStoreAccessRequest {
            provider: "openrouter".to_string(),
            account_label: account_label.map(ToString::to_string),
            operation: AuthStoreOperation::Lookup,
        }
    }

    fn pool_request(account_label: Option<&str>) -> CredentialPoolRequest {
        CredentialPoolRequest {
            provider: "openrouter".to_string(),
            strategy: "round_robin".to_string(),
            account_label: account_label.map(ToString::to_string),
        }
    }

    #[test]
    fn desktop_runtime_skill_service_resolves_explicit_roots_without_content_leaks() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let skill_dir = project_paths.skills_dir.join("review");
        std::fs::create_dir_all(&skill_dir).expect("skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: review\ndescription: Review code\n---\nUse safe review steps.",
        )
        .expect("skill file");
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let resolution = services
            .skills
            .resolve(clankers_runtime::SkillResolutionRequest {
                requested: vec!["review".to_string()],
            })
            .expect("skill resolution");

        assert_eq!(resolution.snippets.len(), 1);
        assert_eq!(resolution.snippets[0].name, "review");
        assert!(resolution.snippets[0].content.contains("Use safe review steps."));
        assert_eq!(resolution.receipt.source, "desktop_skills");
        assert_eq!(resolution.receipt.metadata.fields.get("snippet_count").unwrap(), "1");
        assert!(!serde_json::to_string(&resolution.receipt).unwrap().contains("Use safe review steps"));
        assert!(!resolution.receipt.contains_secret_markers());
    }

    #[test]
    fn desktop_runtime_provider_router_fails_closed_without_injection() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let error = services.extensions.provider_router.complete(test_provider_execution_request()).unwrap_err();

        assert_eq!(error, RuntimeError::ExtensionUnavailable("desktop provider router not injected".to_string()));
    }

    #[test]
    fn desktop_runtime_provider_router_executes_injected_provider_with_safe_receipt() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let provider = Arc::new(RecordingProvider::default());
        let provider_for_assert = Arc::clone(&provider);
        let services = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(&paths, &project_paths, provider);

        let response = services
            .extensions
            .provider_router
            .complete(test_provider_execution_request())
            .expect("provider response");
        let receipt = &response.receipt;

        assert_eq!(response.status, ProviderModelStatus::Completed);
        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(receipt.source, "desktop_provider_router");
        assert_eq!(receipt.metadata.fields.get("provider").unwrap(), "recording-provider");
        assert_eq!(receipt.metadata.fields.get("model").unwrap(), "recording-model");
        assert_eq!(receipt.metadata.fields.get("route_source").unwrap(), "runtime-test");
        assert_eq!(receipt.metadata.fields.get("session_id").unwrap(), "session-provider-runtime");
        assert_eq!(receipt.metadata.fields.get("stream_events").unwrap(), "3");
        assert!(receipt.metadata.fields.contains_key("text_delta_bytes"));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("prompt secret")));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("system secret")));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("model output")));
        assert!(!receipt.contains_secret_markers());
        assert_eq!(response.usage.as_ref().unwrap().output_tokens, 5);
        assert!(response.stream_events.iter().any(|event| matches!(event, ProviderStreamEvent::TextDelta { .. })));

        let requests = provider_for_assert.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].model, "recording-model");
        assert_eq!(requests[0].max_tokens, Some(32));
        assert_eq!(requests[0].system_prompt.as_deref(), Some("system secret text must not appear in receipt"));
        assert_eq!(requests[0].extra_params.get("_session_id"), Some(&serde_json::json!("session-provider-runtime")),);
        assert_eq!(requests[0].extra_params.get("_account_label"), Some(&serde_json::json!("desktop-account")),);
    }

    #[test]
    fn desktop_runtime_provider_router_projects_retryable_and_terminal_failures() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());

        let retryable = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(
            &paths,
            &project_paths,
            Arc::new(FailingProvider {
                status: 429,
                message: "rate limit secret-token should redact",
            }),
        )
        .extensions
        .provider_router
        .complete(test_provider_execution_request())
        .expect("retryable provider failure response");
        assert_eq!(retryable.status, ProviderModelStatus::RetryableFailure);
        assert_eq!(retryable.failure.as_ref().unwrap().status, Some(429));
        assert!(retryable.failure.as_ref().unwrap().retryable);
        assert_eq!(retryable.receipt.status, ExtensionStatus::Failed);
        assert!(!serde_json::to_string(&retryable).unwrap().contains("secret-token"));

        let terminal = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(
            &paths,
            &project_paths,
            Arc::new(FailingProvider {
                status: 400,
                message: "bad request",
            }),
        )
        .extensions
        .provider_router
        .complete(test_provider_execution_request())
        .expect("terminal provider failure response");
        assert_eq!(terminal.status, ProviderModelStatus::TerminalFailure);
        assert_eq!(terminal.failure.as_ref().unwrap().status, Some(400));
        assert!(!terminal.failure.as_ref().unwrap().retryable);
        assert_eq!(terminal.receipt.metadata.fields.get("status").unwrap(), "400");
    }

    #[test]
    fn desktop_runtime_provider_router_preserves_codex_and_openai_model_prefixes() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let provider = Arc::new(RecordingProvider::default());
        let provider_for_assert = Arc::clone(&provider);
        let services = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(&paths, &project_paths, provider);

        services
            .extensions
            .provider_router
            .complete(provider_request_for("openai-codex", "openai-codex/gpt-5.3-codex", "codex-account"))
            .expect("codex response");
        services
            .extensions
            .provider_router
            .complete(provider_request_for("openai", "openai/gpt-5.3", "openai-account"))
            .expect("openai response");

        let requests = provider_for_assert.requests.lock().unwrap();
        assert_eq!(requests[0].model, "openai-codex/gpt-5.3-codex");
        assert_eq!(requests[0].extra_params.get("_account_label"), Some(&serde_json::json!("codex-account")));
        assert_eq!(requests[1].model, "openai/gpt-5.3");
        assert_eq!(requests[1].extra_params.get("_account_label"), Some(&serde_json::json!("openai-account")));
    }

    #[test]
    fn desktop_runtime_known_provider_prefix_fails_closed_without_adapter() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let error = services
            .extensions
            .provider_router
            .complete(provider_request_for("openai-codex", "openai-codex/gpt-5.3-codex", "codex-account"))
            .unwrap_err();

        assert_eq!(error, RuntimeError::ExtensionUnavailable("desktop provider router not injected".to_string()));
    }

    #[test]
    fn desktop_runtime_provider_router_rejects_missing_messages_before_provider_call() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let provider = Arc::new(RecordingProvider::default());
        let provider_for_assert = Arc::clone(&provider);
        let services = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(&paths, &project_paths, provider);
        let mut request = test_provider_execution_request();
        request.messages.clear();

        let error = services.extensions.provider_router.complete(request).unwrap_err();

        assert_eq!(error, RuntimeError::InvalidPrompt("provider model request missing messages".to_string()),);
        assert!(provider_for_assert.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn desktop_runtime_auth_services_fail_closed_without_injection() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let auth_error = services.extensions.auth_store.access(lookup_request(Some("primary"))).unwrap_err();
        let pool_error = services.extensions.credential_pool.select(pool_request(Some("primary"))).unwrap_err();

        assert_eq!(auth_error, RuntimeError::ExtensionUnavailable("desktop auth store not injected".to_string()));
        assert_eq!(pool_error, RuntimeError::ExtensionUnavailable("desktop credential pool not injected".to_string()));
    }

    #[test]
    fn desktop_runtime_auth_lookup_uses_injected_store_with_safe_receipt() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services =
            DesktopRuntimeServiceAdapters::from_paths_with_auth_store(&paths, &project_paths, injected_auth_store());

        let receipt = services.extensions.auth_store.access(lookup_request(Some("backup"))).expect("lookup receipt");

        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(receipt.source, "desktop_extension_auth_store");
        assert_eq!(receipt.action, "lookup");
        assert_eq!(receipt.metadata.fields.get("provider").unwrap(), "openrouter");
        assert_eq!(receipt.metadata.fields.get("requested_account").unwrap(), "backup");
        assert_eq!(receipt.metadata.fields.get("selected_account").unwrap(), "backup");
        assert_eq!(receipt.metadata.fields.get("credential_kind").unwrap(), "oauth");
        assert_eq!(receipt.metadata.fields.get("credential_count").unwrap(), "2");
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("secret-runtime-auth-value")));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("primary label must not leak")));
        assert!(!receipt.contains_secret_markers());
    }

    #[test]
    fn desktop_runtime_credential_pool_selects_from_injected_store_with_safe_receipt() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services =
            DesktopRuntimeServiceAdapters::from_paths_with_auth_store(&paths, &project_paths, injected_auth_store());

        let receipt = services.extensions.credential_pool.select(pool_request(Some("primary"))).expect("pool receipt");

        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(receipt.source, "desktop_credential_pool");
        assert_eq!(receipt.metadata.fields.get("provider").unwrap(), "openrouter");
        assert_eq!(receipt.metadata.fields.get("strategy").unwrap(), "round_robin");
        assert_eq!(receipt.metadata.fields.get("selected_account").unwrap(), "primary");
        assert_eq!(receipt.metadata.fields.get("credential_kind").unwrap(), "static");
        assert_eq!(receipt.metadata.fields.get("available_credentials").unwrap(), "2");
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("secret-runtime-auth-value")));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("primary label must not leak")));
        assert!(!receipt.contains_secret_markers());
    }

    #[test]
    fn desktop_runtime_auth_mutations_fail_closed_even_with_read_only_store() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services =
            DesktopRuntimeServiceAdapters::from_paths_with_auth_store(&paths, &project_paths, injected_auth_store());
        let mut refresh = lookup_request(Some("primary"));
        refresh.operation = AuthStoreOperation::RefreshPersist;
        let mut verifier = lookup_request(Some("primary"));
        verifier.operation = AuthStoreOperation::PendingLoginVerifier;

        let refresh_error = services.extensions.auth_store.access(refresh).unwrap_err();
        let verifier_error = services.extensions.auth_store.access(verifier).unwrap_err();

        assert_eq!(
            refresh_error,
            RuntimeError::ExtensionUnavailable("desktop runtime auth refresh persistence not injected".to_string())
        );
        assert_eq!(
            verifier_error,
            RuntimeError::ExtensionUnavailable("desktop runtime pending login verifier store not injected".to_string())
        );
    }

    #[test]
    fn desktop_runtime_mixed_injected_services_do_not_fall_back_to_ambient() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let provider = Arc::new(RecordingProvider::default());
        let provider_for_assert = Arc::clone(&provider);
        let services = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(&paths, &project_paths, provider);

        let response = services
            .extensions
            .provider_router
            .complete(test_provider_execution_request())
            .expect("injected provider response");
        let auth_error = services.extensions.auth_store.access(lookup_request(Some("primary"))).unwrap_err();
        let pool_error = services.extensions.credential_pool.select(pool_request(Some("primary"))).unwrap_err();
        let runtime_error = services
            .extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("clankers-test-plugin".to_string()),
                visible_tool_name: Some("test_echo".to_string()),
                original_tool_name: Some("test_echo".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: serde_json::json!({"text": "ambient fallback forbidden"}),
            })
            .unwrap_err();

        assert_eq!(response.status, ProviderModelStatus::Completed);
        assert_eq!(response.receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(provider_for_assert.requests.lock().unwrap().len(), 1);
        assert_eq!(auth_error, RuntimeError::ExtensionUnavailable("desktop auth store not injected".to_string()));
        assert_eq!(pool_error, RuntimeError::ExtensionUnavailable("desktop credential pool not injected".to_string()));
        assert_eq!(
            runtime_error,
            RuntimeError::ExtensionUnavailable("desktop plugin runtime not injected".to_string())
        );
    }

    #[test]
    fn desktop_runtime_services_publish_explicit_capabilities() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());

        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);
        let metadata = services.capability_metadata();

        assert_eq!(metadata.fields.get("settings").unwrap(), "desktop_settings");
        assert_eq!(metadata.fields.get("auth").unwrap(), "desktop_auth");
        assert_eq!(metadata.fields.get("sessions").unwrap(), "desktop_sessions");
        assert_eq!(metadata.fields.get("plugins").unwrap(), "desktop_plugins");
        assert_eq!(metadata.fields.get("project_context").unwrap(), "desktop_project_context");
        assert_eq!(metadata.fields.get("provider_router").unwrap(), "desktop_provider_router");
        assert_eq!(metadata.fields.get("extension_auth_store").unwrap(), "desktop_extension_auth_store");
        assert_eq!(metadata.fields.get("credential_pool").unwrap(), "desktop_credential_pool");
        assert_eq!(metadata.fields.get("extension_runtime").unwrap(), "desktop_extension_runtime");
    }

    #[test]
    fn desktop_runtime_extension_executes_injected_wasm_plugin_with_safe_receipt() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let plugins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut plugin_manager = crate::plugin::PluginManager::new(plugins_dir, None);
        plugin_manager.discover();
        plugin_manager.load_wasm("clankers-test-plugin").expect("test plugin loads");
        let plugin_manager = Arc::new(Mutex::new(plugin_manager));

        let services =
            DesktopRuntimeServiceAdapters::from_paths_with_plugin_manager(&paths, &project_paths, plugin_manager);
        let descriptors = services
            .extensions
            .runtime
            .publishable_tools(ExtensionRuntimeKind::Plugin)
            .expect("plugin descriptors");
        assert!(descriptors.iter().any(|descriptor| descriptor.visible_tool_name == "test_echo"));

        let receipt = services
            .extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("clankers-test-plugin".to_string()),
                visible_tool_name: Some("test_echo".to_string()),
                original_tool_name: Some("test_echo".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: serde_json::json!({"text": "hello via runtime seam"}),
            })
            .expect("plugin receipt");

        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(receipt.source, "desktop_plugin_runtime");
        assert_eq!(receipt.metadata.fields.get("plugin").unwrap(), "clankers-test-plugin");
        assert_eq!(receipt.metadata.fields.get("visible_tool").unwrap(), "test_echo");
        assert_eq!(receipt.metadata.fields.get("runtime_entrypoint").unwrap(), "handle_tool_call");
        assert!(receipt.metadata.fields.contains_key("output_bytes"));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("hello via runtime seam")));
        assert!(!receipt.contains_secret_markers());
    }

    #[test]
    fn desktop_runtime_extension_fails_closed_without_injected_plugin_manager() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let error = services
            .extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("clankers-test-plugin".to_string()),
                visible_tool_name: Some("test_echo".to_string()),
                original_tool_name: Some("test_echo".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: serde_json::json!({"text": "not executed"}),
            })
            .unwrap_err();

        assert_eq!(error, RuntimeError::ExtensionUnavailable("desktop plugin runtime not injected".to_string()));
    }
}
