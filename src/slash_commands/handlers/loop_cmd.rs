//! `/loop` slash command — start, stop, and check loop status.
//!
//! Starts an event-loop-level loop that re-sends the prompt after each
//! agent turn until a break condition is met, max iterations reached,
//! or the LLM calls `signal_loop_success`.

use super::SlashContext;
use super::SlashHandler;

pub struct LoopHandler;

impl SlashHandler for LoopHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "loop",
            description: "Run a prompt in a loop",
            help: "Usage:\n  \
                   /loop <N> <prompt>           — run prompt N times\n  \
                   /loop until <cond> <prompt>  — run until condition matches\n  \
                   /loop stop                   — stop the active loop\n  \
                   /loop pause                  — pause/resume the active loop\n  \
                   /loop status                 — show loop status\n\n\
                   Break conditions:\n  \
                   contains:<text>    — output contains text (default)\n  \
                   not_contains:<text> — output does NOT contain text\n  \
                   exit:<code>        — exit code matches\n  \
                   equals:<text>      — output equals text exactly\n  \
                   regex:<pattern>    — output matches pattern\n  \
                   <bare text>        — same as contains:<text>\n\n\
                   The LLM can also call signal_loop_success to break out.\n\n\
                   Examples:\n  \
                   /loop 5 run cargo test and fix any failures\n  \
                   /loop until \"0 failed\" run cargo test and fix failures\n  \
                   /loop until exit:0 run cargo test and fix failures",
            accepts_args: true,
            subcommands: vec![
                ("stop", "stop the active loop"),
                ("status", "show loop status"),
                ("pause", "pause/resume the active loop"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let args = args.trim();

        if args.is_empty() {
            // Show status
            show_status(ctx);
            return;
        }

        match args {
            "stop" | "cancel" | "abort" => {
                stop_loop(ctx);
            }
            "status" => {
                show_status(ctx);
            }
            "pause" | "resume" | "toggle" => {
                toggle_pause(ctx);
            }
            _ => {
                start_loop(args, ctx);
            }
        }
    }
}

fn show_status(ctx: &mut SlashContext<'_>) {
    match &ctx.app.loop_status {
        Some(ls) => {
            let state = if ls.active { "running" } else { "paused" };
            ctx.app.push_system(
                format!("Loop '{}': {} ({}/{})", ls.name, state, ls.iteration, ls.max_iterations),
                false,
            );
        }
        None => {
            ctx.app.push_system("No active loop.".into(), false);
        }
    }
}

fn stop_loop(ctx: &mut SlashContext<'_>) {
    if ctx.app.loop_status.is_some() {
        ctx.app.loop_status = None;
        ctx.app.push_system("Loop stopped.".into(), false);
    } else {
        ctx.app.push_system("No active loop.".into(), false);
    }
}

fn toggle_pause(ctx: &mut SlashContext<'_>) {
    use crate::modes::interactive::AgentCommand;

    let Some(ref mut ls) = ctx.app.loop_status else {
        ctx.app.push_system("No active loop.".into(), false);
        return;
    };

    ls.active = !ls.active;
    let now_active = ls.active;
    let name = ls.name.clone();
    let iteration = ls.iteration;
    let max = ls.max_iterations;
    let prompt = ls.prompt.clone();

    if now_active {
        // Resuming — kick off the next iteration
        ctx.app.push_system(format!("Loop '{name}' resumed ({iteration}/{max})."), false);
        if let Some(prompt) = prompt {
            let _ = ctx.cmd_tx.send(AgentCommand::ResetCancel);
            let _ = ctx.cmd_tx.send(AgentCommand::Prompt(prompt));
        }
    } else {
        ctx.app.push_system(format!("Loop '{name}' paused ({iteration}/{max})."), false);
    }
}

fn start_loop(args: &str, ctx: &mut SlashContext<'_>) {
    use crate::modes::interactive::AgentCommand;

    if ctx.app.loop_status.is_some() {
        ctx.app.push_system("A loop is already active. Use /loop stop first.".into(), true);
        return;
    }

    // Parse: /loop <N> <prompt>  or  /loop until <text> <prompt>
    let (max_iterations, break_text, prompt) = if let Some(rest) = args.strip_prefix("until ") {
        // /loop until "PASS" run cargo test
        let (break_text, prompt) = parse_until_args(rest);
        (100u32, Some(break_text), prompt)
    } else {
        // /loop <N> <prompt>
        match args.split_once(char::is_whitespace) {
            Some((n_str, prompt)) => {
                if let Ok(n) = n_str.parse::<u32>() {
                    (n, None, prompt.trim().to_string())
                } else {
                    // Bare prompt, default to 10 iterations
                    (10, None, args.to_string())
                }
            }
            None => {
                if args.parse::<u32>().is_ok() {
                    ctx.app.push_system("Missing prompt. Usage: /loop <N> <prompt>".into(), true);
                    return;
                }
                (10, None, args.to_string())
            }
        }
    };

    if prompt.is_empty() {
        ctx.app.push_system("Missing prompt. Usage: /loop <N> <prompt>".into(), true);
        return;
    }

    // Set loop display state on the app. The EventLoopRunner reads this
    // to drive iterations. This keeps SlashContext simple — no new channels.
    ctx.app.loop_status = Some(clankers_tui_types::LoopDisplayState {
        iteration: 0,
        max_iterations,
        name: truncate_name(&prompt, 30),
        active: true,
        break_text: break_text.clone(),
        prompt: Some(prompt.clone()),
    });

    // Store the loop config in a well-known place the event loop can read.
    // We use the queued_prompt mechanism: the prompt goes out immediately
    // as a normal agent turn, and the event loop re-sends it on PromptDone.
    ctx.app.push_system(
        format!(
            "Loop started: {} iteration(s){}",
            max_iterations,
            break_text
                .as_ref()
                .map(|t| format!(" (break on: \"{}\")", t))
                .unwrap_or_default(),
        ),
        false,
    );

    // Send the first iteration
    let _ = ctx.cmd_tx.send(AgentCommand::ResetCancel);
    let _ = ctx.cmd_tx.send(AgentCommand::Prompt(prompt));
}

/// Parse "until" args: either `"quoted text" rest` or `word rest`.
fn parse_until_args(s: &str) -> (String, String) {
    let s = s.trim();
    if s.starts_with('"') {
        // Find closing quote
        if let Some(end) = s[1..].find('"') {
            let break_text = s[1..=end].to_string();
            let prompt = s[end + 2..].trim().to_string();
            return (break_text, prompt);
        }
    }
    // No quotes — first word is the break text
    match s.split_once(char::is_whitespace) {
        Some((word, rest)) => (word.to_string(), rest.trim().to_string()),
        None => (s.to_string(), String::new()),
    }
}

/// Truncate a string for display as a loop name.
fn truncate_name(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
