//! Disabled-by-default self-evolution dry-run model.
//!
//! This module intentionally models self-evolution as an offline orchestration
//! run. The first implementation writes only run-scoped artifacts, uses a fake
//! MCP/session-control executor for deterministic tests, and never promotes or
//! mutates active artifacts.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use chrono::SecondsFormat;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SelfEvolutionRunOptions {
    pub target: PathBuf,
    pub baseline_command: String,
    pub candidate_output: PathBuf,
    pub session_id: Option<String>,
    pub dry_run: bool,
    pub candidate_body: Option<String>,
    pub simulate_eval_failure: bool,
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

pub trait SelfEvolutionExecutor {
    fn submit_tool(&mut self, session_id: Option<&str>, tool: &str, arguments: Value) -> McpOrchestrationReceipt;
}

#[derive(Debug, Default)]
pub struct FakeMcpExecutor {
    pub calls: Vec<McpOrchestrationReceipt>,
}

impl SelfEvolutionExecutor for FakeMcpExecutor {
    fn submit_tool(&mut self, session_id: Option<&str>, tool: &str, arguments: Value) -> McpOrchestrationReceipt {
        let receipt = McpOrchestrationReceipt {
            source: "mcp_session_control_fake_executor".to_string(),
            session_id: session_id.map(ToString::to_string),
            tool: tool.to_string(),
            status: "submitted_to_fake_session_control".to_string(),
            submitted: true,
            request_summary: summarize_mcp_arguments(tool, &arguments),
        };
        self.calls.push(receipt.clone());
        receipt
    }
}

pub fn run_self_evolution_dry_run(
    options: &SelfEvolutionRunOptions,
    executor: &mut impl SelfEvolutionExecutor,
) -> std::result::Result<SelfEvolutionRunReceipt, String> {
    validate_run_options(options)?;

    let run_id = format!("self-evolution-{}", Uuid::new_v4());
    let output_dir = options.candidate_output.join(&run_id);
    fs::create_dir_all(&output_dir).map_err(|err| format!("failed to create candidate output dir: {err}"))?;

    let baseline_body = read_target_body(&options.target)?;
    let target = artifact_identity(&options.target, baseline_body.as_deref())?;
    let candidate_body = options.candidate_body.clone().unwrap_or_else(|| baseline_body.clone().unwrap_or_default());
    let candidate_path = output_dir.join("candidate.txt");
    fs::write(&candidate_path, &candidate_body).map_err(|err| format!("failed to write isolated candidate: {err}"))?;

    let candidate_hash = sha256_hex(candidate_body.as_bytes());
    let baseline_hash = baseline_body.as_deref().map(|body| sha256_hex(body.as_bytes()));
    let changed = baseline_hash.as_deref() != Some(candidate_hash.as_str());
    let eval = deterministic_evaluation(changed, options.simulate_eval_failure);

    let prompt_receipt = executor.submit_tool(
        options.session_id.as_deref(),
        "send_prompt",
        json!({
            "prompt_len": self_evolution_prompt_len(&target, &options.baseline_command),
            "purpose": "self_evolution_candidate_review",
        }),
    );
    let history_receipt = executor.submit_tool(
        options.session_id.as_deref(),
        "session_history",
        json!({ "purpose": "self_evolution_receipt_evidence" }),
    );

    let recommendation = if eval.failed {
        PromotionRecommendation {
            recommended: false,
            reason: "baseline-vs-candidate evaluation failed; candidate is not eligible for promotion".to_string(),
            human_approval_required: true,
            promotion_status: "not_promoted_eval_failed".to_string(),
        }
    } else if !changed {
        PromotionRecommendation {
            recommended: false,
            reason: "candidate artifact is unchanged from baseline; score deltas would be treated as evaluation noise"
                .to_string(),
            human_approval_required: true,
            promotion_status: "not_promoted".to_string(),
        }
    } else {
        PromotionRecommendation {
            recommended: true,
            reason: "candidate changed and deterministic fake score improved; human approval is still required before promotion".to_string(),
            human_approval_required: true,
            promotion_status: "awaiting_human_approval".to_string(),
        }
    };

    let candidate = CandidateRecord {
        output_dir: output_dir.display().to_string(),
        artifact_path: candidate_path.display().to_string(),
        sha256: candidate_hash,
        bytes: candidate_body.len() as u64,
        changed_from_baseline: changed,
        score: eval.candidate_score,
        status: eval.candidate_status.clone(),
        evidence: candidate_evidence(eval.failed),
    };

    let baseline = EvaluationRecord {
        command: options.baseline_command.clone(),
        score: eval.baseline_score,
        status: eval.baseline_status,
        evidence: baseline_evidence(eval.failed),
    };

    let receipt_status = if eval.failed {
        "completed_with_failed_evaluation"
    } else {
        "completed"
    };

    let receipt = SelfEvolutionRunReceipt {
        source: "self_evolution_dry_run".to_string(),
        run_id,
        status: receipt_status.to_string(),
        dry_run: true,
        target,
        baseline,
        candidate,
        mcp_receipts: vec![prompt_receipt, history_receipt],
        recommendation,
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };

    let receipt_path = output_dir.join("receipt.json");
    let receipt_json = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(&receipt_path, receipt_json).map_err(|err| format!("failed to write receipt: {err}"))?;

    Ok(receipt)
}

#[derive(Debug, Clone)]
struct DeterministicEvaluation {
    baseline_score: f64,
    candidate_score: f64,
    baseline_status: String,
    candidate_status: String,
    failed: bool,
}

fn deterministic_evaluation(changed: bool, simulate_eval_failure: bool) -> DeterministicEvaluation {
    if simulate_eval_failure {
        return DeterministicEvaluation {
            baseline_score: 0.0,
            candidate_score: 0.0,
            baseline_status: "failed_fake_eval".to_string(),
            candidate_status: "failed_fake_eval".to_string(),
            failed: true,
        };
    }

    DeterministicEvaluation {
        baseline_score: 1.0,
        candidate_score: if changed { 1.1 } else { 1.0 },
        baseline_status: "recorded_not_executed_fake_eval".to_string(),
        candidate_status: "isolated_candidate_written".to_string(),
        failed: false,
    }
}

fn baseline_evidence(failed: bool) -> Vec<String> {
    if failed {
        return vec![
            "baseline command recorded for deterministic dry-run evaluation".to_string(),
            "fake evaluation was configured to fail; no candidate may be promoted from this run".to_string(),
        ];
    }

    vec!["baseline command recorded for deterministic dry-run evaluation".to_string()]
}

fn candidate_evidence(failed: bool) -> Vec<String> {
    if failed {
        return vec![
            "candidate was written under the run-scoped output directory".to_string(),
            "active target artifact was not overwritten".to_string(),
            "candidate evaluation failed in the deterministic fake evaluator".to_string(),
        ];
    }

    vec![
        "candidate was written under the run-scoped output directory".to_string(),
        "active target artifact was not overwritten".to_string(),
    ]
}

pub fn approve_self_evolution_promotion(
    options: &SelfEvolutionApprovalOptions,
    executor: &mut impl SelfEvolutionExecutor,
) -> std::result::Result<SelfEvolutionApprovalReceipt, String> {
    validate_approval_options(options)?;
    let receipt_body = fs::read_to_string(&options.receipt_path)
        .map_err(|err| format!("failed to read self-evolution receipt: {err}"))?;
    let run_receipt: SelfEvolutionRunReceipt =
        serde_json::from_str(&receipt_body).map_err(|err| format!("failed to parse self-evolution receipt: {err}"))?;
    validate_promotable_receipt(&run_receipt)?;
    let candidate_path = Path::new(&run_receipt.candidate.artifact_path);
    if !candidate_path.exists() {
        return Err("candidate artifact from receipt does not exist; promotion approval cannot be recorded".to_string());
    }

    let approval_receipt = executor.submit_tool(
        Some(&options.session_id),
        "approve_confirmation",
        json!({
            "confirmation_id": options.confirmation_id,
            "purpose": "self_evolution_promotion_approval",
        }),
    );
    let history_receipt = executor.submit_tool(
        Some(&options.session_id),
        "session_history",
        json!({ "purpose": "self_evolution_approval_evidence" }),
    );

    let approval = PromotionApprovalRecord {
        approved: true,
        human_approval_required: true,
        applied: false,
        promotion_status: "approval_recorded_not_applied".to_string(),
        evidence: vec![
            "human approval was recorded through the session-control confirmation path".to_string(),
            "candidate was not installed, merged, or copied over the active target by this approval step".to_string(),
        ],
    };
    let receipt = SelfEvolutionApprovalReceipt {
        source: "self_evolution_promotion_gate".to_string(),
        run_id: run_receipt.run_id,
        status: "approval_recorded".to_string(),
        dry_run: true,
        approver: options.approver.clone(),
        confirmation_id: options.confirmation_id.clone(),
        target_path: run_receipt.target.path,
        candidate_path: run_receipt.candidate.artifact_path,
        approval,
        mcp_receipts: vec![approval_receipt, history_receipt],
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };

    let approval_path = approval_receipt_path(&options.receipt_path);
    let approval_json = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(&approval_path, approval_json).map_err(|err| format!("failed to write approval receipt: {err}"))?;
    Ok(receipt)
}

pub fn preflight_self_evolution_application(
    options: &SelfEvolutionApplicationOptions,
) -> std::result::Result<SelfEvolutionApplicationReceipt, String> {
    let dry_run_options = SelfEvolutionApplicationOptions {
        dry_run: true,
        ..options.clone()
    };
    validate_application_options(&dry_run_options)?;
    let plan = validate_application_receipt_chain(&dry_run_options)?;
    Ok(build_dry_run_application_receipt(&dry_run_options, plan))
}

pub fn apply_self_evolution_candidate(
    options: &SelfEvolutionApplicationOptions,
) -> std::result::Result<SelfEvolutionApplicationReceipt, String> {
    validate_application_options(options)?;
    let plan = validate_application_receipt_chain(options)?;
    if options.dry_run {
        return Ok(build_dry_run_application_receipt(options, plan));
    }

    let target_path = Path::new(&plan.run_receipt.target.path);
    let candidate_path = Path::new(&plan.run_receipt.candidate.artifact_path);
    fs::create_dir_all(
        plan.planned_backup_path
            .parent()
            .ok_or_else(|| "planned backup path has no parent directory".to_string())?,
    )
    .map_err(|err| format!("failed to create application backup directory: {err}"))?;
    fs::copy(target_path, &plan.planned_backup_path)
        .map_err(|err| format!("failed to write application backup: {err}"))?;
    fs::copy(candidate_path, target_path).map_err(|err| format!("failed to replace target artifact: {err}"))?;

    let post_apply_bytes =
        fs::read(target_path).map_err(|err| format!("failed to read target after application: {err}"))?;
    let post_apply_sha256 = sha256_hex(&post_apply_bytes);
    let verification = run_application_verification(&options.verification_command);
    let status = if verification.status == "passed" {
        "applied"
    } else {
        "applied_verification_failed"
    };
    let receipt = SelfEvolutionApplicationReceipt {
        source: "self_evolution_candidate_application".to_string(),
        run_id: plan.run_receipt.run_id,
        status: status.to_string(),
        dry_run: false,
        apply_mode: options.apply_mode.clone(),
        run_receipt_path: options.receipt_path.display().to_string(),
        approval_receipt_path: options.approval_path.display().to_string(),
        target_path: target_path.display().to_string(),
        candidate_path: candidate_path.display().to_string(),
        pre_apply_sha256: plan.current_target_sha256.clone(),
        candidate_sha256: plan.candidate_sha256,
        post_apply_sha256: Some(post_apply_sha256),
        planned_backup_path: plan.planned_backup_path.display().to_string(),
        backup_sha256: Some(plan.current_target_sha256),
        verification,
        applied: true,
        rollback: ApplicationRollbackRecord {
            backup_path: plan.planned_backup_path.display().to_string(),
            instructions: vec![
                "review application.json before further promotion".to_string(),
                "restore prior bytes by copying the backup path over the target path".to_string(),
            ],
        },
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    let application_path = application_receipt_path(&options.receipt_path);
    let application_json = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(&application_path, application_json)
        .map_err(|err| format!("failed to write application receipt: {err}"))?;
    Ok(receipt)
}

pub fn rollback_self_evolution_application(
    options: &SelfEvolutionRollbackOptions,
) -> std::result::Result<SelfEvolutionRollbackReceipt, String> {
    let application = read_application_receipt(&options.application_path)?;
    let target_path = Path::new(&application.target_path);
    let backup_path = Path::new(&application.rollback.backup_path);
    if application.source != "self_evolution_candidate_application" {
        return Err("application receipt has an unexpected source".to_string());
    }
    if !application.applied {
        return Err("application receipt was not applied; nothing to roll back".to_string());
    }
    if !target_path.is_file() {
        return Err("rollback requires an existing target file".to_string());
    }
    if !backup_path.is_file() {
        return Err("rollback backup file from application receipt does not exist".to_string());
    }

    let current_target_bytes = fs::read(target_path).map_err(|err| format!("failed to read rollback target: {err}"))?;
    let pre_rollback_sha256 = sha256_hex(&current_target_bytes);
    let expected_current = application
        .post_apply_sha256
        .as_deref()
        .ok_or_else(|| "application receipt does not contain a post-apply target hash".to_string())?;
    if pre_rollback_sha256 != expected_current {
        return Err(format!(
            "target artifact changed since application; expected {expected_current}, found {pre_rollback_sha256}"
        ));
    }

    let backup_bytes = fs::read(backup_path).map_err(|err| format!("failed to read rollback backup: {err}"))?;
    let backup_sha256 = sha256_hex(&backup_bytes);
    let expected_backup = application
        .backup_sha256
        .as_deref()
        .ok_or_else(|| "application receipt does not contain a backup hash".to_string())?;
    if backup_sha256 != expected_backup {
        return Err(format!(
            "rollback backup hash does not match application receipt; expected {expected_backup}, found {backup_sha256}"
        ));
    }

    if options.dry_run {
        return Ok(SelfEvolutionRollbackReceipt {
            source: "self_evolution_application_rollback".to_string(),
            run_id: application.run_id,
            status: "rollback_preflight_validated".to_string(),
            dry_run: true,
            application_receipt_path: options.application_path.display().to_string(),
            target_path: target_path.display().to_string(),
            backup_path: backup_path.display().to_string(),
            pre_rollback_sha256,
            backup_sha256,
            post_rollback_sha256: Some(expected_backup.to_string()),
            restored: false,
            evidence: vec![
                "rollback request validated without mutating the target".to_string(),
                "target and backup hashes match application receipt guards".to_string(),
            ],
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        });
    }

    fs::copy(backup_path, target_path).map_err(|err| format!("failed to restore rollback backup: {err}"))?;
    let restored_bytes = fs::read(target_path).map_err(|err| format!("failed to read target after rollback: {err}"))?;
    let post_rollback_sha256 = sha256_hex(&restored_bytes);
    if post_rollback_sha256 != backup_sha256 {
        return Err("rollback restore completed but target hash does not match backup hash".to_string());
    }

    let receipt = SelfEvolutionRollbackReceipt {
        source: "self_evolution_application_rollback".to_string(),
        run_id: application.run_id,
        status: "rolled_back".to_string(),
        dry_run: false,
        application_receipt_path: options.application_path.display().to_string(),
        target_path: target_path.display().to_string(),
        backup_path: backup_path.display().to_string(),
        pre_rollback_sha256,
        backup_sha256,
        post_rollback_sha256: Some(post_rollback_sha256),
        restored: true,
        evidence: vec![
            "target matched post-apply hash before rollback".to_string(),
            "backup hash matched application receipt before restore".to_string(),
            "backup bytes restored to target".to_string(),
        ],
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    let rollback_path = rollback_receipt_path(&options.application_path);
    let rollback_json = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(&rollback_path, rollback_json).map_err(|err| format!("failed to write rollback receipt: {err}"))?;
    Ok(receipt)
}

fn build_dry_run_application_receipt(
    options: &SelfEvolutionApplicationOptions,
    plan: ApplicationPlan,
) -> SelfEvolutionApplicationReceipt {
    SelfEvolutionApplicationReceipt {
        source: "self_evolution_candidate_application".to_string(),
        run_id: plan.run_receipt.run_id,
        status: "preflight_validated".to_string(),
        dry_run: true,
        apply_mode: options.apply_mode.clone(),
        run_receipt_path: options.receipt_path.display().to_string(),
        approval_receipt_path: options.approval_path.display().to_string(),
        target_path: plan.run_receipt.target.path,
        candidate_path: plan.run_receipt.candidate.artifact_path,
        pre_apply_sha256: plan.current_target_sha256.clone(),
        candidate_sha256: plan.candidate_sha256,
        post_apply_sha256: Some(plan.post_apply_sha256),
        planned_backup_path: plan.planned_backup_path.display().to_string(),
        backup_sha256: Some(plan.current_target_sha256),
        verification: ApplicationVerificationRecord {
            command: options.verification_command.clone(),
            status: "recorded_not_executed_dry_run".to_string(),
            evidence: vec![
                "application request validated without mutating the target".to_string(),
                "verification command was recorded for the later live apply step".to_string(),
            ],
        },
        applied: false,
        rollback: ApplicationRollbackRecord {
            backup_path: plan.planned_backup_path.display().to_string(),
            instructions: vec![
                "dry-run preflight did not create a live backup".to_string(),
                "run live apply only after reviewing receipt, approval, candidate, and planned backup path".to_string(),
            ],
        },
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    }
}

fn run_application_verification(command: &str) -> ApplicationVerificationRecord {
    match Command::new("sh").arg("-c").arg(command).output() {
        Ok(output) if output.status.success() => ApplicationVerificationRecord {
            command: command.to_string(),
            status: "passed".to_string(),
            evidence: vec!["verification command exited successfully".to_string()],
        },
        Ok(output) => ApplicationVerificationRecord {
            command: command.to_string(),
            status: "failed".to_string(),
            evidence: vec![format!(
                "verification command exited with code {}",
                output.status.code().map_or_else(|| "signal".to_string(), |code| code.to_string())
            )],
        },
        Err(err) => ApplicationVerificationRecord {
            command: command.to_string(),
            status: "failed_to_start".to_string(),
            evidence: vec![format!("failed to start verification command: {err}")],
        },
    }
}

pub fn validate_run_options(options: &SelfEvolutionRunOptions) -> std::result::Result<(), String> {
    if !options.dry_run {
        return Err(
            "self-evolution is disabled by default; rerun with --dry-run to use the deterministic fake executor"
                .to_string(),
        );
    }
    if options.baseline_command.trim().is_empty() {
        return Err("baseline command/eval must be non-empty".to_string());
    }
    if options.candidate_output.as_os_str().is_empty() {
        return Err("candidate output path must be non-empty".to_string());
    }
    reject_in_place_candidate(&options.target, &options.candidate_output)
}

pub fn approval_receipt_path(run_receipt_path: &Path) -> PathBuf {
    run_receipt_path.with_file_name("approval.json")
}

pub fn application_receipt_path(run_receipt_path: &Path) -> PathBuf {
    run_receipt_path.with_file_name("application.json")
}

pub fn rollback_receipt_path(application_receipt_path: &Path) -> PathBuf {
    application_receipt_path.with_file_name("rollback.json")
}

pub fn validate_application_options(options: &SelfEvolutionApplicationOptions) -> std::result::Result<(), String> {
    if options.apply_mode.trim() != "replace-file" {
        return Err("unsupported self-evolution apply mode; only replace-file is available".to_string());
    }
    if options.verification_command.trim().is_empty() {
        return Err("application requires a non-empty verification command".to_string());
    }
    Ok(())
}

struct ApplicationPlan {
    run_receipt: SelfEvolutionRunReceipt,
    current_target_sha256: String,
    candidate_sha256: String,
    post_apply_sha256: String,
    planned_backup_path: PathBuf,
}

fn validate_application_receipt_chain(
    options: &SelfEvolutionApplicationOptions,
) -> std::result::Result<ApplicationPlan, String> {
    let run_receipt = read_run_receipt(&options.receipt_path)?;
    let approval_receipt = read_approval_receipt(&options.approval_path)?;
    validate_promotable_receipt(&run_receipt)
        .map_err(|err| format!("candidate is not eligible for application: {err}"))?;
    validate_matching_approval(&run_receipt, &approval_receipt)?;

    let target_path = Path::new(&run_receipt.target.path);
    if !target_path.is_file() {
        return Err("replace-file application requires an existing file target".to_string());
    }
    let candidate_path = Path::new(&run_receipt.candidate.artifact_path);
    if !candidate_path.is_file() {
        return Err("candidate artifact from receipt does not exist".to_string());
    }

    let target_bytes = fs::read(target_path).map_err(|err| format!("failed to read target artifact: {err}"))?;
    let current_target_sha256 = sha256_hex(&target_bytes);
    let expected_target_sha256 = run_receipt
        .target
        .sha256
        .as_deref()
        .ok_or_else(|| "run receipt does not contain a target baseline hash".to_string())?;
    if current_target_sha256 != expected_target_sha256 {
        return Err(format!(
            "target artifact changed since the run receipt was created; expected {expected_target_sha256}, found {current_target_sha256}"
        ));
    }

    let candidate_bytes =
        fs::read(candidate_path).map_err(|err| format!("failed to read candidate artifact: {err}"))?;
    let candidate_sha256 = sha256_hex(&candidate_bytes);
    if candidate_sha256 != run_receipt.candidate.sha256 {
        return Err("candidate artifact hash does not match the run receipt".to_string());
    }
    let planned_backup_path = planned_application_backup_path(&run_receipt, target_path, &current_target_sha256);
    Ok(ApplicationPlan {
        run_receipt,
        current_target_sha256,
        candidate_sha256: candidate_sha256.clone(),
        post_apply_sha256: candidate_sha256,
        planned_backup_path,
    })
}

fn read_run_receipt(path: &Path) -> std::result::Result<SelfEvolutionRunReceipt, String> {
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read self-evolution receipt: {err}"))?;
    serde_json::from_str(&body).map_err(|err| format!("failed to parse self-evolution receipt: {err}"))
}

fn read_approval_receipt(path: &Path) -> std::result::Result<SelfEvolutionApprovalReceipt, String> {
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read approval receipt: {err}"))?;
    serde_json::from_str(&body).map_err(|err| format!("failed to parse approval receipt: {err}"))
}

fn read_application_receipt(path: &Path) -> std::result::Result<SelfEvolutionApplicationReceipt, String> {
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read application receipt: {err}"))?;
    serde_json::from_str(&body).map_err(|err| format!("failed to parse application receipt: {err}"))
}

fn validate_matching_approval(
    run_receipt: &SelfEvolutionRunReceipt,
    approval_receipt: &SelfEvolutionApprovalReceipt,
) -> std::result::Result<(), String> {
    if approval_receipt.run_id != run_receipt.run_id {
        return Err("approval receipt run id does not match the run receipt".to_string());
    }
    if approval_receipt.target_path != run_receipt.target.path {
        return Err("approval receipt target path does not match the run receipt".to_string());
    }
    if approval_receipt.candidate_path != run_receipt.candidate.artifact_path {
        return Err("approval receipt candidate path does not match the run receipt".to_string());
    }
    if !approval_receipt.approval.approved {
        return Err("approval receipt is not approved".to_string());
    }
    if approval_receipt.approval.applied {
        return Err("approval receipt is already marked applied".to_string());
    }
    if approval_receipt.approval.promotion_status != "approval_recorded_not_applied" {
        return Err("approval receipt is not in an apply-ready state".to_string());
    }
    Ok(())
}

fn planned_application_backup_path(
    run_receipt: &SelfEvolutionRunReceipt,
    target_path: &Path,
    target_sha256: &str,
) -> PathBuf {
    let target_name = target_path.file_name().and_then(|name| name.to_str()).unwrap_or("target");
    let short_hash = target_sha256.get(..12).unwrap_or(target_sha256);
    Path::new(&run_receipt.candidate.output_dir)
        .join("backup")
        .join(format!("{target_name}.{short_hash}.bak"))
}

fn validate_approval_options(options: &SelfEvolutionApprovalOptions) -> std::result::Result<(), String> {
    if !options.dry_run {
        return Err(
            "promotion is disabled by default; rerun with --dry-run to record approval without applying the candidate"
                .to_string(),
        );
    }
    if options.session_id.trim().is_empty() {
        return Err("approval requires a session id so the confirmation path is auditable".to_string());
    }
    if options.confirmation_id.trim().is_empty() {
        return Err("approval requires a non-empty confirmation id".to_string());
    }
    if options.approver.trim().is_empty() {
        return Err("approval requires a non-empty approver label".to_string());
    }
    Ok(())
}

fn validate_promotable_receipt(receipt: &SelfEvolutionRunReceipt) -> std::result::Result<(), String> {
    if !receipt.recommendation.human_approval_required {
        return Err("receipt does not require human approval; refusing ambiguous promotion state".to_string());
    }
    if !receipt.recommendation.recommended {
        return Err("candidate is not recommended; approval receipt will not be recorded".to_string());
    }
    if receipt.recommendation.promotion_status != "awaiting_human_approval" {
        return Err("candidate is not awaiting human approval".to_string());
    }
    Ok(())
}

fn reject_in_place_candidate(target: &Path, candidate_output: &Path) -> std::result::Result<(), String> {
    let target_abs = absolutize_lossy(target);
    let candidate_abs = absolutize_lossy(candidate_output);
    if candidate_abs == target_abs {
        return Err("candidate output must be isolated from the target artifact".to_string());
    }
    if target.is_dir() && candidate_abs.starts_with(&target_abs) {
        return Err("candidate output must not be inside the live target directory".to_string());
    }
    if target.is_file() && candidate_abs == target_abs.parent().unwrap_or_else(|| Path::new("")) {
        return Err("candidate output must not be the live target parent directory".to_string());
    }
    Ok(())
}

fn read_target_body(target: &Path) -> std::result::Result<Option<String>, String> {
    if target.is_file() {
        return fs::read_to_string(target).map(Some).map_err(|err| format!("failed to read target artifact: {err}"));
    }
    Ok(None)
}

fn artifact_identity(target: &Path, body: Option<&str>) -> std::result::Result<ArtifactIdentity, String> {
    let exists = target.exists();
    let kind = if target.is_file() {
        "file"
    } else if target.is_dir() {
        "directory"
    } else {
        "missing"
    };
    let bytes = match body {
        Some(body) => body.len() as u64,
        None => 0,
    };
    Ok(ArtifactIdentity {
        path: target.display().to_string(),
        exists,
        kind: kind.to_string(),
        sha256: body.map(|body| sha256_hex(body.as_bytes())),
        bytes,
    })
}

fn summarize_mcp_arguments(tool: &str, arguments: &Value) -> Value {
    match tool {
        "send_prompt" => json!({
            "tool": tool,
            "prompt_len": arguments.get("prompt_len").and_then(Value::as_u64).unwrap_or(0),
            "purpose": arguments.get("purpose").and_then(Value::as_str).unwrap_or("unknown"),
        }),
        _ => json!({
            "tool": tool,
            "argument_keys": arguments.as_object().map(|object| object.len()).unwrap_or(0),
        }),
    }
}

fn self_evolution_prompt_len(target: &ArtifactIdentity, baseline_command: &str) -> usize {
    target.path.len() + baseline_command.len()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn absolutize_lossy(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn self_evolution_dry_run_writes_isolated_receipt_and_uses_fake_mcp() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("skill.md");
        fs::write(&target, "baseline skill\n").unwrap();
        let output = tmp.path().join("candidates");
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "cargo test fake_eval".to_string(),
            candidate_output: output,
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("baseline skill\nimproved\n".to_string()),
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert_eq!(receipt.source, "self_evolution_dry_run");
        assert_eq!(receipt.status, "completed");
        assert!(receipt.candidate.changed_from_baseline);
        assert!(receipt.recommendation.recommended);
        assert!(receipt.recommendation.human_approval_required);
        assert_eq!(executor.calls.len(), 2);
        assert_eq!(executor.calls[0].tool, "send_prompt");
        assert_eq!(executor.calls[1].tool, "session_history");
        assert!(Path::new(&receipt.candidate.artifact_path).exists());
        assert!(Path::new(&receipt.candidate.output_dir).join("receipt.json").exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "baseline skill\n");
    }

    #[test]
    fn self_evolution_unchanged_candidate_is_not_recommended_as_noise() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "same\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: None,
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert!(!receipt.candidate.changed_from_baseline);
        assert!(!receipt.recommendation.recommended);
        assert!(receipt.recommendation.reason.contains("evaluation noise"));
        let receipt_path = Path::new(&receipt.candidate.output_dir).join("receipt.json");
        let saved: SelfEvolutionRunReceipt = serde_json::from_str(&fs::read_to_string(receipt_path).unwrap()).unwrap();
        assert!(!saved.recommendation.recommended);
        assert_eq!(saved.recommendation.promotion_status, "not_promoted");
        assert!(saved.recommendation.human_approval_required);
    }

    #[test]
    fn self_evolution_failed_eval_records_non_promotable_receipt() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval --fixture failure".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: true,
            candidate_body: Some("candidate\n".to_string()),
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert_eq!(receipt.status, "completed_with_failed_evaluation");
        assert_eq!(receipt.baseline.status, "failed_fake_eval");
        assert_eq!(receipt.candidate.status, "failed_fake_eval");
        assert!(receipt.candidate.changed_from_baseline);
        assert!(!receipt.recommendation.recommended);
        assert_eq!(receipt.recommendation.promotion_status, "not_promoted_eval_failed");
        assert!(receipt.recommendation.reason.contains("evaluation failed"));
        assert!(receipt.baseline.evidence.iter().any(|entry| entry.contains("fail")));
        assert_eq!(executor.calls.len(), 2);
    }

    #[test]
    fn self_evolution_rejects_live_in_place_and_non_dry_run() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("live");
        fs::create_dir_all(&target).unwrap();
        let non_dry = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: None,
            dry_run: false,
            simulate_eval_failure: false,
            candidate_body: None,
        };
        assert!(validate_run_options(&non_dry).unwrap_err().contains("disabled by default"));

        let in_place = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: target.join("candidate"),
            session_id: None,
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
        };
        assert!(validate_run_options(&in_place).unwrap_err().contains("live target directory"));
    }

    #[test]
    fn self_evolution_approval_records_confirmation_receipt_without_applying() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("tool.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("candidate\n".to_string()),
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval_options = SelfEvolutionApprovalOptions {
            receipt_path: run_receipt_path.clone(),
            session_id: "sess-1".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "human-reviewer".to_string(),
            dry_run: true,
        };
        let mut approval_executor = FakeMcpExecutor::default();

        let approval = approve_self_evolution_promotion(&approval_options, &mut approval_executor).unwrap();

        assert_eq!(approval.source, "self_evolution_promotion_gate");
        assert_eq!(approval.status, "approval_recorded");
        assert!(approval.approval.approved);
        assert!(!approval.approval.applied);
        assert_eq!(approval.approval.promotion_status, "approval_recorded_not_applied");
        assert_eq!(approval_executor.calls.len(), 2);
        assert_eq!(approval_executor.calls[0].tool, "approve_confirmation");
        assert_eq!(approval_executor.calls[1].tool, "session_history");
        assert!(approval_receipt_path(&run_receipt_path).exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "baseline\n");
    }

    #[test]
    fn self_evolution_approval_rejects_unrecommended_or_ungated_candidates() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "same\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval_options = SelfEvolutionApprovalOptions {
            receipt_path: run_receipt_path,
            session_id: "sess-1".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "reviewer".to_string(),
            dry_run: true,
        };
        let mut approval_executor = FakeMcpExecutor::default();

        let err = approve_self_evolution_promotion(&approval_options, &mut approval_executor).unwrap_err();

        assert!(err.contains("not recommended"));
        assert!(approval_executor.calls.is_empty());
    }

    #[test]
    fn self_evolution_approval_requires_dry_run_and_confirmation_context() {
        let options = SelfEvolutionApprovalOptions {
            receipt_path: PathBuf::from("receipt.json"),
            session_id: "".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "reviewer".to_string(),
            dry_run: true,
        };
        assert!(validate_approval_options(&options).unwrap_err().contains("session id"));

        let non_dry = SelfEvolutionApprovalOptions {
            dry_run: false,
            session_id: "sess-1".to_string(),
            ..options
        };
        assert!(validate_approval_options(&non_dry).unwrap_err().contains("disabled by default"));
    }

    #[test]
    fn self_evolution_application_preflight_validates_without_mutation() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let original_target = fs::read_to_string(&target).unwrap();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path: approval_path.clone(),
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };

        let receipt = preflight_self_evolution_application(&options).unwrap();

        assert_eq!(receipt.source, "self_evolution_candidate_application");
        assert_eq!(receipt.status, "preflight_validated");
        assert!(receipt.dry_run);
        assert!(!receipt.applied);
        assert_eq!(receipt.apply_mode, "replace-file");
        assert_eq!(receipt.verification.status, "recorded_not_executed_dry_run");
        assert_eq!(fs::read_to_string(&target).unwrap(), original_target);
        assert!(!Path::new(&receipt.planned_backup_path).exists());
        assert!(!application_receipt_path(&run_receipt_path).exists());
        assert_eq!(receipt.approval_receipt_path, approval_path.display().to_string());
    }

    #[test]
    fn self_evolution_application_rejects_stale_target_before_mutation() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        fs::write(&target, "changed after receipt\n").unwrap();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path,
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };

        let err = preflight_self_evolution_application(&options).unwrap_err();

        assert!(err.contains("target artifact changed"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "changed after receipt\n");
    }

    #[test]
    fn self_evolution_application_rejects_mismatched_or_applied_approval() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let mut approval: SelfEvolutionApprovalReceipt =
            serde_json::from_str(&fs::read_to_string(&approval_path).unwrap()).unwrap();
        approval.candidate_path = target.display().to_string();
        fs::write(&approval_path, serde_json::to_string_pretty(&approval).unwrap()).unwrap();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path: approval_path.clone(),
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };

        let err = preflight_self_evolution_application(&options).unwrap_err();

        assert!(err.contains("candidate path does not match"));
        approval.candidate_path = read_run_receipt(&run_receipt_path).unwrap().candidate.artifact_path;
        approval.approval.applied = true;
        fs::write(&approval_path, serde_json::to_string_pretty(&approval).unwrap()).unwrap();
        let err = preflight_self_evolution_application(&options).unwrap_err();
        assert!(err.contains("already marked applied"));
    }

    #[test]
    fn self_evolution_application_rejects_non_promotable_missing_candidate_and_unsupported_mode() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "same\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval = SelfEvolutionApprovalReceipt {
            source: "test".to_string(),
            run_id: run_receipt.run_id.clone(),
            status: "approval_recorded".to_string(),
            dry_run: true,
            approver: "reviewer".to_string(),
            confirmation_id: "confirm-1".to_string(),
            target_path: run_receipt.target.path.clone(),
            candidate_path: run_receipt.candidate.artifact_path.clone(),
            approval: PromotionApprovalRecord {
                approved: true,
                human_approval_required: true,
                applied: false,
                promotion_status: "approval_recorded_not_applied".to_string(),
                evidence: Vec::new(),
            },
            mcp_receipts: Vec::new(),
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        };
        let approval_path = approval_receipt_path(&run_receipt_path);
        fs::write(&approval_path, serde_json::to_string_pretty(&approval).unwrap()).unwrap();
        let preflight = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path: approval_path.clone(),
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };
        let err = preflight_self_evolution_application(&preflight).unwrap_err();
        assert!(err.contains("not eligible"));

        let (_tmp, _target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let run_receipt = read_run_receipt(&run_receipt_path).unwrap();
        fs::remove_file(&run_receipt.candidate.artifact_path).unwrap();
        let err = preflight_self_evolution_application(&SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path,
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        })
        .unwrap_err();
        assert!(err.contains("candidate artifact"));

        let unsupported = SelfEvolutionApplicationOptions {
            receipt_path: PathBuf::from("receipt.json"),
            approval_path: PathBuf::from("approval.json"),
            apply_mode: "patch".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };
        assert!(validate_application_options(&unsupported).unwrap_err().contains("unsupported"));
    }

    #[test]
    fn self_evolution_application_live_replace_writes_backup_receipt_and_target() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "true".to_string(),
            dry_run: false,
        };

        let receipt = apply_self_evolution_candidate(&options).unwrap();

        assert_eq!(receipt.status, "applied");
        assert!(receipt.applied);
        assert!(!receipt.dry_run);
        assert_eq!(receipt.verification.status, "passed");
        assert_eq!(fs::read_to_string(&target).unwrap(), "candidate\n");
        assert_eq!(fs::read_to_string(&receipt.planned_backup_path).unwrap(), "baseline\n");
        assert!(application_receipt_path(&run_receipt_path).exists());
        let saved: SelfEvolutionApplicationReceipt =
            serde_json::from_str(&fs::read_to_string(application_receipt_path(&run_receipt_path)).unwrap()).unwrap();
        assert_eq!(saved.status, "applied");
        assert_eq!(saved.backup_sha256, Some(receipt.pre_apply_sha256));
        assert_eq!(saved.post_apply_sha256, Some(receipt.candidate_sha256));
    }

    #[test]
    fn self_evolution_application_records_verification_failure_after_live_replace() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "false".to_string(),
            dry_run: false,
        };

        let receipt = apply_self_evolution_candidate(&options).unwrap();

        assert_eq!(receipt.status, "applied_verification_failed");
        assert!(receipt.applied);
        assert_eq!(receipt.verification.status, "failed");
        assert_eq!(fs::read_to_string(&target).unwrap(), "candidate\n");
        assert_eq!(fs::read_to_string(&receipt.planned_backup_path).unwrap(), "baseline\n");
        assert!(application_receipt_path(&run_receipt_path).exists());
        assert!(receipt.rollback.instructions.iter().any(|step| step.contains("restore")));
    }

    #[test]
    fn self_evolution_rollback_preflights_and_restores_backup_with_hash_guard() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let apply = apply_self_evolution_candidate(&SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "true".to_string(),
            dry_run: false,
        })
        .unwrap();
        let application_path = application_receipt_path(&run_receipt_path);

        let preflight = rollback_self_evolution_application(&SelfEvolutionRollbackOptions {
            application_path: application_path.clone(),
            dry_run: true,
        })
        .unwrap();
        assert_eq!(preflight.status, "rollback_preflight_validated");
        assert!(!preflight.restored);
        assert_eq!(fs::read_to_string(&target).unwrap(), "candidate\n");
        assert!(!rollback_receipt_path(&application_path).exists());

        let rollback = rollback_self_evolution_application(&SelfEvolutionRollbackOptions {
            application_path: application_path.clone(),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(rollback.status, "rolled_back");
        assert!(rollback.restored);
        assert_eq!(rollback.backup_sha256, apply.pre_apply_sha256);
        assert_eq!(fs::read_to_string(&target).unwrap(), "baseline\n");
        assert!(rollback_receipt_path(&application_path).exists());
    }

    #[test]
    fn self_evolution_rollback_rejects_target_changed_after_application() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        apply_self_evolution_candidate(&SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "true".to_string(),
            dry_run: false,
        })
        .unwrap();
        let application_path = application_receipt_path(&run_receipt_path);
        fs::write(&target, "operator changed applied target\n").unwrap();

        let err = rollback_self_evolution_application(&SelfEvolutionRollbackOptions {
            application_path,
            dry_run: false,
        })
        .unwrap_err();

        assert!(err.contains("target artifact changed since application"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "operator changed applied target\n");
    }

    fn approved_candidate_fixture() -> (TempDir, PathBuf, PathBuf, PathBuf) {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("tool.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("candidate\n".to_string()),
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval_options = SelfEvolutionApprovalOptions {
            receipt_path: run_receipt_path.clone(),
            session_id: "sess-1".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "reviewer".to_string(),
            dry_run: true,
        };
        let mut approval_executor = FakeMcpExecutor::default();
        approve_self_evolution_promotion(&approval_options, &mut approval_executor).unwrap();
        let approval_path = approval_receipt_path(&run_receipt_path);
        (tmp, target, run_receipt_path, approval_path)
    }
}
