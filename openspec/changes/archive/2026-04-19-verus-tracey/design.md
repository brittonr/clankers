# Design — Verus + Tracey Integration

## File Layout

```
clankers/
├── docs/
│   └── requirements.md              ← Tracey spec source (copied from openspec on sync)
├── verus/
│   ├── lib.rs                       ← Module root
│   ├── merge_spec.rs                ← Graggle invariant specs + proofs
│   ├── actor_spec.rs                ← Actor link/monitor specs + proofs
│   ├── session_spec.rs              ← Session tree walk specs + proofs
│   └── protocol_spec.rs             ← Frame round-trip specs + proofs
├── .config/
│   └── tracey/
│       └── config.styx              ← Tracey wiring
├── crates/
│   ├── clankers-merge/src/*.rs      ← r[impl merge.*] annotations
│   ├── clankers-actor/src/*.rs      ← r[impl actor.*] annotations
│   ├── clankers-session/src/*.rs    ← r[impl session.*] annotations
│   └── clankers-protocol/src/*.rs   ← r[impl protocol.*] annotations
└── scripts/
    └── verify.sh                    ← Combined verus + tracey check
```

## Tracey Configuration

```styx
specs (
  {
    name clankers-invariants
    include (docs/requirements.md)
    impls (
      {
        name rust
        include (
          crates/clankers-merge/src/**/*.rs
          crates/clankers-actor/src/**/*.rs
          crates/clankers-session/src/**/*.rs
          crates/clankers-protocol/src/**/*.rs
          verus/**/*.rs
        )
        exclude (target/** .cargo-target/**)
        test_include (
          crates/clankers-merge/src/*_tests.rs
          crates/clankers-actor/src/*_tests.rs
          crates/clankers-session/src/tests/**/*.rs
          crates/clankers-protocol/src/*_tests.rs
        )
      }
    )
  }
)
```

`verus/**/*.rs` is in `include` (not `test_include`) because it contains
both `r[depends ...]` on spec fns and `r[verify ...]` on proof fns.

## Verus Module Structure

Each `*_spec.rs` file contains three layers:

1. **Type models** — Pure spec types that mirror the runtime types. A
   `Graggle` in verus is a `Map<VertexId, Vertex>` + `Map<VertexId,
   Set<VertexId>>`, not the runtime `BTreeMap`. This keeps the specs
   independent of collection implementation.

2. **Spec fns** — Annotated `r[depends req.id]`. Define what the invariant
   means mathematically. Example: `spec fn well_formed(g: GraggleModel) ->
   bool` that conjoins sentinel presence + reachability + acyclicity.

3. **Proof fns** — Annotated `r[verify req.id]`. Discharge the spec using
   Verus's SMT backend. For properties like merge order-independence,
   inductive proofs with `decreases` clauses.

## Annotation Strategy

**Combined pattern** for simple properties where the exec fn can carry
`requires`/`ensures` directly. Used for `from_text`, `delete_vertex`,
frame size rejection — functions where the postcondition is local.

**Split pattern** for cross-cutting invariants. Used for DAG well-formedness
(referenced by multiple functions), link bidirectionality (referenced by
link, unlink, and on_process_exit), and walk path validity (recursive
property over the walk output). The split pattern keeps `src/` code free
of verus macro syntax.

## Flake Changes

Add verus as a flake input pinned to a specific commit:

```nix
inputs.verus = {
  url = "github:verus-lang/verus/<pinned-rev>";
  flake = false;
};
```

Build verus in an overlay and add it to the devshell's `buildInputs`.
Tracey is already available system-wide.

## CI Integration

Add a check to `flake.nix`:

```nix
checks.verus-proofs = pkgs.runCommand "verus-proofs" {
  nativeBuildInputs = [ verus ];
} ''
  cd ${self}
  verus --crate-type=lib verus/lib.rs
  touch $out
'';

checks.tracey-coverage = pkgs.runCommand "tracey-coverage" {
  nativeBuildInputs = [ tracey ];
} ''
  cd ${self}
  tracey query status
  # Fail if any requirement is uncovered
  tracey query uncovered --exit-code
  touch $out
'';
```

Both run as part of `nix flake check`.

## Modeling Decisions

### Graggle

The runtime `Graggle` uses `BTreeMap<VertexId, BTreeSet<VertexId>>` for
edges. The verus model uses `Map<VertexId, Set<VertexId>>` from vstd.
Reachability is defined recursively with a `decreases` clause on a visited
set size bound.

Order-independence proof strategy: show that for any two patches P1 and P2
that touch disjoint context regions, `apply(apply(base, P1), P2)` and
`apply(apply(base, P2), P1)` produce identical graggles. This is the
commutativity lemma. The full n-way case follows by induction on the
number of patches.

### Actor registry

Model as a pure state: `(processes: Set<ProcessId>, links:
Map<ProcessId, Set<ProcessId>>, monitors: Map<ProcessId, Set<ProcessId>>)`.
Each operation (link, unlink, spawn, exit) is a pure function from old
state to new state. Prove post-conditions on each transition.

### Session tree

Model `walk_branch` as a recursive function from leaf to root via
parent_id lookup. Prove termination by showing the visited set strictly
grows each step (and is bounded by message count). Prove path validity
by induction on the walk.

### Protocol framing

Model `write_frame` and `read_frame` as operations on `Seq<u8>`. The
round-trip proof shows that `to_be_bytes(len) ++ json_bytes` parsed back
yields the original json_bytes, and `serde_json::from_slice(to_vec(v)) == v`
for any `v` (assumed as an axiom — serde correctness is out of scope).
