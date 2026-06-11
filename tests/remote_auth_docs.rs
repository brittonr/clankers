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
fn remote_auth_docs_use_clap_accurate_token_and_attach_flags() {
    let token_create_help = render_subcommand_help(&["token", "create"]);
    for flag in ["--read-only", "--tools", "--expire", "--for", "--from", "--bot-commands", "--session-manage", "--delegate", "--root"] {
        assert!(token_create_help.contains(flag), "token create help missing {flag}:\n{token_create_help}");
    }

    let attach_help = render_subcommand_help(&["attach"]);
    assert!(attach_help.contains("--remote"), "attach help missing --remote:\n{attach_help}");

    let remote_auth = include_str!("../docs/src/reference/remote-auth.md");
    assert!(remote_auth.contains("clankers token create --read-only --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h"));
    assert!(remote_auth.contains("--from <PARENT_UCAN_ENVELOPE>"));
    assert!(remote_auth.contains("clankers attach --remote <REMOTE_NODE_ID>"));
    assert!(remote_auth.contains("clankers token revoke <TOKEN_HASH>"));
}

#[test]
fn remote_auth_entrypoints_name_public_ucan_basalt_and_legacy_boundary() {
    let combined = [
        include_str!("../README.md"),
        include_str!("../docs/src/getting-started/auth.md"),
        include_str!("../docs/src/reference/daemon.md"),
        include_str!("../docs/src/reference/remote-auth.md"),
    ]
    .join("\n");

    assert!(combined.contains("public UCAN"), "remote auth docs must name public UCAN");
    assert!(combined.contains("Basalt policy"), "remote auth docs must name Basalt policy");
    assert!(combined.contains("legacy `clanker-auth`"), "remote auth docs must explain legacy clanker-auth boundary");
    assert!(combined.contains("remote-auth.md"), "entrypoints should link the remote auth reference");

    let remote_auth = include_str!("../docs/src/reference/remote-auth.md");
    assert!(remote_auth.contains("remote attach"));
    assert!(remote_auth.contains("Matrix"));
    assert!(remote_auth.contains("chat/RPC"));
    assert!(remote_auth.contains("redacted receipts"));
}

#[test]
fn remote_auth_docs_do_not_embed_raw_token_or_key_material() {
    let remote_auth = include_str!("../docs/src/reference/remote-auth.md");
    let forbidden_fragments = [
        "sk-",
        "-----BEGIN",
        "PRIVATE KEY",
        "OPENAI_API_KEY=",
        "ANTHROPIC_API_KEY=",
        "auth.json",
        "eyJ",
    ];
    for fragment in forbidden_fragments {
        assert!(!remote_auth.contains(fragment), "remote auth reference must not embed secret-like fragment {fragment}");
    }

    for word in remote_auth.split(|character: char| character.is_whitespace() || matches!(character, '`' | '"' | '\'')) {
        let compact = word.trim_matches(|character: char| matches!(character, ',' | '.' | ':' | ';' | ')' | '('));
        let is_placeholder = compact.starts_with('<') && compact.ends_with('>');
        let looks_like_unredacted_token = compact.len() > 96
            && compact.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '='));
        assert!(is_placeholder || !looks_like_unredacted_token, "possible raw token/key material in remote auth docs: {compact}");
    }
}

#[test]
fn basalt_source_boundary_is_documented_and_matches_workspace_inputs() {
    let remote_auth = include_str!("../docs/src/reference/remote-auth.md");
    let workspace_manifest = include_str!("../Cargo.toml");
    let flake = include_str!("../flake.nix");

    assert!(remote_auth.contains("basalt = { path = \"../basalt\", default-features = false }"));
    assert!(remote_auth.contains("externalSources"));
    assert!(remote_auth.contains("OnixResearch/basalt"));

    assert!(workspace_manifest.contains("basalt = { path = \"../basalt\", default-features = false }"));
    assert!(flake.contains("url = \"git+ssh://git@github.com/OnixResearch/basalt.git\""));
    assert!(flake.contains("\"../basalt\" = basalt-src"));
}
