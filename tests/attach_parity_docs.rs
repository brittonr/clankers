use std::collections::BTreeSet;

use syn::UseTree;
use syn::visit::Visit;

const ATTACH: &str = include_str!("../src/modes/attach.rs");
const ATTACH_COMMANDS: &str = include_str!("../src/modes/attach/commands.rs");
const ATTACH_REMOTE: &str = include_str!("../src/modes/attach_remote.rs");
const SESSION_COMMAND_POLICY: &str = include_str!("../src/modes/session_command_policy.rs");
const SLASH_EFFECTS: &str = include_str!("../src/slash_commands/effects.rs");
const REQUEST_LIFECYCLE: &str = include_str!("../docs/src/reference/request-lifecycle.md");

#[derive(Default)]
struct SourceInventory {
    structs: BTreeSet<String>,
    functions: BTreeSet<String>,
    methods: BTreeSet<String>,
    call_paths: BTreeSet<String>,
    paths: BTreeSet<String>,
    method_calls: BTreeSet<String>,
    use_paths: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for SourceInventory {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.structs.insert(item.ident.to_string());
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.functions.insert(item.sig.ident.to_string());
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.methods.insert(item.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_expr_call(&mut self, expr: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = expr.func.as_ref() {
            self.call_paths.insert(path_text(&path.path));
        }
        syn::visit::visit_expr_call(self, expr);
    }

    fn visit_expr_method_call(&mut self, expr: &'ast syn::ExprMethodCall) {
        self.method_calls.insert(expr.method.to_string());
        syn::visit::visit_expr_method_call(self, expr);
    }

    fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
        self.paths.insert(path_text(&expr.path));
        syn::visit::visit_expr_path(self, expr);
    }

    fn visit_expr_struct(&mut self, expr: &'ast syn::ExprStruct) {
        self.paths.insert(path_text(&expr.path));
        syn::visit::visit_expr_struct(self, expr);
    }

    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        self.paths.insert(path_text(&ty.path));
        syn::visit::visit_type_path(self, ty);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        collect_use_paths("", &item.tree, &mut self.use_paths);
        syn::visit::visit_item_use(self, item);
    }
}

fn inventory(source: &str) -> SourceInventory {
    let file = syn::parse_file(source).expect("source should parse as Rust");
    let mut inventory = SourceInventory::default();
    inventory.visit_file(&file);
    inventory
}

fn path_text(path: &syn::Path) -> String {
    path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::")
}

fn collect_use_paths(prefix: &str, tree: &UseTree, output: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => {
            let next_prefix = format!("{}{ident}::", prefix, ident = path.ident);
            collect_use_paths(&next_prefix, &path.tree, output);
        }
        UseTree::Name(name) => {
            output.insert(format!("{}{}", prefix, name.ident));
        }
        UseTree::Rename(rename) => {
            output.insert(format!("{}{}", prefix, rename.rename));
        }
        UseTree::Glob(_) => {
            output.insert(format!("{}*", prefix.trim_end_matches("::")));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_paths(prefix, item, output);
            }
        }
    }
}

fn assert_has(set: &BTreeSet<String>, expected: &str, label: &str) {
    assert!(set.contains(expected), "{label} missing `{expected}`");
}

fn assert_ordered_source_anchor(source: &str, first: &str, second: &str, message: &str) {
    let first_index = source.find(first).unwrap_or_else(|| panic!("missing ordered anchor `{first}`"));
    let second_index = source.find(second).unwrap_or_else(|| panic!("missing ordered anchor `{second}`"));
    assert!(first_index < second_index, "{message}");
}

#[test]
fn local_and_remote_attach_thread_the_same_parity_tracker() {
    let attach = inventory(ATTACH);
    let attach_commands = inventory(ATTACH_COMMANDS);
    let attach_remote = inventory(ATTACH_REMOTE);
    let session_policy = inventory(SESSION_COMMAND_POLICY);

    assert_has(&attach_commands.structs, "AttachParityTracker", "attach command structs");
    for method in [
        "should_suppress",
        "expect_thinking_ack_message",
        "expect_disabled_tools_message",
    ] {
        assert_has(&attach_commands.methods, method, "attach parity tracker methods");
    }
    assert_has(&attach_commands.functions, "is_thinking_ack_message", "attach command functions");
    assert_has(&attach_commands.call_paths, "session_command_policy::ack_matches", "attach command calls");
    for path in ["SessionAckPolicy::ThinkingLevel", "SessionAckPolicy::DisabledTools"] {
        assert_has(&attach_commands.paths, path, "attach command policy paths");
    }

    assert_has(&attach.use_paths, "commands::AttachParityTracker", "attach reexports");

    assert_has(&session_policy.functions, "ack_matches", "session command policy functions");
    // The ack predicates live inside `matches!` guards; syn does not parse
    // macro bodies as expressions, so this remains an explicit source anchor.
    for literal in [
        "text.starts_with(\"Thinking\")",
        "text.starts_with(\"Disabled tools updated:\")",
    ] {
        assert!(SESSION_COMMAND_POLICY.contains(literal), "session ack predicate source missing `{literal}`");
    }

    assert_has(&attach_remote.use_paths, "super::attach::AttachParityTracker", "remote attach imports");
    assert_has(&attach_remote.call_paths, "AttachParityTracker::default", "remote attach calls");
    for call in ["drain_daemon_events", "handle_terminal_events"] {
        assert_has(&attach_remote.call_paths, call, "remote attach calls");
    }
}

#[test]
fn thinking_slash_bridges_explicit_and_cycle_paths_before_suppressing_daemon_ack() {
    let slash_effects = inventory(SLASH_EFFECTS);
    let attach_commands = inventory(ATTACH_COMMANDS);
    let session_policy = inventory(SESSION_COMMAND_POLICY);

    for path in ["AgentCommand::SetThinkingLevel", "AgentCommand::CycleThinkingLevel"] {
        assert_has(&slash_effects.paths, path, "slash effect command paths");
    }
    for call in [
        "session_command_policy::set_thinking_level_effect",
        "session_command_policy::cycle_thinking_level_effect",
    ] {
        assert_has(&slash_effects.call_paths, call, "slash effect session policy calls");
    }

    assert_has(
        &attach_commands.call_paths,
        "slash_commands::effects::agent_command_effect",
        "attach command slash effect calls",
    );
    assert_has(&attach_commands.call_paths, "apply_local_session_effect", "attach command local apply calls");
    assert_has(&attach_commands.method_calls, "expect_ack", "attach command ack methods");

    for path in ["SessionCommand::SetThinkingLevel", "SessionCommand::CycleThinkingLevel"] {
        assert_has(&session_policy.paths, path, "session command policy thinking paths");
    }
}

#[test]
fn disabled_tools_attach_bridge_applies_local_state_before_ack_suppression() {
    assert_ordered_source_anchor(
        ATTACH,
        "apply_standalone_disabled_tools(app, app.overlays.tool_toggle.disabled_set())",
        "parity_tracker.expect_disabled_tools_message();",
        "attach should apply disabled-tools state before budgeting daemon ack suppression",
    );
    assert_ordered_source_anchor(
        ATTACH,
        "parity_tracker.expect_disabled_tools_message();",
        "client.send(SessionCommand::SetDisabledTools { tools: disabled });",
        "attach should budget daemon disabled-tools ack suppression before forwarding",
    );
}

#[test]
fn request_lifecycle_doc_keeps_attach_parity_warning() {
    for phrase in [
        "Slash command and attach parity",
        "suppress only the matching daemon acknowledgement",
        "Keep suppression narrow",
        "Update local and remote attach code together",
    ] {
        assert!(REQUEST_LIFECYCLE.contains(phrase), "request lifecycle doc missing attach parity phrase `{phrase}`");
    }
}
