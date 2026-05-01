//! Checkpoint and rollback CLI handlers.

use crate::cli::CheckpointAction;
use crate::commands::CommandContext;
use crate::error::ConfigSnafu;
use crate::error::Result;

pub fn run(_ctx: &CommandContext, action: CheckpointAction) -> Result<()> {
    match action {
        CheckpointAction::Create { .. } => unsupported("checkpoint create"),
        CheckpointAction::List { .. } => unsupported("checkpoint list"),
        CheckpointAction::Rollback { .. } => unsupported("rollback"),
    }
}

fn unsupported(command: &str) -> Result<()> {
    ConfigSnafu {
        message: format!("{command} is not implemented yet; the OpenSpec change is defining the safe surface first"),
    }
    .fail()
}
