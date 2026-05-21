//! Pure interpretation seam for clankers-core effects.
//!
//! This module classifies reducer-emitted `CoreEffect` values into typed
//! controller work plans. It does not mutate controller state, touch agents,
//! emit protocol events, run transports, or persist sessions.

use clankers_core::CoreEffect;
use clankers_core::CoreEffectId;
use clankers_core::CoreLogicalEvent;
use clankers_core::CoreThinkingLevel;

use crate::core_engine_composition::AcceptedPromptKind;
use crate::core_engine_composition::AcceptedPromptStart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThinkingEffectExecution {
    pub previous: CoreThinkingLevel,
    pub current: CoreThinkingLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolFilterApplication {
    pub effect_id: CoreEffectId,
    pub disabled_tools: Vec<String>,
}

pub(crate) fn interpret_prompt_request(
    effects: &[CoreEffect],
    requested_text: &str,
    requested_image_count: u32,
) -> Option<AcceptedPromptStart> {
    effects.iter().find_map(|effect| match effect {
        CoreEffect::StartPrompt {
            effect_id,
            prompt_text,
            image_count,
        } if prompt_text == requested_text && *image_count == requested_image_count => Some(AcceptedPromptStart {
            core_effect_id: *effect_id,
            kind: AcceptedPromptKind::UserPrompt,
            prompt_text: prompt_text.clone(),
            image_count: *image_count,
        }),
        _ => None,
    })
}

pub(crate) fn has_busy_change(effects: &[CoreEffect], expected_busy: bool) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy }) if *busy == expected_busy
        )
    })
}

pub(crate) fn interpret_thinking_change(effects: &[CoreEffect]) -> Option<ThinkingEffectExecution> {
    effects.iter().find_map(|effect| match effect {
        CoreEffect::EmitLogicalEvent(CoreLogicalEvent::ThinkingLevelChanged { previous, current }) => {
            Some(ThinkingEffectExecution {
                previous: *previous,
                current: *current,
            })
        }
        _ => None,
    })
}

pub(crate) fn applied_thinking_levels(effects: &[CoreEffect]) -> impl Iterator<Item = CoreThinkingLevel> + '_ {
    effects.iter().filter_map(|effect| match effect {
        CoreEffect::ApplyThinkingLevel { level } => Some(*level),
        _ => None,
    })
}

pub(crate) fn interpret_tool_filter_application(effects: &[CoreEffect]) -> Option<ToolFilterApplication> {
    effects.iter().find_map(|effect| match effect {
        CoreEffect::ApplyToolFilter {
            effect_id,
            disabled_tools,
        } => Some(ToolFilterApplication {
            effect_id: *effect_id,
            disabled_tools: disabled_tools.clone(),
        }),
        _ => None,
    })
}

pub(crate) fn disabled_tools_changed(effects: &[CoreEffect]) -> Option<Vec<String>> {
    effects.iter().find_map(|effect| match effect {
        CoreEffect::EmitLogicalEvent(CoreLogicalEvent::DisabledToolsChanged { disabled_tools }) => {
            Some(disabled_tools.clone())
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use clankers_core::CoreEffect;
    use clankers_core::CoreEffectId;
    use clankers_core::CoreLogicalEvent;
    use clankers_core::CoreThinkingLevel;

    use super::*;

    const EFFECT_ID: CoreEffectId = CoreEffectId(7);

    #[test]
    fn prompt_request_interpretation_accepts_matching_start_effect() {
        let effects = vec![
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: true }),
            CoreEffect::StartPrompt {
                effect_id: EFFECT_ID,
                prompt_text: "hello".to_string(),
                image_count: 2,
            },
        ];

        let start = interpret_prompt_request(&effects, "hello", 2).expect("matching prompt effect");

        assert_eq!(start.core_effect_id, EFFECT_ID);
        assert_eq!(start.prompt_text, "hello");
        assert_eq!(start.image_count, 2);
        assert_eq!(start.kind, AcceptedPromptKind::UserPrompt);
        assert!(has_busy_change(&effects, true));
    }

    #[test]
    fn prompt_request_interpretation_rejects_mismatched_projection() {
        let effects = vec![CoreEffect::StartPrompt {
            effect_id: EFFECT_ID,
            prompt_text: "hello".to_string(),
            image_count: 1,
        }];

        assert!(interpret_prompt_request(&effects, "hello", 2).is_none());
    }

    #[test]
    fn thinking_interpretation_separates_apply_effect_from_logical_event() {
        let effects = vec![
            CoreEffect::ApplyThinkingLevel {
                level: CoreThinkingLevel::High,
            },
            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::ThinkingLevelChanged {
                previous: CoreThinkingLevel::Off,
                current: CoreThinkingLevel::High,
            }),
        ];

        let levels = applied_thinking_levels(&effects).collect::<Vec<_>>();
        let change = interpret_thinking_change(&effects).expect("thinking logical event");

        assert_eq!(levels, vec![CoreThinkingLevel::High]);
        assert_eq!(change.previous, CoreThinkingLevel::Off);
        assert_eq!(change.current, CoreThinkingLevel::High);
    }

    #[test]
    fn tool_filter_interpretation_keeps_application_and_ack_projection_distinct() {
        let tools = vec!["bash".to_string(), "read".to_string()];
        let application_effects = vec![CoreEffect::ApplyToolFilter {
            effect_id: EFFECT_ID,
            disabled_tools: tools.clone(),
        }];
        let ack_effects = vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::DisabledToolsChanged {
            disabled_tools: tools.clone(),
        })];

        let application = interpret_tool_filter_application(&application_effects).expect("tool filter application");
        let changed = disabled_tools_changed(&ack_effects).expect("disabled tools changed event");

        assert_eq!(application.effect_id, EFFECT_ID);
        assert_eq!(application.disabled_tools, tools);
        assert_eq!(changed, vec!["bash".to_string(), "read".to_string()]);
        assert!(disabled_tools_changed(&application_effects).is_none());
    }
}
