//! Steel Scheme runtime wrapper CLI surfaces.

use std::fs;

use clankers_runtime::SteelRuntimeProfile;
use clankers_runtime::SteelRuntimeRequest;
use clankers_runtime::evaluate_steel_request;
use clankers_runtime::steel_runtime_status;
use snafu::ResultExt;

use crate::cli::SteelAction;
use crate::commands::CommandContext;
use crate::error::JsonSnafu;
use crate::error::Result;

/// Run `clankers steel ...` through the runtime-owned wrapper.
pub fn run(_ctx: &CommandContext, action: SteelAction) -> Result<()> {
    match action {
        SteelAction::Status => print_status(),
        SteelAction::Eval { source } => print_eval(source),
        SteelAction::Run { file } => {
            let source = fs::read_to_string(&file).map_err(|source| crate::error::Error::Config {
                message: format!("failed to read Steel source `{}`: {source}", file.display()),
            })?;
            print_eval(source)
        }
    }
}

fn print_status() -> Result<()> {
    let status = steel_runtime_status(SteelRuntimeProfile::default_deny());
    let output = serde_json::to_string_pretty(&status).context(JsonSnafu)?;
    println!("{output}");
    Ok(())
}

fn print_eval(source: String) -> Result<()> {
    let receipt = evaluate_steel_request(&SteelRuntimeRequest::pure(source));
    let output = serde_json::to_string_pretty(&receipt).context(JsonSnafu)?;
    println!("{output}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steel_eval_uses_wrapper_receipt() {
        let receipt = evaluate_steel_request(&SteelRuntimeRequest::pure("(+ 2 3)"));
        assert_eq!(receipt.output.as_deref(), Some("5"));
        assert_eq!(receipt.profile_name, "default-deny");
    }
}
