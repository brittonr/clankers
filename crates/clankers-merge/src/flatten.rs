//! Flatten a graggle to text output.
//!
//! Walks the DAG from ROOT to END, producing a linear sequence of lines.
//! When the graph is a total order (no parallel vertices), the output is
//! a clean file. When vertices are parallel (unordered), the output contains
//! conflict markers showing the alternatives.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Write;

use crate::graggle::END;
use crate::graggle::Graggle;
use crate::graggle::ROOT;
use crate::graggle::VertexId;

/// The result of flattening a graggle to text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlattenResult {
    /// The output content (with conflict markers if conflicts exist).
    pub content: String,
    /// Whether any conflicts were detected.
    pub has_conflicts: bool,
    /// The individual blocks (ordered content or conflict regions).
    pub blocks: Vec<FlattenBlock>,
}

/// A block in the flattened output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlattenBlock {
    /// A sequence of lines in total order (no conflict).
    Clean(String),
    /// A conflict: multiple alternative orderings for a set of lines.
    Conflict {
        /// Each side is one possible ordering of the conflicting lines.
        sides: Vec<String>,
    },
}

/// Tiger Style: maximum number of vertices a graggle can have for flattening.
/// Prevents unbounded traversal of malformed or enormous graphs.
const MAX_FLATTEN_VERTICES: u32 = 100_000;

/// Flatten a graggle to text, detecting conflicts.
///
/// Uses a topological walk from ROOT to END. At each step:
/// - If the current vertex has exactly one alive successor → no conflict, emit it
/// - If multiple alive successors → detect the conflict region, emit markers
///
/// Ghost (deleted) vertices are traversed for graph connectivity but not emitted.
///
/// # Tiger Style
///
/// - Bounded by `MAX_FLATTEN_VERTICES` to prevent unbounded graph traversal
/// - Asserts ROOT and END sentinels exist in the topological order
pub fn flatten(g: &Graggle) -> FlattenResult {
    // Tiger Style: enforce vertex count bound
    assert!(
        (g.vertices.len() as u32) <= MAX_FLATTEN_VERTICES,
        "graggle has {} vertices, max is {}",
        g.vertices.len(),
        MAX_FLATTEN_VERTICES
    );

    let mut blocks = Vec::new();
    let mut current_clean = String::new();
    let mut visited = BTreeSet::new();

    // Topological traversal using Kahn's algorithm variant.
    // We track in-degree of alive+relevant vertices.
    let topo_order = topological_order(g);

    // Tiger Style: assert topological order covers all vertices
    assert_eq!(topo_order.len(), g.vertices.len(), "topological order must include all vertices (cycle detected?)");

    // Group into clean runs and conflict regions.
    // A conflict happens when a vertex has multiple alive children,
    // or equivalently, when multiple vertices share the same alive parent
    // and there's no ordering between them.
    let mut i = 0;
    while i < topo_order.len() {
        let vid = topo_order[i];
        if vid == ROOT || vid == END {
            i += 1;
            continue;
        }

        if !g.is_alive(vid) {
            i += 1;
            continue;
        }

        // Check if this vertex is part of a conflict region.
        // A vertex is in conflict if it has a sibling (same parent, different vertex)
        // that's also alive and appears in the topo order.
        let parents: Vec<_> = g.parents_of(vid).collect();
        let siblings: Vec<VertexId> = parents
            .iter()
            .flat_map(|&p| g.children_of(p))
            .filter(|&c| c != vid && c != END && g.is_alive(c))
            .collect();

        // Check if any siblings share the exact same parent set (true parallel vertices)
        let conflict_siblings: Vec<VertexId> = siblings
            .into_iter()
            .filter(|&sib| {
                let sib_parents: BTreeSet<_> = g.parents_of(sib).collect();
                let vid_parents: BTreeSet<_> = g.parents_of(vid).collect();
                // They conflict if they share at least one parent and neither
                // is an ancestor of the other in the graggle
                !sib_parents.is_disjoint(&vid_parents) && !is_ancestor(g, vid, sib) && !is_ancestor(g, sib, vid)
            })
            .filter(|sib| !visited.contains(sib))
            .collect();

        if conflict_siblings.is_empty() {
            // Clean line
            if let Some(content) = g.content(vid) {
                current_clean.push_str(&String::from_utf8_lossy(content));
            }
            visited.insert(vid);
            i += 1;
        } else {
            // Flush any accumulated clean content
            if !current_clean.is_empty() {
                blocks.push(FlattenBlock::Clean(std::mem::take(&mut current_clean)));
            }

            // Collect all vertices in this conflict region
            let mut conflict_vertices = vec![vid];
            conflict_vertices.extend(conflict_siblings.iter());
            conflict_vertices.sort();
            conflict_vertices.dedup();

            // Each conflicting vertex becomes a "side"
            let mut sides = Vec::new();
            for &cv in &conflict_vertices {
                if let Some(content) = g.content(cv) {
                    // Follow the chain from this vertex until we hit a vertex
                    // that's not exclusively ours
                    let mut side_content = String::from_utf8_lossy(content).into_owned();
                    let mut cursor = cv;
                    // Tiger Style: bounded traversal — at most MAX_FLATTEN_VERTICES steps
                    let mut chain_steps: u32 = 0;
                    loop {
                        chain_steps += 1;
                        if chain_steps > MAX_FLATTEN_VERTICES {
                            break;
                        }
                        let children: Vec<_> = g.children_of(cursor).filter(|&c| c != END && g.is_alive(c)).collect();
                        if children.len() == 1 && !conflict_vertices.contains(&children[0]) {
                            // Exclusive successor — part of this side
                            let child = children[0];
                            if let Some(cc) = g.content(child) {
                                side_content.push_str(&String::from_utf8_lossy(cc));
                            }
                            visited.insert(child);
                            cursor = child;
                        } else {
                            break;
                        }
                    }
                    sides.push(side_content);
                }
                visited.insert(cv);
            }

            blocks.push(FlattenBlock::Conflict { sides });

            // Skip past all conflict vertices in topo order
            i += 1;
            while i < topo_order.len() && visited.contains(&topo_order[i]) {
                i += 1;
            }
        }
    }

    // Flush remaining clean content
    if !current_clean.is_empty() {
        blocks.push(FlattenBlock::Clean(current_clean));
    }

    let has_conflicts = blocks.iter().any(|b| matches!(b, FlattenBlock::Conflict { .. }));

    // Build the full content string with conflict markers
    let mut content = String::new();
    for block in &blocks {
        match block {
            FlattenBlock::Clean(text) => content.push_str(text),
            FlattenBlock::Conflict { sides } => {
                content.push_str("<<<<<<< side 1\n");
                if let Some(s) = sides.first() {
                    content.push_str(s);
                    if !s.ends_with('\n') {
                        content.push('\n');
                    }
                }
                for (i, side) in sides.iter().enumerate().skip(1) {
                    writeln!(content, "======= side {}", i + 1).unwrap();
                    content.push_str(side);
                    if !side.ends_with('\n') {
                        content.push('\n');
                    }
                }
                content.push_str(">>>>>>>\n");
            }
        }
    }

    FlattenResult {
        content,
        has_conflicts,
        blocks,
    }
}

/// Check if `ancestor` is an ancestor of `descendant` in the graggle (BFS).
///
/// # Tiger Style
///
/// Bounded by vertex count — BFS visits each vertex at most once via `seen` set.
fn is_ancestor(g: &Graggle, ancestor: VertexId, descendant: VertexId) -> bool {
    if ancestor == descendant {
        return false;
    }
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    queue.push_back(ancestor);
    seen.insert(ancestor);

    while let Some(v) = queue.pop_front() {
        for child in g.children_of(v) {
            if child == descendant {
                return true;
            }
            if seen.insert(child) {
                queue.push_back(child);
            }
        }
    }
    false
}

/// Compute a topological order of all vertices in the graggle.
///
/// Uses Kahn's algorithm. Vertices are sorted by ID within each "layer"
/// for deterministic output.
fn topological_order(g: &Graggle) -> Vec<VertexId> {
    let mut in_degree: HashMap<VertexId, usize> = HashMap::new();
    for &vid in g.vertices.keys() {
        let count = g.parents_of(vid).count();
        in_degree.insert(vid, count);
    }

    let mut queue: VecDeque<VertexId> = VecDeque::new();
    // Start with vertices that have no parents (should be ROOT)
    let mut zero_degree: Vec<_> = in_degree.iter().filter(|&(_, d)| *d == 0).map(|(&v, _)| v).collect();
    zero_degree.sort();
    queue.extend(zero_degree);

    let mut order = Vec::with_capacity(g.vertices.len());

    while let Some(v) = queue.pop_front() {
        order.push(v);
        // Collect and sort children for deterministic ordering
        let mut children: Vec<_> = g.children_of(v).collect();
        children.sort();
        for child in children {
            if let Some(deg) = in_degree.get_mut(&child) {
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(child);
                }
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_simple() {
        let g = Graggle::from_text("hello\nworld\n");
        let result = flatten(&g);
        assert!(!result.has_conflicts);
        assert_eq!(result.content, "hello\nworld\n");
    }

    #[test]
    fn flatten_empty() {
        let g = Graggle::new();
        let result = flatten(&g);
        assert!(!result.has_conflicts);
        assert_eq!(result.content, "");
    }

    #[test]
    fn flatten_single_line() {
        let g = Graggle::from_text("hello\n");
        let result = flatten(&g);
        assert!(!result.has_conflicts);
        assert_eq!(result.content, "hello\n");
    }

    #[test]
    fn flatten_with_ghost() {
        let mut g = Graggle::from_text("a\nb\nc\n");
        let line_a = g.children_of(ROOT).next().unwrap();
        let line_b = g.children_of(line_a).next().unwrap();
        g.delete_vertex(line_b);

        let result = flatten(&g);
        assert!(!result.has_conflicts);
        assert_eq!(result.content, "a\nc\n");
    }
}
