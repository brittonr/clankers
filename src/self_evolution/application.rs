use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::SecondsFormat;
use chrono::Utc;

use super::receipts::*;
use super::validation::ApplicationPlan;
use super::validation::application_receipt_path;
use super::validation::sha256_hex;
use super::validation::validate_application_options;
use super::validation::validate_application_receipt_chain;

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
