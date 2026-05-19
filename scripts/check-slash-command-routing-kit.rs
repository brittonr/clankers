#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let slash_tests = fs::read_to_string("src/slash_commands/tests.rs").expect("read slash command tests");
    let attach = fs::read_to_string("src/modes/attach/commands.rs").expect("read attach command routing");
    let docs = fs::read_to_string("docs/src/reference/commands.md").expect("read slash command docs");
    let spec = fs::read_to_string("openspec/specs/slash-command-composition/spec.md")
        .expect("read slash command composition spec");

    assert_contains(
        "src/slash_commands/tests.rs",
        &slash_tests,
        &[
            "slash_command_routing_kit_detects_conflicts_and_prompt_template_fallback",
            "PRIORITY_PLUGIN",
            "plugin:kit",
            "parse_command(\"/fix-tests --dry-run\")",
            "parse_command(\"/bad.template\").is_none()",
            "repeat(65)",
        ],
    );
    assert_contains(
        "src/modes/attach/commands.rs",
        &attach,
        &[
            "route_attach_slash",
            "AttachSlashRoute::RegistryLocal",
            "AttachSlashRoute::ForwardToDaemon",
            "AttachSlashRoute::GetPlugins",
            "ATTACH_REGISTRY_LOCAL_EMPTY_ARG_COMMANDS",
        ],
    );
    assert_contains(
        "docs/src/reference/commands.md",
        &docs,
        &[
            "slash-command-routing-kit",
            "conflict detection",
            "prompt-template fallback",
            "attach routing",
            "scripts/check-slash-command-routing-kit.rs",
        ],
    );
    assert_contains(
        "openspec/specs/slash-command-composition/spec.md",
        &spec,
        &[
            "Slash command kit detects conflicts and route drift",
            "slash-command-routing-kit.boundary",
            "slash-command-routing-kit.evidence",
            "slash-command-routing-kit.drift",
        ],
    );
}

fn assert_contains(path: &str, haystack: &str, needles: &[&str]) {
    let missing: Vec<_> = needles.iter().copied().filter(|needle| !haystack.contains(needle)).collect();
    if missing.is_empty() {
        return;
    }

    eprintln!("slash-command-routing-kit drift check failed for {path}:");
    for needle in missing {
        eprintln!("  - missing {needle}");
    }
    eprintln!("owner: update slash tests, attach routing, docs, and OpenSpec evidence together");
    std::process::exit(1);
}
