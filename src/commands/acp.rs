use std::io::BufRead;
use std::io::Write;

use crate::cli::AcpAction;
use crate::commands::CommandContext;
use crate::error::Result;

pub async fn run(_ctx: &CommandContext, action: AcpAction) -> Result<()> {
    match action {
        AcpAction::Serve { session, new, model } => run_serve(session, new, model),
    }
}

fn run_serve(_session: Option<String>, _new: bool, _model: Option<String>) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|source| crate::error::Error::Io { source })?;
        if line.trim().is_empty() {
            continue;
        }
        let response =
            crate::modes::acp::handle_json_line(&line).map_err(|source| crate::error::Error::Json { source })?;
        writeln!(stdout, "{response}").map_err(|source| crate::error::Error::Io { source })?;
        stdout.flush().map_err(|source| crate::error::Error::Io { source })?;
    }

    Ok(())
}
