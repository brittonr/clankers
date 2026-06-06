//! Backend-neutral process/job contracts shared by tool adapters.
//!
//! This module owns pure process-job request/decision DTOs that do not depend
//! on the Clankers runtime facade, daemon actors, TUI state, procmon handles,
//! filesystem storage, or backend command execution.

/// Backend-neutral native-process admission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessJobNativeAdmissionDecision {
    pub accepted: bool,
    pub active: usize,
    pub limit: usize,
}

/// Named input for native-process admission checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessJobNativeAdmissionInput {
    pub active: usize,
    pub limit: usize,
}

impl ProcessJobNativeAdmissionDecision {
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "native process admission denied: active process limit reached ({active}/{limit})",
            active = self.active,
            limit = self.limit,
        )
    }
}

#[must_use]
pub fn native_process_job_admission_decision(
    input: ProcessJobNativeAdmissionInput,
) -> ProcessJobNativeAdmissionDecision {
    ProcessJobNativeAdmissionDecision {
        accepted: input.active < input.limit,
        active: input.active,
        limit: input.limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_admission_accepts_below_limit_and_denies_at_limit() {
        let accepted = native_process_job_admission_decision(ProcessJobNativeAdmissionInput { active: 1, limit: 2 });
        assert!(accepted.accepted);
        assert_eq!(accepted.active, 1);
        assert_eq!(accepted.limit, 2);

        let denied = native_process_job_admission_decision(ProcessJobNativeAdmissionInput { active: 2, limit: 2 });
        assert!(!denied.accepted);
        assert_eq!(denied.summary(), "native process admission denied: active process limit reached (2/2)");
    }
}
