//! Self-evolution CLI handlers.

use crate::cli::SelfEvolutionAction;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::error::Result;
use crate::self_evolution::FakeMcpExecutor;
use crate::self_evolution::SelfEvolutionApprovalOptions;
use crate::self_evolution::SelfEvolutionApprovalReceipt;
use crate::self_evolution::SelfEvolutionRunOptions;
use crate::self_evolution::SelfEvolutionRunReceipt;
use crate::self_evolution::approve_self_evolution_promotion;
use crate::self_evolution::run_self_evolution_dry_run;

pub fn run(_ctx: &CommandContext, action: SelfEvolutionAction) -> Result<()> {
    match action {
        SelfEvolutionAction::Run {
            target,
            baseline_command,
            candidate_output,
            session,
            dry_run,
            simulate_eval_failure,
            json,
        } => {
            let options = SelfEvolutionRunOptions {
                target,
                baseline_command,
                candidate_output,
                session_id: session,
                dry_run,
                candidate_body: None,
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
    }
    Ok(())
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
