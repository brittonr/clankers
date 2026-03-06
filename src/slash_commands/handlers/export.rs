//! Export slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::tui::app::MessageRole;
use crate::tui::components::block::BlockEntry;

pub struct ExportHandler;

impl SlashHandler for ExportHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let filename = if args.is_empty() {
            format!("clankers-export-{}.md", chrono::Local::now().format("%Y%m%d-%H%M%S"))
        } else {
            args.to_string()
        };
        let mut content = String::new();
        for entry in &ctx.app.blocks {
            match entry {
                BlockEntry::Conversation(block) => {
                    content.push_str("## User\n");
                    content.push_str(&block.prompt);
                    content.push_str("\n\n");
                    for msg in &block.responses {
                        let label = match msg.role {
                            MessageRole::Assistant => "## Assistant",
                            MessageRole::ToolCall => "## Tool Call",
                            MessageRole::ToolResult => "## Tool Result",
                            MessageRole::Thinking => "## Thinking",
                            _ => "## Other",
                        };
                        content.push_str(label);
                        content.push('\n');
                        content.push_str(&msg.content);
                        content.push_str("\n\n");
                    }
                }
                BlockEntry::System(msg) => {
                    content.push_str("## System\n");
                    content.push_str(&msg.content);
                    content.push_str("\n\n");
                }
            }
        }
        let file_path = std::path::Path::new(&filename);
        // If the filename is just a bare name (no directory components), place it in .clankers/exports/
        let resolved = if file_path.parent().is_none_or(|p| p.as_os_str().is_empty()) {
            let cwd_path = std::path::Path::new(&ctx.app.cwd);
            let exports_dir = cwd_path.join(".clankers").join("exports");
            std::fs::create_dir_all(&exports_dir).ok();
            crate::util::fs::ensure_gitignore_entry(cwd_path, ".clankers/exports");
            exports_dir.join(&filename)
        } else {
            std::path::Path::new(&ctx.app.cwd).join(&filename)
        };
        match std::fs::write(&resolved, &content) {
            Ok(()) => {
                ctx.app.push_system(format!("Exported to: {}", resolved.display()), false);
            }
            Err(e) => {
                ctx.app.push_system(format!("Export failed: {}", e), true);
            }
        }
    }
}
