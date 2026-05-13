use std::fs;
use std::path::Path;

use chrono::SecondsFormat;
use chrono::Utc;
use serde_json::json;

use super::SelfEvolutionExecutor;
use super::receipts::*;
use super::validation::approval_receipt_path;
use super::validation::validate_approval_options;
use super::validation::validate_promotable_receipt;

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
