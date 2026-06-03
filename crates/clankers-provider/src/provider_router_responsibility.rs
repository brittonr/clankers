//! Provider/router responsibility inventory.
//!
//! `clankers-provider` keeps public compatibility DTOs, but policy ownership
//! must stay explicit while concerns collapse into `clanker-router` or one
//! bridge module. This inventory is a reviewable map for the staged collapse.

pub const PROVIDER_ROUTER_ABSTRACTION_COLLAPSE_REQUIREMENT: &str =
    "r[provider-router-abstraction-collapse.duplicate-inventory]";

pub const SDK_PROVIDER_EDGE_CONCERNS_REQUIREMENT: &str = "r[sdk-provider-edge-boundary.concerns]";

pub const SDK_PROVIDER_EDGE_NO_DISPLAY_DTOS_REQUIREMENT: &str =
    "r[sdk-provider-edge-boundary.neutral-model-api.no-display-dtos]";

pub const SDK_PROVIDER_EDGE_SDK_HOST_REQUIREMENT: &str =
    "r[sdk-provider-edge-boundary.neutral-model-api.sdk-host-owned]";

pub const SDK_PROVIDER_EDGE_LITERAL_FIXTURE_REQUIREMENT: &str =
    "r[sdk-provider-edge-boundary.verification.literal-fixtures]";

pub const SDK_PROVIDER_EDGE_DEPENDENCY_RAIL_REQUIREMENT: &str =
    "r[sdk-provider-edge-boundary.verification.dependency-rails]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderRouterConcern {
    RequestDto,
    MessageProjection,
    CacheKeyRequestProjection,
    StreamEvents,
    AuthStore,
    CredentialRefresh,
    Discovery,
    Routing,
    RetryFallbackCooldown,
    CostUsage,
    ErrorProjection,
    DisplayDtoBoundary,
    SdkProviderAdapter,
    LiteralRequestFixtures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRouterConcernOwner {
    pub concern: ProviderRouterConcern,
    pub policy_owner: &'static str,
    pub compatibility_boundary: &'static str,
    pub convergence_condition: &'static str,
}

pub const PROVIDER_ROUTER_CONCERN_INVENTORY: &[ProviderRouterConcernOwner] = &[
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::RequestDto,
        policy_owner: "clanker-router::provider::CompletionRequest shared fields",
        compatibility_boundary: "clankers-provider::CompletionRequest keeps AgentMessage history until callers migrate",
        convergence_condition: "all router-bound construction crosses router_request_bridge before routed backend use",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::MessageProjection,
        policy_owner: "clankers-provider::router_request_bridge::build_router_request",
        compatibility_boundary: "RouterCompatAdapter/RpcProvider delegate request projection only",
        convergence_condition: "no local/rpc adapter owns AgentMessage -> provider-native JSON projection",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::CacheKeyRequestProjection,
        policy_owner: "clankers-provider::router_request_bridge::compute_router_cache_key_from_request_projection",
        compatibility_boundary: "RouterProvider cache policy delegates message/request shape before CacheKeyInput hashing",
        convergence_condition: "cache key material is computed from router request JSON, not serialized AgentMessage internals",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::StreamEvents,
        policy_owner: "clanker-message streaming DTOs re-exported by provider and router",
        compatibility_boundary: "RouterCompatAdapter converts only by shared DTO From/Into edges",
        convergence_condition: "provider/router never define independent StreamDelta or stream-event schemas",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::AuthStore,
        policy_owner: "clanker-router::auth provider-scoped store",
        compatibility_boundary: "clankers-provider auth helpers are legacy compatibility entrypoints",
        convergence_condition: "new routed backends use clanker_router::auth::AuthStorePaths and provider-scoped credentials",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::CredentialRefresh,
        policy_owner: "clanker-router::credential::CredentialManager for routed backends",
        compatibility_boundary: "clankers-provider::credential_manager remains Anthropic compatibility",
        convergence_condition: "non-Anthropic OAuth refresh paths do not write through the Anthropic-only manager",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::Discovery,
        policy_owner: "clankers-provider::discovery root assembly",
        compatibility_boundary: "discovery builds compatibility providers or RouterCompatAdapter wrappers",
        convergence_condition: "provider-native probing/auth policy lives in backend owners once wrapped",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::Routing,
        policy_owner: "clanker-router routing model registry and future routed shell",
        compatibility_boundary: "clankers-provider::router::RouterProvider is compatibility routing shell",
        convergence_condition: "RouterProvider routing logic shrinks as clankers callers accept clanker-router request DTOs",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::RetryFallbackCooldown,
        policy_owner: "clanker-router::retry, FallbackConfig, and RouterDb rate-limit state",
        compatibility_boundary: "RouterProvider mirrors retry/fallback for legacy Provider trait callers",
        convergence_condition: "retry/fallback/cooldown policy is removed from compatibility router after DTO convergence",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::CostUsage,
        policy_owner: "clanker-message Usage and clanker-router provider Cost",
        compatibility_boundary: "clankers-provider re-exports canonical usage/cost DTOs",
        convergence_condition: "provider crate does not define independent usage/cost wire schemas",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::ErrorProjection,
        policy_owner: "provider/backend native error classification plus clanker-router::Error for routed backends",
        compatibility_boundary: "RouterCompatAdapter maps routed errors into clankers_provider::ProviderError",
        convergence_condition: "compatibility adapters translate errors without retry/fallback policy duplication",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::DisplayDtoBoundary,
        policy_owner: "clanker-message::ThinkingLevel and ThinkingConfig neutral DTOs",
        compatibility_boundary: "display/app edges translate UI-only thinking levels before provider-facing APIs",
        convergence_condition: "clankers-provider has no display or protocol dependency",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::SdkProviderAdapter,
        policy_owner: "examples/embedded-provider-adapter host-owned ModelHost adapter",
        compatibility_boundary: "embedded examples use clankers-engine-host and clankers-adapters, not provider/router/auth crates",
        convergence_condition: "provider-adapter kit denies clankers-provider, clanker-router, OAuth, discovery, and live credentials",
    },
    ProviderRouterConcernOwner {
        concern: ProviderRouterConcern::LiteralRequestFixtures,
        policy_owner: "explicit JSON fixtures and inline golden request projections",
        compatibility_boundary: "tests compare fixture JSON without calling the body builder under test for expected values",
        convergence_condition: "request-shape tests stay fixture-backed when provider fields change",
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn inventory_names_owner_for_each_provider_router_concern() {
        assert_eq!(
            PROVIDER_ROUTER_ABSTRACTION_COLLAPSE_REQUIREMENT,
            "r[provider-router-abstraction-collapse.duplicate-inventory]"
        );
        assert_eq!(SDK_PROVIDER_EDGE_CONCERNS_REQUIREMENT, "r[sdk-provider-edge-boundary.concerns]");
        assert_eq!(
            SDK_PROVIDER_EDGE_NO_DISPLAY_DTOS_REQUIREMENT,
            "r[sdk-provider-edge-boundary.neutral-model-api.no-display-dtos]"
        );
        assert_eq!(
            SDK_PROVIDER_EDGE_SDK_HOST_REQUIREMENT,
            "r[sdk-provider-edge-boundary.neutral-model-api.sdk-host-owned]"
        );
        assert_eq!(
            SDK_PROVIDER_EDGE_LITERAL_FIXTURE_REQUIREMENT,
            "r[sdk-provider-edge-boundary.verification.literal-fixtures]"
        );
        assert_eq!(
            SDK_PROVIDER_EDGE_DEPENDENCY_RAIL_REQUIREMENT,
            "r[sdk-provider-edge-boundary.verification.dependency-rails]"
        );
        let concerns: BTreeSet<ProviderRouterConcern> =
            PROVIDER_ROUTER_CONCERN_INVENTORY.iter().map(|entry| entry.concern).collect();
        let expected = BTreeSet::from([
            ProviderRouterConcern::RequestDto,
            ProviderRouterConcern::MessageProjection,
            ProviderRouterConcern::CacheKeyRequestProjection,
            ProviderRouterConcern::StreamEvents,
            ProviderRouterConcern::AuthStore,
            ProviderRouterConcern::CredentialRefresh,
            ProviderRouterConcern::Discovery,
            ProviderRouterConcern::Routing,
            ProviderRouterConcern::RetryFallbackCooldown,
            ProviderRouterConcern::CostUsage,
            ProviderRouterConcern::ErrorProjection,
            ProviderRouterConcern::DisplayDtoBoundary,
            ProviderRouterConcern::SdkProviderAdapter,
            ProviderRouterConcern::LiteralRequestFixtures,
        ]);

        assert_eq!(concerns, expected);
        for entry in PROVIDER_ROUTER_CONCERN_INVENTORY {
            assert!(!entry.policy_owner.is_empty(), "missing owner for {:?}", entry.concern);
            assert!(!entry.compatibility_boundary.is_empty(), "missing boundary for {:?}", entry.concern);
            assert!(!entry.convergence_condition.is_empty(), "missing convergence condition for {:?}", entry.concern);
        }
    }

    #[test]
    fn selected_cache_key_projection_has_single_bridge_owner() {
        let selected = PROVIDER_ROUTER_CONCERN_INVENTORY
            .iter()
            .find(|entry| entry.concern == ProviderRouterConcern::CacheKeyRequestProjection)
            .expect("cache-key projection concern should be inventoried");

        assert_eq!(
            selected.policy_owner,
            "clankers-provider::router_request_bridge::compute_router_cache_key_from_request_projection"
        );
        assert!(selected.compatibility_boundary.contains("RouterProvider cache policy delegates"));
        assert!(selected.convergence_condition.contains("router request JSON"));
    }
}
