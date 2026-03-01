//! Anthropic Messages API provider

pub mod api;
pub mod oauth;
pub mod streaming;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::provider::Model;
use crate::provider::Provider;
use crate::provider::auth::Credential;
use crate::provider::credential_manager::CredentialManager;
use crate::provider::streaming::StreamEvent;

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
            models: default_models(),
        }
    }

    /// Create a provider with a credential manager that supports auto-refresh.
    pub fn with_credential_manager(credential_manager: Arc<CredentialManager>, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: None,
            credential_manager: Some(credential_manager),
            models: default_models(),
        }
    }

    /// Get the current credential, refreshing if needed.
    async fn get_credential(&self) -> Result<Credential> {
        if let Some(ref cm) = self.credential_manager {
            cm.get_credential().await
        } else if let Some(ref cred) = self.credential {
            Ok(cred.clone())
        } else {
            Err(crate::error::Error::ProviderAuth {
                message: "No credential configured".to_string(),
            })
        }
    }

    /// Force-refresh the credential (called on 401).
    async fn force_refresh_credential(&self) -> Result<Credential> {
        if let Some(ref cm) = self.credential_manager {
            cm.force_refresh().await
        } else {
            Err(crate::error::Error::ProviderAuth {
                message: "Cannot refresh: no credential manager configured".to_string(),
            })
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
                    return Err(crate::error::Error::Provider {
                        message: format!("Anthropic API error {} (after token refresh): {}", retry_status, retry_body),
                    });
                }

                return streaming::parse_sse_stream(retry_response, tx).await;
            }

            return Err(crate::error::Error::Provider {
                message: format!("Anthropic API error {}: {}", status, body),
            });
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

fn default_models() -> Vec<Model> {
    vec![
        Model {
            id: "claude-sonnet-4-5-20250514".to_string(),
            name: "Claude Sonnet 4.5".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "claude-opus-4-20250514".to_string(),
            name: "Claude Opus 4".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 32_768,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(15.0),
            output_cost_per_mtok: Some(75.0),
        },
        Model {
            id: "claude-haiku-4-5-20250514".to_string(),
            name: "Claude Haiku 4.5".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.8),
            output_cost_per_mtok: Some(4.0),
        },
    ]
}
