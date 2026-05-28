use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use clankers_runtime::ConfirmationAction;
use clankers_runtime::ConfirmationBroker;
use clankers_runtime::ConfirmationDecision;
use clankers_runtime::ConfirmationFuture;
use clankers_runtime::ConfirmationRequest;
use clankers_runtime::RuntimeBuilder;
use clankers_runtime::RuntimeError;
use serde_json::json;

struct StaticBroker(ConfirmationDecision);

impl ConfirmationBroker for StaticBroker {
    fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
        let decision = self.0.clone();
        Box::pin(async move { Ok(decision) })
    }
}

struct UnavailableBroker;

impl ConfirmationBroker for UnavailableBroker {
    fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
        Box::pin(async move { Err(RuntimeError::ConfirmationUnavailable("host broker offline".to_string())) })
    }
}

fn request_for(summary: &str) -> ConfirmationRequest {
    ConfirmationRequest::new(ConfirmationAction::MutateWorkspace, summary)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), RuntimeError> {
    let approved_runtime = RuntimeBuilder::new()
        .confirmation_broker(Arc::new(StaticBroker(ConfirmationDecision::approve("approved by host fixture"))))
        .build()?;
    let approved_executed = Arc::new(AtomicBool::new(false));
    let approved_flag = Arc::clone(&approved_executed);
    let approved_result = approved_runtime
        .run_confirmed_action(request_for("write generated workspace fixture"), || {
            approved_flag.store(true, Ordering::SeqCst);
            Ok("mutation-result")
        })
        .await?;
    assert_eq!(approved_result, "mutation-result");
    assert!(approved_executed.load(Ordering::SeqCst));

    let denied_runtime = RuntimeBuilder::new()
        .confirmation_broker(Arc::new(StaticBroker(ConfirmationDecision::deny("denied by host fixture"))))
        .build()?;
    let denied_executed = Arc::new(AtomicBool::new(false));
    let denied_flag = Arc::clone(&denied_executed);
    let denied_error = denied_runtime
        .run_confirmed_action(request_for("write denied workspace fixture"), || {
            denied_flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect_err("denied confirmation must block the action");
    assert_eq!(denied_error, RuntimeError::ConfirmationDenied("denied by host fixture".to_string()));
    assert!(!denied_executed.load(Ordering::SeqCst));

    let default_runtime = RuntimeBuilder::new().build()?;
    let default_decision = default_runtime.request_confirmation(request_for("default broker must deny")).await?;
    assert!(!default_decision.approved);
    assert_eq!(default_decision.reason, "confirmation broker unavailable");

    let unavailable_runtime = RuntimeBuilder::new().confirmation_broker(Arc::new(UnavailableBroker)).build()?;
    let unavailable_decision =
        unavailable_runtime.request_confirmation(request_for("offline broker must deny")).await?;
    assert!(!unavailable_decision.approved);
    assert_eq!(unavailable_decision.reason, "host broker offline");

    let redacted_request = ConfirmationRequest::new(
        ConfirmationAction::Custom("deploy".to_string()),
        "approve with Authorization: Bearer secret-token",
    );
    assert_eq!(redacted_request.summary, "[REDACTED]");

    let receipt = json!({
        "brick": "confirmation-broker-kit",
        "positive_path": {
            "approved": true,
            "action_executed": approved_executed.load(Ordering::SeqCst),
            "decision_reason": "approved by host fixture",
        },
        "negative_path": {
            "denied": true,
            "action_executed": denied_executed.load(Ordering::SeqCst),
            "error_class": format!("{:?}", denied_error.class()),
        },
        "fail_closed": {
            "default_reason": default_decision.reason,
            "unavailable_reason": unavailable_decision.reason,
        },
        "redaction": {
            "secret_summary": redacted_request.summary,
        },
        "boundaries": [
            "host owns UI/approval source",
            "runtime owns typed request and fail-closed substrate",
            "no daemon, TUI, provider, plugin, Matrix, iroh, or global singleton dependency",
        ],
    });
    let receipt_json = serde_json::to_string(&receipt).expect("receipt serializes");
    let receipt_hash = blake3::hash(receipt_json.as_bytes()).to_hex().to_string();

    println!("confirmation-broker-kit receipt_hash={receipt_hash}");
    println!("confirmation-broker-kit approved_action_executed=true");
    println!("confirmation-broker-kit denied_action_executed=false");
    println!("confirmation-broker-kit fail_closed=2");
    println!("confirmation-broker-kit redacted_summary={}", redacted_request.summary);
    println!("confirmation-broker-kit passed");

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn confirmation_broker_kit_smoke() {
        super::main().expect("confirmation broker example passes");
    }
}
