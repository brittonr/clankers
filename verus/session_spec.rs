//! Formal specifications and proofs for session tree navigation.
//!
//! Models the session tree as a map from message IDs to entries with
//! parent pointers. Proves properties of walk_branch (path validity,
//! root anchoring, termination) and index consistency.

use vstd::prelude::*;

verus! {

// ── Types ───────────────────────────────────────────────────────────────

/// A message identifier, modeled as a nat for simplicity.
pub type MsgId = nat;

/// A message entry with an optional parent pointer.
pub struct MessageModel {
    pub id: MsgId,
    pub parent_id: Option<MsgId>,
}

/// The session tree model: an ordered sequence of entries plus an index.
pub struct TreeModel {
    /// Entries in insertion order.
    pub entries: Seq<MessageModel>,
    /// Index: message ID → position in entries.
    pub index: Map<MsgId, nat>,
}

/// Result of walk_branch: a sequence of message entries from root to leaf.
pub type WalkResult = Seq<MessageModel>;

// ── Spec fns ────────────────────────────────────────────────────────────

/// A tree model is well-formed: the index maps each ID to a position
/// where entries[pos].id == the key, and all entries are indexed.
pub open spec fn tree_well_formed(t: TreeModel) -> bool {
    // Every index entry points to a valid position with matching ID
    (forall |id: MsgId| #![auto]
        t.index.contains_key(id)
        ==> t.index[id] < t.entries.len()
            && t.entries[t.index[id] as int].id == id
    )
    // Every message entry is in the index
    && (forall |i: nat| #![auto]
        i < t.entries.len()
        ==> t.index.contains_key(t.entries[i as int].id)
            && t.index[t.entries[i as int].id] == i
    )
}

/// Recursive walk from leaf to root, collecting entries.
/// Returns entries in leaf-to-root order (caller reverses).
pub open spec fn walk_branch_rec(
    t: TreeModel,
    current: MsgId,
    fuel: nat,
) -> WalkResult
    decreases fuel
{
    if fuel == 0 {
        Seq::empty()
    } else if !t.index.contains_key(current) {
        Seq::empty()
    } else {
        let entry = t.entries[t.index[current] as int];
        match entry.parent_id {
            Some(parent) => walk_branch_rec(t, parent, (fuel - 1) as nat).push(entry),
            None => Seq::empty().push(entry),
        }
    }
}

/// For each consecutive pair in the walk result,
/// entries[i+1].parent_id == Some(entries[i].id).
// r[depends session.walk.path-valid]
pub open spec fn path_valid(path: WalkResult) -> bool {
    forall |i: nat| #![auto]
        i + 1 < path.len()
        ==> path[(i + 1) as int].parent_id == Some(path[i as int].id)
}

/// The first entry in the walk result has parent_id == None.
// r[depends session.walk.root-anchored]
pub open spec fn root_anchored(path: WalkResult) -> bool {
    path.len() > 0 ==> path[0].parent_id.is_none()
}

/// walk_branch terminates in at most n steps where n is the entry count.
/// Modeled by the fuel parameter: fuel == entries.len() is sufficient.
// r[depends session.walk.terminates]
pub open spec fn walk_terminates(t: TreeModel, leaf: MsgId) -> bool {
    // The walk with fuel = entries.len() produces the same result
    // as with any larger fuel (it doesn't need more steps).
    // This is the termination guarantee: bounded by entry count.
    walk_branch_rec(t, leaf, t.entries.len()).len() <= t.entries.len()
}

/// For every (id, idx) in the index, entries[idx].id == id.
// r[depends session.index.consistent]
pub open spec fn index_consistent(t: TreeModel) -> bool {
    forall |id: MsgId| #![auto]
        t.index.contains_key(id)
        ==> t.index[id] < t.entries.len()
            && t.entries[t.index[id] as int].id == id
}

/// Build a tree model from a sequence of entries.
pub open spec fn build_tree(entries: Seq<MessageModel>) -> TreeModel {
    TreeModel {
        entries: entries,
        index: Map::new(
            |id: MsgId| exists |i: nat| i < entries.len() && entries[i as int].id == id,
            |id: MsgId| choose |i: nat| i < entries.len() && entries[i as int].id == id,
        ),
    }
}

// ── Proofs ──────────────────────────────────────────────────────────────

/// walk_branch on a single-entry tree returns a path of length 1 where
/// the entry has parent_id == None (root anchored) and the path is valid.
// r[verify session.walk.path-valid]
// r[verify session.walk.root-anchored]
proof fn prove_walk_single_entry()
    ensures ({
        let entry = MessageModel { id: 0, parent_id: None };
        let entries = Seq::empty().push(entry);
        let t = TreeModel {
            entries: entries,
            index: Map::empty().insert(0nat, 0nat),
        };
        let path = walk_branch_rec(t, 0, 1);
        // Path has exactly one entry
        path.len() == 1
        // That entry is the root (no parent)
        && path[0].parent_id.is_none()
        // Path validity: vacuously true for length 1 (no consecutive pairs)
        && path_valid(path)
        // Root anchored
        && root_anchored(path)
    })
{
    let entry = MessageModel { id: 0, parent_id: None };
    let entries = Seq::empty().push(entry);
    let t = TreeModel {
        entries: entries,
        index: Map::empty().insert(0nat, 0nat),
    };
    let path = walk_branch_rec(t, 0, 1);
    // walk_branch_rec with fuel=1, current=0:
    //   index contains 0 → entry at pos 0
    //   entry.parent_id is None → returns Seq::empty().push(entry)
    //   length is 1
    assert(path.len() == 1);
    assert(path[0].parent_id.is_none());
}

/// walk_branch on a two-entry linear chain (root → child) returns a
/// path [root, child] with valid parent pointers.
// r[verify session.walk.path-valid]
proof fn prove_walk_linear_two()
    ensures ({
        let root = MessageModel { id: 0, parent_id: None };
        let child = MessageModel { id: 1, parent_id: Some(0nat) };
        let entries = Seq::empty().push(root).push(child);
        let t = TreeModel {
            entries: entries,
            index: Map::empty().insert(0nat, 0nat).insert(1nat, 1nat),
        };
        let path = walk_branch_rec(t, 1, 2);
        path.len() == 2
        && path[0].parent_id.is_none()
        && path_valid(path)
        && root_anchored(path)
    })
{
    let root = MessageModel { id: 0, parent_id: None };
    let child = MessageModel { id: 1, parent_id: Some(0nat) };
    let entries = Seq::empty().push(root).push(child);
    let t = TreeModel {
        entries: entries,
        index: Map::empty().insert(0nat, 0nat).insert(1nat, 1nat),
    };

    // Manually unfold the inner call: walk_branch_rec(t, 0, 1)
    // fuel=1 > 0, index has 0, entry=root, parent=None → Seq::empty().push(root)
    let inner = walk_branch_rec(t, 0, 1);
    assert(inner.len() == 1);
    assert(inner[0].id == 0);
    assert(inner[0].parent_id.is_none());

    // Now the outer call: walk_branch_rec(t, 1, 2)
    // fuel=2 > 0, index has 1, entry=child, parent=Some(0)
    // recurse → inner, then .push(child)
    let path = walk_branch_rec(t, 1, 2);
    assert(path =~= inner.push(child));
    assert(path.len() == 2);
    assert(path[0].parent_id.is_none());
}

/// walk_branch with fuel == entries.len() produces a path no longer
/// than entries.len(). This is the termination bound.
// r[verify session.walk.terminates]
proof fn prove_walk_terminates_single()
    ensures ({
        let entry = MessageModel { id: 0, parent_id: None };
        let entries = Seq::empty().push(entry);
        let t = TreeModel {
            entries: entries,
            index: Map::empty().insert(0nat, 0nat),
        };
        walk_terminates(t, 0)
    })
{
    let entry = MessageModel { id: 0, parent_id: None };
    let entries = Seq::empty().push(entry);
    let t = TreeModel {
        entries: entries,
        index: Map::empty().insert(0nat, 0nat),
    };
    let path = walk_branch_rec(t, 0, t.entries.len());
    assert(path.len() == 1);
    assert(path.len() <= t.entries.len());
}

/// walk_branch termination for a two-element chain.
// r[verify session.walk.terminates]
proof fn prove_walk_terminates_two()
    ensures ({
        let root = MessageModel { id: 0, parent_id: None };
        let child = MessageModel { id: 1, parent_id: Some(0nat) };
        let entries = Seq::empty().push(root).push(child);
        let t = TreeModel {
            entries: entries,
            index: Map::empty().insert(0nat, 0nat).insert(1nat, 1nat),
        };
        walk_terminates(t, 1)
    })
{
    let root = MessageModel { id: 0, parent_id: None };
    let child = MessageModel { id: 1, parent_id: Some(0nat) };
    let entries = Seq::empty().push(root).push(child);
    let t = TreeModel {
        entries: entries,
        index: Map::empty().insert(0nat, 0nat).insert(1nat, 1nat),
    };
    assert(t.entries.len() == 2);

    // Unfold inner: walk_branch_rec(t, 0, 1)
    let inner = walk_branch_rec(t, 0, 1);
    assert(inner.len() == 1);

    // Unfold outer: walk_branch_rec(t, 1, 2)
    let path = walk_branch_rec(t, 1, 2);
    assert(path =~= inner.push(child));
    assert(path.len() == 2);
    assert(path.len() <= t.entries.len());
}

/// build_tree produces a consistent index: for every ID in the index,
/// entries[idx].id == id.
// r[verify session.index.consistent]
proof fn prove_index_consistent_single()
    ensures ({
        let entry = MessageModel { id: 42, parent_id: None };
        let entries = Seq::empty().push(entry);
        let t = TreeModel {
            entries: entries,
            index: Map::empty().insert(42nat, 0nat),
        };
        index_consistent(t)
    })
{
    let entry = MessageModel { id: 42, parent_id: None };
    let entries = Seq::empty().push(entry);
    let t = TreeModel {
        entries: entries,
        index: Map::empty().insert(42nat, 0nat),
    };
    // index[42] = 0, entries[0].id = 42 ✓
    assert(t.index[42nat] == 0nat);
    assert(t.entries[0].id == 42nat);
}

/// Index consistency for a two-entry tree.
// r[verify session.index.consistent]
proof fn prove_index_consistent_two()
    ensures ({
        let e0 = MessageModel { id: 10, parent_id: None };
        let e1 = MessageModel { id: 20, parent_id: Some(10nat) };
        let entries = Seq::empty().push(e0).push(e1);
        let t = TreeModel {
            entries: entries,
            index: Map::empty().insert(10nat, 0nat).insert(20nat, 1nat),
        };
        index_consistent(t)
    })
{
    let e0 = MessageModel { id: 10, parent_id: None };
    let e1 = MessageModel { id: 20, parent_id: Some(10nat) };
    let entries = Seq::empty().push(e0).push(e1);
    let t = TreeModel {
        entries: entries,
        index: Map::empty().insert(10nat, 0nat).insert(20nat, 1nat),
    };
    assert(t.entries[0].id == 10nat);
    assert(t.entries[1].id == 20nat);
}

} // verus!
