//! Pure core logic for spec parsing, graph analysis, and verification
//!
//! This module contains all the core functionality as pure functions that
//! operate on strings and data structures without filesystem access.

pub mod artifact;
pub mod change;
pub mod delta;
pub mod merge;
pub mod schema;
pub mod spec;
pub mod templates;
pub mod verify;
