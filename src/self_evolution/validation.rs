use std::fs;
use std::path::Path;
use std::path::PathBuf;

use sha2::Digest;
use sha2::Sha256;

use super::receipts::ArtifactIdentity;
use super::receipts::SelfEvolutionApplicationOptions;
use super::receipts::SelfEvolutionApplicationReceipt;
use super::receipts::SelfEvolutionApprovalOptions;
use super::receipts::SelfEvolutionApprovalReceipt;
use super::receipts::SelfEvolutionRunOptions;
use super::receipts::SelfEvolutionRunReceipt;

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

pub(super) struct ApplicationPlan {
    pub(super) run_receipt: SelfEvolutionRunReceipt,
    pub(super) current_target_sha256: String,
    pub(super) candidate_sha256: String,
    pub(super) post_apply_sha256: String,
    pub(super) planned_backup_path: PathBuf,
}

pub(super) fn validate_application_receipt_chain(
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

pub(super) fn read_run_receipt(path: &Path) -> std::result::Result<SelfEvolutionRunReceipt, String> {
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read self-evolution receipt: {err}"))?;
    serde_json::from_str(&body).map_err(|err| format!("failed to parse self-evolution receipt: {err}"))
}

pub(super) fn read_approval_receipt(path: &Path) -> std::result::Result<SelfEvolutionApprovalReceipt, String> {
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read approval receipt: {err}"))?;
    serde_json::from_str(&body).map_err(|err| format!("failed to parse approval receipt: {err}"))
}

pub(super) fn read_application_receipt(path: &Path) -> std::result::Result<SelfEvolutionApplicationReceipt, String> {
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read application receipt: {err}"))?;
    serde_json::from_str(&body).map_err(|err| format!("failed to parse application receipt: {err}"))
}

pub(super) fn validate_matching_approval(
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

pub(super) fn planned_application_backup_path(
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

pub(super) fn validate_approval_options(options: &SelfEvolutionApprovalOptions) -> std::result::Result<(), String> {
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

pub(super) fn validate_promotable_receipt(receipt: &SelfEvolutionRunReceipt) -> std::result::Result<(), String> {
    if !receipt.recommendation.human_approval_required {
        return Err("receipt does not require human approval; refusing ambiguous promotion state".to_string());
    }
    if !receipt.recommendation.recommended {
        return Err("candidate is not recommended; approval receipt will not be recorded".to_string());
    }
    if receipt.recommendation.promotion_status != "awaiting_human_approval" {
        return Err("candidate is not awaiting human approval".to_string());
    }
    if receipt.readiness.label != "promotion_eligible" {
        return Err(format!(
            "candidate readiness is {}; promotion approval requires promotion_eligible corpus evidence",
            receipt.readiness.label
        ));
    }
    Ok(())
}

pub(super) fn reject_in_place_candidate(target: &Path, candidate_output: &Path) -> std::result::Result<(), String> {
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

pub(super) fn read_target_body(target: &Path) -> std::result::Result<Option<String>, String> {
    if target.is_file() {
        return fs::read_to_string(target).map(Some).map_err(|err| format!("failed to read target artifact: {err}"));
    }
    Ok(None)
}

pub(super) fn artifact_identity(target: &Path, body: Option<&str>) -> std::result::Result<ArtifactIdentity, String> {
    let is_target_present = target.exists();
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
        exists: is_target_present,
        kind: kind.to_string(),
        sha256: body.map(|body| sha256_hex(body.as_bytes())),
        bytes,
    })
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(super) fn absolutize_lossy(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
}
