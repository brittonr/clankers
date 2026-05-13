use std::fs;
use std::path::Path;

use chrono::SecondsFormat;
use chrono::Utc;

use super::receipts::*;
use super::validation::read_application_receipt;
use super::validation::rollback_receipt_path;
use super::validation::sha256_hex;

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
