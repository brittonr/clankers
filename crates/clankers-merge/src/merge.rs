//! N-way merge via categorical pushout on graggles.
//!
//! The merge algorithm:
//! 1. Start with a base graggle
//! 2. For each branch, compute the diff (patch) from base
//! 3. Apply all patches to the base graggle
//!
//! Because patches reference context vertices in the base, they can be
//! applied in any order. When two patches insert at the same location
//! (same up_context and down_context), the graggle gains parallel vertices —
//! these show up as conflicts when flattened.
//!
//! When two patches touch completely different regions, they commute
//! perfectly — the result is identical regardless of application order.
//!
//! This is the key property that eliminates cascading merge conflicts
//! when parallel agents merge back to a shared parent.

use crate::diff::diff;
use crate::flatten::FlattenResult;
use crate::flatten::flatten;
use crate::graggle::Graggle;
use crate::patch::Patch;
use crate::patch::PatchId;

/// The result of merging multiple branches.
#[derive(Clone, Debug)]
pub struct MergeResult {
    /// The merged graggle.
    pub graggle: Graggle,
    /// The flattened output (with conflict markers if any).
    pub output: FlattenResult,
    /// The patches that were applied (one per branch).
    pub patches: Vec<Patch>,
}

/// Merge multiple modified texts against a common base.
///
/// Each entry in `branches` is the full text content of a modified version.
/// The merge computes a patch for each branch relative to the base,
/// then applies all patches to produce the merged result.
///
/// # Order Independence
///
/// The result is the same regardless of the order of `branches`.
/// This is the fundamental guarantee of the graggle merge algorithm.
///
/// # Conflicts
///
/// When two branches insert content at the same location, the merged
/// graggle has parallel (unordered) vertices. These appear as conflicts
/// in the flattened output with `<<<<<<< side N` markers.
///
/// When two branches edit completely different regions, the merge is
/// clean — no conflicts, even if applied in any order.
// r[impl merge.order-independence]
pub fn merge(base: &Graggle, branches: &[&str]) -> MergeResult {
    let mut merged = base.clone();
    let mut patches = Vec::with_capacity(branches.len());

    for (i, &branch_text) in branches.iter().enumerate() {
        let mut patch = diff(base, branch_text);
        // Assign unique patch IDs to avoid collisions
        let pid = PatchId(base.next_patch_id + i as u64);
        reassign_patch_id(&mut patch, pid);
        merged.apply(&patch);
        patches.push(patch);
    }

    let output = flatten(&merged);

    MergeResult {
        graggle: merged,
        output,
        patches,
    }
}

/// Reassign the PatchId for a patch and all internal VertexId references.
///
/// When diff() chains consecutive inserts, each references the previous
/// insert's VertexId as up_context. Those VertexIds use the original PatchId,
/// so we must update them when reassigning.
fn reassign_patch_id(patch: &mut Patch, new_id: PatchId) {
    let old_id = patch.id;
    patch.id = new_id;

    for op in &mut patch.ops {
        if let crate::patch::PatchOp::Insert {
            up_context,
            down_context,
            ..
        } = op
        {
            // Update any context references that point to vertices from this same patch
            for ctx in up_context.iter_mut().chain(down_context.iter_mut()) {
                if ctx.patch == old_id {
                    ctx.patch = new_id;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    // r[verify merge.order-independence]
    #[test]
    fn merge_no_conflict_different_regions() {
        // Base: a, b, c
        // Left: a, X, b, c  (insert X after a)
        // Right: a, b, c, Y (insert Y after c)
        let base = Graggle::from_text("a\nb\nc\n");
        let result = merge(&base, &["a\nX\nb\nc\n", "a\nb\nc\nY\n"]);

        assert!(!result.output.has_conflicts);
        assert_eq!(result.output.content, "a\nX\nb\nc\nY\n");
    }

    #[test]
    fn merge_no_conflict_different_regions_reversed() {
        // Same as above but branches in opposite order — should give same result
        let base = Graggle::from_text("a\nb\nc\n");
        let result = merge(&base, &["a\nb\nc\nY\n", "a\nX\nb\nc\n"]);

        assert!(!result.output.has_conflicts);
        assert_eq!(result.output.content, "a\nX\nb\nc\nY\n");
    }

    #[test]
    fn merge_conflict_same_location() {
        // Base: a, c
        // Left: a, X, c  (insert X between a and c)
        // Right: a, Y, c (insert Y between a and c)
        // → conflict: X vs Y
        let base = Graggle::from_text("a\nc\n");
        let result = merge(&base, &["a\nX\nc\n", "a\nY\nc\n"]);

        assert!(result.output.has_conflicts);
        assert!(result.output.content.contains("<<<<<<"));
        assert!(result.output.content.contains("X\n"));
        assert!(result.output.content.contains("Y\n"));
        // Non-conflicting lines should be present
        assert!(result.output.content.contains("a\n"));
        assert!(result.output.content.contains("c\n"));
    }

    #[test]
    fn merge_three_way_no_conflict() {
        // Base: a, b, c, d
        // Branch 1: X, a, b, c, d  (insert at start)
        // Branch 2: a, b, Y, c, d  (insert in middle)
        // Branch 3: a, b, c, d, Z  (insert at end)
        let base = Graggle::from_text("a\nb\nc\nd\n");
        let result = merge(&base, &["X\na\nb\nc\nd\n", "a\nb\nY\nc\nd\n", "a\nb\nc\nd\nZ\n"]);

        assert!(!result.output.has_conflicts);
        assert_eq!(result.output.content, "X\na\nb\nY\nc\nd\nZ\n");
    }

    // r[verify merge.order-independence]
    #[test]
    fn merge_order_independence_3way() {
        // Verify that 3-way merge gives the same result regardless of branch order
        let base = Graggle::from_text("a\nb\nc\nd\n");
        let branches = ["X\na\nb\nc\nd\n", "a\nb\nY\nc\nd\n", "a\nb\nc\nd\nZ\n"];

        let r1 = merge(&base, &[branches[0], branches[1], branches[2]]);
        let r2 = merge(&base, &[branches[2], branches[0], branches[1]]);
        let r3 = merge(&base, &[branches[1], branches[2], branches[0]]);

        assert_eq!(r1.output.content, r2.output.content);
        assert_eq!(r2.output.content, r3.output.content);
    }

    #[test]
    fn merge_delete_vs_modify_different_lines() {
        // Base: a, b, c
        // Left: a, c      (delete b)
        // Right: a, b, c, d (insert d at end)
        let base = Graggle::from_text("a\nb\nc\n");
        let result = merge(&base, &["a\nc\n", "a\nb\nc\nd\n"]);

        assert!(!result.output.has_conflicts);
        assert_eq!(result.output.content, "a\nc\nd\n");
    }

    #[test]
    fn merge_both_delete_same_line() {
        // Base: a, b, c
        // Left: a, c  (delete b)
        // Right: a, c (delete b)
        // → should be clean (both agree)
        let base = Graggle::from_text("a\nb\nc\n");
        let result = merge(&base, &["a\nc\n", "a\nc\n"]);

        assert!(!result.output.has_conflicts);
        assert_eq!(result.output.content, "a\nc\n");
    }

    #[test]
    fn merge_four_agents_realistic() {
        // Simulates 3 parallel agents editing different parts of a file.
        // Uses unique lines to avoid LCS ambiguity with duplicate content.
        // (Duplicate-line handling is tracked as a known limitation — see
        // merge_duplicate_lines_known_limitation below.)
        let base = Graggle::from_text(
            "// imports\n\
             use std::io;\n\
             // main function\n\
             fn main() {\n\
                 println!(\"hello\");\n\
             } // end main\n\
             // helper function\n\
             fn helper() {\n\
                 // todo: implement\n\
             } // end helper\n",
        );

        // Agent A: adds import at top
        let agent_a = "// imports\n\
                        use std::io;\n\
                        use std::fs;\n\
                        // main function\n\
                        fn main() {\n\
                            println!(\"hello\");\n\
                        } // end main\n\
                        // helper function\n\
                        fn helper() {\n\
                            // todo: implement\n\
                        } // end helper\n";

        // Agent B: implements helper
        let agent_b = "// imports\n\
                        use std::io;\n\
                        // main function\n\
                        fn main() {\n\
                            println!(\"hello\");\n\
                        } // end main\n\
                        // helper function\n\
                        fn helper() {\n\
                            println!(\"helping\");\n\
                        } // end helper\n";

        // Agent C: adds a new function at the end
        let agent_c = "// imports\n\
                        use std::io;\n\
                        // main function\n\
                        fn main() {\n\
                            println!(\"hello\");\n\
                        } // end main\n\
                        // helper function\n\
                        fn helper() {\n\
                            // todo: implement\n\
                        } // end helper\n\
                        // new function\n\
                        fn new_func() {\n\
                            // added by agent C\n\
                        } // end new_func\n";

        let result = merge(&base, &[agent_a, agent_b, agent_c]);

        // No conflicts — all edits are in different regions
        assert!(!result.output.has_conflicts, "Expected no conflicts but got:\n{}", result.output.content);

        // All changes should be present
        assert!(result.output.content.contains("use std::fs;\n"));
        assert!(result.output.content.contains("println!(\"helping\");\n"));
        assert!(result.output.content.contains("fn new_func()"));
    }

    #[test]
    fn merge_duplicate_lines_patience_diff() {
        // Previously a known limitation: duplicate lines (like `}`) caused
        // ambiguous LCS matching and false conflicts. With patience diff,
        // the algorithm anchors on unique lines first, producing correct results.
        let base = Graggle::from_text("a\n}\nb\n}\n");

        // Replace b with B
        let left = "a\n}\nB\n}\n";
        // Append new content after last }
        let right = "a\n}\nb\n}\nc\n";

        let result = merge(&base, &[left, right]);

        // Patience diff anchors on unique lines (a, b/B) and handles
        // the duplicate `}` lines correctly — no false conflicts.
        assert!(!result.output.has_conflicts, "Expected no conflicts but got:\n{}", result.output.content);
        assert_eq!(result.output.content, "a\n}\nB\n}\nc\n");
    }

    #[test]
    fn merge_duplicate_braces_realistic() {
        // Realistic scenario: two agents edit different functions in a file
        // that has many duplicate `}` lines.
        let base = Graggle::from_text(
            "fn foo() {\n\
             }\n\
             fn bar() {\n\
             }\n\
             fn baz() {\n\
             }\n",
        );

        // Agent A: adds body to foo
        let agent_a = "fn foo() {\n\
                        println!(\"foo\");\n\
                        }\n\
                        fn bar() {\n\
                        }\n\
                        fn baz() {\n\
                        }\n";

        // Agent B: adds body to baz
        let agent_b = "fn foo() {\n\
                        }\n\
                        fn bar() {\n\
                        }\n\
                        fn baz() {\n\
                        println!(\"baz\");\n\
                        }\n";

        let result = merge(&base, &[agent_a, agent_b]);
        assert!(!result.output.has_conflicts, "Expected no conflicts but got:\n{}", result.output.content);
        assert!(result.output.content.contains("println!(\"foo\");\n"));
        assert!(result.output.content.contains("println!(\"baz\");\n"));
    }

    // r[verify merge.order-independence]
    #[test]
    fn merge_order_independence_4agents() {
        let base = Graggle::from_text("a\nb\nc\nd\ne\n");

        let branches = [
            "X\na\nb\nc\nd\ne\n", // insert at start
            "a\nb\nY\nc\nd\ne\n", // insert in middle-1
            "a\nb\nc\nd\nZ\ne\n", // insert in middle-2
            "a\nb\nc\nd\ne\nW\n", // insert at end
        ];

        // Try all 24 permutations... well, try a few key ones
        let perms: Vec<[usize; 4]> = vec![[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2], [2, 0, 3, 1]];

        let mut results: Vec<String> = Vec::new();
        for perm in &perms {
            let ordered: Vec<&str> = perm.iter().map(|&i| branches[i]).collect();
            let r = merge(&base, &ordered);
            results.push(r.output.content.clone());
        }

        // All permutations should give the same result
        for r in &results {
            assert_eq!(r, &results[0], "Order independence violated!");
        }
    }
}
