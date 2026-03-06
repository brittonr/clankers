//! Model slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;

pub struct ModelHandler;

impl SlashHandler for ModelHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system(format!("Current model: {}\n\nUsage: /model <model-name>", ctx.app.model), false);
        } else {
            let old_model = std::mem::replace(&mut ctx.app.model, args.to_string());
            let _ = ctx.cmd_tx.send(AgentCommand::SetModel(args.to_string()));
            ctx.app.context_gauge.set_model(&ctx.app.model);
            ctx.app.push_system(format!("Model switched: {} → {}", old_model, ctx.app.model), false);
        }
    }
}

pub struct ThinkHandler;

impl SlashHandler for ThinkHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            // No args: cycle to next level
            let _ = ctx.cmd_tx.send(AgentCommand::CycleThinkingLevel);
        } else if let Some(level) = crate::provider::ThinkingLevel::from_str_or_budget(args) {
            // Named level: /think off, /think low, /think high, etc.
            let _ = ctx.cmd_tx.send(AgentCommand::SetThinkingLevel(level));
        } else if let Ok(budget) = args.trim().parse::<usize>() {
            // Raw number: /think 20000 → find closest level
            let level = crate::provider::ThinkingLevel::from_budget(budget);
            let _ = ctx.cmd_tx.send(AgentCommand::SetThinkingLevel(level));
        } else {
            ctx.app.push_system("Usage: /think [off|low|medium|high|max] or /think <budget>".to_string(), true);
        }
    }
}

pub struct RoleHandler;

impl SlashHandler for RoleHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            // List all roles with their resolved models
            let roles_config = crate::config::model_roles::ModelRolesConfig::with_defaults(&ctx.app.model);
            let mut roles_info = String::from("Model Roles:\n\n");
            roles_info.push_str(&roles_config.summary(&ctx.app.model));
            roles_info.push_str("\n\nUsage:\n  /role <name>           Switch to a role's model");
            roles_info.push_str("\n  /role <name> <model>   Set a role's model and switch");
            roles_info.push_str("\n  /role reset            Clear all role overrides");
            ctx.app.push_system(roles_info, false);
        } else {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts[0] == "reset" {
                ctx.app.push_system("Model role overrides cleared.".to_string(), false);
            } else if let Some(role) = crate::config::model_roles::ModelRole::parse(parts[0]) {
                let roles_config = crate::config::model_roles::ModelRolesConfig::with_defaults(&ctx.app.model);
                if parts.len() > 1 {
                    // Set role to specific model and switch to it now
                    let model_name = parts[1].to_string();
                    let old_model = std::mem::replace(&mut ctx.app.model, model_name.clone());
                    let _ = ctx.cmd_tx.send(AgentCommand::SetModel(model_name.clone()));
                    ctx.app.context_gauge.set_model(&ctx.app.model);
                    ctx.app.push_system(
                        format!("Role '{}' → {} (switched: {} → {})", role, model_name, old_model, ctx.app.model),
                        false,
                    );
                } else {
                    // Switch to the model assigned to this role
                    let target_model = roles_config.resolve(role, &ctx.app.model);
                    if target_model == ctx.app.model {
                        ctx.app.push_system(format!("Already using '{}' model: {}", role, target_model), false);
                    } else {
                        let old_model = std::mem::replace(&mut ctx.app.model, target_model.clone());
                        let _ = ctx.cmd_tx.send(AgentCommand::SetModel(target_model.clone()));
                        ctx.app.context_gauge.set_model(&ctx.app.model);
                        ctx.app.push_system(format!("Role '{}': {} → {}", role, old_model, target_model), false);
                    }
                }
            } else {
                ctx.app.push_system(
                    format!("Unknown role: '{}'. Available: default, smol, slow, plan, commit, review", parts[0]),
                    true,
                );
            }
        }
    }
}
