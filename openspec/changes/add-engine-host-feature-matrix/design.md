## Context

Current tests cover many individual engine-host behaviors, but the gap is combinatorial confidence: streaming with tool calls and usage, retries with cancellation, budget exhaustion with tool feedback, malformed streams with terminal failures, and wrong-correlation feedback across phases.

## Goals / Non-Goals

**Goals:** define a bounded matrix with named axes, generate or enumerate cases, and make missing combinations fail deterministically.

**Non-Goals:** exhaustive Cartesian explosion, live provider calls, or shell/TUI runtime testing.

## Decisions

### 1. Bounded pairwise-plus-critical matrix

**Choice:** maintain a matrix fixture with pairwise coverage across common axes plus hand-picked critical triples.

**Rationale:** full Cartesian coverage would be expensive and brittle; pairwise coverage catches most wiring drift while critical triples protect known risky seams.

### 2. Provider-neutral fixtures

**Choice:** drive all matrix cases through fake `ModelHost`, `ToolExecutor`, cancellation, retry sleeper, event sink, and usage observer implementations.

**Rationale:** the generic SDK contract must not depend on provider/router availability.

### 3. Matrix freshness gate

**Choice:** add a checker that maps each declared axis value to at least one executed case and each critical interaction to an assertion.

**Rationale:** future engine features should extend the matrix intentionally instead of silently shrinking coverage.
