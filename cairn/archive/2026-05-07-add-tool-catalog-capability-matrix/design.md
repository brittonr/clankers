## Context

The catalog builder must compose read-only/default/dangerous/custom/extension packs with disabled tools and collision policy. Single-case tests can miss accidental pack coupling, eager extension startup, or metadata leaks when features are combined.

## Goals / Non-Goals

**Goals:** matrix-test the catalog policy boundary and ensure side-effect class metadata is stable.

**Non-Goals:** executing every built-in tool or starting real plugins/MCP/gateway services.

## Decisions

### 1. Catalog matrix is descriptor-first

**Choice:** assert over tool descriptors, source labels, side-effect classes, prerequisites, omission reasons, and safe metadata.

**Rationale:** catalog construction should be safe for host inspection without executing tools.

### 2. Extension services use instrumented fakes

**Choice:** use fake extension services with counters for publish/execute/start attempts.

**Rationale:** this proves absent runtimes do not start and present runtimes publish only requested tools.

### 3. Collision and disabled filtering are first-class axes

**Choice:** include host custom tool collision policy and disabled-tool filtering as matrix axes rather than one-off tests.

**Rationale:** these policies interact with pack selection and extension publication.
