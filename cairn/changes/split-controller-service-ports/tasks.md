# Tasks: Split Controller Service Ports

## Phase 1: Inventory

- [ ] [serial] R1: Inventory controller concrete dependency sites by responsibility and record which service port or projection owner should own each edge. r[remaining-coupling-drain.controller-service-ports.inventory] [covers=remaining-coupling-drain.controller-service-ports.inventory]

## Phase 2: Implementation

- [ ] [serial] I1: Move agent/provider execution and provider-thinking compatibility behind the controller runtime adapter using neutral controller intents. r[remaining-coupling-drain.controller-service-ports.runtime-adapter] [covers=remaining-coupling-drain.controller-service-ports.runtime-adapter]
- [ ] [serial] I2: Move DB/session persistence and search/index behavior behind a typed session persistence service port. r[remaining-coupling-drain.controller-service-ports.persistence-port] [covers=remaining-coupling-drain.controller-service-ports.persistence-port]
- [ ] [serial] I3: Keep hook dispatch and daemon/protocol/TUI projection in declared adapter modules and update constructor-owner inventories. r[remaining-coupling-drain.controller-service-ports.projection-owners] [covers=remaining-coupling-drain.controller-service-ports.projection-owners]
- [x] [serial] I4: Remove production `clankers-provider` and `clankers-config` dependencies from `clankers-controller` after provider execution moved behind agent/controller runtime adapters and controller config became crate-owned. r[remaining-coupling-drain.controller-service-ports.runtime-adapter] [covers=remaining-coupling-drain.controller-service-ports.inventory,remaining-coupling-drain.controller-service-ports.runtime-adapter] [evidence=cairn/changes/split-controller-service-ports/evidence/controller-provider-config-dependency-drain.md]

## Phase 3: Verification

- [ ] [serial] V1: Run focused controller command/effect/runtime adapter tests, persistence service-port tests, and resume/request-metadata regression tests. r[remaining-coupling-drain.controller-service-ports.behavior-validation] [covers=remaining-coupling-drain.controller-service-ports.behavior-validation]
- [ ] [serial] V2: Run FCIS shell-boundary rails, transport-construction rails, `cargo check --tests` for affected crates, Cairn gates/validate, and `git diff --check`. r[remaining-coupling-drain.controller-service-ports.closeout] [covers=remaining-coupling-drain.controller-service-ports.closeout]
- [x] [serial] V3: Validate the controller provider/config dependency drain with affected controller/root cargo checks plus lego/workspace layering rails. r[remaining-coupling-drain.controller-service-ports.behavior-validation] [covers=remaining-coupling-drain.controller-service-ports.behavior-validation] [evidence=cairn/changes/split-controller-service-ports/evidence/controller-provider-config-dependency-drain.md]
