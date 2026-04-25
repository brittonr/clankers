//! Selector action types — commands returned by popup selectors.
//!
//! When a popup selector (model, account, session) completes, it returns a
//! `SelectorAction` that the caller maps to an application-specific command.

use std::path::PathBuf;

/// An action returned by a selector handler when the user makes a selection
/// that requires backend processing.
///
/// Selectors that only mutate `App` state (branch switcher, merge interactive)
/// handle everything internally and never produce a `SelectorAction`.
#[derive(Debug, Clone)]
pub enum SelectorAction {
    /// The user selected a new model in the model picker.
    SetModel(String),
    /// The user selected a different account in the account picker.
    SwitchAccount(String),
    /// The user selected a session to resume.
    ResumeSession { file_path: PathBuf, session_id: String },
}
