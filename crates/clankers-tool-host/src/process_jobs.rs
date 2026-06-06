//! Backend-neutral process/job contracts shared by tool adapters.
//!
//! This module owns pure process-job request/decision DTOs that do not depend
//! on the Clankers runtime facade, daemon actors, TUI state, procmon handles,
//! filesystem storage, or backend command execution.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

pub const PROCESS_JOB_PROFILE_METADATA_NAME: &str = "profile";
pub const PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION: &str = "identity.profile.schema_version";
pub const PROCESS_JOB_PROFILE_METADATA_SOURCE: &str = "identity.profile.source";
pub const PROCESS_JOB_PROFILE_METADATA_POLICY: &str = "identity.profile.policy";

/// Safe, backend-neutral profile metadata copied into process/job receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobProfileReceiptMetadata {
    pub profile_name: String,
    pub manifest_schema_version: u32,
    pub profile_source: String,
    pub policy_source: String,
}

impl ProcessJobProfileReceiptMetadata {
    #[must_use]
    pub fn from_metadata(metadata: &BTreeMap<String, String>) -> Option<Self> {
        let profile_name = metadata.get(PROCESS_JOB_PROFILE_METADATA_NAME)?.clone();
        let manifest_schema_version = metadata.get(PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION)?.parse().ok()?;
        let profile_source = metadata.get(PROCESS_JOB_PROFILE_METADATA_SOURCE)?.clone();
        let policy_source = metadata.get(PROCESS_JOB_PROFILE_METADATA_POLICY)?.clone();
        Some(Self {
            profile_name,
            manifest_schema_version,
            profile_source,
            policy_source,
        })
    }
}

/// Resource limits accepted by policy before backend dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobResourcePolicy {
    pub timeout: Option<Duration>,
    pub memory_max_bytes: Option<u64>,
    pub cpu_quota_percent: Option<u32>,
    pub max_log_bytes: Option<u64>,
}

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

    #[test]
    fn resource_policy_is_plain_backend_neutral_data() {
        let policy = ProcessJobResourcePolicy {
            timeout: Some(Duration::from_secs(30)),
            memory_max_bytes: Some(1024),
            cpu_quota_percent: Some(50),
            max_log_bytes: Some(4096),
        };

        assert_eq!(policy.timeout, Some(Duration::from_secs(30)));
        assert_eq!(policy.memory_max_bytes, Some(1024));
        assert_eq!(policy.cpu_quota_percent, Some(50));
        assert_eq!(policy.max_log_bytes, Some(4096));
    }

    #[test]
    fn profile_receipt_metadata_projects_from_safe_metadata() {
        let mut metadata = BTreeMap::new();
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_NAME.to_string(), "quick-check".to_string());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION.to_string(), "1".to_string());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_SOURCE.to_string(), "inline".to_string());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_POLICY.to_string(), "test-policy".to_string());

        let receipt = ProcessJobProfileReceiptMetadata::from_metadata(&metadata)
            .expect("safe profile receipt metadata should project from metadata");
        assert_eq!(receipt.profile_name, "quick-check");
        assert_eq!(receipt.manifest_schema_version, 1);
        assert_eq!(receipt.profile_source, "inline");
        assert_eq!(receipt.policy_source, "test-policy");
    }
}
