//! Structured work tracking — lightweight task graph
//!
//! A redb-backed task graph for decomposing, tracking, and resuming work.
//! This is the clankers equivalent of Beads' git-backed issue tracker,
//! but simpler and embedded in the agent binary.
//!
//! # Core concepts
//!
//! - **WorkItem** — A trackable unit of work (task, epic, bug, etc.)
//! - **Dependencies** — `blocked_by` edges form a DAG; `bd ready` equivalent
//! - **Status lifecycle** — Open → InProgress → Done / Failed / Cancelled
//! - **Priorities** — P0 (critical) through P3 (low)
//! - **Agent assignment** — Which agent identity owns this item
//!
//! # Usage
//!
//! ```rust,ignore
//! let store = WorkStore::open(path)?;
//!
//! // Create work items
//! let epic = WorkItem::new("Implement auth system", Priority::P1)
//!     .with_kind(WorkKind::Epic);
//! store.put(&epic)?;
//!
//! let task1 = WorkItem::new("Add JWT validation", Priority::P1)
//!     .with_parent(&epic.id)
//!     .blocked_by(&["setup-db-id"]);
//! store.put(&task1)?;
//!
//! // Find ready work (no open blockers)
//! let ready = store.ready()?;
//!
//! // Claim work atomically
//! store.claim(&task1.id, "agent-abc")?;
//! ```

pub mod item;
pub mod store;

pub use item::*;
pub use store::WorkStore;
