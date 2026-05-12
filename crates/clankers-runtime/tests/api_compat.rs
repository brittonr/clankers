use clankers_runtime::ErrorClass;
use clankers_runtime::EventMetadata;
use clankers_runtime::PromptId;
use clankers_runtime::SessionEvent;
use clankers_runtime::SessionId;
use clankers_runtime::StopReason;
use clankers_runtime::ToolStatus;
use clankers_runtime::events;

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
