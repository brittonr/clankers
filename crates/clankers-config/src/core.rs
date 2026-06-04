//! Display-neutral configuration DTOs for embeddable/runtime consumers.
//!
//! This module is intentionally plain serde data. Desktop/TUI projection lives at
//! shell/display adapters instead of in these reusable DTOs.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Display-neutral snapshot of the settings fields reusable hosts usually need.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralSettingsSummary {
    pub model: String,
    pub thinking_level: String,
    pub theme: Option<ThemeSelection>,
    pub keymap: NeutralKeymapConfig,
    pub skills: SkillServiceConfig,
    pub prompt: PromptServiceConfig,
}

/// Theme selection before projection into terminal colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSelection {
    pub name: String,
    pub auto_detect: bool,
}

impl ThemeSelection {
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            auto_detect: name == "auto",
            name,
        }
    }
}

/// Keymap settings before projection into key event/action types.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralKeymapConfig {
    pub preset: String,
    #[serde(default)]
    pub normal: BTreeMap<String, String>,
    #[serde(default)]
    pub insert: BTreeMap<String, String>,
}

/// Prompt-source policy selected by the host or desktop adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptServiceConfig {
    pub allow_filesystem_context: bool,
    pub allow_context_references: bool,
    pub skill_service_required: bool,
}

/// Skill resolver policy before desktop root discovery is attached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillServiceConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub requested: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_core_services_are_display_neutral() {
        let summary = NeutralSettingsSummary {
            model: "test-model".to_string(),
            thinking_level: "max".to_string(),
            theme: Some(ThemeSelection::named("auto")),
            keymap: NeutralKeymapConfig {
                preset: "helix".to_string(),
                normal: BTreeMap::from([("q".to_string(), "quit".to_string())]),
                insert: BTreeMap::new(),
            },
            skills: SkillServiceConfig {
                enabled: true,
                requested: vec!["review".to_string()],
            },
            prompt: PromptServiceConfig {
                allow_filesystem_context: false,
                allow_context_references: false,
                skill_service_required: true,
            },
        };
        let json = serde_json::to_value(&summary).expect("neutral config serializes");
        assert_eq!(json["model"], "test-model");
        assert_eq!(json["theme"]["autoDetect"], true);
        assert_eq!(json["keymap"]["normal"]["q"], "quit");
        assert!(!serde_json::to_string(&json).unwrap().contains("ratatui"));
        assert!(!serde_json::to_string(&json).unwrap().contains("tui"));
    }
}
