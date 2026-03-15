//! Anthropic Messages API provider

pub mod api;
pub mod streaming;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

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
    models: Vec<Model>,
}

impl AnthropicProvider {
    /// Create a provider with a simple credential (no auto-refresh).
    pub fn new(credential: Credential, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: Some(credential),
            credential_manager: None,
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Create a provider with a credential manager that supports auto-refresh.
    pub fn with_credential_manager(credential_manager: Arc<CredentialManager>, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: None,
            credential_manager: Some(credential_manager),
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
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // Get credential (proactively refreshing if expired)
        let credential = self.get_credential().await?;
        let api_request = api::build_api_request(&request, credential.is_oauth());
        let response = self.client.send_streaming(&api_request, &credential).await?;

        // Check for HTTP errors before parsing SSE
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
                    return Err(crate::error::provider_err(format!(
                        "Anthropic API error {} (after token refresh): {}",
                        retry_status, retry_body
                    )));
                }

                return streaming::parse_sse_stream(retry_response, tx).await;
            }

            return Err(crate::error::provider_err(format!("Anthropic API error {}: {}", status, body)));
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
    }
}
