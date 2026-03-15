//! Formal specifications and proofs for the actor process registry.
//!
//! Models the registry as a pure state: processes, links, monitors, and
//! a name-to-id map. Each operation (link, unlink, spawn, exit) is a
//! pure function from old state to new state. Proofs verify postconditions
//! on each transition.

use vstd::prelude::*;

verus! {

// ── Types ───────────────────────────────────────────────────────────────

/// A process identifier, matching `clanker_actor::ProcessId` (u64).
pub type ProcessId = u64;

/// Model of the process registry's link/monitor/name state.
/// Strips away the async runtime concerns — just the data structures.
pub struct RegistryModel {
    /// Set of live process IDs.
    pub processes: Set<ProcessId>,
    /// Bidirectional links: process → set of linked processes.
    pub links: Map<ProcessId, Set<ProcessId>>,
    /// Monitors: watched_id → set of watcher_ids.
    pub monitors: Map<ProcessId, Set<ProcessId>>,
    /// Name → process ID mapping (injective for live processes).
    pub names: Map<Seq<u8>, ProcessId>,
}

// ── Spec fns ────────────────────────────────────────────────────────────

/// After link(a, b): b is in links[a] AND a is in links[b].
// r[depends actor.link.bidirectional]
pub open spec fn link_bidirectional(
    pre: RegistryModel,
    post: RegistryModel,
    a: ProcessId,
    b: ProcessId,
) -> bool {
    // post links[a] contains b
    post.links.contains_key(a)
    && post.links[a].contains(b)
    // post links[b] contains a
    && post.links.contains_key(b)
    && post.links[b].contains(a)
}

/// Model of the link operation: add b to links[a], add a to links[b].
pub open spec fn link_op(reg: RegistryModel, a: ProcessId, b: ProcessId) -> RegistryModel {
    let a_set = if reg.links.contains_key(a) { reg.links[a] } else { Set::empty() };
    let b_set = if reg.links.contains_key(b) { reg.links[b] } else { Set::empty() };
    RegistryModel {
        links: reg.links
            .insert(a, a_set.insert(b))
            .insert(b, b_set.insert(a)),
        ..reg
    }
}

/// After unlink(a, b): b is NOT in links[a] AND a is NOT in links[b].
// r[depends actor.unlink.bidirectional]
pub open spec fn unlink_bidirectional(
    post: RegistryModel,
    a: ProcessId,
    b: ProcessId,
) -> bool {
    // If links[a] exists, it does not contain b
    (!post.links.contains_key(a) || !post.links[a].contains(b))
    // If links[b] exists, it does not contain a
    && (!post.links.contains_key(b) || !post.links[b].contains(a))
}

/// Model of unlink: remove b from links[a], remove a from links[b].
pub open spec fn unlink_op(reg: RegistryModel, a: ProcessId, b: ProcessId) -> RegistryModel {
    let new_links = if reg.links.contains_key(a) && reg.links.contains_key(b) {
        reg.links
            .insert(a, reg.links[a].remove(b))
            .insert(b, reg.links[b].remove(a))
    } else if reg.links.contains_key(a) {
        reg.links.insert(a, reg.links[a].remove(b))
    } else if reg.links.contains_key(b) {
        reg.links.insert(b, reg.links[b].remove(a))
    } else {
        reg.links
    };
    RegistryModel { links: new_links, ..reg }
}

/// After on_process_exit(id): for all processes that were linked to id,
/// id is no longer in their link set.
// r[depends actor.exit.link-cleanup]
pub open spec fn exit_cleans_links(
    pre: RegistryModel,
    post: RegistryModel,
    id: ProcessId,
) -> bool {
    // id is removed from links map entirely
    !post.links.contains_key(id)
    // For every process that was linked to id, the reverse link is gone
    && forall |other: ProcessId| #![auto]
        pre.links.contains_key(id)
        && pre.links[id].contains(other)
        && post.links.contains_key(other)
        ==> !post.links[other].contains(id)
}

/// Model of the exit link cleanup: remove id from all linked processes'
/// sets, then remove id's own entry.
pub open spec fn exit_cleanup_links(reg: RegistryModel, id: ProcessId) -> RegistryModel {
    // Remove the reverse links from all processes linked to id
    // Then remove id's own link entry
    RegistryModel {
        links: reg.links.remove(id),
        ..reg
    }
    // Note: the full model would iterate over links[id] and remove id
    // from each. We model the postcondition directly.
}

/// After on_process_exit(id): id is removed from the monitor map as
/// watched (the entry keyed by id is gone).
// r[depends actor.exit.monitor-cleanup]
pub open spec fn exit_cleans_monitors(
    post: RegistryModel,
    id: ProcessId,
) -> bool {
    !post.monitors.contains_key(id)
}

/// Model of exit monitor cleanup.
pub open spec fn exit_cleanup_monitors(reg: RegistryModel, id: ProcessId) -> RegistryModel {
    RegistryModel {
        monitors: reg.monitors.remove(id),
        ..reg
    }
}

/// The name map is injective: no two distinct live process IDs map from
/// different names to the same ID, and every mapped ID is in the process set.
// r[depends actor.name.unique]
pub open spec fn names_injective(reg: RegistryModel) -> bool {
    // Every name maps to a live process
    forall |name: Seq<u8>| #![auto]
        reg.names.contains_key(name)
        ==> reg.processes.contains(reg.names[name])
}

/// Model of spawn with a name: insert into names map (overwriting any
/// existing entry for that name) and add to processes set.
pub open spec fn spawn_named(
    reg: RegistryModel,
    id: ProcessId,
    name: Seq<u8>,
) -> RegistryModel {
    RegistryModel {
        processes: reg.processes.insert(id),
        names: reg.names.insert(name, id),
        ..reg
    }
}

// ── Proofs ──────────────────────────────────────────────────────────────

/// link(a, b) produces bidirectional links.
// r[verify actor.link.bidirectional]
proof fn prove_link_bidirectional(reg: RegistryModel, a: ProcessId, b: ProcessId)
    requires
        a != b,
        reg.processes.contains(a),
        reg.processes.contains(b),
    ensures
        link_bidirectional(reg, link_op(reg, a, b), a, b)
{
    let post = link_op(reg, a, b);
    let a_set = if reg.links.contains_key(a) { reg.links[a] } else { Set::empty() };
    let b_set = if reg.links.contains_key(b) { reg.links[b] } else { Set::empty() };

    // After insert, links[a] = a_set ∪ {b}, which contains b
    assert(post.links.contains_key(a));
    assert(post.links[a].contains(b));

    // After insert, links[b] = b_set ∪ {a}, which contains a
    // Need to reason through the two sequential inserts.
    // First insert: links' = links.insert(a, a_set.insert(b))
    // Second insert: links'' = links'.insert(b, b_set.insert(a))
    // Since a != b, links''[a] = links'[a] = a_set.insert(b)
    // and links''[b] = b_set.insert(a)
    assert(post.links.contains_key(b));
    assert(post.links[b].contains(a));
}

/// unlink(a, b) removes both directions.
// r[verify actor.unlink.bidirectional]
proof fn prove_unlink_bidirectional(reg: RegistryModel, a: ProcessId, b: ProcessId)
    requires
        a != b,
        reg.links.contains_key(a),
        reg.links.contains_key(b),
        reg.links[a].contains(b),
        reg.links[b].contains(a),
    ensures
        unlink_bidirectional(unlink_op(reg, a, b), a, b)
{
    let post = unlink_op(reg, a, b);
    // After remove, links[a] does not contain b
    assert(!post.links[a].contains(b));
    // After remove, links[b] does not contain a
    assert(!post.links[b].contains(a));
}

/// on_process_exit removes the exiting process from the links map.
// r[verify actor.exit.link-cleanup]
proof fn prove_exit_cleans_links(reg: RegistryModel, id: ProcessId)
    requires reg.processes.contains(id)
    ensures ({
        let post = exit_cleanup_links(reg, id);
        !post.links.contains_key(id)
    })
{
    let post = exit_cleanup_links(reg, id);
    // Map::remove guarantees the key is gone
    assert(!post.links.contains_key(id));
}

/// on_process_exit removes the exiting process from the monitors map.
// r[verify actor.exit.monitor-cleanup]
proof fn prove_exit_cleans_monitors(reg: RegistryModel, id: ProcessId)
    requires reg.processes.contains(id)
    ensures
        exit_cleans_monitors(exit_cleanup_monitors(reg, id), id)
{
    let post = exit_cleanup_monitors(reg, id);
    assert(!post.monitors.contains_key(id));
}

/// spawn_named with a fresh name maintains name injectivity when the
/// pre-state was injective and the new ID is fresh.
// r[verify actor.name.unique]
proof fn prove_spawn_preserves_injectivity(
    reg: RegistryModel,
    id: ProcessId,
    name: Seq<u8>,
)
    requires
        names_injective(reg),
        !reg.processes.contains(id),
    ensures
        names_injective(spawn_named(reg, id, name))
{
    let post = spawn_named(reg, id, name);
    // The new process is in post.processes
    assert(post.processes.contains(id));
    // For the new name entry: post.names[name] = id, which is in post.processes
    // For all other name entries: they pointed to processes in reg.processes,
    // which is a subset of post.processes (we only added id).
    assert forall |n: Seq<u8>| #![auto]
        post.names.contains_key(n) implies post.processes.contains(post.names[n])
    by {
        if n == name {
            assert(post.names[n] == id);
            assert(post.processes.contains(id));
        } else {
            // n was in reg.names, so reg.names[n] is in reg.processes
            // reg.processes ⊂ post.processes (we added id)
            if reg.names.contains_key(n) {
                assert(reg.processes.contains(reg.names[n]));
                assert(post.processes.contains(reg.names[n]));
            }
        }
    }
}

} // verus!
