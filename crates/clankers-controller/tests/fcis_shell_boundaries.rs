use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use syn::Attribute;
use syn::File;
use syn::ImplItem;
use syn::Item;
use syn::Meta;
use syn::Path as SynPath;
use syn::Token;
use syn::TraitItem;
use syn::parse::Parser;
use syn::parse_quote;
use syn::punctuated::Punctuated;
use syn::visit::Visit;

const REPO_ROOT_PARENT_COUNT: usize = 2;
const CONTROLLER_SOURCE_DIR: &str = "crates/clankers-controller/src";
const RUST_SOURCE_EXTENSION: &str = "rs";
const CFG_ATTRIBUTE_NAME: &str = "cfg";
const TEST_CONFIGURATION_NAME: &str = "test";
const ANY_CONFIGURATION_NAME: &str = "any";
const ALL_CONFIGURATION_NAME: &str = "all";
const NOT_CONFIGURATION_NAME: &str = "not";
const CORE_CRATE_NAME: &str = "clankers_core";
const CORE_REDUCE_PATH: &str = "clankers_core::reduce";
const CORE_EFFECT_SEGMENT: &str = "CoreEffect";
const CORE_LOGICAL_EVENT_SEGMENT: &str = "CoreLogicalEvent";
const CORE_INPUT_SEGMENT: &str = "CoreInput";
const CORE_OUTCOME_SEGMENT: &str = "CoreOutcome";
const CORE_STATE_SEGMENT: &str = "CoreState";
const TUI_EVENT_SEGMENT: &str = "TuiEvent";
const AGENT_EVENT_TO_DAEMON_FUNCTION: &str = "agent_event_to_daemon_event";
const DAEMON_EVENT_TO_TUI_FUNCTION: &str = "daemon_event_to_tui_event";
const AGENT_MESSAGE_TO_TUI_FUNCTION: &str = "agent_message_to_tui_events";

const AGENT_RUNTIME_FILES: [&str; 3] = [
    "crates/clankers-agent/src/lib.rs",
    "crates/clankers-agent/src/turn/mod.rs",
    "crates/clankers-agent/src/turn/execution.rs",
];
const EVENT_LOOP_RUNTIME_FILE: &str = "src/modes/event_loop_runner/mod.rs";
const CONTROLLER_EFFECT_INTERPRETER_FILE: &str = "crates/clankers-controller/src/core_effects.rs";
const CONTROLLER_INPUT_TRANSLATION_FILES: [&str; 2] = [
    "crates/clankers-controller/src/command.rs",
    "crates/clankers-controller/src/auto_test.rs",
];
const CONTROLLER_EVENT_TRANSLATION_FILE: &str = "crates/clankers-controller/src/convert.rs";
const CONTROLLER_EVENT_TRANSLATION_CALLER_FILE: &str = "crates/clankers-controller/src/event_processing.rs";
const TRANSPORT_PROTOCOL_CONVERSION_FILE: &str = "crates/clankers-controller/src/transport_convert.rs";
const TRANSPORT_PROTOCOL_FRAMING_FILES: [&str; 4] = [
    "crates/clankers-controller/src/client.rs",
    "crates/clankers-controller/src/transport.rs",
    "src/modes/attach_remote.rs",
    "src/modes/daemon/quic_bridge.rs",
];

const CORE_EFFECTS_REQUIRED_PATHS: [&str; 4] = [
    "CoreEffect::StartPrompt",
    "CoreEffect::ApplyThinkingLevel",
    "CoreEffect::ApplyToolFilter",
    "CoreEffect::ReplayQueuedPrompt",
];
const COMMAND_REQUIRED_INPUT_PATHS: [&str; 8] = [
    "CoreInput::SetThinkingLevel",
    "CoreInput::CycleThinkingLevel",
    "CoreInput::SetDisabledTools",
    "CoreInput::ToolFilterApplied",
    "CoreInput::StartLoop",
    "CoreInput::StopLoop",
    "CoreInput::PromptRequested",
    "CoreInput::PromptCompleted",
];
const AUTO_TEST_REQUIRED_INPUT_PATHS: [&str; 4] = [
    "clankers_core::CoreInput::PromptRequested",
    "clankers_core::CoreInput::EvaluatePostPrompt",
    "clankers_core::CoreInput::FollowUpDispatchAcknowledged",
    "clankers_core::CoreInput::LoopFollowUpCompleted",
];
const EVENT_TRANSLATION_REQUIRED_FUNCTIONS: [&str; 3] = [
    AGENT_EVENT_TO_DAEMON_FUNCTION,
    DAEMON_EVENT_TO_TUI_FUNCTION,
    AGENT_MESSAGE_TO_TUI_FUNCTION,
];
const EVENT_TRANSLATION_REQUIRED_PATHS: [&str; 12] = [
    "DaemonEvent::ContentBlockStart",
    "DaemonEvent::ContentBlockStop",
    "DaemonEvent::TextDelta",
    "DaemonEvent::ThinkingDelta",
    "DaemonEvent::ToolCall",
    "DaemonEvent::ToolStart",
    "DaemonEvent::ToolOutput",
    "DaemonEvent::ToolDone",
    "DaemonEvent::ToolProgressUpdate",
    "DaemonEvent::ToolChunk",
    "DaemonEvent::UserInput",
    "DaemonEvent::UsageUpdate",
];
const TRANSLATION_ONLY_DAEMON_EVENT_PATHS: [&str; 12] = [
    "DaemonEvent::ContentBlockStart",
    "DaemonEvent::ContentBlockStop",
    "DaemonEvent::TextDelta",
    "DaemonEvent::ThinkingDelta",
    "DaemonEvent::ToolCall",
    "DaemonEvent::ToolStart",
    "DaemonEvent::ToolOutput",
    "DaemonEvent::ToolDone",
    "DaemonEvent::ToolProgressUpdate",
    "DaemonEvent::ToolChunk",
    "DaemonEvent::UserInput",
    "DaemonEvent::UsageUpdate",
];
const EVENT_TRANSLATION_CALLER_REQUIRED_PATHS: [&str; 1] = [AGENT_EVENT_TO_DAEMON_FUNCTION];
const TRANSPORT_PROTOCOL_CONVERSION_REQUIRED_FUNCTIONS: [&str; 4] = [
    "client_handshake",
    "session_info_event",
    "session_summary",
    "daemon_status",
];
const TRANSPORT_PROTOCOL_CONVERSION_REQUIRED_STRUCT_EXPR_PATHS: [&str; 4] = [
    "Handshake",
    "DaemonEvent::SessionInfo",
    "SessionSummary",
    "DaemonStatus",
];
const CLIENT_PROTOCOL_REQUIRED_PATHS: [&str; 1] = ["client_handshake"];
const TRANSPORT_PROTOCOL_REQUIRED_PATHS: [&str; 3] = ["session_info_event", "session_summary", "daemon_status"];
const QUIC_ATTACH_PROTOCOL_REQUIRED_PATHS: [&str; 1] = ["client_handshake"];
const QUIC_BRIDGE_PROTOCOL_REQUIRED_PATHS: [&str; 1] = ["session_info_event"];
const CONTROL_PROTOCOL_CONVERSION_REQUIRED_FUNCTIONS: [&str; 12] = [
    "control_sessions",
    "control_created",
    "control_attached",
    "control_tree",
    "control_killed",
    "control_shutting_down",
    "control_status",
    "control_restarting",
    "control_plugins",
    "control_error",
    "attach_ok",
    "attach_error",
];
const CONTROL_PROTOCOL_CONVERSION_REQUIRED_CONSTRUCTOR_PATHS: [&str; 12] = [
    "ControlResponse::Sessions",
    "ControlResponse::Created",
    "ControlResponse::Attached",
    "ControlResponse::Tree",
    "ControlResponse::Killed",
    "ControlResponse::ShuttingDown",
    "ControlResponse::Status",
    "ControlResponse::Restarting",
    "ControlResponse::Plugins",
    "ControlResponse::Error",
    "AttachResponse::Ok",
    "AttachResponse::Error",
];
const CONTROL_PROTOCOL_DAEMON_BRIDGE_FILES: [&str; 2] =
    ["src/modes/daemon/socket_bridge.rs", "src/modes/daemon/quic_bridge.rs"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CfgEnvelope {
    can_be_true_without_test: bool,
    can_be_false_without_test: bool,
}

#[derive(Default)]
struct NonTestPathCollector {
    paths: BTreeSet<String>,
}

#[derive(Default)]
struct NonTestStructExprCollector {
    paths: BTreeSet<String>,
}

#[derive(Default)]
struct NonTestConstructorCollector {
    paths: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for NonTestPathCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if impl_item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if trait_item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_trait_item(self, item);
    }

    fn visit_path(&mut self, path: &'ast SynPath) {
        self.paths.insert(path_to_string(path));
        syn::visit::visit_path(self, path);
    }
}

impl<'ast> Visit<'ast> for NonTestStructExprCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if impl_item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if trait_item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_trait_item(self, item);
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        self.paths.insert(path_to_string(&expression.path));
        syn::visit::visit_expr_struct(self, expression);
    }
}

impl<'ast> Visit<'ast> for NonTestConstructorCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if impl_item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if trait_item_has_test_only_cfg(item) {
            return;
        }
        syn::visit::visit_trait_item(self, item);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let syn::Expr::Path(function) = &*expression.func {
            self.paths.insert(path_to_string(&function.path));
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        self.paths.insert(path_to_string(&expression.path));
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        self.paths.insert(path_to_string(&expression.path));
        syn::visit::visit_expr_struct(self, expression);
    }
}

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..REPO_ROOT_PARENT_COUNT {
        path.pop();
    }
    path
}

fn parse_rust_file(source: &str) -> File {
    syn::parse_file(source).unwrap_or_else(|error| panic!("failed to parse source for FCIS boundary rail: {error}"))
}

fn read_relative_file(relative_path: &str) -> String {
    let absolute_path = repo_root().join(relative_path);
    fs::read_to_string(&absolute_path)
        .unwrap_or_else(|error| panic!("failed to read {} ({}): {error}", relative_path, absolute_path.display()))
}

fn collect_non_test_paths_from_source(source: &str) -> BTreeSet<String> {
    let file = parse_rust_file(source);
    let mut collector = NonTestPathCollector::default();
    collector.visit_file(&file);
    collector.paths
}

fn collect_non_test_paths(relative_path: &str) -> BTreeSet<String> {
    let source = read_relative_file(relative_path);
    collect_non_test_paths_from_source(&source)
}

fn collect_non_test_function_names_from_source(source: &str) -> BTreeSet<String> {
    let file = parse_rust_file(source);
    file.items
        .into_iter()
        .filter_map(|item| match item {
            Item::Fn(item_fn) if !has_test_only_cfg_attribute(&item_fn.attrs) => Some(item_fn.sig.ident.to_string()),
            _ => None,
        })
        .collect()
}

fn collect_non_test_function_names(relative_path: &str) -> BTreeSet<String> {
    let source = read_relative_file(relative_path);
    collect_non_test_function_names_from_source(&source)
}

fn collect_non_test_struct_expr_paths_from_source(source: &str) -> BTreeSet<String> {
    let file = parse_rust_file(source);
    let mut collector = NonTestStructExprCollector::default();
    collector.visit_file(&file);
    collector.paths
}

fn collect_non_test_struct_expr_paths(relative_path: &str) -> BTreeSet<String> {
    let source = read_relative_file(relative_path);
    collect_non_test_struct_expr_paths_from_source(&source)
}

fn collect_non_test_constructor_paths_from_source(source: &str) -> BTreeSet<String> {
    let file = parse_rust_file(source);
    let mut collector = NonTestConstructorCollector::default();
    collector.visit_file(&file);
    collector.paths
}

fn collect_non_test_constructor_paths(relative_path: &str) -> BTreeSet<String> {
    let source = read_relative_file(relative_path);
    collect_non_test_constructor_paths_from_source(&source)
}

fn path_to_string(path: &SynPath) -> String {
    path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::")
}

fn parse_cfg_meta(attribute: &Attribute) -> Meta {
    attribute
        .parse_args::<Meta>()
        .unwrap_or_else(|error| panic!("failed to parse cfg attribute in FCIS boundary rail: {error}"))
}

fn parse_meta_list_items(meta_list: &syn::MetaList) -> Vec<Meta> {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    parser
        .parse2(meta_list.tokens.clone())
        .unwrap_or_else(|error| panic!("failed to parse cfg meta list in FCIS boundary rail: {error}"))
        .into_iter()
        .collect()
}

fn cfg_envelope(meta: &Meta) -> CfgEnvelope {
    match meta {
        Meta::Path(path) if path.is_ident(TEST_CONFIGURATION_NAME) => CfgEnvelope {
            can_be_true_without_test: false,
            can_be_false_without_test: true,
        },
        Meta::Path(_) | Meta::NameValue(_) => CfgEnvelope {
            can_be_true_without_test: true,
            can_be_false_without_test: true,
        },
        Meta::List(meta_list) if meta_list.path.is_ident(ANY_CONFIGURATION_NAME) => {
            let mut can_be_true_without_test = false;
            let mut can_be_false_without_test = true;
            for nested_meta in parse_meta_list_items(meta_list) {
                let nested = cfg_envelope(&nested_meta);
                can_be_true_without_test |= nested.can_be_true_without_test;
                can_be_false_without_test &= nested.can_be_false_without_test;
            }
            CfgEnvelope {
                can_be_true_without_test,
                can_be_false_without_test,
            }
        }
        Meta::List(meta_list) if meta_list.path.is_ident(ALL_CONFIGURATION_NAME) => {
            let mut can_be_true_without_test = true;
            let mut can_be_false_without_test = false;
            for nested_meta in parse_meta_list_items(meta_list) {
                let nested = cfg_envelope(&nested_meta);
                can_be_true_without_test &= nested.can_be_true_without_test;
                can_be_false_without_test |= nested.can_be_false_without_test;
            }
            CfgEnvelope {
                can_be_true_without_test,
                can_be_false_without_test,
            }
        }
        Meta::List(meta_list) if meta_list.path.is_ident(NOT_CONFIGURATION_NAME) => {
            let nested_items = parse_meta_list_items(meta_list);
            assert_eq!(nested_items.len(), 1, "cfg(not(...)) must contain exactly one nested item");
            let nested = cfg_envelope(&nested_items[0]);
            CfgEnvelope {
                can_be_true_without_test: nested.can_be_false_without_test,
                can_be_false_without_test: nested.can_be_true_without_test,
            }
        }
        Meta::List(_) => CfgEnvelope {
            can_be_true_without_test: true,
            can_be_false_without_test: true,
        },
    }
}

fn attribute_is_test_only_cfg(attribute: &Attribute) -> bool {
    if !attribute.path().is_ident(CFG_ATTRIBUTE_NAME) {
        return false;
    }

    !cfg_envelope(&parse_cfg_meta(attribute)).can_be_true_without_test
}

fn has_test_only_cfg_attribute(attributes: &[Attribute]) -> bool {
    attributes.iter().any(attribute_is_test_only_cfg)
}

fn item_has_test_only_cfg(item: &Item) -> bool {
    match item {
        Item::Const(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Enum(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::ExternCrate(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Fn(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::ForeignMod(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Impl(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Macro(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Mod(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Static(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Struct(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Trait(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::TraitAlias(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Type(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Union(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Use(item) => has_test_only_cfg_attribute(&item.attrs),
        Item::Verbatim(_) => false,
        _ => false,
    }
}

fn impl_item_has_test_only_cfg(item: &ImplItem) -> bool {
    match item {
        ImplItem::Const(item) => has_test_only_cfg_attribute(&item.attrs),
        ImplItem::Fn(item) => has_test_only_cfg_attribute(&item.attrs),
        ImplItem::Macro(item) => has_test_only_cfg_attribute(&item.attrs),
        ImplItem::Type(item) => has_test_only_cfg_attribute(&item.attrs),
        ImplItem::Verbatim(_) => false,
        _ => false,
    }
}

fn trait_item_has_test_only_cfg(item: &TraitItem) -> bool {
    match item {
        TraitItem::Const(item) => has_test_only_cfg_attribute(&item.attrs),
        TraitItem::Fn(item) => has_test_only_cfg_attribute(&item.attrs),
        TraitItem::Macro(item) => has_test_only_cfg_attribute(&item.attrs),
        TraitItem::Type(item) => has_test_only_cfg_attribute(&item.attrs),
        TraitItem::Verbatim(_) => false,
        _ => false,
    }
}

fn path_has_segment(path: &str, segment: &str) -> bool {
    path.split("::").any(|part| part == segment)
}

fn find_paths_with_segment(paths: &BTreeSet<String>, segment: &str) -> Vec<String> {
    paths.iter().filter(|path| path_has_segment(path, segment)).cloned().collect()
}

fn find_exact_path(paths: &BTreeSet<String>, exact_path: &str) -> Vec<String> {
    paths.iter().filter(|path| path.as_str() == exact_path).cloned().collect()
}

fn assert_segment_absent(relative_path: &str, paths: &BTreeSet<String>, segment: &str) {
    let offending_paths = find_paths_with_segment(paths, segment);
    assert!(
        offending_paths.is_empty(),
        "{} crossed FCIS boundary with '{}' paths: {:?}",
        relative_path,
        segment,
        offending_paths
    );
}

fn assert_exact_path_absent(relative_path: &str, paths: &BTreeSet<String>, exact_path: &str) {
    let offending_paths = find_exact_path(paths, exact_path);
    assert!(
        offending_paths.is_empty(),
        "{} crossed FCIS boundary with '{}' references: {:?}",
        relative_path,
        exact_path,
        offending_paths
    );
}

fn assert_required_paths_present(relative_path: &str, paths: &BTreeSet<String>, required_paths: &[&str]) {
    let missing_paths: Vec<&str> =
        required_paths.iter().copied().filter(|required_path| !paths.contains(*required_path)).collect();
    assert!(missing_paths.is_empty(), "{} lost expected FCIS boundary paths: {:?}", relative_path, missing_paths);
}

fn assert_required_function_names_present(
    relative_path: &str,
    function_names: &BTreeSet<String>,
    required_function_names: &[&str],
) {
    let missing_function_names: Vec<&str> = required_function_names
        .iter()
        .copied()
        .filter(|required_function_name| !function_names.contains(*required_function_name))
        .collect();
    assert!(
        missing_function_names.is_empty(),
        "{} lost expected FCIS boundary functions: {:?}",
        relative_path,
        missing_function_names
    );
}

fn repo_relative_path(path: &Path) -> String {
    let relative_path = path
        .strip_prefix(repo_root())
        .unwrap_or_else(|error| panic!("failed to strip repo root from {}: {error}", path.display()));
    relative_path.to_string_lossy().into_owned()
}

fn rust_source_files_under(relative_directory: &str) -> Vec<String> {
    let mut directories = vec![repo_root().join(relative_directory)];
    let mut files = Vec::new();

    while let Some(directory) = directories.pop() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", directory.display()))
            .map(|entry| entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}")).path())
            .collect();
        entries.sort();

        for entry in entries {
            if entry.is_dir() {
                directories.push(entry);
                continue;
            }

            if entry.extension() == Some(OsStr::new(RUST_SOURCE_EXTENSION)) {
                files.push(repo_relative_path(&entry));
            }
        }
    }

    files.sort();
    files
}

fn file_uses_allowed_controller_input_translation_boundary(relative_path: &str) -> bool {
    CONTROLLER_INPUT_TRANSLATION_FILES.contains(&relative_path)
}

#[test]
fn cfg_attribute_detection_handles_literal_and_composite_test_only_forms() {
    let literal: Attribute = parse_quote!(#[cfg(test)]);
    let composite: Attribute = parse_quote!(#[cfg(all(test, unix))]);
    let negated: Attribute = parse_quote!(#[cfg(not(test))]);
    let optional: Attribute = parse_quote!(#[cfg(any(test, unix))]);

    assert!(attribute_is_test_only_cfg(&literal));
    assert!(attribute_is_test_only_cfg(&composite));
    assert!(!attribute_is_test_only_cfg(&negated));
    assert!(!attribute_is_test_only_cfg(&optional));
}

#[test]
fn collect_non_test_paths_skips_test_only_modules_without_hiding_later_runtime_items() {
    let source = r#"
use clankers_core::CoreState;

#[cfg(all(test, unix))]
mod test_helpers {
    use clankers_core::CoreInput;
}

fn runtime_boundary() -> Option<CoreState> {
    None
}
"#;

    let paths = collect_non_test_paths_from_source(source);
    assert!(
        !find_paths_with_segment(&paths, CORE_STATE_SEGMENT).is_empty(),
        "expected runtime paths to retain CoreState after skipping test-only module"
    );
    assert!(find_paths_with_segment(&paths, CORE_INPUT_SEGMENT).is_empty());
}

#[test]
fn collect_non_test_struct_expr_paths_skips_test_only_constructors() {
    let source = r#"
struct RuntimeWire;
struct TestWire;

#[cfg(test)]
fn test_only_constructor() {
    let _wire = TestWire {};
}

fn runtime_constructor() {
    let _wire = RuntimeWire {};
}
"#;

    let struct_expr_paths = collect_non_test_struct_expr_paths_from_source(source);
    assert!(struct_expr_paths.contains("RuntimeWire"));
    assert!(!struct_expr_paths.contains("TestWire"));
}

#[test]
fn collect_non_test_constructor_paths_skips_test_only_enum_variants() {
    let source = r#"
enum RuntimeWire {
    Unit,
    Tuple(i32),
    Struct { value: i32 },
}

enum TestWire {
    Unit,
    Tuple(i32),
    Struct { value: i32 },
}

#[cfg(test)]
fn test_only_constructor() {
    let _unit = TestWire::Unit;
    let _tuple = TestWire::Tuple(1);
    let _struct = TestWire::Struct { value: 1 };
}

fn runtime_constructor() {
    let _unit = RuntimeWire::Unit;
    let _tuple = RuntimeWire::Tuple(1);
    let _struct = RuntimeWire::Struct { value: 1 };
}
"#;

    let constructor_paths = collect_non_test_constructor_paths_from_source(source);
    assert!(constructor_paths.contains("RuntimeWire::Unit"));
    assert!(constructor_paths.contains("RuntimeWire::Tuple"));
    assert!(constructor_paths.contains("RuntimeWire::Struct"));
    assert!(!constructor_paths.contains("TestWire::Unit"));
    assert!(!constructor_paths.contains("TestWire::Tuple"));
    assert!(!constructor_paths.contains("TestWire::Struct"));
}

#[test]
fn agent_runtime_files_stay_shell_native() {
    for relative_path in AGENT_RUNTIME_FILES {
        let paths = collect_non_test_paths(relative_path);
        assert_segment_absent(relative_path, &paths, CORE_CRATE_NAME);
    }
}

#[test]
fn embedded_event_loop_runner_stays_adapter_only() {
    let paths = collect_non_test_paths(EVENT_LOOP_RUNTIME_FILE);
    assert_exact_path_absent(EVENT_LOOP_RUNTIME_FILE, &paths, CORE_REDUCE_PATH);
    assert_segment_absent(EVENT_LOOP_RUNTIME_FILE, &paths, CORE_INPUT_SEGMENT);
    assert_segment_absent(EVENT_LOOP_RUNTIME_FILE, &paths, CORE_OUTCOME_SEGMENT);
    assert_segment_absent(EVENT_LOOP_RUNTIME_FILE, &paths, CORE_STATE_SEGMENT);
    assert_segment_absent(EVENT_LOOP_RUNTIME_FILE, &paths, CORE_EFFECT_SEGMENT);
    assert_segment_absent(EVENT_LOOP_RUNTIME_FILE, &paths, CORE_LOGICAL_EVENT_SEGMENT);
}

#[test]
fn controller_effect_interpretation_stays_centralized_repo_wide() {
    for relative_path in rust_source_files_under(CONTROLLER_SOURCE_DIR) {
        if relative_path == CONTROLLER_EFFECT_INTERPRETER_FILE {
            continue;
        }

        let paths = collect_non_test_paths(&relative_path);
        assert_segment_absent(&relative_path, &paths, CORE_EFFECT_SEGMENT);
        assert_segment_absent(&relative_path, &paths, CORE_LOGICAL_EVENT_SEGMENT);
    }

    let interpreter_paths = collect_non_test_paths(CONTROLLER_EFFECT_INTERPRETER_FILE);
    assert_required_paths_present(CONTROLLER_EFFECT_INTERPRETER_FILE, &interpreter_paths, &CORE_EFFECTS_REQUIRED_PATHS);
}

#[test]
fn controller_input_translation_stays_in_controller_translation_files() {
    for relative_path in rust_source_files_under(CONTROLLER_SOURCE_DIR) {
        if file_uses_allowed_controller_input_translation_boundary(&relative_path) {
            continue;
        }

        let paths = collect_non_test_paths(&relative_path);
        assert_segment_absent(&relative_path, &paths, CORE_INPUT_SEGMENT);
        assert_exact_path_absent(&relative_path, &paths, CORE_REDUCE_PATH);
    }

    let command_paths = collect_non_test_paths(CONTROLLER_INPUT_TRANSLATION_FILES[0]);
    assert_required_paths_present(CONTROLLER_INPUT_TRANSLATION_FILES[0], &command_paths, &COMMAND_REQUIRED_INPUT_PATHS);

    let auto_test_paths = collect_non_test_paths(CONTROLLER_INPUT_TRANSLATION_FILES[1]);
    assert_required_paths_present(
        CONTROLLER_INPUT_TRANSLATION_FILES[1],
        &auto_test_paths,
        &AUTO_TEST_REQUIRED_INPUT_PATHS,
    );
}

#[test]
fn controller_output_and_event_translation_stays_centralized() {
    for relative_path in rust_source_files_under(CONTROLLER_SOURCE_DIR) {
        if relative_path == CONTROLLER_EVENT_TRANSLATION_FILE {
            continue;
        }

        let paths = collect_non_test_paths(&relative_path);
        assert_segment_absent(&relative_path, &paths, TUI_EVENT_SEGMENT);
        assert_exact_path_absent(&relative_path, &paths, DAEMON_EVENT_TO_TUI_FUNCTION);
        assert_exact_path_absent(&relative_path, &paths, AGENT_MESSAGE_TO_TUI_FUNCTION);
        if relative_path != CONTROLLER_EVENT_TRANSLATION_CALLER_FILE {
            assert_exact_path_absent(&relative_path, &paths, AGENT_EVENT_TO_DAEMON_FUNCTION);
        }
        for translation_path in TRANSLATION_ONLY_DAEMON_EVENT_PATHS {
            assert_exact_path_absent(&relative_path, &paths, translation_path);
        }
    }

    let convert_paths = collect_non_test_paths(CONTROLLER_EVENT_TRANSLATION_FILE);
    let convert_function_names = collect_non_test_function_names(CONTROLLER_EVENT_TRANSLATION_FILE);
    assert_required_paths_present(CONTROLLER_EVENT_TRANSLATION_FILE, &convert_paths, &EVENT_TRANSLATION_REQUIRED_PATHS);
    assert_required_function_names_present(
        CONTROLLER_EVENT_TRANSLATION_FILE,
        &convert_function_names,
        &EVENT_TRANSLATION_REQUIRED_FUNCTIONS,
    );
    assert!(
        !find_paths_with_segment(&convert_paths, TUI_EVENT_SEGMENT).is_empty(),
        "{} lost TuiEvent translation paths",
        CONTROLLER_EVENT_TRANSLATION_FILE
    );

    let event_processing_paths = collect_non_test_paths(CONTROLLER_EVENT_TRANSLATION_CALLER_FILE);
    assert_required_paths_present(
        CONTROLLER_EVENT_TRANSLATION_CALLER_FILE,
        &event_processing_paths,
        &EVENT_TRANSLATION_CALLER_REQUIRED_PATHS,
    );
    for translation_path in TRANSLATION_ONLY_DAEMON_EVENT_PATHS {
        assert_exact_path_absent(CONTROLLER_EVENT_TRANSLATION_CALLER_FILE, &event_processing_paths, translation_path);
    }
}

#[test]
fn transport_protocol_construction_stays_in_pure_conversion_files() {
    for relative_path in rust_source_files_under(CONTROLLER_SOURCE_DIR) {
        if relative_path == TRANSPORT_PROTOCOL_CONVERSION_FILE {
            continue;
        }

        let struct_expr_paths = collect_non_test_struct_expr_paths(&relative_path);
        for constructor_path in TRANSPORT_PROTOCOL_CONVERSION_REQUIRED_STRUCT_EXPR_PATHS {
            assert_exact_path_absent(&relative_path, &struct_expr_paths, constructor_path);
        }
    }

    for relative_path in TRANSPORT_PROTOCOL_FRAMING_FILES {
        let struct_expr_paths = collect_non_test_struct_expr_paths(relative_path);
        for constructor_path in TRANSPORT_PROTOCOL_CONVERSION_REQUIRED_STRUCT_EXPR_PATHS {
            assert_exact_path_absent(relative_path, &struct_expr_paths, constructor_path);
        }
    }

    let conversion_function_names = collect_non_test_function_names(TRANSPORT_PROTOCOL_CONVERSION_FILE);
    assert_required_function_names_present(
        TRANSPORT_PROTOCOL_CONVERSION_FILE,
        &conversion_function_names,
        &TRANSPORT_PROTOCOL_CONVERSION_REQUIRED_FUNCTIONS,
    );

    let conversion_struct_expr_paths = collect_non_test_struct_expr_paths(TRANSPORT_PROTOCOL_CONVERSION_FILE);
    assert_required_paths_present(
        TRANSPORT_PROTOCOL_CONVERSION_FILE,
        &conversion_struct_expr_paths,
        &TRANSPORT_PROTOCOL_CONVERSION_REQUIRED_STRUCT_EXPR_PATHS,
    );

    let client_paths = collect_non_test_paths(TRANSPORT_PROTOCOL_FRAMING_FILES[0]);
    assert_required_paths_present(TRANSPORT_PROTOCOL_FRAMING_FILES[0], &client_paths, &CLIENT_PROTOCOL_REQUIRED_PATHS);

    let transport_paths = collect_non_test_paths(TRANSPORT_PROTOCOL_FRAMING_FILES[1]);
    assert_required_paths_present(
        TRANSPORT_PROTOCOL_FRAMING_FILES[1],
        &transport_paths,
        &TRANSPORT_PROTOCOL_REQUIRED_PATHS,
    );

    let quic_attach_paths = collect_non_test_paths(TRANSPORT_PROTOCOL_FRAMING_FILES[2]);
    assert_required_paths_present(
        TRANSPORT_PROTOCOL_FRAMING_FILES[2],
        &quic_attach_paths,
        &QUIC_ATTACH_PROTOCOL_REQUIRED_PATHS,
    );

    let quic_bridge_paths = collect_non_test_paths(TRANSPORT_PROTOCOL_FRAMING_FILES[3]);
    assert_required_paths_present(
        TRANSPORT_PROTOCOL_FRAMING_FILES[3],
        &quic_bridge_paths,
        &QUIC_BRIDGE_PROTOCOL_REQUIRED_PATHS,
    );
}

#[test]
fn control_protocol_construction_stays_in_pure_conversion_files() {
    for relative_path in rust_source_files_under(CONTROLLER_SOURCE_DIR) {
        if relative_path == TRANSPORT_PROTOCOL_CONVERSION_FILE {
            continue;
        }

        let constructor_paths = collect_non_test_constructor_paths(&relative_path);
        for constructor_path in CONTROL_PROTOCOL_CONVERSION_REQUIRED_CONSTRUCTOR_PATHS {
            assert_exact_path_absent(&relative_path, &constructor_paths, constructor_path);
        }
    }

    for relative_path in CONTROL_PROTOCOL_DAEMON_BRIDGE_FILES {
        let constructor_paths = collect_non_test_constructor_paths(relative_path);
        for constructor_path in CONTROL_PROTOCOL_CONVERSION_REQUIRED_CONSTRUCTOR_PATHS {
            assert_exact_path_absent(relative_path, &constructor_paths, constructor_path);
        }
    }

    let conversion_function_names = collect_non_test_function_names(TRANSPORT_PROTOCOL_CONVERSION_FILE);
    assert_required_function_names_present(
        TRANSPORT_PROTOCOL_CONVERSION_FILE,
        &conversion_function_names,
        &CONTROL_PROTOCOL_CONVERSION_REQUIRED_FUNCTIONS,
    );

    let conversion_constructor_paths = collect_non_test_constructor_paths(TRANSPORT_PROTOCOL_CONVERSION_FILE);
    assert_required_paths_present(
        TRANSPORT_PROTOCOL_CONVERSION_FILE,
        &conversion_constructor_paths,
        &CONTROL_PROTOCOL_CONVERSION_REQUIRED_CONSTRUCTOR_PATHS,
    );
}
