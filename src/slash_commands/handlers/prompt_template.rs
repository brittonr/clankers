//! Prompt Template slash command handlers.

use super::SlashContext;
use super::SlashHandler;

pub struct PromptTemplateHandler {
    pub template_name: String,
}

impl SlashHandler for PromptTemplateHandler {
    fn command(&self) -> super::super::SlashCommand {
        // PromptTemplateHandler is dynamic — command metadata depends on the template.
        // We return a placeholder. The real metadata is discovered at runtime.
        super::super::SlashCommand {
            name: Box::leak(self.template_name.clone().into_boxed_str()),
            description: "User-defined prompt template",
            help: "Executes a custom prompt template from ~/.pi/prompts/ or .pi/prompts/",
            accepts_args: true,
            subcommands: vec![],
            leader_key: None,
        }
    }
    
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        // Look up the prompt template from the discovered resources
        let global_dir = &crate::config::paths::ClankersPaths::get().global_prompts_dir;
        let project_dir =
            crate::config::paths::ProjectPaths::resolve(&std::env::current_dir().unwrap_or_default()).prompts_dir;
        let prompts = crate::prompts::discover_prompts(global_dir, Some(&project_dir));
        if let Some(template) = prompts.iter().find(|p| p.name == self.template_name) {
            let mut vars = std::collections::HashMap::new();
            vars.insert("input".to_string(), args.to_string());
            let expanded = crate::prompts::expand_template(&template.content, &vars);
            // Strip frontmatter before sending
            let prompt = crate::modes::interactive::strip_frontmatter(&expanded);
            ctx.app.push_system(format!("/{} — {}", self.template_name, template.description), false);
            ctx.app.queued_prompt = Some(prompt);
        } else {
            ctx.app.push_system(format!("Unknown command or prompt template: /{}", self.template_name), true);
        }
    }
}
