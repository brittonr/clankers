//! The graggle (graph-file) data structure.
//!
//! A graggle is a DAG of line vertices with two sentinels: ROOT and END.
//! Every content vertex is reachable from ROOT and can reach END.
//! A normal file is a graggle where the vertices form a total order.
//! Parallel (unordered) vertices represent conflicts.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;
use serde::Serialize;

use crate::patch::PatchId;

/// Unique identifier for a vertex within a graggle.
///
/// Vertices are identified by (patch_id, index_within_patch).
/// Sentinel vertices use PatchId(0).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VertexId {
    pub patch: PatchId,
    pub index: u32,
}

impl fmt::Debug for VertexId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == ROOT {
            write!(f, "ROOT")
        } else if *self == END {
            write!(f, "END")
        } else {
            write!(f, "V({}.{})", self.patch.0, self.index)
        }
    }
}

/// The root sentinel — every content vertex is reachable from ROOT.
pub const ROOT: VertexId = VertexId {
    patch: PatchId(0),
    index: 0,
};

/// The end sentinel — every content vertex can reach END.
pub const END: VertexId = VertexId {
    patch: PatchId(0),
    index: 1,
};

/// A vertex (line) in the graggle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vertex {
    /// The line content (including trailing newline if present).
    pub content: Vec<u8>,
    /// Whether this vertex is alive (visible) or a ghost (deleted).
    pub alive: bool,
    /// Which patch introduced this vertex.
    pub introduced_by: PatchId,
}

/// A directed acyclic graph of lines.
///
/// Invariants:
/// - ROOT and END always exist
/// - Every content vertex is reachable from ROOT
/// - Every content vertex can reach END
/// - The graph is acyclic
// r[impl merge.dag.sentinels]
// r[impl merge.dag.reachability]
// r[impl merge.dag.acyclicity]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Graggle {
    /// All vertices indexed by their ID.
    pub(crate) vertices: BTreeMap<VertexId, Vertex>,

    /// Forward edges: source → set of targets.
    /// An edge (A, B) means "A comes before B".
    pub(crate) children: BTreeMap<VertexId, BTreeSet<VertexId>>,

    /// Reverse edges: target → set of sources.
    pub(crate) parents: BTreeMap<VertexId, BTreeSet<VertexId>>,

    /// Counter for the next PatchId to assign when building from text.
    pub(crate) next_patch_id: u64,
}

impl Graggle {
    /// Create an empty graggle with only ROOT and END (ROOT → END).
    // r[impl merge.dag.sentinels]
    pub fn new() -> Self {
        let mut vertices = BTreeMap::new();
        vertices.insert(ROOT, Vertex {
            content: Vec::new(),
            alive: true,
            introduced_by: PatchId(0),
        });
        vertices.insert(END, Vertex {
            content: Vec::new(),
            alive: true,
            introduced_by: PatchId(0),
        });

        let mut children = BTreeMap::new();
        children.insert(ROOT, BTreeSet::from([END]));

        let mut parents = BTreeMap::new();
        parents.insert(END, BTreeSet::from([ROOT]));

        Self {
            vertices,
            children,
            parents,
            next_patch_id: 1,
        }
    }

    /// Build a graggle from text content.
    ///
    /// Creates a linear chain: ROOT → line1 → line2 → ... → lineN → END
    // r[impl merge.from-text.linear]
    pub fn from_text(text: &str) -> Self {
        let mut g = Self::new();
        let patch_id = PatchId(g.next_patch_id);
        g.next_patch_id += 1;

        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        if lines.is_empty() {
            return g;
        }

        // Remove ROOT → END edge, we'll rebuild the chain
        g.remove_edge(ROOT, END);

        let mut prev = ROOT;
        for (i, line) in lines.iter().enumerate() {
            let vid = VertexId {
                patch: patch_id,
                index: i as u32,
            };
            g.vertices.insert(vid, Vertex {
                content: line.as_bytes().to_vec(),
                alive: true,
                introduced_by: patch_id,
            });
            g.add_edge(prev, vid);
            prev = vid;
        }
        g.add_edge(prev, END);

        g
    }

    /// Add a directed edge from `src` to `dst`.
    pub(crate) fn add_edge(&mut self, src: VertexId, dst: VertexId) {
        self.children.entry(src).or_default().insert(dst);
        self.parents.entry(dst).or_default().insert(src);
    }

    /// Remove a directed edge from `src` to `dst`.
    pub(crate) fn remove_edge(&mut self, src: VertexId, dst: VertexId) {
        if let Some(set) = self.children.get_mut(&src) {
            set.remove(&dst);
        }
        if let Some(set) = self.parents.get_mut(&dst) {
            set.remove(&src);
        }
    }

    /// Insert a vertex between context parents and context children.
    ///
    /// For each (parent, child) pair where parent → child exists:
    /// - Remove parent → child
    /// - Add parent → new_vertex → child
    // r[impl merge.insert.preserves-dag]
    pub(crate) fn insert_vertex(
        &mut self,
        id: VertexId,
        vertex: Vertex,
        up_context: &[VertexId],
        down_context: &[VertexId],
    ) {
        self.vertices.insert(id, vertex);

        // Connect up_context → new vertex
        for &parent in up_context {
            // Remove existing edges from parent to any down_context vertex
            for &child in down_context {
                self.remove_edge(parent, child);
            }
            self.add_edge(parent, id);
        }

        // Connect new vertex → down_context
        for &child in down_context {
            self.add_edge(id, child);
        }
    }

    /// Mark a vertex as deleted (ghost).
    // r[impl merge.delete.ghost]
    pub(crate) fn delete_vertex(&mut self, id: VertexId) {
        if let Some(v) = self.vertices.get_mut(&id) {
            v.alive = false;
        }
    }

    /// Get the children (successors) of a vertex.
    pub fn children_of(&self, id: VertexId) -> impl Iterator<Item = VertexId> + '_ {
        self.children.get(&id).into_iter().flat_map(|set| set.iter().copied())
    }

    /// Get the parents (predecessors) of a vertex.
    pub fn parents_of(&self, id: VertexId) -> impl Iterator<Item = VertexId> + '_ {
        self.parents.get(&id).into_iter().flat_map(|set| set.iter().copied())
    }

    /// Check if a vertex exists and is alive.
    pub fn is_alive(&self, id: VertexId) -> bool {
        self.vertices.get(&id).is_some_and(|v| v.alive)
    }

    /// Get vertex content.
    pub fn content(&self, id: VertexId) -> Option<&[u8]> {
        self.vertices.get(&id).map(|v| v.content.as_slice())
    }

    /// Get all alive content vertex IDs (excludes ROOT and END).
    pub fn alive_vertices(&self) -> impl Iterator<Item = VertexId> + '_ {
        self.vertices.iter().filter(|(id, v)| **id != ROOT && **id != END && v.alive).map(|(id, _)| *id)
    }

    /// Get the total number of content vertices (alive + ghost, excludes sentinels).
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() - 2 // exclude ROOT and END
    }
}

impl Default for Graggle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify merge.dag.sentinels]
    #[test]
    fn empty_graggle() {
        let g = Graggle::new();
        assert_eq!(g.vertex_count(), 0);
        assert!(g.children_of(ROOT).collect::<Vec<_>>() == vec![END]);
        assert!(g.parents_of(END).collect::<Vec<_>>() == vec![ROOT]);
    }

    // r[verify merge.from-text.linear]
    #[test]
    fn from_text_basic() {
        let g = Graggle::from_text("hello\nworld\n");
        assert_eq!(g.vertex_count(), 2);

        // ROOT → line0 → line1 → END
        let root_children: Vec<_> = g.children_of(ROOT).collect();
        assert_eq!(root_children.len(), 1);

        let line0 = root_children[0];
        assert_eq!(g.content(line0).unwrap(), b"hello\n");

        let line0_children: Vec<_> = g.children_of(line0).collect();
        assert_eq!(line0_children.len(), 1);

        let line1 = line0_children[0];
        assert_eq!(g.content(line1).unwrap(), b"world\n");

        let line1_children: Vec<_> = g.children_of(line1).collect();
        assert_eq!(line1_children, vec![END]);
    }

    #[test]
    fn from_text_empty() {
        let g = Graggle::from_text("");
        assert_eq!(g.vertex_count(), 0);
        assert!(g.children_of(ROOT).collect::<Vec<_>>() == vec![END]);
    }

    #[test]
    fn from_text_single_line_no_newline() {
        let g = Graggle::from_text("hello");
        assert_eq!(g.vertex_count(), 1);
        let root_children: Vec<_> = g.children_of(ROOT).collect();
        let line0 = root_children[0];
        assert_eq!(g.content(line0).unwrap(), b"hello");
    }

    // r[verify merge.insert.preserves-dag]
    #[test]
    fn insert_vertex_between() {
        let mut g = Graggle::from_text("a\nb\n");

        let line_a = g.children_of(ROOT).next().unwrap();
        let line_b = g.children_of(line_a).next().unwrap();

        let new_id = VertexId {
            patch: PatchId(99),
            index: 0,
        };
        g.insert_vertex(
            new_id,
            Vertex {
                content: b"inserted\n".to_vec(),
                alive: true,
                introduced_by: PatchId(99),
            },
            &[line_a],
            &[line_b],
        );

        // a → inserted → b
        let a_children: Vec<_> = g.children_of(line_a).collect();
        assert_eq!(a_children, vec![new_id]);

        let new_children: Vec<_> = g.children_of(new_id).collect();
        assert_eq!(new_children, vec![line_b]);
    }

    // r[verify merge.delete.ghost]
    #[test]
    fn delete_vertex() {
        let mut g = Graggle::from_text("a\nb\n");
        let line_a = g.children_of(ROOT).next().unwrap();
        assert!(g.is_alive(line_a));

        g.delete_vertex(line_a);
        assert!(!g.is_alive(line_a));

        // Ghost vertices are not in alive_vertices
        let alive: Vec<_> = g.alive_vertices().collect();
        assert_eq!(alive.len(), 1);
    }
}
