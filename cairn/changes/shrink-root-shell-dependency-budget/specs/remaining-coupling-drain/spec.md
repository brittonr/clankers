## ADDED Requirements

### Requirement: Root shell dependency budget shrinks by owned slices [r[remaining-coupling-drain.root-shell-dependency-budget]]

The root `clankers` crate MUST distinguish permanent product-shell wiring from temporary root-owned policy and MUST shrink or narrow the temporary coupling budget as reusable behavior moves to owner crates or neutral adapters.

#### Scenario: Root dependencies are classified by ownership [r[remaining-coupling-drain.root-shell-dependency-budget.inventory]]
- GIVEN root Cargo and source inventory report internal workspace dependencies
- WHEN root shell coupling is reviewed
- THEN every touched dependency row MUST be classified as app-edge wiring, edge projection, adapter exception, or temporary policy
- AND temporary-policy rows MUST include a convergence target and focused validation path

#### Scenario: Root policy drains by behavior slice [r[remaining-coupling-drain.root-shell-dependency-budget.slice-drain]]
- GIVEN root code owns reusable provider, storage, prompt, skill, plugin, process/tool, display, daemon/session, or runtime-service behavior
- WHEN a drain slice touches that behavior
- THEN reusable policy MUST move to the named owner crate, service port, or neutral adapter
- AND root code MUST retain only parsing, service assembly, adapter selection, or edge projection responsibilities

#### Scenario: Budget evidence is deterministic [r[remaining-coupling-drain.root-shell-dependency-budget.budget-evidence]]
- GIVEN a root behavior slice is drained
- WHEN the ownership rail regenerates its receipt
- THEN the temporary-policy budget, dependency budget, or exception scope MUST decrease or narrow
- AND receipt diagnostics MUST include source hashes, owner category, adapter module, and convergence condition

#### Scenario: Root behavior validation preserves UX [r[remaining-coupling-drain.root-shell-dependency-budget.behavior-validation]]
- GIVEN root policy moves to an owner crate or adapter
- WHEN focused and smoke tests run
- THEN affected CLI, TUI, daemon, slash, or tool behavior MUST remain user-visible compatible unless a spec explicitly changes it

#### Scenario: Root closeout validation runs [r[remaining-coupling-drain.root-shell-dependency-budget.closeout]]
- GIVEN the root shell dependency-budget slice is ready to close
- WHEN closeout validation runs
- THEN the root ownership rail, affected cargo checks, Cairn gates, Cairn validation, and diff checks MUST pass
