//! # clankers-merge
//!
//! Order-independent merge engine inspired by pijul's patch algebra.
//!
//! Core idea: files are directed acyclic graphs of lines ("graggles"),
//! patches are graph morphisms, and merging is a categorical pushout.
//! This guarantees that merging patches A and B produces the same result
//! regardless of the order they're applied — eliminating false cascading
//! conflicts when parallel agents merge back to a shared parent.
//!
//! ## Theory (Mimram & Di Giusto)
//!
//! A **graggle** (graph-file) is a DAG where:
//! - Each vertex is a line of content (or a sentinel: root/end)
//! - Edges encode ordering: A → B means A comes before B
//! - A normal file is a graggle that happens to be a total order
//! - When lines have no prescribed order (parallel in the DAG), that's a conflict
//!
//! A **patch** transforms one graggle into another by:
//! - Inserting new vertices between context vertices
//! - Marking vertices as deleted ("ghost" lines)
//!
//! The **perfect merge** of two diverged graggles is their categorical pushout:
//! the smallest graggle that contains both sets of changes. It's unique and
//! the merge order doesn't matter (associative + commutative up to isomorphism).
//!
//! ## Usage
//!
//! ```
//! use clankers_merge::{Graggle, merge};
//!
//! let base = Graggle::from_text("hello\nworld\n");
//!
//! let result = merge(&base, &[
//!     "hello\nbeautiful\nworld\n",  // branch 1: insert "beautiful"
//!     "hello\nworld\ngoodbye\n",    // branch 2: append "goodbye"
//! ]);
//!
//! assert!(!result.output.has_conflicts);
//! assert_eq!(result.output.content, "hello\nbeautiful\nworld\ngoodbye\n");
//! ```

mod diff;
mod flatten;
mod graggle;
mod merge;
mod patch;

pub use diff::diff;
pub use flatten::FlattenBlock;
pub use flatten::FlattenResult;
pub use flatten::flatten;
pub use graggle::END;
pub use graggle::Graggle;
pub use graggle::ROOT;
pub use graggle::Vertex;
pub use graggle::VertexId;
pub use merge::merge;
pub use patch::Patch;
pub use patch::PatchId;
pub use patch::PatchOp;
