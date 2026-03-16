//! Types for the leader menu system — re-exported from `clankers-tui-types`.

pub use clankers_tui_types::HiddenSet;
pub use clankers_tui_types::LeaderAction;
pub use clankers_tui_types::LeaderMenuDef;
pub use clankers_tui_types::LeaderMenuItem;
pub use clankers_tui_types::MenuContribution;
pub use clankers_tui_types::MenuContributor;
pub use clankers_tui_types::MenuPlacement;

/// Result of building a leader menu.
pub type BuildResult = (super::LeaderMenu, Vec<clankers_tui_types::Conflict>);
