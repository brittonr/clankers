//! Branching slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;
use crate::provider::message::MessageId;

pub struct ForkHandler;

impl SlashHandler for ForkHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if let Some(sm) = ctx.session_manager {
            if sm.message_count() == 0 {
                ctx.app.push_system("Cannot fork: no messages yet.".to_string(), true);
            } else {
                let reason = if args.is_empty() {
                    let ts = chrono::Utc::now().format("%Y-%m-%d-%H:%M");
                    format!("branch-{}", ts)
                } else {
                    args.to_string()
                };
                // The fork point is the current active leaf
                if let Some(fork_point) = sm.active_leaf_id().cloned() {
                    match sm.record_branch(fork_point.clone(), &reason) {
                        Ok(()) => {
                            ctx.app.push_system(
                                format!("Forked at message {}. Branch: \"{}\"", fork_point, reason),
                                false,
                            );
                            // Rebuild agent context from the new branch head
                            if let Ok(context) = sm.build_context() {
                                let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
                                let _ = ctx.cmd_tx.send(AgentCommand::SeedMessages(context));
                            }
                        }
                        Err(e) => {
                            ctx.app.push_system(format!("Fork failed: {}", e), true);
                        }
                    }
                } else {
                    ctx.app.push_system("Cannot fork: no active message.".to_string(), true);
                }
            }
        } else {
            ctx.app.push_system("No active session.".to_string(), true);
        }
    }
}

pub struct RewindHandler;

impl SlashHandler for RewindHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /rewind <N> or /rewind <message-id> or /rewind <label>".to_string(), true);
        } else if let Some(sm) = ctx.session_manager {
            match sm.resolve_target(args) {
                Ok(target_id) => {
                    match sm.set_active_head(target_id.clone()) {
                        Ok(()) => {
                            if let Ok(context) = sm.build_context() {
                                let msg_count = context.len();
                                let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
                                let _ = ctx.cmd_tx.send(AgentCommand::SeedMessages(context));
                                ctx.app.push_system(
                                    format!("Rewound to message {} ({} messages in context)", target_id, msg_count),
                                    false,
                                );
                            }
                        }
                        Err(e) => ctx.app.push_system(format!("Rewind failed: {}", e), true),
                    }
                }
                Err(e) => ctx.app.push_system(format!("Cannot resolve target '{}': {}", args, e), true),
            }
        } else {
            ctx.app.push_system("No active session.".to_string(), true);
        }
    }
}

pub struct BranchesHandler;

impl SlashHandler for BranchesHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        if let Some(sm) = ctx.session_manager {
            match sm.find_branches() {
                Ok(branches) => {
                    if branches.len() <= 1 {
                        ctx.app.push_system(
                            "No forks. Use /fork to explore alternatives.".to_string(),
                            false,
                        );
                    } else {
                        let mut output = String::from("Branches:\n\n");
                        for branch in &branches {
                            let marker = if branch.is_active { " *" } else { "  " };
                            let active_label = if branch.is_active { " (current)" } else { "" };
                            let ago = crate::modes::interactive::format_time_ago(branch.last_activity);
                            output.push_str(&format!(
                                "{} {}{}\n    {} messages    {}\n",
                                marker, branch.name, active_label, branch.message_count, ago,
                            ));
                        }
                        output.push_str("\n  Use /switch <name> to change branches");
                        ctx.app.push_system(output, false);
                    }
                }
                Err(e) => ctx.app.push_system(format!("Failed to list branches: {}", e), true),
            }
        } else {
            ctx.app.push_system("No active session.".to_string(), true);
        }
    }
}

pub struct SwitchHandler;

impl SlashHandler for SwitchHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /switch <branch-name> or /switch <message-id>".to_string(), true);
        } else if let Some(sm) = ctx.session_manager {
            // First try to resolve as a branch name
            let target = sm.find_branches().ok().and_then(|branches| {
                branches
                    .iter()
                    .find(|b| b.name.eq_ignore_ascii_case(args))
                    .map(|b| b.leaf_id.clone())
            });
            // Fall back to resolving as message ID
            let target = target.or_else(|| sm.resolve_target(args).ok());
            match target {
                Some(target_id) => {
                    if sm.active_leaf_id() == Some(&target_id) {
                        let branch_name = sm.find_branches().ok()
                            .and_then(|bs| bs.iter().find(|b| b.leaf_id == target_id).map(|b| b.name.clone()))
                            .unwrap_or_else(|| target_id.to_string());
                        ctx.app.push_system(format!("Already on branch \"{}\"", branch_name), false);
                    } else {
                        match sm.set_active_head(target_id.clone()) {
                            Ok(()) => {
                                if let Ok(context) = sm.build_context() {
                                    let msg_count = context.len();
                                    let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
                                    let _ = ctx.cmd_tx.send(AgentCommand::SeedMessages(context));
                                    let branch_name = sm.find_branches().ok()
                                        .and_then(|bs| bs.iter().find(|b| b.leaf_id == target_id).map(|b| b.name.clone()))
                                        .unwrap_or_else(|| target_id.to_string());
                                    ctx.app.push_system(
                                        format!("Switched to branch \"{}\" ({} messages)", branch_name, msg_count),
                                        false,
                                    );
                                }
                            }
                            Err(e) => ctx.app.push_system(format!("Switch failed: {}", e), true),
                        }
                    }
                }
                None => {
                    let available = sm.find_branches().ok()
                        .map(|bs| bs.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", "))
                        .unwrap_or_default();
                    ctx.app.push_system(
                        format!("Branch '{}' not found. Available: {}", args, available),
                        true,
                    );
                }
            }
        } else {
            ctx.app.push_system("No active session.".to_string(), true);
        }
    }
}

pub struct CompareHandler;

impl SlashHandler for CompareHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() != 2 {
            ctx.app.push_system("Usage: /compare <block-id-a> <block-id-b>  (e.g. /compare #1 #3)".to_string(), true);
            return;
        }

        let parse_id = |s: &str| -> Option<usize> {
            s.strip_prefix('#').unwrap_or(s).parse().ok()
        };

        let id_a = match parse_id(parts[0]) {
            Some(id) => id,
            None => {
                ctx.app.push_system(format!("Invalid block ID: {}", parts[0]), true);
                return;
            }
        };
        let id_b = match parse_id(parts[1]) {
            Some(id) => id,
            None => {
                ctx.app.push_system(format!("Invalid block ID: {}", parts[1]), true);
                return;
            }
        };

        if id_a == id_b {
            ctx.app.push_system("Cannot compare a branch with itself.".to_string(), true);
            return;
        }

        // Verify both blocks exist
        let a_exists = ctx.app.all_blocks.iter().any(|b| b.id == id_a);
        let b_exists = ctx.app.all_blocks.iter().any(|b| b.id == id_b);
        if !a_exists {
            ctx.app.push_system(format!("Block #{} not found.", id_a), true);
            return;
        }
        if !b_exists {
            ctx.app.push_system(format!("Block #{} not found.", id_b), true);
            return;
        }

        ctx.app.branch_compare.open(id_a, id_b, &ctx.app.all_blocks.clone());
    }
}

pub struct MergeHandler;

impl SlashHandler for MergeHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() != 2 {
            ctx.app.push_system(
                "Usage: /merge <source-branch> <target-branch>".to_string(),
                true,
            );
            return;
        }

        let Some(sm) = ctx.session_manager else {
            ctx.app.push_system("No active session.".to_string(), true);
            return;
        };

        let branches = match sm.find_branches() {
            Ok(b) => b,
            Err(e) => {
                ctx.app.push_system(format!("Failed to list branches: {}", e), true);
                return;
            }
        };

        // Resolve source and target branch names to leaf IDs
        let source_leaf = branches.iter().find(|b| b.name.eq_ignore_ascii_case(parts[0])).map(|b| b.leaf_id.clone())
            .or_else(|| sm.resolve_target(parts[0]).ok());
        let target_leaf = branches.iter().find(|b| b.name.eq_ignore_ascii_case(parts[1])).map(|b| b.leaf_id.clone())
            .or_else(|| sm.resolve_target(parts[1]).ok());

        let Some(source) = source_leaf else {
            let available = branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", ");
            ctx.app.push_system(format!("Source branch '{}' not found. Available: {}", parts[0], available), true);
            return;
        };
        let Some(target) = target_leaf else {
            let available = branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", ");
            ctx.app.push_system(format!("Target branch '{}' not found. Available: {}", parts[1], available), true);
            return;
        };

        match sm.merge_branch(source, target) {
            Ok((count, _new_leaf)) => {
                // Rebuild agent context from the merged branch
                if let Ok(context) = sm.build_context() {
                    let msg_count = context.len();
                    let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
                    let _ = ctx.cmd_tx.send(AgentCommand::SeedMessages(context));
                    ctx.app.push_system(
                        format!("Merged {} messages from \"{}\" into \"{}\" ({} messages in context)", count, parts[0], parts[1], msg_count),
                        false,
                    );
                }
            }
            Err(e) => ctx.app.push_system(format!("Merge failed: {}", e), true),
        }
    }
}

pub struct MergeInteractiveHandler;

impl SlashHandler for MergeInteractiveHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() != 2 {
            ctx.app.push_system(
                "Usage: /merge-interactive <source-branch> <target-branch>".to_string(),
                true,
            );
            return;
        }

        let Some(sm) = ctx.session_manager.as_mut() else {
            ctx.app.push_system("No active session.".to_string(), true);
            return;
        };

        let branches = match sm.find_branches() {
            Ok(b) => b,
            Err(e) => {
                ctx.app.push_system(format!("Failed to list branches: {}", e), true);
                return;
            }
        };

        // Resolve source and target branch names to leaf IDs
        let source_leaf = branches
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(parts[0]))
            .map(|b| b.leaf_id.clone())
            .or_else(|| sm.resolve_target(parts[0]).ok());
        let target_leaf = branches
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(parts[1]))
            .map(|b| b.leaf_id.clone())
            .or_else(|| sm.resolve_target(parts[1]).ok());

        let Some(source) = source_leaf else {
            let available = branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", ");
            ctx.app.push_system(
                format!("Source branch '{}' not found. Available: {}", parts[0], available),
                true,
            );
            return;
        };
        let Some(target) = target_leaf else {
            let available = branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", ");
            ctx.app.push_system(
                format!("Target branch '{}' not found. Available: {}", parts[1], available),
                true,
            );
            return;
        };

        if source == target {
            ctx.app.push_system("Cannot merge a branch into itself.".to_string(), true);
            return;
        }

        // Load unique messages
        let tree = match sm.load_tree() {
            Ok(t) => t,
            Err(e) => {
                ctx.app.push_system(format!("Failed to load session tree: {}", e), true);
                return;
            }
        };

        let unique = tree.find_unique_messages(&source, &target);
        if unique.is_empty() {
            ctx.app.push_system(
                "No unique messages to merge — branches share the same content.".to_string(),
                false,
            );
            return;
        }

        let source_name = branches
            .iter()
            .find(|b| b.leaf_id == source)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| source.to_string());
        let target_name = branches
            .iter()
            .find(|b| b.leaf_id == target)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| target.to_string());

        ctx.app
            .merge_interactive
            .open(source, target, &source_name, &target_name, &unique);
    }
}

pub struct CherryPickHandler;

impl SlashHandler for CherryPickHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() < 2 {
            ctx.app.push_system(
                "Usage: /cherry-pick <message-id> <target-branch> [--with-children]".to_string(),
                true,
            );
            return;
        }

        let Some(sm) = ctx.session_manager else {
            ctx.app.push_system("No active session.".to_string(), true);
            return;
        };

        let with_children = parts.contains(&"--with-children");
        let msg_id = MessageId::new(parts[0]);

        // Resolve target branch
        let branches = match sm.find_branches() {
            Ok(b) => b,
            Err(e) => {
                ctx.app.push_system(format!("Failed to list branches: {}", e), true);
                return;
            }
        };

        let target_leaf = branches.iter().find(|b| b.name.eq_ignore_ascii_case(parts[1])).map(|b| b.leaf_id.clone())
            .or_else(|| sm.resolve_target(parts[1]).ok());

        let Some(target) = target_leaf else {
            let available = branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", ");
            ctx.app.push_system(format!("Target branch '{}' not found. Available: {}", parts[1], available), true);
            return;
        };

        match sm.cherry_pick(msg_id, target, with_children) {
            Ok((count, _new_leaf)) => {
                if let Ok(context) = sm.build_context() {
                    let msg_count = context.len();
                    let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
                    let _ = ctx.cmd_tx.send(AgentCommand::SeedMessages(context));
                    let suffix = if with_children { " (with children)" } else { "" };
                    ctx.app.push_system(
                        format!("Cherry-picked {} message(s){} into \"{}\" ({} messages in context)", count, suffix, parts[1], msg_count),
                        false,
                    );
                }
            }
            Err(e) => ctx.app.push_system(format!("Cherry-pick failed: {}", e), true),
        }
    }
}

pub struct LabelHandler;

impl SlashHandler for LabelHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /label <name>".to_string(), true);
        } else if let Some(sm) = ctx.session_manager {
            match sm.record_label(args) {
                Ok(()) => {
                    if let Some(head) = sm.active_leaf_id() {
                        ctx.app.push_system(
                            format!("Labeled message {} as \"{}\"", head, args),
                            false,
                        );
                    }
                }
                Err(e) => ctx.app.push_system(format!("Label failed: {}", e), true),
            }
        } else {
            ctx.app.push_system("No active session.".to_string(), true);
        }
    }
}
