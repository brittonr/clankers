//! Checkpoint and rollback CLI handlers.

use crate::checkpoints;
use crate::cli::CheckpointAction;
use crate::commands::CommandContext;
use crate::error::Result;

pub fn run(ctx: &CommandContext, action: CheckpointAction) -> Result<()> {
    let cwd = std::path::Path::new(&ctx.cwd);
    match action {
        CheckpointAction::Create { label, json } => {
            let outcome = checkpoints::create_checkpoint(cwd, label)?;
            print_outcome(&outcome, json)?;
        }
        CheckpointAction::List { json } => {
            let outcome = checkpoints::list_checkpoints(cwd)?;
            print_outcome(&outcome, json)?;
        }
        CheckpointAction::Rollback {
            checkpoint_id,
            yes,
            json,
        } => {
            let outcome = checkpoints::rollback_checkpoint(cwd, &checkpoint_id, yes)?;
            print_outcome(&outcome, json)?;
        }
    }
    Ok(())
}

fn print_outcome(outcome: &checkpoints::CheckpointOutcome, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(outcome).map_err(|source| crate::error::Error::Json { source })?);
        return Ok(());
    }

    match outcome.action.as_str() {
        "create" | "rollback" => {
            if let Some(record) = &outcome.record {
                println!(
                    "{} {} ({} file(s), repo: {})",
                    outcome.action, record.id, record.changed_file_count, record.repo_root
                );
            }
        }
        "list" => {
            for record in &outcome.records {
                println!("{}\t{}\t{} file(s)", record.id, record.created_at, record.changed_file_count);
            }
        }
        _ => println!("{}", outcome.status),
    }
    Ok(())
}
