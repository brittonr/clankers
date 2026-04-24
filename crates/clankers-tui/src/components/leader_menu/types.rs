//! Types for the leader menu system — re-exported from `clanker-tui-types`.

pub use clanker_tui_types::HiddenSet;
pub use clanker_tui_types::LeaderAction;
pub use clanker_tui_types::LeaderMenuDef;
pub use clanker_tui_types::LeaderMenuItem;
pub use clanker_tui_types::MenuContribution;
pub use clanker_tui_types::MenuContributor;
pub use clanker_tui_types::MenuPlacement;

/// Result of building a leader menu.
pub type BuildResult = (super::LeaderMenu, Vec<clanker_tui_types::Conflict>);
