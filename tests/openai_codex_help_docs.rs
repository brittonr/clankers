use clap::CommandFactory;

fn render_subcommand_help(path: &[&str]) -> String {
    let mut command = clankers::cli::Cli::command();
    let mut current = &mut command;
    for segment in path {
        current = current.find_subcommand_mut(segment).unwrap_or_else(|| panic!("missing subcommand: {segment}"));
    }

    let mut out = Vec::new();
    current.write_long_help(&mut out).expect("help should render");
    String::from_utf8(out).expect("help should be utf8")
}

#[test]
fn cli_auth_help_mentions_codex_account_naming_and_plan_limits() {
    let login_help = render_subcommand_help(&["auth", "login"]);
    assert!(login_help.contains("openai-codex"), "{login_help}");
    assert!(login_help.contains("Anthropic default"), "{login_help}");
    assert!(login_help.contains("--account"), "{login_help}");
    assert!(login_help.contains("Plus or Pro"), "{login_help}");
    assert!(login_help.contains("API-key openai"), "{login_help}");

    let status_help = render_subcommand_help(&["auth", "status"]);
    assert!(status_help.contains("openai-codex"), "{status_help}");
    assert!(status_help.contains("authenticated-but-not-entitled"), "{status_help}");
    assert!(status_help.contains("API-key openai"), "{status_help}");
}

#[test]
fn slash_auth_help_mentions_codex_model_selection_and_fail_closed_behavior() {
    let commands = clankers::slash_commands::builtin_commands();
    let login = commands.iter().find(|cmd| cmd.name == "login").expect("login command should exist");
    assert!(login.help.contains("openai-codex"), "{}", login.help);
    assert!(login.help.contains("--account <name>"), "{}", login.help);
    assert!(login.help.contains("Plus or Pro"), "{}", login.help);
    assert!(login.help.contains("/model openai-codex/gpt-5.3-codex"), "{}", login.help);
    assert!(login.help.contains("openai/gpt-4o"), "{}", login.help);

    let account = commands.iter().find(|cmd| cmd.name == "account").expect("account command should exist");
    assert!(account.help.contains("fail closed"), "{}", account.help);
    assert!(account.help.contains("openai-codex"), "{}", account.help);
    assert!(account.help.contains("API-key `openai`"), "{}", account.help);
}

#[test]
fn provider_docs_cover_codex_login_model_selection_and_openai_separation() {
    let combined = [
        include_str!("../README.md"),
        include_str!("../docs/src/getting-started/auth.md"),
        include_str!("../docs/src/reference/commands.md"),
    ]
    .join("\n");

    assert!(combined.contains("openai-codex"), "{combined}");
    assert!(combined.contains("--account"), "{combined}");
    assert!(combined.contains("openai-codex/gpt-5.3-codex"), "{combined}");
    assert!(combined.contains("openai/gpt-4o"), "{combined}");
    assert!(combined.contains("Plus or Pro"), "{combined}");
    assert!(combined.contains("authenticated but unavailable for Codex use"), "{combined}");
    assert!(combined.contains("fail closed"), "{combined}");
    assert!(combined.contains("API-key `openai`"), "{combined}");
}
