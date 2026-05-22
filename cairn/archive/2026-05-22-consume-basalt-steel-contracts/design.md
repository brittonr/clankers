# Design: consume Basalt Steel contracts

## Current state

Clankers already has:

- a Rust-owned `steel.host.plan_turn` adapter in `clankers-runtime::steel_orchestration`;
- agent-side settings/profile activation in `clankers-agent::turn::steel_planning`;
- dynamic-runtime action authorization after Steel returns a typed plan;
- an external `examples/basalt-consumer-fixture` crate proving Clankers can compile against Basalt's public DTO API.

That fixture does not yet prove the product path constructs the same Basalt Steel evaluation contract that Basalt validates.

## Decision

Add a narrow Basalt bridge at the Clankers runtime boundary:

1. Construct a Basalt `SteelEvaluationRequest` for the `steel.host.plan_turn` evaluation from safe metadata:
   - backend/evaluator/schema identifiers,
   - Clankers planning seam,
   - required UCAN ability and session capabilities,
   - script/profile/source hashes,
   - redacted input/request metadata.
2. Validate the request with Basalt before Steel evaluation is treated as contract-backed.
3. Bind Clankers orchestration receipts to Basalt request/receipt hashes and schema names.
4. Validate any Basalt-shaped receipt metadata before treating the Steel result as contract evidence.
5. Continue to use Clankers' existing Steel runtime wrapper and dynamic-runtime action authorization for actual orchestration and effects.

Basalt remains a contract/policy crate. Clankers remains the runtime shell and only authority for host effects.

## Dependency shape

The first product slice may use the sibling path dependency already exercised by the Clankers fixture. If that is too broad for release packaging, keep the dependency feature/narrow and document the packageability follow-up rather than weakening the product contract test.

## Safety

The bridge must fail closed:

- invalid Basalt request -> fallback/block according to Clankers policy;
- missing UCAN ability -> fallback/block before host effects;
- malformed Basalt receipt evidence -> fallback/block or mark receipt invalid;
- no raw prompts, provider payloads, scripts, credentials, tokens, or connection strings in Basalt-bound receipt summaries.

## Verification strategy

- Focused runtime tests for Basalt DTO construction and validation.
- Negative tests for missing UCAN/session capability and malformed receipt hash/schema.
- Agent turn-planning test proving the real turn path emits Basalt-bound receipt metadata when configured.
- Existing `examples/basalt-consumer-fixture` remains as external compile/API guard.
- Maintain Cairn validate/gates, Rust fmt/clippy/tests, and no secret-like fixture material.
