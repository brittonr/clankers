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
            let _ = ctx.cmd_tx.send(AgentCommand::CycleThinkingLevel);
        } else if let Some(level) = crate::provider::ThinkingLevel::from_str_or_budget(args) {
            let _ = ctx.cmd_tx.send(AgentCommand::SetThinkingLevel(level));
        } else if let Ok(budget) = args.trim().parse::<usize>() {
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
        let roles = crate::config::model_roles::ModelRoles::with_defaults();

        if args.is_empty() {
            let mut info = String::from("Model Roles:\n\n");
            info.push_str(&roles.summary(&ctx.app.model));
            info.push_str("\n\nUsage:\n  /role <name>           Switch to a role's model");
            info.push_str("\n  /role <name> <model>   Set a role's model and switch");
            info.push_str("\n  /role reset            Clear all role overrides");
            ctx.app.push_system(info, false);
            return;
        }

        let parts: Vec<&str> = args.splitn(2, ' ').collect();

        if parts[0] == "reset" {
            ctx.app.push_system("Model role overrides cleared.".to_string(), false);
            return;
        }

        if let Some(role) = roles.get(parts[0]) {
            if parts.len() > 1 {
                // Set role to specific model and switch
                let model_name = parts[1].to_string();
                let old_model = std::mem::replace(&mut ctx.app.model, model_name.clone());
                let _ = ctx.cmd_tx.send(AgentCommand::SetModel(model_name.clone()));
                ctx.app.context_gauge.set_model(&ctx.app.model);
                ctx.app.push_system(
                    format!("Role '{}' → {} (switched: {} → {})", role.name, model_name, old_model, ctx.app.model),
                    false,
                );
            } else {
                // Switch to the model assigned to this role
                let target_model = roles.resolve(&role.name, &ctx.app.model);
                if target_model == ctx.app.model {
                    ctx.app.push_system(format!("Already using '{}' model: {}", role.name, target_model), false);
                } else {
                    let old_model = std::mem::replace(&mut ctx.app.model, target_model.clone());
                    let _ = ctx.cmd_tx.send(AgentCommand::SetModel(target_model.clone()));
                    ctx.app.context_gauge.set_model(&ctx.app.model);
                    ctx.app.push_system(format!("Role '{}': {} → {}", role.name, old_model, target_model), false);
                }
            }
        } else {
            let available = roles.names().join(", ");
            ctx.app.push_system(
                format!("Unknown role: '{}'. Available: {}", parts[0], available),
                true,
            );
        }
    }
}
