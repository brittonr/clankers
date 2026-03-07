//! Run modes

pub mod common;
pub mod daemon;
pub(crate) mod clipboard;
pub(crate) mod event_loop;
pub mod interactive;
pub mod json;
mod mouse;
pub(crate) mod peers_background;
pub(crate) mod plugin_dispatch;
mod selectors;
pub(crate) mod session_restore;
pub mod plan;
pub mod print;
pub mod rpc;
