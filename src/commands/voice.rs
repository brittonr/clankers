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
            let reply_mode = parse_reply(reply.as_deref())?;
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
        VoiceAction::Start {
            input,
            reply,
            enable,
            auto_submit,
            json,
        } => {
            let reply_mode = parse_reply(reply.as_deref())?;
            let policy = voice_mode::VoiceCapturePolicy {
                enabled: enable,
                provider: voice_mode::SttProviderPolicy::LocalFake,
                retain_audio: false,
                auto_submit,
            };
            let receipt = voice_mode::start_capture(&policy, capture_request(input, reply_mode));
            print_capture(&receipt, json)?;
            if receipt.error_kind.is_some() {
                return Err(crate::error::Error::Config {
                    message: receipt.error_message.unwrap_or_else(|| "voice capture could not start".to_string()),
                });
            }
        }
        VoiceAction::Stop { input, json } => {
            let policy = voice_mode::VoiceCapturePolicy {
                enabled: true,
                provider: voice_mode::SttProviderPolicy::LocalFake,
                retain_audio: false,
                auto_submit: false,
            };
            let receipt = voice_mode::stop_capture(&policy, capture_request(input, voice_mode::VoiceReplyMode::Text));
            print_capture(&receipt, json)?;
        }
        VoiceAction::SubmitTranscript {
            transcript,
            reply,
            auto_submit,
            json,
        } => {
            let reply_mode = parse_reply(reply.as_deref())?;
            let prompt = voice_mode::session_prompt_from_transcript(&transcript, reply_mode, auto_submit)
                .map_err(|message| crate::error::Error::Config { message })?;
            print_prompt(&prompt, json)?;
        }
    }
    Ok(())
}

fn parse_reply(reply: Option<&str>) -> Result<voice_mode::VoiceReplyMode> {
    voice_mode::parse_reply_mode(reply).map_err(|message| crate::error::Error::Config { message })
}

fn capture_request(input: String, reply_mode: voice_mode::VoiceReplyMode) -> voice_mode::VoiceCaptureRequest {
    voice_mode::VoiceCaptureRequest {
        session_id: None,
        source: voice_mode::parse_input_source(&input),
        reply_mode,
    }
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

fn print_capture(receipt: &voice_mode::VoiceCaptureReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt).map_err(|source| crate::error::Error::Json { source })?);
        return Ok(());
    }
    println!(
        "voice capture: {} {} input via {} (active: {}, raw audio retained: {})",
        receipt.status, receipt.input_label, receipt.backend, receipt.capture_active, receipt.raw_audio_retained
    );
    Ok(())
}

fn print_prompt(prompt: &voice_mode::VoiceSessionPrompt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(prompt).map_err(|source| crate::error::Error::Json { source })?);
        return Ok(());
    }
    println!(
        "voice transcript: {} session prompt ({} chars, reply: {}, auto-submit: {})",
        prompt.status, prompt.transcript_chars, prompt.reply_mode, prompt.auto_submit
    );
    Ok(())
}
