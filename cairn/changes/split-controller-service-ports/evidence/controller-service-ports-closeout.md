Artifact-Type: validation-log
Task-ID: R1,I1,I2,I3,V1,V2
Covers: r[remaining-coupling-drain.controller-service-ports.inventory], r[remaining-coupling-drain.controller-service-ports.runtime-adapter], r[remaining-coupling-drain.controller-service-ports.persistence-port], r[remaining-coupling-drain.controller-service-ports.projection-owners], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Closeout validation for the controller service-port split umbrella tasks. The detailed implementation slices are recorded in the per-slice evidence files under `cairn/changes/split-controller-service-ports/evidence/`; this receipt ties those slices back to the umbrella inventory, implementation, and verification tasks.

## Inventory and ownership result

Controller concrete dependency sites are now classified by responsibility:

| Responsibility | Owner/seam | Evidence |
|---|---|---|
| Agent/provider execution and provider thinking compatibility | Controller runtime adapter plus agent-owned model/runtime services; provider/config details stay outside production controller code | `controller-provider-config-dependency-drain.md`, `controller-engine-dev-dependency-drain.md` |
| Hook dispatch | Controller-owned hook service port; concrete hook pipelines projected at root/daemon edges | `controller-hook-service-port-drain.md` |
| DB/search persistence | Controller-owned persistence service port with DB/search side effects at the root shell edge | `controller-db-persistence-service-port-drain.md` |
| Session transcript/ledger persistence | `ControllerSessionLedger` port with concrete `SessionManager` adapter at root/daemon edges | `controller-session-ledger-port-drain.md` |
| Tool inventory metadata | Neutral `clanker_message::ToolInfo`; daemon wire projection remains in protocol/transport conversion owners | `controller-tool-inventory-neutral-dto.md` |
| Session list/status/control-plane metadata | Neutral `clanker_message` DTOs; `ControlResponse` construction remains in protocol adapter conversion owners | `controller-control-plane-neutral-dtos.md` |
| Transport session identity | Neutral `clanker_message::SessionKey`; protocol re-exports preserve wire/API shape | `controller-session-key-neutral-dto.md` |

The aggregate drain leaves the controller with explicit runtime, persistence, hook, and projection seams instead of direct provider/config/hooks/db/session/protocol DTO policy ownership.

## Behavior validation

Commands run from repository root with `TMPDIR=/home/brittonr/.cargo-target/tmp` and `RUSTC_WRAPPER=`. A first cold run completed the same tests but exceeded the Steel wrapper timeout before returning a status; the warmed rerun below exited 0 and is the recorded evidence.

```text
cargo test -p clankers-controller --lib
cargo test -p clankers resume_selected_session_preserves_session_id_in_local_router_request --lib
cargo test -p clankers --test session_resume_deterministic_replay
```

Outcomes:

- `clankers-controller --lib`: 192 passed, 0 failed, 2 ignored.
- Resume/request metadata regression: 1 passed, 0 failed; `_session_id` is preserved in the local routed request after persisted-session resume.
- Deterministic session resume replay: 1 passed, 0 failed.

## Closeout rails

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers-controller -p clankers --tests
cargo test -p clankers-controller --test fcis_shell_boundaries
./scripts/check-controller-runtime-boundary.rs
./scripts/check-controller-protocol-boundary.rs
./scripts/check-lego-architecture-boundaries.rs
./scripts/check-workspace-layering-rails.rs
```

Outcomes:

- Affected cargo check exited 0.
- FCIS shell-boundary rail: 44 passed, 0 failed.
- Controller runtime boundary rail exited 0 (`ok: controller runtime boundary covers 6 owners`).
- Controller/protocol boundary rail exited 0.
- Lego architecture ownership rail exited 0 and refreshed `target/lego-architecture/dependency-ownership-inventory.json`.
- Workspace layering rail exited 0 and refreshed `target/workspace-layering/workspace-layering-inventory.json`.

## Final lifecycle checks

Commands run after this evidence/task update:

```text
nix run .#cairn -- gate tasks split-controller-service-ports --root .
nix run .#cairn -- validate --root .
git diff --check
```

Outcomes:

- Cairn tasks gate returned `"valid": true` and `"verdict": "PASS"`.
- Cairn validate returned `"valid": true` with 5 changes and 128 specs validated.
- `git diff --check` exited 0.
