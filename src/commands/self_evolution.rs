//! Self-evolution CLI handlers.

use std::path::PathBuf;

use crate::cli::SelfEvolutionAction;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::error::Result;
use crate::self_evolution::FakeMcpExecutor;
use crate::self_evolution::SelfEvolutionApplicationOptions;
use crate::self_evolution::SelfEvolutionApplicationReceipt;
use crate::self_evolution::SelfEvolutionApprovalOptions;
use crate::self_evolution::SelfEvolutionApprovalReceipt;
use crate::self_evolution::SelfEvolutionRunOptions;
use crate::self_evolution::SelfEvolutionRunReceipt;
use crate::self_evolution::apply_self_evolution_candidate;
use crate::self_evolution::approve_self_evolution_promotion;
use crate::self_evolution::run_self_evolution_dry_run;

pub fn run(_ctx: &CommandContext, action: SelfEvolutionAction) -> Result<()> {
    match action {
        SelfEvolutionAction::Run {
            target,
            baseline_command,
            candidate_output,
            session,
            candidate_body,
            candidate_file,
            dry_run,
            simulate_eval_failure,
            json,
        } => {
            let candidate_body = load_candidate_body(candidate_body, candidate_file)?;
            let options = SelfEvolutionRunOptions {
                target,
                baseline_command,
                candidate_output,
                session_id: session,
                dry_run,
                candidate_body,
                simulate_eval_failure,
            };
            let mut executor = FakeMcpExecutor::default();
            let receipt =
                run_self_evolution_dry_run(&options, &mut executor).map_err(|message| Error::Config { message })?;
            print_run_receipt(&receipt, json)?;
        }
        SelfEvolutionAction::Approve {
            receipt,
            session,
            confirmation_id,
            approver,
            dry_run,
            json,
        } => {
            let options = SelfEvolutionApprovalOptions {
                receipt_path: receipt,
                session_id: session,
                confirmation_id,
                approver,
                dry_run,
            };
            let mut executor = FakeMcpExecutor::default();
            let approval = approve_self_evolution_promotion(&options, &mut executor)
                .map_err(|message| Error::Config { message })?;
            print_approval_receipt(&approval, json)?;
        }
        SelfEvolutionAction::Apply {
            receipt,
            approval,
            mode,
            verify_command,
            dry_run,
            live_apply,
            json,
        } => {
            let options = SelfEvolutionApplicationOptions {
                receipt_path: receipt,
                approval_path: approval,
                apply_mode: mode,
                verification_command: verify_command,
                dry_run: dry_run || !live_apply,
            };
            let application = apply_self_evolution_candidate(&options).map_err(|message| Error::Config { message })?;
            print_application_receipt(&application, json)?;
        }
    }
    Ok(())
}

fn load_candidate_body(candidate_body: Option<String>, candidate_file: Option<PathBuf>) -> Result<Option<String>> {
    if let Some(body) = candidate_body {
        return Ok(Some(body));
    }
    if let Some(path) = candidate_file {
        let body = std::fs::read_to_string(&path).map_err(|source| Error::Io { source })?;
        return Ok(Some(body));
    }
    Ok(None)
}

fn print_run_receipt(receipt: &SelfEvolutionRunReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt).map_err(|source| Error::Json { source })?);
        return Ok(());
    }

    println!(
        "self-evolution dry-run complete: run_id={} target={} candidate={} recommended={} promotion={}",
        receipt.run_id,
        receipt.target.path,
        receipt.candidate.artifact_path,
        receipt.recommendation.recommended,
        receipt.recommendation.promotion_status
    );
    println!("receipt: {}/receipt.json", receipt.candidate.output_dir);
    Ok(())
}

fn print_approval_receipt(receipt: &SelfEvolutionApprovalReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt).map_err(|source| Error::Json { source })?);
        return Ok(());
    }

    println!(
        "self-evolution approval recorded: run_id={} target={} candidate={} promotion={}",
        receipt.run_id, receipt.target_path, receipt.candidate_path, receipt.approval.promotion_status
    );
    println!("approval receipt: approval.json next to the run receipt");
    Ok(())
}

fn print_application_receipt(receipt: &SelfEvolutionApplicationReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt).map_err(|source| Error::Json { source })?);
        return Ok(());
    }

    println!(
        "self-evolution application: run_id={} target={} candidate={} status={} applied={} verification={}",
        receipt.run_id,
        receipt.target_path,
        receipt.candidate_path,
        receipt.status,
        receipt.applied,
        receipt.verification.status
    );
    println!("application receipt: application.json next to the run receipt when live apply is used");
    println!("backup path: {}", receipt.planned_backup_path);
    Ok(())
}
