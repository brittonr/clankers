//! Host-facing confirmation request and broker types.

use std::future::Future;
use std::pin::Pin;

pub use clanker_message::ConfirmationAction;
pub use clanker_message::ConfirmationDecision;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::EventMetadata;
use crate::RuntimeError;
use crate::events::sanitize_metadata_value;

/// Confirmation request passed to a host broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    pub id: String,
    pub action: ConfirmationAction,
    pub summary: String,
    pub metadata: EventMetadata,
    pub timeout_ms: Option<u64>,
}

impl ConfirmationRequest {
    #[must_use]
    pub fn new(action: ConfirmationAction, summary: impl Into<String>) -> Self {
        Self {
            id: format!("confirm_{}", Uuid::new_v4()),
            action,
            summary: sanitize_metadata_value(summary.into()),
            metadata: EventMetadata::empty(),
            timeout_ms: None,
        }
    }
}

pub type ConfirmationFuture<'a> = Pin<Box<dyn Future<Output = Result<ConfirmationDecision, RuntimeError>> + Send + 'a>>;

pub trait ConfirmationBroker: Send + Sync + 'static {
    fn decide(&self, request: ConfirmationRequest) -> ConfirmationFuture<'_>;
}

pub struct FailClosedConfirmationBroker;

impl ConfirmationBroker for FailClosedConfirmationBroker {
    fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
        Box::pin(async { Ok(ConfirmationDecision::deny("confirmation broker unavailable")) })
    }
}

pub async fn request_confirmation_fail_closed(
    broker: &dyn ConfirmationBroker,
    request: ConfirmationRequest,
) -> Result<ConfirmationDecision, RuntimeError> {
    match broker.decide(request).await {
        Ok(decision) => Ok(decision),
        Err(RuntimeError::ConfirmationUnavailable(reason)) => Ok(ConfirmationDecision::deny(reason)),
        Err(RuntimeError::ConfirmationTimedOut) => Ok(ConfirmationDecision::deny("confirmation timed out")),
        Err(RuntimeError::ConfirmationCancelled) => Ok(ConfirmationDecision::deny("confirmation cancelled")),
        Err(error) => Err(error),
    }
}
