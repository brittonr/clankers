//! Configuration loading and path resolution for clankers.

pub mod keybindings;
pub mod model_roles;
#[cfg(feature = "nickel")]
pub mod nickel;
pub mod paths;
pub mod settings;
pub mod theme;

pub use paths::ClankersPaths;
pub use paths::ProjectPaths;
pub use settings::Settings;
