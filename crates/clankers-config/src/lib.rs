//! Configuration loading and path resolution for clankers.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::explicit_defaults,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::bool_naming,
        tigerstyle::ambiguous_params,
        tigerstyle::float_for_currency,
        reason = "configuration structures preserve serde defaults and documented TOML surface during Tigerstyle drain"
    )
)]

pub mod core;
pub mod keybindings;
pub mod model_roles;
#[cfg(feature = "nickel")]
pub mod nickel;
pub mod paths;
pub mod settings;
pub mod theme;

pub use core::NeutralKeymapConfig;
pub use core::NeutralSettingsSummary;
pub use core::PromptServiceConfig;
pub use core::SkillServiceConfig;
pub use core::ThemeSelection;

pub use paths::ClankersPaths;
pub use paths::ProjectPaths;
pub use settings::BrowserAutomationBackend;
pub use settings::BrowserAutomationConfigError;
pub use settings::BrowserAutomationSettings;
pub use settings::ExternalMemoryConfigError;
pub use settings::ExternalMemoryProvider;
pub use settings::ExternalMemorySettings;
pub use settings::McpServerConfig;
pub use settings::McpServerConfigError;
pub use settings::McpSettings;
pub use settings::McpTransport;
pub use settings::Settings;
pub use settings::SteelToolSubstrateConfigError;
pub use settings::SteelToolSubstrateFallbackMode;
pub use settings::SteelToolSubstrateRolloutStage;
pub use settings::SteelToolSubstrateSettings;
pub use settings::SteelTurnPlanningAuthorityGrantSettings;
pub use settings::SteelTurnPlanningConfigError;
pub use settings::SteelTurnPlanningFallbackMode;
pub use settings::SteelTurnPlanningRolloutStage;
pub use settings::SteelTurnPlanningSettings;
