//! SOUL/personality CLI handlers.

use crate::cli::SoulAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::soul_personality;

pub fn run(_ctx: &CommandContext, action: SoulAction) -> Result<()> {
    match action {
        SoulAction::Status { json } => {
            let status = soul_personality::status_summary();
            print_validation(&status, json)?;
        }
        SoulAction::Validate {
            soul,
            personality,
            json,
        } => {
            let source = soul_personality::parse_soul_source(soul.as_deref());
            let personality = soul_personality::parse_personality(personality.as_deref())
                .map_err(|message| crate::error::Error::Config { message })?;
            let validation = soul_personality::validate(&source, personality.as_ref());
            print_validation(&validation, json)?;
            if !validation.supported {
                return Err(crate::error::Error::Config {
                    message: validation
                        .error_message
                        .unwrap_or_else(|| "SOUL/personality source is unsupported".to_string()),
                });
            }
        }
    }
    Ok(())
}

fn print_validation(validation: &soul_personality::SoulValidation, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(validation).map_err(|source| crate::error::Error::Json { source })?
        );
        return Ok(());
    }

    let personality = validation.personality.as_deref().unwrap_or("none");
    if validation.supported {
        println!(
            "SOUL/personality: {} source via {} (personality: {})",
            validation.soul_label, validation.backend, personality
        );
    } else {
        println!(
            "SOUL/personality: unsupported {} source ({})",
            validation.soul_label,
            validation.error_message.as_deref().unwrap_or("unsupported source")
        );
    }
    Ok(())
}
