const SUMMARY: &str = include_str!("../docs/src/SUMMARY.md");
const PROVIDER_CONTRACTS: &str = include_str!("../docs/src/reference/provider-contracts.md");
const PROVIDER_LIB: &str = include_str!("../crates/clankers-provider/src/lib.rs");
const RPC_PROVIDER: &str = include_str!("../crates/clankers-provider/src/rpc_provider.rs");
const AGENT_EXECUTION: &str = include_str!("../crates/clankers-agent/src/turn/execution.rs");
const ROUTER_CODEX: &str = include_str!("../crates/clanker-router/src/backends/openai_codex.rs");

#[test]
fn provider_contracts_doc_is_linked_from_summary() {
    assert!(
        SUMMARY.contains("[Provider Contracts](./reference/provider-contracts.md)"),
        "docs SUMMARY should link the provider contracts page"
    );
}

#[test]
fn provider_contracts_doc_names_required_request_invariants() {
    for phrase in [
        "CompletionRequest",
        "extra_params",
        "_session_id",
        "provider-native message content",
        "shared-field serde projection parity",
        "constructor-count inventory",
        "runtime parser-entrypoint test using raw SSE bytes",
        "fail closed",
    ] {
        assert!(PROVIDER_CONTRACTS.contains(phrase), "provider contracts doc missing invariant phrase `{phrase}`");
    }
}

#[test]
fn provider_contracts_doc_tracks_existing_source_anchors() {
    for (source, anchor) in [
        (AGENT_EXECUTION, "completion_request_from_engine_request"),
        (AGENT_EXECUTION, "extra_params: build_extra_params(&engine_request.session_id)"),
        (PROVIDER_LIB, "completion_request_constructor_inventory_requires_extra_params"),
        (PROVIDER_LIB, "provider_and_router_request_shared_schema_fields_stay_in_parity"),
        (PROVIDER_LIB, "provider_and_router_request_omit_empty_extra_params_consistently"),
        (RPC_PROVIDER, "fn build_router_request(request: CompletionRequest)"),
        (RPC_PROVIDER, "fn convert_messages_to_api(messages: &[crate::message::AgentMessage])"),
        (ROUTER_CODEX, "text/event-stream"),
    ] {
        assert!(source.contains(anchor), "provider contracts source anchor missing `{anchor}`");
    }
}
