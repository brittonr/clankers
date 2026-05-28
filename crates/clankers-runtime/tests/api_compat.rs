use clankers_runtime::CapabilityPack;
use clankers_runtime::ConfirmationAction;
use clankers_runtime::ConfirmationDecision;
use clankers_runtime::ConfirmationRequest;
use clankers_runtime::ContextReferenceKind;
use clankers_runtime::ContextReferenceRequest;
use clankers_runtime::ErrorClass;
use clankers_runtime::EventMetadata;
use clankers_runtime::ExtensionReceipt;
use clankers_runtime::ExtensionRuntimeKind;
use clankers_runtime::ExtensionRuntimeRequest;
use clankers_runtime::ExtensionServices;
use clankers_runtime::ExtensionStatus;
use clankers_runtime::HostContext;
use clankers_runtime::PromptAssemblyPolicy;
use clankers_runtime::PromptId;
use clankers_runtime::PromptInput;
use clankers_runtime::PromptSources;
use clankers_runtime::Runtime;
use clankers_runtime::RuntimeBuilder;
use clankers_runtime::RuntimeServices;
use clankers_runtime::SessionEvent;
use clankers_runtime::SessionId;
use clankers_runtime::SessionLedgerEntry;
use clankers_runtime::SessionLedgerMessage;
use clankers_runtime::SessionLedgerRole;
use clankers_runtime::SessionOptions;
use clankers_runtime::SideEffectLevel;
use clankers_runtime::StopReason;
use clankers_runtime::ToolCatalog;
use clankers_runtime::ToolDescriptor;
use clankers_runtime::ToolStatus;
use clankers_runtime::confirmation;
use clankers_runtime::events;
use clankers_runtime::ledger;
use clankers_runtime::prompt;
use clankers_runtime::runtime;
use clankers_runtime::services;
use clankers_runtime::session;
use clankers_runtime::tools;

fn assert_same_type<T>(_left: T, _right: T) {}

#[test]
fn event_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<EventMetadata>(EventMetadata::empty(), events::EventMetadata::empty());
    assert_same_type::<SessionEvent>(
        SessionEvent::Completed {
            prompt_id: PromptId::from_host("root-prompt"),
            stop_reason: StopReason::Complete,
            metadata: EventMetadata::new(SessionId::from_host("root-session")),
        },
        events::SessionEvent::Completed {
            prompt_id: PromptId::from_host("module-prompt"),
            stop_reason: events::StopReason::Interrupted,
            metadata: events::EventMetadata::new(SessionId::from_host("module-session")),
        },
    );
    assert_eq!(ToolStatus::Succeeded, events::ToolStatus::Succeeded);
    assert_eq!(ErrorClass::Extension, events::ErrorClass::Extension);
}

#[test]
fn prompt_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<PromptInput>(PromptInput::new("root"), prompt::PromptInput::new("module"));
    assert_same_type::<PromptAssemblyPolicy>(
        PromptAssemblyPolicy::host_context_only(),
        prompt::PromptAssemblyPolicy::desktop_default(),
    );
    assert_same_type::<PromptSources>(
        PromptSources {
            system_prompt: Some("system".to_string()),
            host_context: vec![HostContext {
                label: "root".to_string(),
                content: "context".to_string(),
            }],
            filesystem_context: Vec::new(),
            filesystem_context_requested: false,
            context_references: vec![ContextReferenceRequest::new("file", ContextReferenceKind::File)],
            skill_snippets: Vec::new(),
        },
        prompt::PromptSources {
            system_prompt: None,
            host_context: vec![prompt::HostContext {
                label: "module".to_string(),
                content: "context".to_string(),
            }],
            filesystem_context: Vec::new(),
            filesystem_context_requested: false,
            context_references: vec![prompt::ContextReferenceRequest::new(
                "url",
                prompt::ContextReferenceKind::Url,
            )],
            skill_snippets: Vec::new(),
        },
    );
}

#[test]
fn confirmation_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<ConfirmationRequest>(
        ConfirmationRequest::new(ConfirmationAction::RunCommand, "root"),
        confirmation::ConfirmationRequest::new(confirmation::ConfirmationAction::MutateWorkspace, "module"),
    );
    assert_same_type::<ConfirmationDecision>(
        ConfirmationDecision::approve("root ok"),
        confirmation::ConfirmationDecision::deny("module no"),
    );
}

#[test]
fn runtime_module_and_root_reexports_are_source_compatible() {
    let root_runtime: Runtime = RuntimeBuilder::new().build().unwrap();
    let module_runtime: runtime::Runtime = runtime::RuntimeBuilder::new().build().unwrap();
    assert_eq!(root_runtime.tool_catalog().tools().count(), module_runtime.tool_catalog().tools().count());
}

#[test]
fn session_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<SessionId>(SessionId::from_host("root"), session::SessionId::from_host("module"));
    assert_same_type::<SessionOptions>(
        SessionOptions {
            session_id: Some(SessionId::from_host("root-session")),
            model: Some("root-model".to_string()),
        },
        session::SessionOptions {
            session_id: Some(session::SessionId::from_host("module-session")),
            model: Some("module-model".to_string()),
        },
    );
}

#[test]
fn services_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<RuntimeServices>(RuntimeServices::in_memory(), services::RuntimeServices::in_memory());
    assert_same_type::<RuntimeServices>(RuntimeServices::stateless(), services::RuntimeServices::stateless());
    assert_same_type::<ExtensionServices>(ExtensionServices::disabled(), services::ExtensionServices::disabled());
    assert_same_type::<ExtensionReceipt>(
        ExtensionReceipt::new("root", "action", ExtensionStatus::Disabled),
        services::ExtensionReceipt::new("module", "action", services::ExtensionStatus::Unavailable),
    );
    assert_same_type::<ExtensionRuntimeRequest>(
        ExtensionRuntimeRequest {
            kind: ExtensionRuntimeKind::Plugin,
            action: "call".to_string(),
            extension_name: None,
            visible_tool_name: None,
            original_tool_name: None,
            runtime_entrypoint: None,
            arguments: serde_json::json!({}),
        },
        services::ExtensionRuntimeRequest {
            kind: services::ExtensionRuntimeKind::Mcp,
            action: "call".to_string(),
            extension_name: None,
            visible_tool_name: None,
            original_tool_name: None,
            runtime_entrypoint: None,
            arguments: serde_json::json!({}),
        },
    );
}

#[test]
fn ledger_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<SessionLedgerEntry>(
        SessionLedgerEntry::message(SessionLedgerMessage::text(SessionLedgerRole::User, "root")),
        ledger::SessionLedgerEntry::message(ledger::SessionLedgerMessage::text(
            ledger::SessionLedgerRole::Assistant,
            "module",
        )),
    );
}

#[test]
fn tools_module_and_root_reexports_are_source_compatible() {
    assert_same_type::<ToolCatalog>(ToolCatalog::embedding_safe(), tools::ToolCatalog::embedding_safe());
    assert_same_type::<ToolDescriptor>(
        ToolDescriptor::new("root_tool", "root descriptor", SideEffectLevel::ReadOnly),
        tools::ToolDescriptor::new("module_tool", "module descriptor", tools::SideEffectLevel::WorkspaceMutation),
    );
    assert_same_type::<CapabilityPack>(CapabilityPack::ReadOnly, tools::CapabilityPack::ShellCommands);
}
