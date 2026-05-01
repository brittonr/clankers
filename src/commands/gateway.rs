//! Tool gateway CLI handlers.

use crate::cli::GatewayAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::tool_gateway;

pub fn run(_ctx: &CommandContext, action: GatewayAction) -> Result<()> {
    match action {
        GatewayAction::Status { json } => {
            let status = tool_gateway::status_summary();
            print_validation(&status, json)?;
        }
        GatewayAction::Validate {
            toolsets,
            deliver,
            json,
        } => {
            let toolsets =
                tool_gateway::parse_toolsets(&toolsets).map_err(|message| crate::error::Error::Config { message })?;
            let target = tool_gateway::parse_delivery_target(deliver.as_deref());
            let validation = tool_gateway::validate(&toolsets, &target, false);
            print_validation(&validation, json)?;
            if !validation.supported {
                return Err(crate::error::Error::Config {
                    message: validation
                        .error_message
                        .unwrap_or_else(|| "gateway delivery target is unsupported".to_string()),
                });
            }
        }
    }
    Ok(())
}

fn print_validation(validation: &tool_gateway::GatewayValidation, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(validation).map_err(|source| crate::error::Error::Json { source })?
        );
        return Ok(());
    }

    if validation.supported {
        println!(
            "tool gateway: {} delivery via {} (toolsets: {})",
            validation.delivery_target,
            validation.backend,
            validation.toolsets.join(", ")
        );
    } else {
        println!(
            "tool gateway: unsupported {} delivery ({})",
            validation.delivery_target,
            validation.error_message.as_deref().unwrap_or("unsupported target")
        );
    }
    Ok(())
}
