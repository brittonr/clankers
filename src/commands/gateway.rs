//! Tool gateway CLI handlers.

use std::path::PathBuf;

use crate::cli::GatewayAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::tool_gateway;

pub fn run(_ctx: &CommandContext, action: GatewayAction) -> Result<()> {
    match action {
        GatewayAction::Status { json } => {
            let status = tool_gateway::status_summary();
            print_json_or_validation(&status, json)?;
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
            print_json_or_validation(&validation, json)?;
            if !validation.supported {
                return Err(crate::error::Error::Config {
                    message: validation
                        .error_message
                        .unwrap_or_else(|| "gateway delivery target is unsupported".to_string()),
                });
            }
        }
        GatewayAction::Deliver {
            artifact_type,
            path,
            deliver,
            outbox,
            matrix_active,
            matrix_binding,
            json,
        } => {
            let artifact_type = parse_artifact_kind(&artifact_type)?;
            let path = path.map(PathBuf::from);
            let target = tool_gateway::parse_delivery_target(deliver.as_deref());
            let context = delivery_context(matrix_active, matrix_binding);
            let attempt = tool_gateway::deliver_artifact(artifact_type, path.as_deref(), &target, &context);
            let attempt = if let Some(outbox) = outbox {
                tool_gateway::record_attempt(&PathBuf::from(outbox), attempt)
                    .map_err(|message| crate::error::Error::Config { message })?
            } else {
                attempt
            };
            print_json_or_attempt(&attempt, json)?;
            if attempt.status != "success" {
                return Err(crate::error::Error::Config {
                    message: attempt
                        .receipt
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "gateway delivery target is unsupported".to_string()),
                });
            }
        }
        GatewayAction::DeliverReceipt {
            artifact_type,
            path,
            deliver,
            json,
        } => {
            let artifact_type = parse_artifact_kind(&artifact_type)?;
            let path = path.map(PathBuf::from);
            let target = tool_gateway::parse_delivery_target(deliver.as_deref());
            let receipt = tool_gateway::local_delivery_receipt(artifact_type, path.as_deref(), &target);
            print_json_or_receipt(&receipt, json)?;
            if receipt.status != "success" {
                return Err(crate::error::Error::Config {
                    message: receipt
                        .error_message
                        .unwrap_or_else(|| "gateway delivery target is unsupported".to_string()),
                });
            }
        }
        GatewayAction::DeliveryStatus { outbox, json } => {
            let outbox = tool_gateway::read_outbox(&PathBuf::from(outbox))
                .map_err(|message| crate::error::Error::Config { message })?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&outbox).map_err(|source| crate::error::Error::Json { source })?
                );
            } else {
                println!("tool gateway: {} recorded delivery attempts", outbox.attempts.len());
            }
        }
        GatewayAction::Retry {
            outbox,
            attempt_id,
            matrix_active,
            matrix_binding,
            json,
        } => {
            let context = delivery_context(matrix_active, matrix_binding);
            let attempt = tool_gateway::retry_attempt(&PathBuf::from(outbox), &attempt_id, &context)
                .map_err(|message| crate::error::Error::Config { message })?;
            print_json_or_attempt(&attempt, json)?;
        }
    }
    Ok(())
}

fn delivery_context(matrix_active: bool, matrix_binding: Option<String>) -> tool_gateway::DeliveryContext {
    if matrix_active {
        tool_gateway::DeliveryContext::matrix(matrix_binding.unwrap_or_else(|| "active_matrix_session".to_string()))
    } else {
        tool_gateway::DeliveryContext::local()
    }
}

fn parse_artifact_kind(input: &str) -> Result<tool_gateway::ArtifactKind> {
    match input.trim().to_ascii_lowercase().as_str() {
        "file" => Ok(tool_gateway::ArtifactKind::File),
        "media" => Ok(tool_gateway::ArtifactKind::Media),
        "scheduled-output" | "scheduled_output" | "scheduled" => Ok(tool_gateway::ArtifactKind::ScheduledOutput),
        other => Err(crate::error::Error::Config {
            message: format!("unknown artifact type '{other}'"),
        }),
    }
}

fn print_json_or_validation(validation: &tool_gateway::GatewayValidation, json: bool) -> Result<()> {
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

fn print_json_or_attempt(attempt: &tool_gateway::DeliveryAttempt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(attempt).map_err(|source| crate::error::Error::Json { source })?);
        return Ok(());
    }
    println!("tool gateway: {} {} delivery attempt {}", attempt.status, attempt.target_kind, attempt.attempt_id);
    Ok(())
}

fn print_json_or_receipt(receipt: &tool_gateway::PlatformDeliveryReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt).map_err(|source| crate::error::Error::Json { source })?);
        return Ok(());
    }

    if receipt.status == "success" {
        println!(
            "tool gateway: {} delivery receipt via {} ({})",
            receipt.artifact_type, receipt.backend, receipt.target_kind
        );
    } else {
        println!(
            "tool gateway: unsupported {} delivery ({})",
            receipt.target_kind,
            receipt.error_message.as_deref().unwrap_or("unsupported target")
        );
    }
    Ok(())
}
