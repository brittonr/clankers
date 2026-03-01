//! Compute a patch from a base graggle and modified text.
//!
//! This is the bridge between git's snapshot model and the graggle model.
//! Given a base graggle and modified file content, compute the minimal patch
//! that transforms the base into the modified version.
//!
//! Uses the **histogram diff** algorithm on the line level to identify which
//! lines are kept, which are deleted, and where new lines are inserted.
//! Histogram diff extends patience diff: it anchors on low-occurrence common
//! lines (starting with unique lines, then relaxing to count ≤ 2, etc.),
//! recursively diffs the regions between anchors, and falls back to Myers'
//! O(ND) diff for small regions with no viable anchors. This produces high
//! quality diffs even when files contain many duplicate lines (e.g. closing
//! braces, blank lines), while the Myers fallback keeps worst-case performance
//! linear in the edit distance rather than quadratic.

use crate::graggle::END;
use crate::graggle::Graggle;
use crate::graggle::ROOT;
use crate::graggle::VertexId;
use crate::patch::Patch;
use crate::patch::PatchId;
use crate::patch::PatchOp;

// Note: consecutive inserts chain their VertexIds. The first insert at a gap
// uses the base vertex as up_context. Each subsequent insert references the
// previous insert's VertexId (patch_id, index) as up_context. This keeps
// consecutive inserts in a total order within the graggle, avoiding false
// conflicts when the same patch inserts multiple lines at one location.

/// Compute a patch that transforms `base` into a graggle matching `modified` text.
///
/// The patch contains:
/// - Delete ops for lines in base that don't appear in modified
/// - Insert ops for lines in modified that don't appear in base
///
/// Context references point to existing vertices in the base graggle,
/// ensuring the patch can be applied independently of other patches.
pub fn diff(base: &Graggle, modified: &str) -> Patch {
    let patch_id = PatchId(base.next_patch_id);

    // Get the linear order of alive vertices from base
    let base_lines = linear_alive_order(base);
    let base_contents: Vec<&[u8]> = base_lines.iter().map(|&vid| base.content(vid).unwrap_or(b"")).collect();

    // Split modified into lines (preserving newlines)
    let modified_lines: Vec<&[u8]> = if modified.is_empty() {
        vec![]
    } else {
        modified.split_inclusive('\n').map(|s| s.as_bytes()).collect()
    };

    // LCS to find matching lines
    let matches = lcs_matches(&base_contents, &modified_lines);

    let mut ops = Vec::new();
    let mut insert_index: u32 = 0;

    // Walk through the LCS alignment to produce ops.
    //
    // Key insight: consecutive inserts at the same position must form a chain,
    // not parallel vertices. The first insert references the base context; each
    // subsequent insert references the previous insert as its up_context.
    // This keeps them in a total order (no false conflicts).
    let mut base_idx = 0;
    let mut mod_idx = 0;
    let mut match_idx = 0;

    loop {
        // Find the next match
        let next_match = if match_idx < matches.len() {
            Some(matches[match_idx])
        } else {
            None
        };

        let (next_base, next_mod) = next_match.unwrap_or((base_contents.len(), modified_lines.len()));

        // Delete base lines before the next match
        while base_idx < next_base {
            ops.push(PatchOp::Delete {
                vertex: base_lines[base_idx],
            });
            base_idx += 1;
        }

        // Insert modified lines before the next match.
        // Chain consecutive inserts: each references the previous insert's
        // VertexId as up_context, keeping them in total order.
        if mod_idx < next_mod {
            // Initial up_context: the last base vertex before this gap
            let base_up_ctx = if base_idx > 0 { base_lines[base_idx - 1] } else { ROOT };

            // Down context: the matched vertex, or END
            let down_ctx = if base_idx < base_lines.len() {
                base_lines[base_idx]
            } else {
                END
            };

            let mut prev_insert_id = base_up_ctx;
            while mod_idx < next_mod {
                let this_id = VertexId {
                    patch: patch_id,
                    index: insert_index,
                };
                ops.push(PatchOp::Insert {
                    index: insert_index,
                    content: modified_lines[mod_idx].to_vec(),
                    up_context: vec![prev_insert_id],
                    down_context: vec![down_ctx],
                });
                prev_insert_id = this_id;
                insert_index += 1;
                mod_idx += 1;
            }
        }

        if next_match.is_none() {
            break;
        }

        // Advance past the match
        base_idx += 1;
        mod_idx += 1;
        match_idx += 1;
    }

    Patch {
        id: patch_id,
        dependencies: vec![],
        ops,
    }
}

/// Get the linear order of alive vertices from ROOT to END.
///
/// Follows the unique alive path through the graggle.
/// Panics if the graggle is not a total order (has conflicts).
fn linear_alive_order(g: &Graggle) -> Vec<VertexId> {
    let mut order = Vec::new();
    let mut current = ROOT;

    loop {
        // Find alive children (skip ghosts, but traverse through them)
        let next = find_next_alive(g, current);
        match next {
            Some(vid) if vid == END => break,
            Some(vid) => {
                order.push(vid);
                current = vid;
            }
            None => break,
        }
    }

    order
}

/// Find the next alive vertex reachable from `start`.
///
/// If `start` has an alive child, return it directly.
/// If all children are ghosts, follow through them to find the next alive vertex.
fn find_next_alive(g: &Graggle, start: VertexId) -> Option<VertexId> {
    let mut stack = vec![start];
    let mut visited = std::collections::BTreeSet::new();
    visited.insert(start);

    while let Some(v) = stack.pop() {
        for child in g.children_of(v) {
            if child == END {
                return Some(END);
            }
            if !visited.insert(child) {
                continue;
            }
            if g.is_alive(child) {
                return Some(child);
            }
            // Ghost — traverse through it
            stack.push(child);
        }
    }

    None
}

/// Compute line matches using the histogram diff algorithm.
///
/// Histogram diff extends patience diff by relaxing the uniqueness constraint.
/// It works by:
///
/// 1. Find the lowest occurrence count among common lines in both sequences
/// 2. Use lines at that count as anchors (patience diff only uses count == 1)
/// 3. Compute the LCS of those anchor lines to get stable matches
/// 4. Recursively diff the regions between anchors
/// 5. Fall back to Myers' O(ND) diff for small regions with no viable anchors
///
/// Returns a list of (base_index, modified_index) pairs for matched lines.
fn lcs_matches(base: &[&[u8]], modified: &[&[u8]]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    histogram_diff_recursive(base, 0, base.len(), modified, 0, modified.len(), &mut result);
    result
}

/// Maximum occurrence count we'll consider for histogram anchors.
/// Beyond this, lines are too common to be useful anchors.
const MAX_ANCHOR_COUNT: usize = 64;

/// Recursively apply histogram diff on the given ranges.
fn histogram_diff_recursive(
    base: &[&[u8]],
    base_start: usize,
    base_end: usize,
    modified: &[&[u8]],
    mod_start: usize,
    mod_end: usize,
    result: &mut Vec<(usize, usize)>,
) {
    let base_len = base_end - base_start;
    let mod_len = mod_end - mod_start;

    if base_len == 0 || mod_len == 0 {
        return;
    }

    // First, match equal lines at the start and end (cheap optimization)
    let mut prefix_len = 0;
    while prefix_len < base_len
        && prefix_len < mod_len
        && base[base_start + prefix_len] == modified[mod_start + prefix_len]
    {
        result.push((base_start + prefix_len, mod_start + prefix_len));
        prefix_len += 1;
    }

    let mut suffix_len = 0;
    while suffix_len < (base_len - prefix_len)
        && suffix_len < (mod_len - prefix_len)
        && base[base_end - 1 - suffix_len] == modified[mod_end - 1 - suffix_len]
    {
        suffix_len += 1;
    }

    let inner_base_start = base_start + prefix_len;
    let inner_base_end = base_end - suffix_len;
    let inner_mod_start = mod_start + prefix_len;
    let inner_mod_end = mod_end - suffix_len;

    if inner_base_start < inner_base_end && inner_mod_start < inner_mod_end {
        // Try to find anchors at increasing occurrence thresholds
        let anchors =
            histogram_anchors(base, inner_base_start, inner_base_end, modified, inner_mod_start, inner_mod_end);

        if anchors.is_empty() {
            // No viable anchors — fall back to Myers diff on this region
            myers_diff(base, inner_base_start, inner_base_end, modified, inner_mod_start, inner_mod_end, result);
        } else {
            // Recursively diff between anchors
            let mut prev_base = inner_base_start;
            let mut prev_mod = inner_mod_start;

            for &(bi, mi) in &anchors {
                histogram_diff_recursive(base, prev_base, bi, modified, prev_mod, mi, result);
                result.push((bi, mi));
                prev_base = bi + 1;
                prev_mod = mi + 1;
            }

            // After last anchor
            histogram_diff_recursive(base, prev_base, inner_base_end, modified, prev_mod, inner_mod_end, result);
        }
    }

    // Append suffix matches
    for i in 0..suffix_len {
        result.push((inner_base_end + i, inner_mod_end + i));
    }
}

/// Find histogram anchors: low-occurrence common lines matched via LIS.
///
/// Extends patience diff by trying occurrence thresholds 1, 2, 4, ..., up to
/// MAX_ANCHOR_COUNT. At each threshold, lines appearing at most that many times
/// in both sequences are considered as anchor candidates. This handles cases
/// where no lines are truly unique but some are relatively rare.
///
/// Returns (base_index, modified_index) pairs in order.
fn histogram_anchors(
    base: &[&[u8]],
    base_start: usize,
    base_end: usize,
    modified: &[&[u8]],
    mod_start: usize,
    mod_end: usize,
) -> Vec<(usize, usize)> {
    use std::collections::HashMap;

    // Build histograms for both sides
    let mut base_counts: HashMap<&[u8], usize> = HashMap::new();
    for line in &base[base_start..base_end] {
        *base_counts.entry(*line).or_insert(0) += 1;
    }

    let mut mod_counts: HashMap<&[u8], usize> = HashMap::new();
    for line in &modified[mod_start..mod_end] {
        *mod_counts.entry(*line).or_insert(0) += 1;
    }

    // Try increasing occurrence thresholds: 1, 2, 4, 8, ...
    let mut threshold = 1usize;
    while threshold <= MAX_ANCHOR_COUNT {
        // Collect base positions for lines at or below threshold in both sides
        let mut base_positions: HashMap<&[u8], Vec<usize>> = HashMap::new();
        for i in base_start..base_end {
            let bc = base_counts[base[i]];
            let mc = mod_counts.get(base[i]).copied().unwrap_or(0);
            if bc <= threshold && mc <= threshold && mc > 0 {
                base_positions.entry(base[i]).or_default().push(i);
            }
        }

        if base_positions.is_empty() {
            threshold *= 2;
            continue;
        }

        // Build pairs ordered by modified position.
        // For lines with count > 1, pair them positionally (first with first, etc.)
        let mut line_mod_indices: HashMap<&[u8], Vec<usize>> = HashMap::new();
        for i in mod_start..mod_end {
            let mc = mod_counts[modified[i]];
            let bc = base_counts.get(modified[i]).copied().unwrap_or(0);
            if mc <= threshold && bc <= threshold && bc > 0 {
                line_mod_indices.entry(modified[i]).or_default().push(i);
            }
        }

        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for (line, base_idxs) in &base_positions {
            if let Some(mod_idxs) = line_mod_indices.get(line) {
                // Pair positionally: zip the sorted index lists
                for (&bi, &mi) in base_idxs.iter().zip(mod_idxs.iter()) {
                    pairs.push((bi, mi));
                }
            }
        }

        // Sort by modified index for LIS computation
        pairs.sort_by_key(|&(_, mi)| mi);

        let anchors = lis_by_first(&pairs);
        if !anchors.is_empty() {
            return anchors;
        }

        threshold *= 2;
    }

    vec![]
}

/// Longest increasing subsequence by the first element of each pair.
/// Input is ordered by second element; output is ordered by both.
fn lis_by_first(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if pairs.is_empty() {
        return vec![];
    }

    let base_indices: Vec<usize> = pairs.iter().map(|&(b, _)| b).collect();

    // Patience sorting for LIS
    let mut tails: Vec<usize> = Vec::new(); // tails[i] = smallest tail of IS of length i+1
    let mut pred: Vec<Option<usize>> = vec![None; pairs.len()]; // predecessor in optimal IS
    let mut tail_indices: Vec<usize> = Vec::new(); // which pair index has tails[i]

    for (idx, &val) in base_indices.iter().enumerate() {
        // Binary search for the leftmost tail >= val
        let pos = tails.partition_point(|&t| t < val);
        if pos == tails.len() {
            tails.push(val);
            tail_indices.push(idx);
        } else {
            tails[pos] = val;
            tail_indices[pos] = idx;
        }
        if pos > 0 {
            pred[idx] = Some(tail_indices[pos - 1]);
        }
    }

    // Reconstruct
    let mut result = Vec::with_capacity(tails.len());
    let mut i = *tail_indices.last().unwrap();
    loop {
        result.push(pairs[i]);
        if let Some(p) = pred[i] {
            i = p;
        } else {
            break;
        }
    }
    result.reverse();
    result
}

/// Myers' O(ND) diff algorithm as fallback for regions without viable anchors.
///
/// This is much faster than O(NM) DP-LCS for similar sequences (small edit
/// distance D), and never worse than O(N+M) × O(D) which is typically much
/// better than O(NM) for real-world diffs.
fn myers_diff(
    base: &[&[u8]],
    base_start: usize,
    base_end: usize,
    modified: &[&[u8]],
    mod_start: usize,
    mod_end: usize,
    result: &mut Vec<(usize, usize)>,
) {
    let n = base_end - base_start;
    let m = mod_end - mod_start;

    if n == 0 || m == 0 {
        return;
    }

    // For very small regions, use simple O(NM) DP to avoid Myers overhead
    if n <= 4 && m <= 4 {
        small_lcs(base, base_start, base_end, modified, mod_start, mod_end, result);
        return;
    }

    // Myers algorithm: find shortest edit script
    // We work in a coordinate system where x = base index, y = modified index
    // A diagonal k = x - y. We find furthest-reaching point on each diagonal.
    let max_d = n + m; // maximum possible edit distance
    let array_len = 2 * max_d + 1;

    // v[k + offset] = furthest x on diagonal k
    let offset = max_d;
    let mut v = vec![0usize; array_len];

    // Store the v arrays for each d to reconstruct the path
    let mut trace: Vec<Vec<usize>> = Vec::new();

    let mut found_d = None;

    for d in 0..=max_d {
        trace.push(v.clone());

        let d_i = d as isize;
        let mut k = -d_i;
        while k <= d_i {
            let ki = (k + offset as isize) as usize;

            // Choose whether to go down (insert) or right (delete)
            let mut x = if k == -d_i || (k != d_i && v[ki - 1] < v[ki + 1]) {
                v[ki + 1] // move down: x stays, y increases (insert from modified)
            } else {
                v[ki - 1] + 1 // move right: x increases (delete from base)
            };

            let mut y = (x as isize - k) as usize;

            // Follow diagonal (matching lines)
            while x < n && y < m && base[base_start + x] == modified[mod_start + y] {
                x += 1;
                y += 1;
            }

            v[ki] = x;

            if x >= n && y >= m {
                found_d = Some(d);
                break;
            }

            k += 2;
        }

        if found_d.is_some() {
            break;
        }
    }

    // Reconstruct the path from the trace
    let d = found_d.expect("Myers should always find a solution");

    // Backtrack: for each step d..0, determine which edit was made and
    // collect the diagonal (matching) segments.
    let mut x = n;
    let mut y = m;
    let mut snakes: Vec<(usize, usize, usize)> = Vec::new(); // (start_x, start_y, len)

    for step in (1..=d).rev() {
        let prev_v = &trace[step - 1];
        let k = x as isize - y as isize;
        let ki = (k + offset as isize) as usize;

        let step_i = step as isize;
        // Determine which diagonal we came from
        let prev_k = if k == -(step_i) || (k != step_i && prev_v[ki - 1] < prev_v[ki + 1]) {
            k + 1 // insert (down): came from diagonal k+1
        } else {
            k - 1 // delete (right): came from diagonal k-1
        };
        let prev_ki = (prev_k + offset as isize) as usize;
        let prev_x = prev_v[prev_ki];
        let prev_y = (prev_x as isize - prev_k) as usize;

        // After the non-diagonal move, we're at (start_x, start_y),
        // then the diagonal snake runs to (x, y).
        let start_x = if prev_k == k + 1 { prev_x } else { prev_x + 1 };
        let start_y = (start_x as isize - k) as usize;
        let snake_len = x - start_x;
        if snake_len > 0 {
            snakes.push((start_x, start_y, snake_len));
        }

        x = prev_x;
        y = prev_y;
    }

    // d == 0: the entire region from (0,0) to (x,y) is diagonal (all matches)
    // But x,y at this point should be 0,0 if d > 0, or n,m if d == 0.
    // Handle the initial diagonal from (0,0)
    if d == 0 {
        // Everything matches
        for i in 0..n.min(m) {
            result.push((base_start + i, mod_start + i));
        }
        return;
    }

    // There may be a leading diagonal from (0,0) to (x,y) before step 1
    if x > 0 && y > 0 {
        let len = x.min(y);
        snakes.push((0, 0, len));
    }

    // Snakes are in reverse order
    snakes.reverse();
    for (sx, sy, len) in snakes {
        for i in 0..len {
            result.push((base_start + sx + i, mod_start + sy + i));
        }
    }
}

/// Simple O(NM) DP-LCS for very small regions (avoids Myers overhead).
fn small_lcs(
    base: &[&[u8]],
    base_start: usize,
    base_end: usize,
    modified: &[&[u8]],
    mod_start: usize,
    mod_end: usize,
    result: &mut Vec<(usize, usize)>,
) {
    let n = base_end - base_start;
    let m = mod_end - mod_start;

    if n == 0 || m == 0 {
        return;
    }

    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if base[base_start + i - 1] == modified[mod_start + j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let mut matches = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 && j > 0 {
        if base[base_start + i - 1] == modified[mod_start + j - 1] {
            matches.push((base_start + i - 1, mod_start + j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    matches.reverse();
    result.extend(matches);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatten::flatten;

    #[test]
    fn diff_no_change() {
        let base = Graggle::from_text("a\nb\nc\n");
        let patch = diff(&base, "a\nb\nc\n");
        assert!(patch.ops.is_empty());
    }

    #[test]
    fn diff_insert_middle() {
        let base = Graggle::from_text("a\nc\n");
        let patch = diff(&base, "a\nb\nc\n");

        // Should have one Insert op
        assert_eq!(patch.ops.len(), 1);
        assert!(matches!(&patch.ops[0], PatchOp::Insert { content, .. } if content == b"b\n"));
    }

    #[test]
    fn diff_delete_middle() {
        let base = Graggle::from_text("a\nb\nc\n");
        let patch = diff(&base, "a\nc\n");

        // Should have one Delete op
        assert_eq!(patch.ops.len(), 1);
        assert!(matches!(&patch.ops[0], PatchOp::Delete { .. }));
    }

    #[test]
    fn diff_insert_at_beginning() {
        let base = Graggle::from_text("b\n");
        let patch = diff(&base, "a\nb\n");

        assert_eq!(patch.ops.len(), 1);
        assert!(matches!(&patch.ops[0], PatchOp::Insert { content, .. } if content == b"a\n"));
    }

    #[test]
    fn diff_insert_at_end() {
        let base = Graggle::from_text("a\n");
        let patch = diff(&base, "a\nb\n");

        assert_eq!(patch.ops.len(), 1);
        assert!(matches!(&patch.ops[0], PatchOp::Insert { content, .. } if content == b"b\n"));
    }

    #[test]
    fn diff_replace_line() {
        let base = Graggle::from_text("a\nb\nc\n");
        let patch = diff(&base, "a\nB\nc\n");

        // Should delete b and insert B
        assert_eq!(patch.ops.len(), 2);
        assert!(matches!(&patch.ops[0], PatchOp::Delete { .. }));
        assert!(matches!(&patch.ops[1], PatchOp::Insert { content, .. } if content == b"B\n"));
    }

    #[test]
    fn diff_roundtrip() {
        let base = Graggle::from_text("hello\nworld\n");
        let modified = "hello\nbeautiful\nworld\ngoodbye\n";

        let patch = diff(&base, modified);
        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, modified);
    }

    #[test]
    fn diff_complete_rewrite() {
        let base = Graggle::from_text("a\nb\nc\n");
        let modified = "x\ny\nz\n";

        let patch = diff(&base, modified);
        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, modified);
    }

    #[test]
    fn diff_to_empty() {
        let base = Graggle::from_text("a\nb\n");
        let patch = diff(&base, "");

        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, "");
    }

    #[test]
    fn diff_from_empty() {
        let base = Graggle::from_text("");
        let patch = diff(&base, "a\nb\n");

        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, "a\nb\n");
    }

    #[test]
    fn diff_many_duplicate_lines() {
        // Histogram diff should handle files with many duplicate lines
        // by using low-occurrence anchors (not just unique lines).
        let base = Graggle::from_text("{\n}\n{\n}\n{\n}\n");
        let modified = "{\nfoo\n}\n{\n}\n{\nbar\n}\n";

        let patch = diff(&base, modified);
        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, modified);
    }

    #[test]
    fn diff_large_identical_prefix_suffix() {
        // Test that prefix/suffix optimization handles large common regions
        let mut base_text = String::new();
        for i in 0..50 {
            base_text.push_str(&format!("line {}\n", i));
        }
        let mut modified = base_text.clone();
        // Insert a line in the middle
        modified.insert_str(modified.find("line 25").unwrap(), "INSERTED\n");

        let base = Graggle::from_text(&base_text);
        let patch = diff(&base, &modified);

        // Should have exactly 1 insert op
        assert_eq!(patch.ops.len(), 1);
        assert!(matches!(&patch.ops[0], PatchOp::Insert { content, .. } if content == b"INSERTED\n"));
    }

    #[test]
    fn diff_myers_fallback_no_unique_lines() {
        // All lines are duplicated — forces Myers fallback
        let base = Graggle::from_text("a\na\nb\nb\n");
        let modified = "a\nc\na\nb\nb\n";

        let patch = diff(&base, modified);
        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, modified);
    }

    #[test]
    fn diff_roundtrip_realistic_code() {
        let base_text = "\
fn main() {
    let x = 1;
    let y = 2;
    println!(\"{}\", x + y);
}

fn helper() {
    // does nothing
}
";
        let modified = "\
fn main() {
    let x = 10;
    let y = 2;
    let z = 3;
    println!(\"{}\", x + y + z);
}

fn helper() {
    println!(\"helping\");
}
";
        let base = Graggle::from_text(base_text);
        let patch = diff(&base, modified);
        let mut result = base.clone();
        result.apply(&patch);

        let flat = flatten(&result);
        assert!(!flat.has_conflicts);
        assert_eq!(flat.content, modified);
    }
}
