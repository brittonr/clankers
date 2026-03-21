//! Anthropic Messages API provider

pub mod api;
pub mod streaming;

use std::sync::Arc;

use async_trait::async_trait;
use clanker_router::credential_pool::CredentialPool;
use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use crate::CompletionRequest;
use crate::Model;
use crate::Provider;
use crate::auth::Credential;
use crate::credential_manager::CredentialManager;
use crate::error::Result;
use crate::streaming::StreamEvent;

pub struct AnthropicProvider {
    client: api::AnthropicClient,
    /// Legacy direct credential (for API key users or when no auth path is available)
    credential: Option<Credential>,
    /// Credential manager with auto-refresh (preferred for OAuth)
    credential_manager: Option<Arc<CredentialManager>>,
    /// Multi-account credential pool with failover/round-robin
    credential_pool: Option<CredentialPool>,
    models: Vec<Model>,
}

impl AnthropicProvider {
    /// Create a provider with a simple credential (no auto-refresh).
    pub fn new(credential: Credential, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: Some(credential),
            credential_manager: None,
            credential_pool: None,
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Create a provider with a credential manager that supports auto-refresh.
    pub fn with_credential_manager(credential_manager: Arc<CredentialManager>, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: None,
            credential_manager: Some(credential_manager),
            credential_pool: None,
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Create a provider with a credential pool for multi-account failover.
    ///
    /// The credential manager handles OAuth refresh for the primary account.
    /// The pool provides failover to other accounts when one gets rate-limited.
    pub fn with_credential_pool(
        credential_manager: Arc<CredentialManager>,
        pool: CredentialPool,
        base_url: Option<String>,
    ) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: None,
            credential_manager: Some(credential_manager),
            credential_pool: Some(pool),
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Get the current credential, refreshing if needed.
    async fn get_credential(&self) -> Result<Credential> {
        if let Some(ref cm) = self.credential_manager {
            cm.get_credential().await
        } else if let Some(ref cred) = self.credential {
            Ok(cred.clone())
        } else {
            Err(crate::error::auth_err("No credential configured"))
        }
    }

    /// Force-refresh the credential (called on 401).
    async fn force_refresh_credential(&self) -> Result<Credential> {
        if let Some(ref cm) = self.credential_manager {
            cm.force_refresh().await
        } else {
            Err(crate::error::auth_err("Cannot refresh: no credential manager configured"))
        }
    }

    /// Try a request with a specific credential.
    async fn try_with_credential(
        &self,
        request: &CompletionRequest,
        credential: &Credential,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> std::result::Result<(), (u16, String)> {
        let api_request = api::build_api_request(request, credential.is_oauth());
        let response = match self.client.send_streaming(&api_request, credential).await {
            Ok(r) => r,
            Err(e) => return Err((500, e.to_string())),
        };

        if response.status().is_success() {
            streaming::parse_sse_stream(response, tx.clone())
                .await
                .map_err(|e| (500, e.to_string()))
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err((status, format!("Anthropic API error {}: {}", status, body)))
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // ── Multi-account path: try each credential from the pool ────
        if let Some(ref pool) = self.credential_pool {
            let leases = pool.select_all_available().await;
            if leases.is_empty() {
                // Pool exhausted — fall through to single-credential path
                // which may refresh the primary OAuth token
                warn!("All credential pool slots unavailable, trying primary credential");
            } else {
                let mut last_status = 0u16;
                let mut last_error = String::new();

                for lease in &leases {
                    let cred = lease.credential().clone();
                    match self.try_with_credential(&request, &cred, &tx).await {
                        Ok(()) => {
                            lease.report_success().await;
                            return Ok(());
                        }
                        Err((status, msg)) => {
                            lease.report_failure(status).await;
                            last_status = status;
                            last_error = msg;

                            // 401 on OAuth → try refreshing before moving to next credential
                            if status == 401 && cred.is_oauth() && self.credential_manager.is_some() {
                                info!("Got 401 on '{}', attempting token refresh", lease.account());
                                if let Ok(refreshed) = self.force_refresh_credential().await {
                                    match self.try_with_credential(&request, &refreshed, &tx).await {
                                        Ok(()) => {
                                            lease.report_success().await;
                                            return Ok(());
                                        }
                                        Err((s, m)) => {
                                            last_status = s;
                                            last_error = m;
                                        }
                                    }
                                }
                            }

                            // Non-retryable errors stop immediately
                            if !clanker_router::retry::is_retryable_status(status) && status != 401 {
                                return Err(crate::error::provider_err_with_status(last_status, last_error));
                            }

                            info!("Credential '{}' failed (HTTP {}), trying next", lease.account(), status);
                        }
                    }
                }

                return Err(crate::error::provider_err_with_status(last_status, last_error));
            }
        }

        // ── Single-credential path ───────────────────────────────────
        let credential = self.get_credential().await?;
        let api_request = api::build_api_request(&request, credential.is_oauth());
        let response = self.client.send_streaming(&api_request, &credential).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // On 401, try refreshing the token and retrying once
            if status.as_u16() == 401 && self.credential_manager.is_some() {
                info!("Got 401, attempting token refresh and retry");
                let refreshed = self.force_refresh_credential().await?;
                let api_request = api::build_api_request(&request, refreshed.is_oauth());
                let retry_response = self.client.send_streaming(&api_request, &refreshed).await?;

                if !retry_response.status().is_success() {
                    let retry_status = retry_response.status();
                    let retry_body = retry_response.text().await.unwrap_or_default();
                    return Err(crate::error::provider_err_with_status(
                        retry_status.as_u16(),
                        format!("Anthropic API error {} (after token refresh): {}", retry_status, retry_body),
                    ));
                }

                return streaming::parse_sse_stream(retry_response, tx).await;
            }

            return Err(crate::error::provider_err_with_status(
                status.as_u16(),
                format!("Anthropic API error {}: {}", status, body),
            ));
        }

        streaming::parse_sse_stream(response, tx).await
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    async fn reload_credentials(&self) {
        if let Some(ref cm) = self.credential_manager {
            cm.reload_from_disk().await;
        }
        // Reset pool health after credential reload (fresh tokens)
        if let Some(ref pool) = self.credential_pool {
            pool.reset_health().await;
        }
    }
}
