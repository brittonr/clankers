//! Runtime admission helpers that route low-risk built-in effects through UCAN.
//!
//! This module is intentionally small: it proves the first built-in routing seam
//! by wrapping the read-only `read` tool in the host-owned runtime effect gate.

use clankers_runtime::ConfirmationAction;
use clankers_runtime::ConfirmationBroker;
use clankers_runtime::ConfirmationRequest;
use clankers_runtime::EffectAbilityClass;
use clankers_runtime::EffectCorrelationId;
use clankers_runtime::EffectGate;
use clankers_runtime::EffectHandler;
use clankers_runtime::EffectRequest;
use clankers_runtime::EffectResult;
use clankers_runtime::EffectResultStatus;
use clankers_runtime::SideEffectLevel;
use clankers_runtime::ToolDescriptor;
use clankers_runtime::request_confirmation_fail_closed;
use clankers_runtime::run_effect_fail_closed;

use crate::external_adapter::EffectInvocation;
use crate::external_adapter::UcanAuthorizer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UcanAdmissionDecision {
    Allowed,
    Denied { reason: String },
}

pub trait RuntimeUcanAdmission: Send + Sync {
    fn admit(&self, invocation: &EffectInvocation) -> UcanAdmissionDecision;
}

impl<T> RuntimeUcanAdmission for T
where T: UcanAuthorizer + Send + Sync
{
    fn admit(&self, invocation: &EffectInvocation) -> UcanAdmissionDecision {
        let decision = self.authorize(invocation);
        if decision.is_allowed() {
            UcanAdmissionDecision::Allowed
        } else {
            UcanAdmissionDecision::Denied {
                reason: format!("{decision:?}"),
            }
        }
    }
}

pub const READ_TOOL_NAME: &str = "read";
pub const READ_TOOL_DESCRIPTION: &str = "Read files selected by the host";
pub const READ_TOOL_ABILITY: &str = "file/read";
pub const WRITE_TOOL_NAME: &str = "write";
pub const WRITE_TOOL_DESCRIPTION: &str = "Write files in host-approved workspace roots";
pub const WRITE_TOOL_ABILITY: &str = "file/write";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinAdmissionError {
    MalformedInvocation { message: String },
    Confirmation { message: String },
}

impl std::fmt::Display for BuiltinAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedInvocation { message } => write!(formatter, "malformed built-in UCAN invocation: {message}"),
            Self::Confirmation { message } => write!(formatter, "confirmation failed after UCAN admission: {message}"),
        }
    }
}

impl std::error::Error for BuiltinAdmissionError {}

pub type BuiltinAdmissionResult<T> = Result<T, BuiltinAdmissionError>;

pub struct UcanEffectAdmissionHandler<'a> {
    class: EffectAbilityClass,
    invocation: EffectInvocation,
    admission: &'a dyn RuntimeUcanAdmission,
}

impl<'a> UcanEffectAdmissionHandler<'a> {
    #[must_use]
    pub const fn new(
        class: EffectAbilityClass,
        invocation: EffectInvocation,
        admission: &'a dyn RuntimeUcanAdmission,
    ) -> Self {
        Self {
            class,
            invocation,
            admission,
        }
    }
}

impl EffectHandler for UcanEffectAdmissionHandler<'_> {
    fn class(&self) -> EffectAbilityClass {
        self.class
    }

    fn handle(&self, request: &EffectRequest) -> EffectResult {
        if request.class != self.class {
            return EffectResult::new(request, EffectResultStatus::Unavailable, "UCAN handler class mismatch");
        }
        match self.admission.admit(&self.invocation) {
            UcanAdmissionDecision::Allowed => {
                EffectResult::new(request, EffectResultStatus::Allowed, "allowed by UCAN admission")
            }
            UcanAdmissionDecision::Denied { reason } => {
                EffectResult::new(request, EffectResultStatus::Denied, format!("UCAN authorization denied: {reason}"))
            }
        }
    }
}

#[must_use]
pub fn read_tool_descriptor() -> ToolDescriptor {
    ToolDescriptor::new(READ_TOOL_NAME, READ_TOOL_DESCRIPTION, SideEffectLevel::ReadOnly)
}

#[must_use]
pub fn write_tool_descriptor() -> ToolDescriptor {
    ToolDescriptor::new(WRITE_TOOL_NAME, WRITE_TOOL_DESCRIPTION, SideEffectLevel::WorkspaceMutation)
}

pub fn tool_invocation(
    resource_uri: impl Into<String>,
    ability: impl Into<String>,
) -> BuiltinAdmissionResult<EffectInvocation> {
    EffectInvocation::new(resource_uri, ability).map_err(|error| BuiltinAdmissionError::MalformedInvocation {
        message: error.to_string(),
    })
}

pub fn read_tool_invocation(resource_uri: impl Into<String>) -> BuiltinAdmissionResult<EffectInvocation> {
    tool_invocation(resource_uri, READ_TOOL_ABILITY)
}

pub fn read_tool_effect_request(correlation_id: EffectCorrelationId, invocation: &EffectInvocation) -> EffectRequest {
    read_tool_descriptor()
        .effect_request(correlation_id)
        .with_safe_metadata("tool_name", READ_TOOL_NAME)
        .with_safe_metadata("tool_source", "clankers")
        .with_safe_metadata("ucan_resource", invocation.resource())
        .with_safe_metadata("ucan_ability", invocation.ability())
}

pub fn run_read_tool_with_ucan_admission<T>(
    correlation_id: EffectCorrelationId,
    resource_uri: impl Into<String>,
    admission: &dyn RuntimeUcanAdmission,
    operation: impl FnOnce() -> T,
) -> BuiltinAdmissionResult<EffectGate<T>> {
    let invocation = read_tool_invocation(resource_uri)?;
    let request = read_tool_effect_request(correlation_id, &invocation);
    let handler = UcanEffectAdmissionHandler::new(EffectAbilityClass::Filesystem, invocation, admission);
    Ok(run_effect_fail_closed(&request, Some(&handler), operation))
}

pub async fn run_tool_with_ucan_then_confirmation<T>(
    descriptor: &ToolDescriptor,
    correlation_id: EffectCorrelationId,
    invocation: EffectInvocation,
    admission: &dyn RuntimeUcanAdmission,
    confirmation_broker: &dyn ConfirmationBroker,
    confirmation_action: ConfirmationAction,
    operation: impl FnOnce() -> T,
) -> BuiltinAdmissionResult<EffectGate<T>> {
    let request = descriptor
        .effect_request(correlation_id)
        .with_safe_metadata("ucan_resource", invocation.resource())
        .with_safe_metadata("ucan_ability", invocation.ability());
    let handler = UcanEffectAdmissionHandler::new(descriptor.effect_class(), invocation, admission);
    let receipt = if handler.class() == request.class {
        handler.handle(&request)
    } else {
        EffectResult::new(&request, EffectResultStatus::Unavailable, "UCAN handler class mismatch")
    };
    if receipt.status != EffectResultStatus::Allowed {
        return Ok(EffectGate::Blocked { receipt });
    }
    if descriptor.requires_confirmation {
        let confirmation = ConfirmationRequest::new(
            confirmation_action,
            format!("{} requires human confirmation after UCAN admission", descriptor.name),
        );
        let decision = request_confirmation_fail_closed(confirmation_broker, confirmation).await.map_err(|error| {
            BuiltinAdmissionError::Confirmation {
                message: error.safe_message(),
            }
        })?;
        if !decision.approved {
            return Ok(EffectGate::Blocked {
                receipt: EffectResult::new(
                    &request,
                    EffectResultStatus::Denied,
                    format!("human confirmation denied: {}", decision.reason),
                ),
            });
        }
    }
    Ok(EffectGate::Executed {
        value: operation(),
        receipt,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;

    const RESOURCE: &str = "clankers:file:///workspace/project/src/lib.rs";

    struct FixedAdmission {
        decision: UcanAdmissionDecision,
        calls: AtomicUsize,
    }

    impl FixedAdmission {
        fn allow() -> Self {
            Self {
                decision: UcanAdmissionDecision::Allowed,
                calls: AtomicUsize::new(0),
            }
        }

        fn deny() -> Self {
            Self {
                decision: UcanAdmissionDecision::Denied {
                    reason: "fixture denied".to_string(),
                },
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl RuntimeUcanAdmission for FixedAdmission {
        fn admit(&self, _invocation: &EffectInvocation) -> UcanAdmissionDecision {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.decision.clone()
        }
    }

    struct RecordingAdmission {
        decision: UcanAdmissionDecision,
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl RuntimeUcanAdmission for RecordingAdmission {
        fn admit(&self, _invocation: &EffectInvocation) -> UcanAdmissionDecision {
            self.events.lock().expect("events lock").push("ucan");
            self.decision.clone()
        }
    }

    struct RecordingConfirmationBroker {
        approved: bool,
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl ConfirmationBroker for RecordingConfirmationBroker {
        fn decide(&self, _request: ConfirmationRequest) -> clankers_runtime::ConfirmationFuture<'_> {
            let approved = self.approved;
            let events = Arc::clone(&self.events);
            Box::pin(async move {
                events.lock().expect("events lock").push("confirm");
                let decision = if approved {
                    clankers_runtime::ConfirmationDecision::approve("approved by fixture")
                } else {
                    clankers_runtime::ConfirmationDecision::deny("denied by fixture")
                };
                Ok(decision)
            })
        }
    }

    #[test]
    fn read_tool_ucan_allow_executes_after_admission() {
        let authorizer = FixedAdmission::allow();
        let operation_ran = std::sync::atomic::AtomicBool::new(false);

        let gate = run_read_tool_with_ucan_admission(
            EffectCorrelationId::from_static("read-1"),
            RESOURCE,
            &authorizer,
            || {
                operation_ran.store(true, Ordering::SeqCst);
                "file body"
            },
        )
        .expect("admission route");

        assert_eq!(authorizer.calls(), 1);
        assert!(operation_ran.load(Ordering::SeqCst));
        match gate {
            EffectGate::Executed { value, receipt } => {
                assert_eq!(value, "file body");
                assert_eq!(receipt.status, EffectResultStatus::Allowed);
                assert_eq!(receipt.request.class, EffectAbilityClass::Filesystem);
            }
            EffectGate::Blocked { .. } => panic!("UCAN allow should execute read operation"),
        }
    }

    #[test]
    fn read_tool_ucan_denial_blocks_before_handler_execution() {
        let authorizer = FixedAdmission::deny();
        let operation_ran = std::sync::atomic::AtomicBool::new(false);

        let gate = run_read_tool_with_ucan_admission(
            EffectCorrelationId::from_static("read-2"),
            RESOURCE,
            &authorizer,
            || {
                operation_ran.store(true, Ordering::SeqCst);
                "should not run"
            },
        )
        .expect("admission route");

        assert_eq!(authorizer.calls(), 1);
        assert!(!operation_ran.load(Ordering::SeqCst));
        match gate {
            EffectGate::Executed { .. } => panic!("UCAN denial must not execute read operation"),
            EffectGate::Blocked { receipt } => {
                assert_eq!(receipt.status, EffectResultStatus::Denied);
                assert_eq!(receipt.request.class, EffectAbilityClass::Filesystem);
            }
        }
    }

    #[test]
    fn read_tool_request_records_safe_ucan_facts() {
        let invocation = read_tool_invocation(RESOURCE).expect("read invocation");
        let request = read_tool_effect_request(EffectCorrelationId::from_static("read-3"), &invocation);

        assert_eq!(request.class, EffectAbilityClass::Filesystem);
        assert_eq!(request.safe_source_metadata.get("tool_name"), Some(&READ_TOOL_NAME.to_owned()));
        assert_eq!(request.safe_source_metadata.get("ucan_ability"), Some(&READ_TOOL_ABILITY.to_owned()));
        assert_eq!(request.safe_source_metadata.get("ucan_resource"), Some(&RESOURCE.to_owned()));
    }

    #[tokio::test]
    async fn ucan_allow_requests_confirmation_before_mutating_operation() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let admission = RecordingAdmission {
            decision: UcanAdmissionDecision::Allowed,
            events: Arc::clone(&events),
        };
        let confirmation = RecordingConfirmationBroker {
            approved: true,
            events: Arc::clone(&events),
        };
        let descriptor = write_tool_descriptor();
        let invocation = tool_invocation(RESOURCE, WRITE_TOOL_ABILITY).expect("write invocation");

        let gate = run_tool_with_ucan_then_confirmation(
            &descriptor,
            EffectCorrelationId::from_static("write-1"),
            invocation,
            &admission,
            &confirmation,
            ConfirmationAction::MutateWorkspace,
            || {
                events.lock().expect("events lock").push("operation");
                "write applied"
            },
        )
        .await
        .expect("admission route");

        assert_eq!(*events.lock().expect("events lock"), vec!["ucan", "confirm", "operation"]);
        assert!(gate.executed());
    }

    #[tokio::test]
    async fn ucan_denial_skips_confirmation_and_operation() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let admission = RecordingAdmission {
            decision: UcanAdmissionDecision::Denied {
                reason: "no grant".to_string(),
            },
            events: Arc::clone(&events),
        };
        let confirmation = RecordingConfirmationBroker {
            approved: true,
            events: Arc::clone(&events),
        };
        let descriptor = write_tool_descriptor();
        let invocation = tool_invocation(RESOURCE, WRITE_TOOL_ABILITY).expect("write invocation");

        let gate = run_tool_with_ucan_then_confirmation(
            &descriptor,
            EffectCorrelationId::from_static("write-2"),
            invocation,
            &admission,
            &confirmation,
            ConfirmationAction::MutateWorkspace,
            || {
                events.lock().expect("events lock").push("operation");
                "write applied"
            },
        )
        .await
        .expect("admission route");

        assert_eq!(*events.lock().expect("events lock"), vec!["ucan"]);
        assert!(!gate.executed());
        assert_eq!(gate.receipt().status, EffectResultStatus::Denied);
    }

    #[tokio::test]
    async fn confirmation_denial_blocks_after_ucan_allow_before_operation() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let admission = RecordingAdmission {
            decision: UcanAdmissionDecision::Allowed,
            events: Arc::clone(&events),
        };
        let confirmation = RecordingConfirmationBroker {
            approved: false,
            events: Arc::clone(&events),
        };
        let descriptor = write_tool_descriptor();
        let invocation = tool_invocation(RESOURCE, WRITE_TOOL_ABILITY).expect("write invocation");

        let gate = run_tool_with_ucan_then_confirmation(
            &descriptor,
            EffectCorrelationId::from_static("write-3"),
            invocation,
            &admission,
            &confirmation,
            ConfirmationAction::MutateWorkspace,
            || {
                events.lock().expect("events lock").push("operation");
                "write applied"
            },
        )
        .await
        .expect("admission route");

        assert_eq!(*events.lock().expect("events lock"), vec!["ucan", "confirm"]);
        assert!(!gate.executed());
        assert_eq!(gate.receipt().status, EffectResultStatus::Denied);
        assert!(gate.receipt().safe_summary.contains("human confirmation denied"));
    }
}
