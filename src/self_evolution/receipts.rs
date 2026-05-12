//! Receipt, option, and data models for self-evolution orchestration.

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct SelfEvolutionRunOptions {
    pub target: PathBuf,
    pub baseline_command: String,
    pub candidate_output: PathBuf,
    pub session_id: Option<String>,
    pub dry_run: bool,
    pub candidate_body: Option<String>,
    pub simulate_eval_failure: bool,
    pub production_profile: String,
    pub corpus_manifest: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelfEvolutionRunReceipt {
    pub source: String,
    pub run_id: String,
    pub status: String,
    pub dry_run: bool,
    pub target: ArtifactIdentity,
    pub baseline: EvaluationRecord,
    pub candidate: CandidateRecord,
    pub mcp_receipts: Vec<McpOrchestrationReceipt>,
    pub recommendation: PromotionRecommendation,
    pub readiness: SelfEvolutionReadinessReport,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SelfEvolutionApprovalOptions {
    pub receipt_path: PathBuf,
    pub session_id: String,
    pub confirmation_id: String,
    pub approver: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelfEvolutionApprovalReceipt {
    pub source: String,
    pub run_id: String,
    pub status: String,
    pub dry_run: bool,
    pub approver: String,
    pub confirmation_id: String,
    pub target_path: String,
    pub candidate_path: String,
    pub approval: PromotionApprovalRecord,
    pub mcp_receipts: Vec<McpOrchestrationReceipt>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SelfEvolutionApplicationOptions {
    pub receipt_path: PathBuf,
    pub approval_path: PathBuf,
    pub apply_mode: String,
    pub verification_command: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelfEvolutionApplicationReceipt {
    pub source: String,
    pub run_id: String,
    pub status: String,
    pub dry_run: bool,
    pub apply_mode: String,
    pub run_receipt_path: String,
    pub approval_receipt_path: String,
    pub target_path: String,
    pub candidate_path: String,
    pub pre_apply_sha256: String,
    pub candidate_sha256: String,
    pub post_apply_sha256: Option<String>,
    pub planned_backup_path: String,
    pub backup_sha256: Option<String>,
    pub verification: ApplicationVerificationRecord,
    pub applied: bool,
    pub rollback: ApplicationRollbackRecord,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SelfEvolutionRollbackOptions {
    pub application_path: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelfEvolutionRollbackReceipt {
    pub source: String,
    pub run_id: String,
    pub status: String,
    pub dry_run: bool,
    pub application_receipt_path: String,
    pub target_path: String,
    pub backup_path: String,
    pub pre_rollback_sha256: String,
    pub backup_sha256: String,
    pub post_rollback_sha256: Option<String>,
    pub restored: bool,
    pub evidence: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationVerificationRecord {
    pub command: String,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationRollbackRecord {
    pub backup_path: String,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromotionApprovalRecord {
    pub approved: bool,
    pub human_approval_required: bool,
    pub applied: bool,
    pub promotion_status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactIdentity {
    pub path: String,
    pub exists: bool,
    pub kind: String,
    pub sha256: Option<String>,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationRecord {
    pub command: String,
    pub score: f64,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateRecord {
    pub output_dir: String,
    pub artifact_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub changed_from_baseline: bool,
    pub score: f64,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpOrchestrationReceipt {
    pub source: String,
    pub session_id: Option<String>,
    pub tool: String,
    pub status: String,
    pub submitted: bool,
    pub request_summary: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromotionRecommendation {
    pub recommended: bool,
    pub reason: String,
    pub human_approval_required: bool,
    pub promotion_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalCorpusManifest {
    pub version: u32,
    pub targets: Vec<String>,
    pub cases: Vec<EvalCorpusCase>,
    pub redaction_policy: String,
    pub min_improvement: f64,
    pub regression_budget: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalCorpusCase {
    pub id: String,
    pub objective: String,
    pub oracle_command: String,
    pub expected_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelfEvolutionReadinessReport {
    pub label: String,
    pub profile: String,
    pub corpus_manifest_path: Option<String>,
    pub corpus_cases: usize,
    pub threshold_passed: bool,
    pub regression_budget_passed: bool,
    pub unchanged_candidate_control_passed: bool,
    pub daemon_session_observable: bool,
    pub evidence_refs: Vec<String>,
    pub reasons: Vec<String>,
    pub known_limitations: Vec<String>,
}
