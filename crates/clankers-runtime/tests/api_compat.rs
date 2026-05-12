use clankers_runtime::CapabilityPack;
use clankers_runtime::ContextReferenceKind;
use clankers_runtime::ContextReferenceRequest;
use clankers_runtime::ErrorClass;
use clankers_runtime::EventMetadata;
use clankers_runtime::HostContext;
use clankers_runtime::PromptAssemblyPolicy;
use clankers_runtime::PromptId;
use clankers_runtime::PromptInput;
use clankers_runtime::PromptSources;
use clankers_runtime::SessionEvent;
use clankers_runtime::SessionId;
use clankers_runtime::SideEffectLevel;
use clankers_runtime::StopReason;
use clankers_runtime::ToolCatalog;
use clankers_runtime::ToolDescriptor;
use clankers_runtime::ToolStatus;
use clankers_runtime::events;
use clankers_runtime::prompt;
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
            filesystem_context_requested: false,
            context_references: vec![ContextReferenceRequest::new("file", ContextReferenceKind::File)],
        },
        prompt::PromptSources {
            system_prompt: None,
            host_context: vec![prompt::HostContext {
                label: "module".to_string(),
                content: "context".to_string(),
            }],
            filesystem_context_requested: false,
            context_references: vec![prompt::ContextReferenceRequest::new(
                "url",
                prompt::ContextReferenceKind::Url,
            )],
        },
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
