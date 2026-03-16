//! Formal specifications and proofs for UCAN capability authorization.
//!
//! Models the capability/operation matching and delegation containment
//! logic. Proves that delegation never escalates privileges, that
//! read-only blocks writes, and that wildcard/pattern matching behaves
//! correctly.

use vstd::prelude::*;

verus! {

// ── Types ───────────────────────────────────────────────────────────────

/// Model of a glob/list pattern: either wildcard (matches everything) or
/// a finite set of items.
pub enum PatternModel {
    Wildcard,
    Items { set: Set<nat> },
}

/// Model of a file access capability.
pub struct FileAccessModel {
    /// Path prefix as a byte sequence.
    pub prefix: Seq<u8>,
    /// If true, only read operations are allowed.
    pub read_only: bool,
}

/// Model of a file operation.
pub enum FileOp {
    Read { path: Seq<u8> },
    Write { path: Seq<u8> },
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Whether `prefix` is a prefix of `path` (byte-level).
pub open spec fn is_prefix_of(prefix: Seq<u8>, path: Seq<u8>) -> bool {
    prefix.len() <= path.len()
    && path.subrange(0, prefix.len() as int) =~= prefix
}

// ── Pattern specs ───────────────────────────────────────────────────────

/// Pattern matching: wildcard matches everything, items match if contained.
// r[depends ucan.auth.wildcard-matches-all]
pub open spec fn pattern_matches(p: PatternModel, item: nat) -> bool {
    match p {
        PatternModel::Wildcard => true,
        PatternModel::Items { set } => set.contains(item),
    }
}

/// Pattern containment: P1 contains P2 iff everything P2 matches, P1
/// also matches.
// r[depends ucan.auth.pattern-set-containment]
pub open spec fn pattern_contains(p1: PatternModel, p2: PatternModel) -> bool {
    match (p1, p2) {
        (PatternModel::Wildcard, _) => true,
        (_, PatternModel::Wildcard) => false,
        (PatternModel::Items { set: s1 }, PatternModel::Items { set: s2 }) => {
            s2.subset_of(s1)
        }
    }
}

// ── File access specs ───────────────────────────────────────────────────

/// File access authorization: reads check prefix, writes also require
/// read_only == false.
// r[depends ucan.auth.read-only-blocks-write]
pub open spec fn file_access_authorizes(cap: FileAccessModel, op: FileOp) -> bool {
    match op {
        FileOp::Read { path } => is_prefix_of(cap.prefix, path),
        FileOp::Write { path } => !cap.read_only && is_prefix_of(cap.prefix, path),
    }
}

/// File access containment for delegation: child prefix must extend
/// parent prefix, and read-only cannot be escalated to read-write.
// r[depends ucan.auth.no-escalation]
pub open spec fn file_access_contains(
    parent: FileAccessModel,
    child: FileAccessModel,
) -> bool {
    is_prefix_of(parent.prefix, child.prefix)
    && (child.read_only || !parent.read_only)
}

// ── Capability gate specs ───────────────────────────────────────────────

/// Model of a tool capability gate: a set of allowed tool IDs (nat).
/// If None, all tools are blocked.
pub enum ToolGate {
    AllowSet { tools: Set<nat> },
    AllowAll,
    DenyAll,
}

/// Tool gate authorization: AllowAll permits everything, AllowSet checks
/// membership, DenyAll blocks everything.
// r[depends ucan.gate.tool-check]
pub open spec fn tool_gate_allows(gate: ToolGate, tool: nat) -> bool {
    match gate {
        ToolGate::AllowAll => true,
        ToolGate::AllowSet { tools } => tools.contains(tool),
        ToolGate::DenyAll => false,
    }
}

/// File gate for reads: at least one FileAccess capability must have a
/// prefix covering the path.
// r[depends ucan.gate.file-read-check]
pub open spec fn file_read_gate_allows(
    caps: Seq<FileAccessModel>,
    path: Seq<u8>,
) -> bool {
    exists |i: int| 0 <= i < caps.len() && is_prefix_of(caps[i].prefix, path)
}

/// File gate for writes: at least one FileAccess capability must have a
/// prefix covering the path AND read_only == false.
// r[depends ucan.gate.file-write-check]
pub open spec fn file_write_gate_allows(
    caps: Seq<FileAccessModel>,
    path: Seq<u8>,
) -> bool {
    exists |i: int|
        0 <= i < caps.len()
        && !caps[i].read_only
        && is_prefix_of(caps[i].prefix, path)
}

// ── Lemmas ──────────────────────────────────────────────────────────────

/// Prefix transitivity: if a is a prefix of b, and b is a prefix of c,
/// then a is a prefix of c.
proof fn prefix_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
    requires
        is_prefix_of(a, b),
        is_prefix_of(b, c),
    ensures
        is_prefix_of(a, c)
{
    // a.len() <= b.len() <= c.len()
    // b[0..a.len()] == a  and  c[0..b.len()] == b
    // So c[0..a.len()] == b[0..a.len()] == a
    assert(a.len() <= c.len());
    assert forall |k: int| 0 <= k < a.len() implies c[k] == a[k]
    by {
        // c[k] == b[k] because k < a.len() <= b.len() and c[0..b.len()] =~= b
        assert(c.subrange(0, b.len() as int)[k] == b[k]);
        assert(c[k] == b[k]);
        // b[k] == a[k] because k < a.len() and b[0..a.len()] =~= a
        assert(b.subrange(0, a.len() as int)[k] == a[k]);
        assert(b[k] == a[k]);
    }
    assert(c.subrange(0, a.len() as int) =~= a);
}

// ── Proofs ──────────────────────────────────────────────────────────────

/// Wildcard pattern matches any item.
// r[verify ucan.auth.wildcard-matches-all]
proof fn prove_wildcard_matches_all(item: nat)
    ensures pattern_matches(PatternModel::Wildcard, item)
{
}

/// Pattern containment implies match containment: if P1 contains P2 and
/// P2 matches some item, then P1 also matches that item.
// r[verify ucan.auth.pattern-set-containment]
proof fn prove_pattern_containment(
    p1: PatternModel,
    p2: PatternModel,
    item: nat,
)
    requires
        pattern_contains(p1, p2),
        pattern_matches(p2, item),
    ensures
        pattern_matches(p1, item)
{
    match (p1, p2) {
        (PatternModel::Wildcard, _) => {}
        (PatternModel::Items { set: s1 }, PatternModel::Items { set: s2 }) => {
            assert(s2.subset_of(s1));
            assert(s2.contains(item));
        }
        _ => {} // (Items, Wildcard) can't happen: pattern_contains is false
    }
}

/// Wildcard "*" contains any other pattern.
// r[verify ucan.auth.pattern-set-containment]
proof fn prove_wildcard_contains_all(p: PatternModel)
    ensures pattern_contains(PatternModel::Wildcard, p)
{
}

/// No non-wildcard pattern contains the wildcard.
// r[verify ucan.auth.pattern-set-containment]
proof fn prove_no_items_contains_wildcard(set: Set<nat>)
    ensures !pattern_contains(PatternModel::Items { set }, PatternModel::Wildcard)
{
}

/// Read-only FileAccess never authorizes writes.
// r[verify ucan.auth.read-only-blocks-write]
proof fn prove_read_only_blocks_write(cap: FileAccessModel, path: Seq<u8>)
    requires cap.read_only
    ensures !file_access_authorizes(cap, FileOp::Write { path })
{
}

/// Read-only FileAccess does authorize reads on matching paths.
// r[verify ucan.auth.read-only-blocks-write]
proof fn prove_read_only_allows_read(cap: FileAccessModel, path: Seq<u8>)
    requires
        cap.read_only,
        is_prefix_of(cap.prefix, path),
    ensures
        file_access_authorizes(cap, FileOp::Read { path })
{
}

/// File access no-escalation: if parent contains child, anything child
/// authorizes, parent also authorizes.
// r[verify ucan.auth.no-escalation]
proof fn prove_file_access_no_escalation(
    parent: FileAccessModel,
    child: FileAccessModel,
    op: FileOp,
)
    requires
        file_access_contains(parent, child),
        file_access_authorizes(child, op),
    ensures
        file_access_authorizes(parent, op)
{
    match op {
        FileOp::Read { path } => {
            // child authorizes read: is_prefix_of(child.prefix, path)
            // parent contains child: is_prefix_of(parent.prefix, child.prefix)
            // By transitivity: is_prefix_of(parent.prefix, path)
            prefix_transitive(parent.prefix, child.prefix, path);
        }
        FileOp::Write { path } => {
            // child authorizes write: !child.read_only && is_prefix_of(child.prefix, path)
            // parent contains child: is_prefix_of(parent.prefix, child.prefix)
            //   && (child.read_only || !parent.read_only)
            // Since !child.read_only, the containment condition gives !parent.read_only
            prefix_transitive(parent.prefix, child.prefix, path);
        }
    }
}

/// The tool gate rejects unknown tools.
// r[verify ucan.gate.tool-check]
proof fn prove_tool_gate_rejects_unknown(tool: nat, allowed: Set<nat>)
    requires !allowed.contains(tool)
    ensures !tool_gate_allows(ToolGate::AllowSet { tools: allowed }, tool)
{
}

/// DenyAll gate blocks everything.
// r[verify ucan.gate.tool-check]
proof fn prove_deny_all_blocks()
    ensures forall |tool: nat| !tool_gate_allows(ToolGate::DenyAll, tool)
{
}

/// File read gate: an empty capability set blocks all reads.
// r[verify ucan.gate.file-read-check]
proof fn prove_empty_caps_block_reads(path: Seq<u8>)
    ensures !file_read_gate_allows(Seq::empty(), path)
{
}

/// File write gate: read-only capabilities block writes even when
/// prefix matches.
// r[verify ucan.gate.file-write-check]
proof fn prove_read_only_caps_block_writes(
    cap: FileAccessModel,
    path: Seq<u8>,
)
    requires cap.read_only
    ensures !file_write_gate_allows(seq![cap], path)
{
    assert forall |i: int| !(
        0 <= i < seq![cap].len()
        && !seq![cap][i].read_only
        && is_prefix_of(seq![cap][i].prefix, path)
    ) by {
        if i == 0 {
            assert(seq![cap][0].read_only);
        }
    }
}

/// File write gate: a read-write capability with matching prefix
/// allows writes.
// r[verify ucan.gate.file-write-check]
proof fn prove_rw_cap_allows_writes(
    cap: FileAccessModel,
    path: Seq<u8>,
)
    requires
        !cap.read_only,
        is_prefix_of(cap.prefix, path),
    ensures
        file_write_gate_allows(seq![cap], path)
{
    assert(
        0 <= 0 < seq![cap].len()
        && !seq![cap][0].read_only
        && is_prefix_of(seq![cap][0].prefix, path)
    );
}

} // verus!
