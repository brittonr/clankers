## MODIFIED Requirements

### Requirement: Host-owned effect handlers

The system MUST execute effect requests through host-owned handlers that can allow, deny, simulate, replay, or fail closed before side effects occur. For protected effect classes, handler admission MUST include UCAN authorization verification before the handler touches the protected resource.

#### Scenario: absent handler fails closed

- GIVEN an effect request targets a class with no installed handler
- WHEN dispatch evaluates the request
- THEN it returns a denial or unavailable result before touching filesystem, process, network, browser, provider, plugin, scheduler, delivery, or secret resources

#### Scenario: UCAN denial fails before handler contact
r[effect-ability-runtime.handlers.ucan-denial]

- GIVEN a protected effect request has no matching UCAN grant or violates grant caveats
- WHEN dispatch evaluates the request
- THEN Clankers returns a structured authorization denial before invoking the effect handler
- THEN the denial receipt records safe UCAN metadata and omits raw token material

### Requirement: Remote execution declares hashed dependencies

The system MUST allow subagent and remote-daemon execution requests to declare required safe artifacts by content hash before execution. Remote execution requests that require protected side effects MUST also declare safe UCAN grant/proof references needed for admission, while secret-bearing authority material remains host-owned and is not synced as an artifact.

#### Scenario: remote peer requests missing safe artifacts

- GIVEN a remote or subagent execution request references safe prompt, skill, tool schema, manifest, policy, or redacted authorization artifact hashes that the peer lacks
- WHEN the peer prepares execution
- THEN it requests the missing artifacts by hash
- THEN the sender provides only safe artifact envelopes whose hashes match the request

#### Scenario: secret dependency is not synced

- GIVEN an execution request would require credentials, raw environment values, provider tokens, compact UCAN tokens not approved for sync, or other secret material not present on the peer
- WHEN dependency sync evaluates the request
- THEN Clankers does not transmit the secret material
- THEN execution fails with an explicit missing-secret, missing-authority, or unavailable-handler result
