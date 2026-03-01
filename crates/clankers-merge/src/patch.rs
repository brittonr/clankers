//! Patches — transformations on graggles.
//!
//! A patch is a set of operations that transform a graggle. Operations are:
//! - Insert: add a new vertex between context vertices
//! - Delete: mark a vertex as a ghost (deleted)
//!
//! Patches are identified by a PatchId and carry their dependency list
//! (which patches must be applied before this one makes sense).

use serde::Deserialize;
use serde::Serialize;

use crate::graggle::Graggle;
use crate::graggle::Vertex;
use crate::graggle::VertexId;

/// Unique identifier for a patch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PatchId(pub u64);

impl std::fmt::Debug for PatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P({})", self.0)
    }
}

/// A single operation within a patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchOp {
    /// Insert a new vertex between context vertices.
    ///
    /// `up_context` are the vertices that come before (parents).
    /// `down_context` are the vertices that come after (children).
    /// The edge from each up_context to each down_context is replaced by:
    ///   up_context → new_vertex → down_context
    Insert {
        /// Index of this insert within the patch (becomes VertexId.index).
        index: u32,
        /// Content of the new line.
        content: Vec<u8>,
        /// Vertices that come immediately before.
        up_context: Vec<VertexId>,
        /// Vertices that come immediately after.
        down_context: Vec<VertexId>,
    },

    /// Delete a vertex (mark as ghost).
    ///
    /// The vertex remains in the graph but is no longer visible in output.
    /// Edges are preserved — this ensures that context references from
    /// other patches remain valid.
    Delete {
        /// The vertex to delete.
        vertex: VertexId,
    },
}

/// A patch: a named set of operations on a graggle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    /// Unique identifier for this patch.
    pub id: PatchId,

    /// Patches that must be applied before this one.
    /// All vertices referenced in ops must exist after applying deps.
    pub dependencies: Vec<PatchId>,

    /// The operations in this patch, applied in order.
    pub ops: Vec<PatchOp>,
}

impl Graggle {
    /// Apply a patch to this graggle, mutating it in place.
    ///
    /// All context vertices referenced by the patch must already exist.
    /// Insert operations create new vertices; delete operations mark vertices as ghosts.
    ///
    /// # Panics
    ///
    /// Panics if a context vertex referenced by an Insert doesn't exist,
    /// or if a Delete references a non-existent vertex.
    pub fn apply(&mut self, patch: &Patch) {
        for op in &patch.ops {
            match op {
                PatchOp::Insert {
                    index,
                    content,
                    up_context,
                    down_context,
                } => {
                    let vid = VertexId {
                        patch: patch.id,
                        index: *index,
                    };
                    let vertex = Vertex {
                        content: content.clone(),
                        alive: true,
                        introduced_by: patch.id,
                    };

                    // Assert context vertices exist
                    for ctx in up_context.iter().chain(down_context.iter()) {
                        assert!(self.vertices.contains_key(ctx), "context vertex {ctx:?} not found in graggle");
                    }

                    self.insert_vertex(vid, vertex, up_context, down_context);
                }
                PatchOp::Delete { vertex } => {
                    assert!(self.vertices.contains_key(vertex), "delete target {vertex:?} not found in graggle");
                    self.delete_vertex(*vertex);
                }
            }
        }
    }

    /// Allocate the next PatchId for this graggle.
    pub fn next_patch_id(&mut self) -> PatchId {
        let id = PatchId(self.next_patch_id);
        self.next_patch_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graggle::END;
    use crate::graggle::ROOT;

    #[test]
    fn apply_insert_at_beginning() {
        let mut g = Graggle::from_text("b\n");
        let line_b = g.children_of(ROOT).next().unwrap();

        let patch = Patch {
            id: PatchId(10),
            dependencies: vec![],
            ops: vec![PatchOp::Insert {
                index: 0,
                content: b"a\n".to_vec(),
                up_context: vec![ROOT],
                down_context: vec![line_b],
            }],
        };

        g.apply(&patch);

        // ROOT → a → b → END
        let first = g.children_of(ROOT).next().unwrap();
        assert_eq!(g.content(first).unwrap(), b"a\n");
        let second = g.children_of(first).next().unwrap();
        assert_eq!(second, line_b);
    }

    #[test]
    fn apply_insert_at_end() {
        let mut g = Graggle::from_text("a\n");
        let line_a = g.children_of(ROOT).next().unwrap();

        let patch = Patch {
            id: PatchId(10),
            dependencies: vec![],
            ops: vec![PatchOp::Insert {
                index: 0,
                content: b"b\n".to_vec(),
                up_context: vec![line_a],
                down_context: vec![END],
            }],
        };

        g.apply(&patch);

        // ROOT → a → b → END
        let second = g.children_of(line_a).next().unwrap();
        assert_eq!(g.content(second).unwrap(), b"b\n");
        assert_eq!(g.children_of(second).next().unwrap(), END);
    }

    #[test]
    fn apply_delete() {
        let mut g = Graggle::from_text("a\nb\nc\n");
        let line_a = g.children_of(ROOT).next().unwrap();
        let line_b = g.children_of(line_a).next().unwrap();

        let patch = Patch {
            id: PatchId(10),
            dependencies: vec![],
            ops: vec![PatchOp::Delete { vertex: line_b }],
        };

        g.apply(&patch);

        // b is now a ghost
        assert!(!g.is_alive(line_b));
        // But the graph structure is preserved: a → b → c
        assert!(g.children_of(line_a).collect::<Vec<_>>().contains(&line_b));
    }

    #[test]
    fn apply_two_patches_different_locations() {
        // Base: "a\nb\n"
        // Patch 1: insert "x\n" before a (at start)
        // Patch 2: insert "y\n" after b (at end)
        // These should commute — order shouldn't matter.

        let base = Graggle::from_text("a\nb\n");
        let line_a = base.children_of(ROOT).next().unwrap();
        let line_b = base.children_of(line_a).next().unwrap();

        let p1 = Patch {
            id: PatchId(10),
            dependencies: vec![],
            ops: vec![PatchOp::Insert {
                index: 0,
                content: b"x\n".to_vec(),
                up_context: vec![ROOT],
                down_context: vec![line_a],
            }],
        };

        let p2 = Patch {
            id: PatchId(11),
            dependencies: vec![],
            ops: vec![PatchOp::Insert {
                index: 0,
                content: b"y\n".to_vec(),
                up_context: vec![line_b],
                down_context: vec![END],
            }],
        };

        // Apply p1 then p2
        let mut g1 = base.clone();
        g1.apply(&p1);
        g1.apply(&p2);

        // Apply p2 then p1
        let mut g2 = base.clone();
        g2.apply(&p2);
        g2.apply(&p1);

        // Both should produce the same alive vertex set and edges
        let alive1: Vec<_> = g1.alive_vertices().collect();
        let alive2: Vec<_> = g2.alive_vertices().collect();
        assert_eq!(alive1.len(), alive2.len());
        assert_eq!(alive1.len(), 4); // x, a, b, y
    }
}
