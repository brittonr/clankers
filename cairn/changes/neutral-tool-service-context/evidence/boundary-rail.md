# Boundary rail evidence

Evidence-ID: boundary-rail
Artifact-Type: command-output-summary
Task-ID: V2
Covers: neutral-tool-service-context.verification.boundary-rail
Date: 2026-05-31
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
```

## Relevant output

```text
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
```

## Notes

The rail now includes `tool_host_service_context_signature()`, which parses `crates/clankers-tool-host/src/lib.rs` and rejects concrete DB/search-index, hook pipeline, agent event, TUI DTO, daemon protocol, legacy `ToolContext`, and root tool-state imports. It also requires the neutral service traits/DTOs for storage, search, hooks, progress, capability, cancellation, and runtime policy to stay present, including the boxed `ToolHostFuture` alias used by async service decisions. The same rail checks `ControllerToolPort` and legacy `ToolContext` field inventories against `CONTROLLER_TOOL_PORT_SERVICE_INVENTORY` and `LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY` in `crates/clankers-agent/src/turn/ports.rs`.
