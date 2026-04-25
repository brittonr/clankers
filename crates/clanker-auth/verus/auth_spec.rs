//! Formal specifications and proofs for the capability token framework.
//!
//! Models the builder's escalation/depth/delegate checks, the verifier's
//! revocation/audience/chain-walk logic, and the credential's self-contained
//! verification. Capabilities are abstract (nat + containment relation) so
//! these proofs hold for any Cap implementation.

use vstd::prelude::*;

verus! {

// ── Abstract Models ─────────────────────────────────────────────────────

/// Abstract token: issuer, capabilities, delegation depth, proof link.
/// Crypto (signatures, hashing) provides integrity — we model the
/// authorization logic only.
pub struct TokenModel {
    pub issuer: nat,
    pub caps: Seq<nat>,
    pub depth: nat,
    pub proof_hash: Option<nat>,
    pub token_hash: nat,
}

// ── Builder Specs ───────────────────────────────────────────────────────

/// A single child capability is covered by at least one parent capability.
pub open spec fn cap_covered(
    parent_caps: Seq<nat>,
    child_cap: nat,
    contains: spec_fn(nat, nat) -> bool,
) -> bool {
    exists |j: int| 0 <= j < parent_caps.len() && contains(parent_caps[j], child_cap)
}

/// Every child capability is contained by at least one parent capability.
/// Models the builder's `for cap in &self.capabilities { ... }` loop.
// r[depends auth.build.no-escalation]
pub open spec fn all_contained(
    parent_caps: Seq<nat>,
    child_caps: Seq<nat>,
    contains: spec_fn(nat, nat) -> bool,
) -> bool {
    forall |i: int| 0 <= i < child_caps.len() ==>
        cap_covered(parent_caps, child_caps[i], contains)
}

/// At least one capability in the set satisfies the delegate predicate.
/// Models the builder's `parent.capabilities.iter().any(Cap::is_delegate)`.
// r[depends auth.build.delegate-required]
pub open spec fn has_delegate(
    caps: Seq<nat>,
    is_delegate: spec_fn(nat) -> bool,
) -> bool {
    exists |i: int| 0 <= i < caps.len() && is_delegate(caps[i])
}

/// Delegation depth is within the allowed bound.
/// Models the builder's `new_depth > MAX_DELEGATION_DEPTH` check.
// r[depends auth.build.depth-bound]
pub open spec fn depth_within_bound(parent_depth: nat, max_depth: nat) -> bool {
    parent_depth < max_depth
}

// ── Verifier Specs ──────────────────────────────────────────────────────

/// Token passes the revocation check (hash not in revoked set).
// r[depends auth.verify.revocation]
pub open spec fn passes_revocation(revoked: Set<nat>, token_hash: nat) -> bool {
    !revoked.contains(token_hash)
}

/// Audience matches: bearer tokens always pass, key-bound tokens require
/// the presenter's key to match.
// r[depends auth.verify.audience]
pub open spec fn audience_ok(
    token_audience: Option<nat>,
    presenter: Option<nat>,
) -> bool {
    match (token_audience, presenter) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// Every adjacent pair in the chain is linked by proof hash.
// r[depends auth.verify.chain-complete]
pub open spec fn chain_linked(chain: Seq<TokenModel>) -> bool {
    forall |i: int| 0 <= i && i + 1 < chain.len() ==>
        chain[i].proof_hash == Some(chain[i + 1].token_hash)
}

/// The chain terminates at a trusted root issuer.
pub open spec fn chain_rooted(
    chain: Seq<TokenModel>,
    trusted_roots: Set<nat>,
) -> bool {
    chain.len() > 0 && trusted_roots.contains(chain[chain.len() - 1].issuer)
}

/// A chain is valid if every link is present and it reaches a trusted root.
pub open spec fn chain_valid(
    chain: Seq<TokenModel>,
    trusted_roots: Set<nat>,
) -> bool {
    chain_linked(chain) && chain_rooted(chain, trusted_roots)
}

// ── Credential Spec ─────────────────────────────────────────────────────

/// A credential is self-contained if its leaf + proofs form a valid chain.
// r[depends auth.credential.self-contained]
pub open spec fn credential_self_contained(
    leaf: TokenModel,
    proofs: Seq<TokenModel>,
    trusted_roots: Set<nat>,
) -> bool {
    chain_valid(seq![leaf] + proofs, trusted_roots)
}

// ── Composition Spec ────────────────────────────────────────────────────

/// The containment relation is transitive: if a contains b and b contains c,
/// then a contains c. Required for multi-level delegation soundness.
// r[depends auth.delegation.transitivity]
pub open spec fn contains_transitive(
    contains: spec_fn(nat, nat) -> bool,
) -> bool {
    forall |a: nat, b: nat, c: nat|
        #![trigger contains(a, b), contains(b, c)]
        contains(a, b) && contains(b, c) ==> contains(a, c)
}

// ── Builder Proofs ──────────────────────────────────────────────────────

/// If any child cap is not covered by a parent cap, all_contained is false.
/// This is the contrapositive of the escalation check: the builder's loop
/// finds the first uncovered cap and returns CapabilityEscalation.
// r[verify auth.build.no-escalation]
proof fn prove_escalation_detected(
    parent_caps: Seq<nat>,
    child_caps: Seq<nat>,
    bad_idx: int,
    contains: spec_fn(nat, nat) -> bool,
)
    requires
        0 <= bad_idx < child_caps.len(),
        !cap_covered(parent_caps, child_caps[bad_idx], contains),
    ensures
        !all_contained(parent_caps, child_caps, contains)
{
}

/// If all caps are contained, every individual cap is covered.
/// Positive direction: when the check passes, every cap has a parent.
// r[verify auth.build.no-escalation]
proof fn prove_contained_caps_pass(
    parent_caps: Seq<nat>,
    child_caps: Seq<nat>,
    idx: int,
    contains: spec_fn(nat, nat) -> bool,
)
    requires
        all_contained(parent_caps, child_caps, contains),
        0 <= idx < child_caps.len(),
    ensures
        cap_covered(parent_caps, child_caps[idx], contains)
{
}

/// When parent depth is at or beyond max, delegation is rejected.
// r[verify auth.build.depth-bound]
proof fn prove_depth_bound_enforced(parent_depth: nat, max_depth: nat)
    requires parent_depth >= max_depth
    ensures !depth_within_bound(parent_depth, max_depth)
{
}

/// When no capability satisfies is_delegate, has_delegate is false.
// r[verify auth.build.delegate-required]
proof fn prove_no_delegate_blocks(
    caps: Seq<nat>,
    is_delegate: spec_fn(nat) -> bool,
)
    requires forall |i: int| 0 <= i < caps.len() ==> !is_delegate(caps[i])
    ensures !has_delegate(caps, is_delegate)
{
}

// ── Verifier Proofs ─────────────────────────────────────────────────────

/// A revoked token fails the revocation check.
// r[verify auth.verify.revocation]
proof fn prove_revocation_blocks(revoked: Set<nat>, hash: nat)
    requires revoked.contains(hash)
    ensures !passes_revocation(revoked, hash)
{
}

/// A non-revoked token passes the revocation check.
// r[verify auth.verify.revocation]
proof fn prove_non_revoked_passes(revoked: Set<nat>, hash: nat)
    requires !revoked.contains(hash)
    ensures passes_revocation(revoked, hash)
{
}

/// Wrong presenter key fails audience check.
// r[verify auth.verify.audience]
proof fn prove_wrong_audience_rejected(expected: nat, actual: nat)
    requires expected != actual
    ensures !audience_ok(Some(expected), Some(actual))
{
}

/// Missing presenter fails audience check for key-bound tokens.
// r[verify auth.verify.audience]
proof fn prove_missing_presenter_rejected(expected: nat)
    ensures !audience_ok(Some(expected), None)
{
}

/// Bearer tokens pass audience check regardless of presenter.
// r[verify auth.verify.audience]
proof fn prove_bearer_always_passes(presenter: Option<nat>)
    ensures audience_ok(None, presenter)
{
}

/// A broken link in the chain invalidates chain_valid.
// r[verify auth.verify.chain-complete]
proof fn prove_broken_chain_rejected(
    chain: Seq<TokenModel>,
    break_idx: int,
    trusted_roots: Set<nat>,
)
    requires
        chain.len() > 1,
        0 <= break_idx,
        break_idx + 1 < chain.len(),
        chain[break_idx].proof_hash != Some(chain[break_idx + 1].token_hash),
    ensures
        !chain_valid(chain, trusted_roots)
{
    // break_idx is a counterexample to the forall in chain_linked
    assert(!chain_linked(chain));
}

/// A chain not terminating at a trusted root is invalid.
// r[verify auth.verify.chain-complete]
proof fn prove_untrusted_root_rejected(
    chain: Seq<TokenModel>,
    trusted_roots: Set<nat>,
)
    requires
        chain.len() > 0,
        !trusted_roots.contains(chain[chain.len() - 1].issuer),
    ensures
        !chain_valid(chain, trusted_roots)
{
    assert(!chain_rooted(chain, trusted_roots));
}

// ── Credential Proofs ───────────────────────────────────────────────────

/// A self-contained credential has a valid chain by definition.
// r[verify auth.credential.self-contained]
proof fn prove_credential_valid(
    leaf: TokenModel,
    proofs: Seq<TokenModel>,
    trusted_roots: Set<nat>,
)
    requires credential_self_contained(leaf, proofs, trusted_roots)
    ensures chain_valid(seq![leaf] + proofs, trusted_roots)
{
}

/// Adding a correctly-linked leaf to a valid chain produces a valid chain.
/// Models Credential::delegate() growing the proof list.
// r[verify auth.credential.self-contained]
proof fn prove_delegate_extends_chain(
    new_leaf: TokenModel,
    old_leaf: TokenModel,
    old_proofs: Seq<TokenModel>,
    trusted_roots: Set<nat>,
)
    requires
        chain_valid(seq![old_leaf] + old_proofs, trusted_roots),
        new_leaf.proof_hash == Some(old_leaf.token_hash),
    ensures
        chain_linked(seq![new_leaf] + (seq![old_leaf] + old_proofs))
{
    let old_chain = seq![old_leaf] + old_proofs;
    let new_chain = seq![new_leaf] + old_chain;

    // old_chain is linked (from chain_valid precondition)
    assert(chain_linked(old_chain));

    // Prove new_chain is linked: check every adjacent pair
    assert forall |i: int| 0 <= i && i + 1 < new_chain.len()
    implies new_chain[i].proof_hash == Some(new_chain[i + 1].token_hash)
    by {
        if i == 0 {
            // new_chain[0] = new_leaf, new_chain[1] = old_leaf
            assert(new_chain[0] == new_leaf);
            assert(new_chain[1] == old_leaf);
            assert(new_leaf.proof_hash == Some(old_leaf.token_hash));
        } else {
            // i >= 1: new_chain[i] = old_chain[i-1], new_chain[i+1] = old_chain[i]
            assert(new_chain[i] == old_chain[i - 1]);
            assert(new_chain[i + 1] == old_chain[i]);
            // old_chain is linked, so old_chain[i-1].proof_hash == Some(old_chain[i].token_hash)
            assert(old_chain[i - 1].proof_hash == Some(old_chain[i].token_hash));
        }
    }
}

// ── Composition Proof ───────────────────────────────────────────────────

/// If containment is transitive and the builder checks containment at
/// each delegation level, then multi-level delegation cannot escalate
/// beyond the root's capabilities.
///
/// Concretely: if root contains mid (checked at level 1) and mid contains
/// leaf (checked at level 2), then root contains leaf (by transitivity).
/// This generalizes to any chain depth.
// r[verify auth.delegation.transitivity]
proof fn prove_no_transitive_escalation(
    root_caps: Seq<nat>,
    mid_caps: Seq<nat>,
    leaf_caps: Seq<nat>,
    contains: spec_fn(nat, nat) -> bool,
)
    requires
        contains_transitive(contains),
        all_contained(root_caps, mid_caps, contains),
        all_contained(mid_caps, leaf_caps, contains),
    ensures
        all_contained(root_caps, leaf_caps, contains)
{
    assert forall |i: int| 0 <= i < leaf_caps.len()
    implies cap_covered(root_caps, leaf_caps[i], contains)
    by {
        // leaf_caps[i] is covered by some mid cap
        assert(cap_covered(mid_caps, leaf_caps[i], contains));
        let mid_j = choose |j: int|
            0 <= j < mid_caps.len() && contains(mid_caps[j], leaf_caps[i]);

        // that mid cap is covered by some root cap
        assert(cap_covered(root_caps, mid_caps[mid_j], contains));
        let root_k = choose |k: int|
            0 <= k < root_caps.len() && contains(root_caps[k], mid_caps[mid_j]);

        // By transitivity: root_caps[root_k] contains leaf_caps[i]
        assert(contains(root_caps[root_k], mid_caps[mid_j]));
        assert(contains(mid_caps[mid_j], leaf_caps[i]));
        assert(contains(root_caps[root_k], leaf_caps[i]));
    }
}

} // verus!
