#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let leader_menu = fs::read_to_string("crates/clankers-tui/src/components/leader_menu/mod.rs")
        .expect("read leader menu tests");
    require(
        &leader_menu,
        "tui_action_menu_kit_validates_typed_actions_conflicts_and_hide_rules",
        "focused TUI action/menu kit test",
    );
    require(&leader_menu, "parse_action(\"open-leader-menu\")", "typed action parse positive fixture");
    require(&leader_menu, "missing_secret_env_TOKEN", "negative unknown-action fixture");
    require(&leader_menu, "HiddenSet::new", "hide-rule fail-closed fixture");
    require(&leader_menu, "conflicts[0].winner", "conflict winner diagnostic fixture");

    let actions = fs::read_to_string("crates/clanker-tui-types/src/actions.rs").expect("read action types");
    require(&actions, "pub enum CoreAction", "core action typed enum");
    require(&actions, "pub enum ExtendedAction", "extended action typed enum");
    require(&actions, "pub fn parse_action", "typed action parser");

    let menu_types = fs::read_to_string("crates/clanker-tui-types/src/menu.rs").expect("read menu types");
    require(&menu_types, "pub trait MenuContributor", "copyable menu contributor trait");
    require(&menu_types, "pub type MenuContribution", "copyable menu contribution type");

    let docs = fs::read_to_string("docs/src/reference/commands.md").expect("read command docs");
    require(&docs, "tui-action-menu-kit", "documented TUI action/menu kit");
    require(&docs, "typed `Action`", "documented typed action boundary");
    require(&docs, "hidden-menu", "documented hide-rule negative path");

    let spec = fs::read_to_string("cairn/specs/tui-action-menu-composition/spec.md")
        .expect("read promoted OpenSpec");
    require(&spec, "tui-action-menu-kit", "promoted OpenSpec requirement");
    require(&spec, "conflict-resolution", "OpenSpec conflict scenario");
    require(&spec, "hidden-menu", "OpenSpec hidden-menu scenario");
}

fn require(haystack: &str, needle: &str, label: &str) {
    assert!(haystack.contains(needle), "missing {label}: {needle}");
}
