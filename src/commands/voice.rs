//! Voice/STT CLI handlers.

use crate::cli::VoiceAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::voice_mode;

pub fn run(_ctx: &CommandContext, action: VoiceAction) -> Result<()> {
    match action {
        VoiceAction::Status { json } => {
            let status = voice_mode::status_summary();
            print_validation(&status, json)?;
        }
        VoiceAction::Validate { input, reply, json } => {
            let source = voice_mode::parse_input_source(&input);
            let reply_mode = voice_mode::parse_reply_mode(reply.as_deref())
                .map_err(|message| crate::error::Error::Config { message })?;
            let validation = voice_mode::validate(&source, reply_mode, false);
            print_validation(&validation, json)?;
            if !validation.supported {
                return Err(crate::error::Error::Config {
                    message: validation
                        .error_message
                        .unwrap_or_else(|| "voice input source is unsupported".to_string()),
                });
            }
        }
    }
    Ok(())
}

fn print_validation(validation: &voice_mode::VoiceValidation, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(validation).map_err(|source| crate::error::Error::Json { source })?
        );
        return Ok(());
    }

    if validation.supported {
        println!(
            "voice mode: {} input via {} (reply: {})",
            validation.input_label, validation.backend, validation.reply_mode
        );
    } else {
        println!(
            "voice mode: unsupported {} input ({})",
            validation.input_label,
            validation.error_message.as_deref().unwrap_or("unsupported input")
        );
    }
    Ok(())
}
