//! Layout definitions (KDL) for agent workspaces

use std::path::Path;
use std::path::PathBuf;

/// Default layout (single agent pane — no command, plain shell)
pub const DEFAULT_LAYOUT: &str = r#"layout {
    pane name="main" focus=true
}
"#;

/// Swarm layout (plain shell — no embedded commands)
pub const SWARM_LAYOUT: &str = r#"layout {
    pane split_direction="vertical" {
        pane name="main" size="65%" focus=true
        pane split_direction="horizontal" size="35%" {
            pane stacked=true name="workers" size="80%"
            pane size="20%" name="merge-daemon"
        }
    }
}
"#;

/// Generate a default layout with the clankers command embedded in the main pane.
///
/// Layout: main pane (67%) on the left, stacked subagent area (33%) on the right.
///
/// `clankers_cmd` is the executable (e.g. `"cargo"` or `"/usr/bin/clankers"`).
/// `prefix_args` are args before clankers flags (e.g. `["run", "--quiet", "--"]` for cargo).
/// `extra_args` are CLI flags to forward (e.g. `--model`, `--agent`, etc.).
/// `--no-zellij` is always added to prevent re-launch loops.
pub fn default_layout_with_command(clankers_cmd: &str, prefix_args: &[String], extra_args: &[String]) -> String {
    let mut all_args: Vec<String> = prefix_args.to_vec();
    all_args.push("--no-zellij".to_string());
    all_args.extend(extra_args.iter().cloned());
    let args_kdl = format_kdl_args(&all_args);

    format!(
        r#"layout {{
    pane split_direction="vertical" {{
        pane name="main" size="67%" focus=true {{
            command "{cmd}"
            {args_kdl}
        }}
        pane stacked=true name="subagents" size="33%"
    }}
}}
"#,
        cmd = escape_kdl(clankers_cmd),
        args_kdl = args_kdl,
    )
}

/// Generate a swarm layout with the clankers command in the main pane
/// and the merge daemon in its own pane.
pub fn swarm_layout_with_command(clankers_cmd: &str, prefix_args: &[String], extra_args: &[String]) -> String {
    let mut main_args: Vec<String> = prefix_args.to_vec();
    main_args.push("--no-zellij".to_string());
    main_args.extend(extra_args.iter().cloned());
    let main_args_kdl = format_kdl_args(&main_args);

    let mut daemon_args: Vec<String> = prefix_args.to_vec();
    daemon_args.push("merge-daemon".to_string());
    let daemon_args_kdl = format_kdl_args(&daemon_args);

    format!(
        r#"layout {{
    pane split_direction="vertical" {{
        pane name="main" size="65%" focus=true {{
            command "{cmd}"
            {main_args_kdl}
        }}
        pane split_direction="horizontal" size="35%" {{
            pane stacked=true name="workers" size="80%"
            pane size="20%" name="merge-daemon" {{
                command "{cmd}"
                {daemon_args_kdl}
            }}
        }}
    }}
}}
"#,
        cmd = escape_kdl(clankers_cmd),
        main_args_kdl = main_args_kdl,
        daemon_args_kdl = daemon_args_kdl,
    )
}

/// Format args as a KDL `args` line: `args "--no-zellij" "--model" "claude-sonnet-4-20250514"`
fn format_kdl_args(args: &[String]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let quoted: Vec<String> = args.iter().map(|a| format!("\"{}\"", escape_kdl(a))).collect();
    format!("args {}", quoted.join(" "))
}

/// Escape a string for embedding in KDL (double quotes)
fn escape_kdl(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Write layout to a temp file and return the path
pub fn write_temp_layout(content: &str) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join("clankers-layouts");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("layout-{}.kdl", crate::util::id::generate_id()));
    std::fs::write(&path, content)?;
    Ok(path)
}

/// Load a custom layout from the user's layout directory
pub fn load_custom_layout(layouts_dir: &Path, name: &str) -> Option<String> {
    let path = layouts_dir.join(format!("{}.kdl", name));
    std::fs::read_to_string(&path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout_is_valid_kdl() {
        // Basic structural checks — KDL parsing requires a full parser,
        // but we can verify the layout contains expected elements
        assert!(DEFAULT_LAYOUT.contains("layout"));
        assert!(DEFAULT_LAYOUT.contains("pane"));
        assert!(DEFAULT_LAYOUT.contains("name=\"main\""));
        assert!(DEFAULT_LAYOUT.contains("focus=true"));
    }

    #[test]
    fn test_swarm_layout_has_workers_and_merge() {
        assert!(SWARM_LAYOUT.contains("layout"));
        assert!(SWARM_LAYOUT.contains("name=\"main\""));
        assert!(SWARM_LAYOUT.contains("name=\"workers\""));
        assert!(SWARM_LAYOUT.contains("name=\"merge-daemon\""));
        assert!(SWARM_LAYOUT.contains("stacked=true"));
    }

    #[test]
    fn test_write_temp_layout() {
        let path = write_temp_layout(DEFAULT_LAYOUT).expect("failed to write temp layout");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("failed to read layout file");
        assert_eq!(content, DEFAULT_LAYOUT);
        std::fs::remove_file(&path).expect("failed to remove temp layout file");
    }

    #[test]
    fn test_write_temp_layout_unique_paths() {
        let p1 = write_temp_layout(DEFAULT_LAYOUT).expect("failed to write first layout");
        let p2 = write_temp_layout(DEFAULT_LAYOUT).expect("failed to write second layout");
        assert_ne!(p1, p2);
        std::fs::remove_file(&p1).expect("failed to remove first layout");
        std::fs::remove_file(&p2).expect("failed to remove second layout");
    }

    #[test]
    fn test_load_custom_layout_missing() {
        let result = load_custom_layout(Path::new("/nonexistent"), "missing");
        assert!(result.is_none());
    }

    #[test]
    fn test_load_custom_layout_present() {
        let dir = std::env::temp_dir().join("clankers-layout-test");
        std::fs::create_dir_all(&dir).expect("failed to create test directory");
        let layout_path = dir.join("test.kdl");
        std::fs::write(&layout_path, "layout { pane }").expect("failed to write test layout");

        let result = load_custom_layout(&dir, "test");
        assert_eq!(result, Some("layout { pane }".to_string()));

        std::fs::remove_dir_all(&dir).expect("failed to remove test directory");
    }

    // ── Production mode (direct binary) ────────────────────────────

    #[test]
    fn test_default_layout_with_command_no_extra_args() {
        let layout = default_layout_with_command("/usr/bin/clankers", &[], &[]);
        assert!(layout.contains("command \"/usr/bin/clankers\""));
        assert!(layout.contains("\"--no-zellij\""));
        assert!(layout.contains("name=\"main\""));
        assert!(layout.contains("focus=true"));
    }

    #[test]
    fn test_default_layout_with_command_forwards_args() {
        let args = vec![
            "--model".to_string(),
            "claude-sonnet-4-20250514".to_string(),
            "--agent".to_string(),
            "coder".to_string(),
        ];
        let layout = default_layout_with_command("/usr/bin/clankers", &[], &args);
        assert!(layout.contains("\"--no-zellij\""), "layout: {}", layout);
        assert!(layout.contains("\"--model\""), "layout: {}", layout);
        assert!(layout.contains("\"claude-sonnet-4-20250514\""), "layout: {}", layout);
        assert!(layout.contains("\"--agent\""), "layout: {}", layout);
        assert!(layout.contains("\"coder\""), "layout: {}", layout);
    }

    #[test]
    fn test_swarm_layout_with_command() {
        let layout = swarm_layout_with_command("/usr/bin/clankers", &[], &[]);
        assert!(layout.contains("command \"/usr/bin/clankers\""));
        assert!(layout.contains("\"--no-zellij\""));
        assert!(layout.contains("name=\"main\""));
        assert!(layout.contains("name=\"workers\""));
        assert!(layout.contains("name=\"merge-daemon\""));
        assert!(layout.contains("\"merge-daemon\""));
    }

    #[test]
    fn test_swarm_layout_with_command_forwards_args() {
        let args = vec!["--thinking".to_string()];
        let layout = swarm_layout_with_command("/usr/bin/clankers", &[], &args);
        assert!(layout.contains("\"--no-zellij\""));
        assert!(layout.contains("\"--thinking\""));
    }

    // ── Dev mode (cargo run) ─────────────────────────────────────

    #[test]
    fn test_default_layout_dev_mode_uses_cargo() {
        let prefix = vec![
            "run".to_string(),
            "--manifest-path=/home/user/clankers/Cargo.toml".to_string(),
            "--quiet".to_string(),
            "--".to_string(),
        ];
        let layout = default_layout_with_command("cargo", &prefix, &[]);
        assert!(layout.contains("command \"cargo\""), "layout:\n{}", layout);
        assert!(layout.contains("\"run\""), "layout:\n{}", layout);
        assert!(layout.contains("\"--quiet\""), "layout:\n{}", layout);
        assert!(layout.contains("\"--\""), "layout:\n{}", layout);
        assert!(layout.contains("\"--no-zellij\""), "layout:\n{}", layout);
    }

    #[test]
    fn test_default_layout_dev_mode_args_order() {
        let prefix = vec!["run".to_string(), "--quiet".to_string(), "--".to_string()];
        let extra = vec!["--model".to_string(), "test".to_string()];
        let layout = default_layout_with_command("cargo", &prefix, &extra);
        // Args line should be: args "run" "--quiet" "--" "--no-zellij" "--model" "test"
        let args_line = layout.lines().find(|l| l.trim().starts_with("args")).expect("args line not found in layout");
        let run_pos = args_line.find("\"run\"").expect("run arg not found");
        let no_zellij_pos = args_line.find("\"--no-zellij\"").expect("--no-zellij arg not found");
        let model_pos = args_line.find("\"--model\"").expect("--model arg not found");
        assert!(run_pos < no_zellij_pos, "prefix args should come before --no-zellij");
        assert!(no_zellij_pos < model_pos, "--no-zellij should come before extra args");
    }

    #[test]
    fn test_swarm_layout_dev_mode_merge_daemon() {
        let prefix = vec!["run".to_string(), "--quiet".to_string(), "--".to_string()];
        let layout = swarm_layout_with_command("cargo", &prefix, &[]);
        assert!(layout.contains("command \"cargo\""), "layout:\n{}", layout);
        // Main pane: cargo run --quiet -- --no-zellij
        assert!(layout.contains("\"--no-zellij\""), "layout:\n{}", layout);
        // Merge daemon pane: cargo run --quiet -- merge-daemon
        assert!(layout.contains("\"merge-daemon\""), "layout:\n{}", layout);
    }

    #[test]
    fn test_escape_kdl() {
        assert_eq!(escape_kdl("simple"), "simple");
        assert_eq!(escape_kdl("has \"quotes\""), "has \\\"quotes\\\"");
        assert_eq!(escape_kdl("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn test_format_kdl_args_empty() {
        assert_eq!(format_kdl_args(&[]), "");
    }

    #[test]
    fn test_format_kdl_args_single() {
        let args = vec!["--no-zellij".to_string()];
        assert_eq!(format_kdl_args(&args), "args \"--no-zellij\"");
    }

    #[test]
    fn test_format_kdl_args_multiple() {
        let args = vec!["--model".to_string(), "gpt-4".to_string()];
        assert_eq!(format_kdl_args(&args), "args \"--model\" \"gpt-4\"");
    }
}
