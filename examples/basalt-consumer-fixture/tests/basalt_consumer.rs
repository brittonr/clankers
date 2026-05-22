use basalt::{
    CapabilityGrant, ContractEnvelope, EnforcementRequest, SteelEvaluationRequest, default_policy,
    enforce, parse_policy_json, validate_steel_evaluation_request,
};

#[test]
fn clankers_consumes_basalt_packaged_policy() {
    let policy = default_policy().expect("Basalt packaged policy parses");
    let request = EnforcementRequest::new(
        "document-write",
        "did:key:z6Mkresource/docs/report",
        "document/write",
    )
    .with_capability(CapabilityGrant::new(
        "did:key:z6Mkresource/docs/report",
        "document/write",
    ));

    let receipt = enforce(&policy, &request).expect("Basalt enforcement succeeds");

    assert!(receipt.is_allowed());
}

#[test]
fn clankers_consumes_supplied_policy_and_fail_closed_receipts() {
    let policy = parse_policy_json(
        r#"{
          "schema_version":"ucan-nickel-contracts.policy.v1",
          "contracts":{
            "document-write":{
              "id":"document-write",
              "description":"fixture",
              "resource_prefixes":["did:key:z6Mkresource/docs/"],
              "abilities":["document/write"]
            }
          }
        }"#,
    )
    .expect("supplied policy parses");
    let request = EnforcementRequest::new(
        "document-write",
        "did:key:z6Mkresource/private/report",
        "document/write",
    )
    .with_capability(CapabilityGrant::new(
        "did:key:z6Mkresource/private/report",
        "document/write",
    ));

    let receipt = enforce(&policy, &request).expect("policy denial is a receipt");

    assert!(receipt.is_denied());
    assert!(receipt.reason().contains("outside contract"));
}

#[test]
fn clankers_consumes_basalt_steel_dto_boundary() {
    let request = SteelEvaluationRequest {
        envelope: ContractEnvelope::new(
            "steel",
            "clankers-agent-predicate",
            "v1",
            "blake3:fixture-source",
            "clankers.agent.input.v1",
            "clankers.agent.output.v1",
            "basalt.steel.receipt.v1",
        ),
        input: serde_json::json!({"resource":"did:key:z6Mkresource/docs/report"}),
        max_input_bytes: 256,
        callable: None,
        requested_host_capability: None,
    };

    assert!(validate_steel_evaluation_request(&request).is_accepted());
}
