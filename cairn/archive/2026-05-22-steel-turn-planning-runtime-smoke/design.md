# Design: Steel Turn Planning Runtime Smoke

## Runtime seam

The smoke rail exercises the Rust-owned path:

1. reviewed `Settings.steel_turn_planning`,
2. Nickel-exported profile + script fixtures,
3. `Agent::steel_turn_planning_config()`,
4. normal prompt handling through `SessionController::handle_command(SessionCommand::Prompt)`,
5. daemon-client-visible events carrying the redacted `steel.host.plan_turn` receipt.

The test uses an in-memory provider and temp profile/script files. It does not contact a live provider, daemon socket, remote QUIC endpoint, or credential store.

## Fail-closed coverage

Negative smokes MUST prove that invalid hash or missing authority prevents successful prompt completion and reports an error instead of silently falling back to Rust-only planning.

## Receipt checker

A Rust script checker MUST inspect durable source/test/doc surfaces and write a deterministic receipt to `target/steel-turn-planning-runtime-smoke/receipt.json`. The receipt MUST hash source artifacts and must not include raw prompts, credentials, profile bodies, or script bodies.

## Seams preserved

Steel Scheme remains a constrained trusted planning/request layer. Rust continues to own settings loading, hash checks, session authority checks, provider calls, event emission, receipts, and all effects.
