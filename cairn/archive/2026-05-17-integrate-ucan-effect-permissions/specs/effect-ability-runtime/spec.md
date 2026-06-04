## MODIFIED Requirements

### Requirement: Host-owned effect handlers

The system MUST execute effect requests through host-owned handlers that can allow, deny, simulate, replay, or fail closed before side effects occur. For protected effect classes, handler admission MUST include UCAN authorization verification before the handler touches the protected resource.
r[effect-ability-runtime.handlers]

#### Scenario: absent handler fails closed
r[effect-ability-runtime.handlers.absent-fail-closed]

- GIVEN an effect request targets a class with no installed handler
- WHEN dispatch evaluates the request
- THEN it returns a denial or unavailable result before touching filesystem, process, network, browser, provider, plugin, scheduler, delivery, or secret resources

#### Scenario: simulation records no real side effects
r[effect-ability-runtime.handlers.simulate]

- GIVEN a simulation handler is installed for an effect class
- WHEN an operation requests that effect
- THEN the handler returns a simulated result and receipt
- THEN side-effect sentinels prove the real resource was not touched

#### Scenario: replay uses recorded receipts
r[effect-ability-runtime.handlers.replay]

- GIVEN replay mode has a matching prior effect receipt for the request hash and correlation policy
- WHEN the effect is requested during replay
- THEN Clankers returns the recorded result according to replay policy
- THEN mismatched or missing receipts fail explicitly instead of executing the live effect

#### Scenario: UCAN denial fails before handler contact
r[effect-ability-runtime.handlers.ucan-denial]

- GIVEN a protected effect request has no matching UCAN grant or violates grant caveats
- WHEN dispatch evaluates the request
- THEN Clankers returns a structured authorization denial before invoking the effect handler
- THEN the denial receipt records safe UCAN metadata and omits raw token material

#### Scenario: UCAN admission does not bypass confirmation
r[effect-ability-runtime.handlers.confirmation-order]

- GIVEN an effect request requires human confirmation under existing host policy
- AND UCAN authorization allows the request
- WHEN dispatch evaluates admission
- THEN Clankers still requires the human confirmation gate before handler execution
- THEN a UCAN allow decision alone is not sufficient to touch the protected resource

### Requirement: Remote execution declares hashed dependencies

The system MUST allow subagent and remote-daemon execution requests to declare required safe artifacts by content hash before execution. Remote execution requests that require protected side effects MUST also declare safe UCAN grant/proof references needed for admission, while secret-bearing authority material remains host-owned and is not synced as an artifact.
r[effect-ability-runtime.remote-deps]

#### Scenario: remote peer requests missing safe artifacts
r[effect-ability-runtime.remote-deps.missing-safe]

- GIVEN a remote or subagent execution request references safe prompt, skill, tool schema, manifest, policy, or redacted authorization artifact hashes that the peer lacks
- WHEN the peer prepares execution
- THEN it requests the missing artifacts by hash
- THEN the sender provides only safe artifact envelopes whose hashes match the request

#### Scenario: secret dependency is not synced
r[effect-ability-runtime.remote-deps.secret-denied]

- GIVEN an execution request would require credentials, raw environment values, provider tokens, compact UCAN tokens not approved for sync, or other secret material not present on the peer
- WHEN dependency sync evaluates the request
- THEN Clankers does not transmit the secret material
- THEN execution fails with an explicit missing-secret, missing-authority, or unavailable-handler result
