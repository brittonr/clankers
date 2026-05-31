# Design: Drain Remaining Coupling Hotspots

## Context

The current lego architecture baseline already records owner receipts for root, agent, controller, process tool, provider/router, and attach policy seams. The remaining problem is not absence of seams; it is that several seams are still transitional. Large root modules and concrete crate dependencies make it easy for new behavior to land at the wrong layer, while string-presence rails can turn harmless refactors into test churn.

## Decisions

### 1. Drain by ownership slice, not by broad rewrite

**Choice:** Keep one Cairn change that inventories every remaining hotspot, but implement each drain as a small, reviewable slice with focused validation.

**Rationale:** The hotspots interact. For example, shrinking `clankers-agent` dependencies affects root service assembly, process tools, and provider adapters. A single inventory keeps priorities visible, while small slices prevent risky rewrites.

### 2. Prefer dependency-count and typed-boundary movement over cosmetic file splits

**Choice:** A drain is complete only when it removes or narrows an ownership edge, centralizes policy in one owner, or upgrades a brittle validation rail. Moving code without changing dependency or policy ownership is not enough.

**Rationale:** Prior decoupling work already split several modules. The next value is reducing the number of concrete dependencies and product-shell responsibilities that reusable crates must understand.

### 3. Start with validation coupling

**Choice:** The first implementation slice hardens attach-parity architecture tests away from raw string anchors where practical.

**Rationale:** This is low-risk and immediately reduces refactor friction. The previous warning cleanup moved slash-effect ownership and required updating brittle source strings. Typed/behavioral rail improvements make later draining safer.

### 4. Keep root as explicit application edge

**Choice:** Root may continue to wire concrete services, but reusable behavior should move into workspace crates or root modules with clear adapter ownership. New root dependencies require owner receipts and convergence conditions.

**Rationale:** The root crate is the product shell; it will remain broad. The target is not zero dependencies, but thin, explicit assembly around reusable bricks.

### 5. Shrink inward display/protocol DTO usage gradually

**Choice:** Replace inward uses of display/protocol DTOs with neutral message/runtime/core DTOs when touching a seam; avoid a single large DTO migration.

**Rationale:** `clanker-tui-types` and protocol DTOs are widely used. Incremental replacement limits regressions while moving the canonical domain model away from display surfaces.

## Hotspot Drain Order

1. Validation coupling: typed/behavioral architecture rails before refactors.
2. Root shell thinness: move obvious reusable policy out of root mode/tool modules.
3. Agent concrete dependency budget: remove display/procmon/DB/provider-shaped dependencies from turn policy first.
4. Process-job split: isolate backend/native/storage policy behind service traits and leave root `process` as JSON projection.
5. Controller command seams: split command translation, authorization, runtime dispatch, and event projection.
6. Daemon actor construction: separate session runtime assembly from actor loop multiplexing.
7. Display/protocol leakage: replace inward DTO imports with neutral DTOs.
8. Provider/router convergence: collapse duplicate provider abstractions or confine conversion to one bridge.

## Risks / Trade-offs

- **Large dependency graph churn:** Reducing concrete dependencies may require manifest and test updates across several crates. Mitigate with focused rails and one dependency edge at a time.
- **Behavioral parity regressions:** Attach/daemon/standalone paths have subtle parity rules. Keep focused runtime seam tests before and after each movement.
- **Architecture rail brittleness:** Typed rails can also overfit if they assert implementation details. Prefer ownership contracts and semantic call-path checks over exact formatting.
- **Cairn scope creep:** This change tracks all hotspots, but tasks should be split or archived progressively if a slice becomes its own substantial feature.
