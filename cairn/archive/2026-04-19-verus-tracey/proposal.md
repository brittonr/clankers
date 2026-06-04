# verus-tracey

## Intent

The merge algorithm, actor registry, session tree, and protocol framing all
have invariants that are stated in comments and tested with examples, but
never machine-checked. A test that merges three branches and checks the
output is evidence — it doesn't prove the property holds for all inputs.
The merge function's order-independence claim is especially vulnerable:
a permutation test over 4 orderings is not a proof over all n! orderings.

Add Verus for machine-checked proofs of the critical invariants. Add Tracey
to track which requirements have specs, which specs have proofs, and which
have neither. Together they close the gap between "we wrote a test" and
"this property holds unconditionally."

Start narrow. Pick the four crates with the richest pure-functional
invariants, formalize their core properties, prove them, and wire Tracey
coverage into CI. Expand later.

## Scope

### In Scope

- Requirements spec in `docs/requirements.md` covering invariants for:
  - `clankers-merge` — graggle DAG well-formedness, merge order independence
  - `clankers-actor` — link bidirectionality, monitor cleanup on exit
  - `clankers-session` — tree walk correctness, index consistency
  - `clankers-protocol` — frame round-trip, size bound enforcement
- `verus/` directory with spec fns and proof fns for the above
- Tracey annotations (`r[impl ...]`, `r[depends ...]`, `r[verify ...]`) on
  existing source and new verus code
- `.config/tracey/config.styx` wiring up specs → source → tests
- Verus added to the flake devshell
- CI gate: `verus --crate-type=lib verus/lib.rs && tracey query status`

### Out of Scope

- Verifying async code (Verus doesn't support `async fn`)
- Verifying the TUI, LLM provider layer, or config system
- Rewriting existing code to fit Verus — the split pattern keeps exec code
  in `src/` untouched; verus specs model the same logic separately
- Tracey LSP or dashboard deployment (CLI checks are enough for now)
- Verifying WASM plugin sandboxing

## Approach

Three phases:

### Phase 1 — Scaffold and graggle proofs

Set up the tooling: Verus in the flake, Tracey config, requirements doc.
Start with `clankers-merge` because Graggle is a pure data structure with
no IO, no async, no external dependencies. Its invariants are stated right
in the doc comments:

- ROOT and END always exist
- Every content vertex is reachable from ROOT
- Every content vertex can reach END
- The graph is acyclic
- Merge result is independent of branch ordering

Write `spec fn` definitions for these properties, `proof fn` lemmas that
discharge them, and annotate both the verus code and the existing `src/`
implementations with Tracey markers.

### Phase 2 — Actor and session proofs

Extend to `clankers-actor` and `clankers-session`. Model the actor registry
as a pure state machine (links map, monitors map, process set) and prove:

- `link(a, b)` makes both `links[a]` contain b and `links[b]` contain a
- `unlink(a, b)` removes both directions
- `on_process_exit(id)` cleans up all links and monitors referencing id
- No process ID appears in both a link entry and the process map after removal

For the session tree, prove:

- `walk_branch(leaf)` returns a path where each entry's parent_id matches
  the previous entry's id
- `walk_branch(leaf)` starts from a root (parent_id = None)
- The index map is consistent: every key maps to a valid index into entries

These are pure-function properties over HashMap/Vec state — no async needed.

### Phase 3 — Protocol proofs and CI

Prove `write_frame` / `read_frame` round-trip: for any `T: Serialize +
DeserializeOwned`, if `write_frame` succeeds, `read_frame` on the output
yields the original value. Prove the size bound: `write_frame` rejects
payloads > MAX_FRAME_SIZE, and `read_frame` rejects length headers >
MAX_FRAME_SIZE before allocating.

Wire `verus` and `tracey query status` into `nix flake check`. Gate PRs on
both passing.

## Risks

- **Verus version churn.** Verus is pre-1.0. Syntax or vstd API may change.
  Mitigate by pinning a specific verus commit in the flake input.
- **Split pattern drift.** The verus specs model the same logic as `src/`
  but aren't compiled together. A refactor could change runtime behavior
  without updating the spec. Mitigate with Tracey version bumps — when a
  requirement changes, stale annotations are flagged.
- **Proof effort.** Order-independence for n-way merge is nontrivial. May
  need to start with 2-way and generalize via induction. Budget time for
  the proof to take longer than the code.
