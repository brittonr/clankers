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
use clankers_runtime::ProviderExecutionRequest;
use clankers_runtime::ProviderRouterService;
use clankers_runtime::RuntimeError;
use clankers_runtime::RuntimeServices;
use clankers_runtime::SessionId;
use clankers_runtime::SessionRecord;
use clankers_runtime::SessionStore;
use clankers_runtime::SettingsService;
use clankers_runtime::SideEffectLevel;
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

    fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let Some(provider_router) = &self.provider_router else {
            return Err(RuntimeError::ExtensionUnavailable("desktop provider router not injected".to_string()));
        };
        execute_injected_provider_router(Arc::clone(provider_router), request)
    }
}

fn execute_injected_provider_router(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: ProviderExecutionRequest,
) -> Result<ExtensionReceipt, RuntimeError> {
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
    let stats = block_on_provider_execution(provider_router, provider_request)?;

    let mut receipt =
        ExtensionReceipt::new("desktop_provider_router", "execute", clankers_runtime::ExtensionStatus::Succeeded)
            .with_metadata("provider", provider_name)
            .with_metadata("model", model)
            .with_metadata("route_source", route_source)
            .with_metadata("stream_events", stats.stream_events.to_string())
            .with_metadata("text_delta_bytes", stats.text_delta_bytes.to_string())
            .with_metadata("thinking_delta_bytes", stats.thinking_delta_bytes.to_string());
    if let Some(session_id) = session_id {
        receipt = receipt.with_metadata("session_id", session_id);
    }
    Ok(receipt)
}

fn build_provider_completion_request(
    request: ProviderExecutionRequest,
    model: String,
) -> Result<clankers_provider::CompletionRequest, RuntimeError> {
    let prompt = request
        .prompt
        .filter(|prompt| !prompt.trim().is_empty())
        .ok_or_else(|| RuntimeError::InvalidPrompt("provider execution request missing prompt".to_string()))?;
    let mut extra_params = HashMap::new();
    if let Some(session_id) = request.session_id {
        extra_params.insert("_session_id".to_string(), serde_json::json!(session_id));
    }
    if let Some(account_label) = request.account_label {
        extra_params.insert("_account_label".to_string(), serde_json::json!(account_label));
    }
    Ok(clankers_provider::CompletionRequest {
        model,
        messages: vec![clankers_provider::message::AgentMessage::User(
            clankers_provider::message::UserMessage {
                id: clankers_provider::message::MessageId::new("runtime-provider-user"),
                content: vec![clankers_provider::message::Content::Text { text: prompt }],
                timestamp: chrono::Utc::now(),
            },
        )],
        system_prompt: request.system_prompt,
        max_tokens: request.max_tokens,
        temperature: None,
        tools: Vec::new(),
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params,
    })
}

#[derive(Default)]
struct ProviderExecutionStats {
    stream_events: usize,
    text_delta_bytes: usize,
    thinking_delta_bytes: usize,
}

fn block_on_provider_execution(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: clankers_provider::CompletionRequest,
) -> Result<ProviderExecutionStats, RuntimeError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), tokio::runtime::RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(|| handle.block_on(run_provider_completion(provider_router, request)))
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
) -> Result<ProviderExecutionStats, RuntimeError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RuntimeError::ExtensionUnavailable(format!("provider runtime unavailable: {error}")))?
        .block_on(run_provider_completion(provider_router, request))
}

async fn run_provider_completion(
    provider_router: Arc<dyn clankers_provider::Provider>,
    request: clankers_provider::CompletionRequest,
) -> Result<ProviderExecutionStats, RuntimeError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let mut completion = Box::pin(provider_router.complete(request, tx));
    let mut stats = ProviderExecutionStats::default();
    loop {
        tokio::select! {
            result = &mut completion => {
                result.map_err(|error| RuntimeError::Model(error.to_string()))?;
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
    stats.stream_events += 1;
    if let clankers_provider::streaming::StreamEvent::ContentBlockDelta { delta, .. } = event {
        match delta {
            clankers_provider::streaming::ContentDelta::TextDelta { text } => {
                stats.text_delta_bytes += text.len();
            }
            clankers_provider::streaming::ContentDelta::ThinkingDelta { thinking } => {
                stats.thinking_delta_bytes += thinking.len();
            }
            clankers_provider::streaming::ContentDelta::InputJsonDelta { .. }
            | clankers_provider::streaming::ContentDelta::SignatureDelta { .. } => {}
        }
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
    use clankers_runtime::ProviderExecutionRequest;
    use clankers_runtime::RuntimeError;

    use super::DesktopRuntimeServiceAdapters;

    #[derive(Default)]
    struct RecordingProvider {
        requests: Mutex<Vec<clankers_provider::CompletionRequest>>,
        models: Vec<clankers_provider::Model>,
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for RecordingProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            self.requests.lock().unwrap().push(request);
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: clankers_provider::streaming::ContentDelta::TextDelta {
                    text: "model output that must not appear in receipt".to_string(),
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

    fn test_provider_execution_request() -> ProviderExecutionRequest {
        ProviderExecutionRequest {
            provider: "recording-provider".to_string(),
            model: Some("recording-model".to_string()),
            account_label: Some("desktop-account".to_string()),
            route_source: "runtime-test".to_string(),
            prompt: Some("prompt secret text must not appear in receipt".to_string()),
            system_prompt: Some("system secret text must not appear in receipt".to_string()),
            max_tokens: Some(32),
            session_id: Some("session-provider-runtime".to_string()),
        }
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
    fn desktop_runtime_provider_router_fails_closed_without_injection() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let error = services.extensions.provider_router.execute(test_provider_execution_request()).unwrap_err();

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

        let receipt = services
            .extensions
            .provider_router
            .execute(test_provider_execution_request())
            .expect("provider receipt");

        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(receipt.source, "desktop_provider_router");
        assert_eq!(receipt.metadata.fields.get("provider").unwrap(), "recording-provider");
        assert_eq!(receipt.metadata.fields.get("model").unwrap(), "recording-model");
        assert_eq!(receipt.metadata.fields.get("route_source").unwrap(), "runtime-test");
        assert_eq!(receipt.metadata.fields.get("session_id").unwrap(), "session-provider-runtime");
        assert_eq!(receipt.metadata.fields.get("stream_events").unwrap(), "1");
        assert!(receipt.metadata.fields.contains_key("text_delta_bytes"));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("prompt secret")));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("system secret")));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("model output")));
        assert!(!receipt.contains_secret_markers());

        let requests = provider_for_assert.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].model, "recording-model");
        assert_eq!(requests[0].max_tokens, Some(32));
        assert_eq!(requests[0].system_prompt.as_deref(), Some("system secret text must not appear in receipt"));
        assert_eq!(requests[0].extra_params.get("_session_id"), Some(&serde_json::json!("session-provider-runtime")),);
        assert_eq!(requests[0].extra_params.get("_account_label"), Some(&serde_json::json!("desktop-account")),);
    }

    #[test]
    fn desktop_runtime_provider_router_rejects_missing_prompt_before_provider_call() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let provider = Arc::new(RecordingProvider::default());
        let provider_for_assert = Arc::clone(&provider);
        let services = DesktopRuntimeServiceAdapters::from_paths_with_provider_router(&paths, &project_paths, provider);
        let mut request = test_provider_execution_request();
        request.prompt = Some("   ".to_string());

        let error = services.extensions.provider_router.execute(request).unwrap_err();

        assert_eq!(error, RuntimeError::InvalidPrompt("provider execution request missing prompt".to_string()),);
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
