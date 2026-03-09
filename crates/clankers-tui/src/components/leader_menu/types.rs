//! Types and traits for the leader menu system.
//!
//! Canonical definitions are in `clankers-tui-types`; re-exported here for
//! backward compatibility.

use clankers_tui_types::Conflict;
pub use clankers_tui_types::HiddenSet;
pub use clankers_tui_types::LeaderAction;
pub use clankers_tui_types::LeaderMenuDef;
pub use clankers_tui_types::LeaderMenuItem;
pub use clankers_tui_types::MenuContribution;
pub use clankers_tui_types::MenuContributor;
pub use clankers_tui_types::MenuPlacement;

/// Signature for the build function (defined in builder.rs).
pub type BuildResult = (super::LeaderMenu, Vec<Conflict>);
