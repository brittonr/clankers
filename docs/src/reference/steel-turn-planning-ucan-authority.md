# Steel Turn Planning UCAN Authority

This rail binds reviewed Steel Scheme turn-planning activation to explicit runtime authority before `steel.host.plan_turn` can affect a real turn.

## Seam ownership

- **Nickel/settings** declare the reviewed profile, script hash, rollout, and authority grant metadata.
- **Basalt/UCAN vocabulary** models the delegated ability for `clankers/steel/orchestrate.plan_turn` over the concrete turn resource.
- **Rust runtime** performs the authority check, expiry/revocation/scope validation, Basalt enforcement, dynamic-runtime authorization, provider calls, receipts, and fail-closed behavior.
- **Steel Scheme** can only produce typed planning data through the reviewed `steel.host.plan_turn` host seam.
- **Wasm/tool execution** remains a separate capability-limited boundary.

Steel receives no raw token, filesystem, shell, network, provider, daemon, session mutation, or credential authority. Authority grants are data for Rust-owned validation only.

## Authority behavior

The Steel planner may run only after the Rust runtime finds a matching authority grant for the selected turn resource and required ability:

- resource must match the concrete candidate target, e.g. `session:<id>`;
- ability must match `clankers/steel/orchestrate.plan_turn`;
- audience must match `clankers:agent-turn-planning`;
- expired or revoked grants fail closed;
- unknown caveats fail closed;
- Basalt enforcement must allow the derived contract request.

Missing, expired, revoked, wrong-resource, wrong-ability, wrong-audience, or overbroad grants produce `UcanAuthorityDenied` and block before Steel planning can influence provider/tool execution.

## Receipts

`OrchestrationPlanReceipt` carries a redacted `ucan_authority_receipt` with:

- authority status/reason;
- redacted route-safe resource/audience/proof labels;
- caveat classes;
- Basalt reason class;
- BLAKE3 receipt hash.

It must not include raw UCAN tokens, credential values, prompt bodies, profile bodies, script bodies, provider payloads, or secret session data. Agent-visible system messages may include only redacted authority status/reason such as `ucan_authority=Allowed` and `ucan_reason=Allowed`.

## Verification receipt

The static checker writes a bounded receipt to:

```text
target/steel-turn-planning-ucan-authority/receipt.json
```

The receipt hashes the implementation, tests, docs, summary, and Cairn task artifact while omitting raw proofs and prompt/script bodies.
