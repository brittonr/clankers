//! Leader menu types — thin aliases over `rat_leaderkey` parameterized with
//! this crate's [`Action`] type.

use crate::actions::Action;

// Re-export non-generic types directly.
pub use rat_leaderkey::HiddenSet;
pub use rat_leaderkey::MenuPlacement;

// Concrete type aliases for the generic rat-leaderkey types.
pub type LeaderAction = rat_leaderkey::LeaderAction<Action>;
pub type LeaderMenuItem = rat_leaderkey::LeaderMenuItem<Action>;
pub type LeaderMenuDef = rat_leaderkey::LeaderMenuDef<Action>;
pub type MenuContribution = rat_leaderkey::MenuContribution<Action>;

/// Anything that contributes items to the leader menu.
///
/// Non-generic wrapper around [`rat_leaderkey::MenuContributor`] so that
/// downstream crates don't need to spell out the `Action` type parameter.
pub trait MenuContributor {
    fn menu_items(&self) -> Vec<MenuContribution>;
}
