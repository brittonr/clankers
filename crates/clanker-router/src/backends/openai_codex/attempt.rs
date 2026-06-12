use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use super::build_codex_request;
use super::codex_sleep;
use super::common;
use super::map_codex_error;
use super::parse_codex_sse;
use crate::auth::StoredCredential;
use crate::credential::CredentialManager;
use crate::error::Error;
use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::retry::RetryConfig;
use crate::retry::is_retryable_status;
use crate::streaming::StreamEvent;

pub(crate) struct OpenAICodexAttempt {
    request: CompletionRequest,
    tx: mpsc::Sender<StreamEvent>,
    credential: StoredCredential,
    credential_manager: Arc<CredentialManager>,
    retry: RetryConfig,
}

impl OpenAICodexAttempt {
    pub(crate) fn new(
        request: CompletionRequest,
        tx: mpsc::Sender<StreamEvent>,
        credential: StoredCredential,
        credential_manager: Arc<CredentialManager>,
    ) -> Self {
        Self {
            request,
            tx,
            credential,
            credential_manager,
            retry: RetryConfig::deterministic(),
        }
    }

    pub(crate) async fn run(&mut self) -> Result<()> {
        let mut transient_attempt = 0;
        let mut is_refresh_done = false;

        loop {
            let response = self.send_request().await?;
            let status = response.status().as_u16();
            if response.status().is_success() {
                return parse_codex_sse(response, &self.request.model, self.tx.clone()).await;
            }

            let body_text = response.text().await.unwrap_or_default();
            if status == 401 && !is_refresh_done {
                match self.credential_manager.force_refresh().await {
                    Ok(refreshed) => {
                        self.credential = refreshed;
                        is_refresh_done = true;
                        continue;
                    }
                    Err(e) => {
                        return Err(Error::Auth {
                            message: format!("OpenAI Codex token refresh failed: {e}"),
                        });
                    }
                }
            }

            if is_retryable_status(status) && transient_attempt < self.retry.max_retries {
                codex_sleep(self.retry.backoff_for(transient_attempt)).await;
                transient_attempt += 1;
                continue;
            }

            return Err(map_codex_error(status, &body_text));
        }
    }

    async fn send_request(&self) -> Result<reqwest::Response> {
        let client = common::build_http_client(Duration::from_secs(600))?;
        let request = build_codex_request(&client, &self.credential, &self.request)?;
        client.execute(request).await.map_err(Into::into)
    }
}
