//! Adapter between Clankers effect requests and the sibling `ucan` crate.
//!
//! This module intentionally depends only on public exports from `ucan`.

use std::fmt;

pub use ucan::AudienceDid;
pub use ucan::CapabilitySet;
pub use ucan::CaveatDecision;
pub use ucan::CaveatDocument;
pub use ucan::CaveatIdentifier;
pub use ucan::CaveatPolicy;
pub use ucan::CaveatPolicySet;
pub use ucan::CompactToken;
pub use ucan::InvocationRequest;
pub use ucan::InvocationResult;
pub use ucan::ProofCollection;
pub use ucan::ProofReference;
pub use ucan::ReplayAdmission;
pub use ucan::TokenSigner;
pub use ucan::TokenTimeBounds;
pub use ucan::VerificationContext;
pub use ucan::issue_token_with_signer;
pub use ucan::proof_reference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectInvocation {
    resource: String,
    ability: String,
}

impl EffectInvocation {
    /// Build a concrete UCAN invocation for a Clankers effect.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the requested resource or ability is not a
    /// concrete public-UCAN authorization request.
    pub fn new(resource: impl Into<String>, ability: impl Into<String>) -> AdapterResult<Self> {
        let invocation = Self {
            resource: resource.into(),
            ability: ability.into(),
        };
        invocation.validate_concrete()?;
        Ok(invocation)
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability.as_str()
    }

    fn validate_concrete(&self) -> AdapterResult<()> {
        ucan::AuthorizationRequest::new(self.resource.clone(), self.ability.clone())
            .map(|_| ())
            .map_err(|source| AdapterError::MalformedInvocation {
                message: source.to_string(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    MalformedInvocation { message: String },
    DelegationDenied { message: String },
    DelegationIssue { message: String },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedInvocation { message } => {
                write!(formatter, "malformed UCAN invocation: {message}")
            }
            Self::DelegationDenied { message } => write!(formatter, "UCAN delegation denied: {message}"),
            Self::DelegationIssue { message } => write!(formatter, "UCAN delegation issue failed: {message}"),
        }
    }
}

impl std::error::Error for AdapterError {}

pub type AdapterResult<T> = Result<T, AdapterError>;

pub trait UcanAuthorizer {
    fn authorize(&self, invocation: &EffectInvocation) -> InvocationResult;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedGrant {
    token: CompactToken,
    parent_reference: ProofReference,
}

impl DelegatedGrant {
    #[must_use]
    pub const fn token(&self) -> &CompactToken {
        &self.token
    }

    #[must_use]
    pub const fn parent_reference(&self) -> &ProofReference {
        &self.parent_reference
    }
}

pub fn delegate_child_grant<S>(
    parent_authorizer: &dyn UcanAuthorizer,
    parent_token: &CompactToken,
    child_signer: &S,
    child_audience: &AudienceDid,
    child_invocation: &EffectInvocation,
    time_bounds: TokenTimeBounds,
) -> AdapterResult<DelegatedGrant>
where
    S: TokenSigner + ?Sized,
{
    let parent_decision = parent_authorizer.authorize(child_invocation);
    if !parent_decision.is_allowed() {
        return Err(AdapterError::DelegationDenied {
            message: format!(
                "parent grant does not cover {} {}: {parent_decision:?}",
                child_invocation.resource(),
                child_invocation.ability()
            ),
        });
    }
    let capabilities =
        CapabilitySet::single(child_invocation.resource(), child_invocation.ability()).map_err(|error| {
            AdapterError::DelegationIssue {
                message: error.to_string(),
            }
        })?;
    let proofs = ProofCollection::from_tokens(vec![parent_token.clone()]);
    let token = issue_token_with_signer(child_signer, child_audience, &capabilities, &proofs, time_bounds).map_err(
        |error| AdapterError::DelegationIssue {
            message: error.to_string(),
        },
    )?;
    Ok(DelegatedGrant {
        token,
        parent_reference: proof_reference(parent_token),
    })
}

pub struct PublicUcanAuthorizer<'a> {
    token: &'a CompactToken,
    context: &'a VerificationContext,
    policies: &'a dyn CaveatPolicySet,
    replay: Option<&'a dyn ReplayAdmission>,
}

impl<'a> PublicUcanAuthorizer<'a> {
    #[must_use]
    pub const fn new(
        token: &'a CompactToken,
        context: &'a VerificationContext,
        policies: &'a dyn CaveatPolicySet,
    ) -> Self {
        Self {
            token,
            context,
            policies,
            replay: None,
        }
    }

    #[must_use]
    pub const fn with_replay(mut self, replay: &'a dyn ReplayAdmission) -> Self {
        self.replay = Some(replay);
        self
    }
}

impl UcanAuthorizer for PublicUcanAuthorizer<'_> {
    fn authorize(&self, invocation: &EffectInvocation) -> InvocationResult {
        let request = InvocationRequest::new(
            self.token,
            self.context,
            invocation.resource(),
            invocation.ability(),
            self.policies,
        );
        match self.replay {
            Some(replay) => ucan::verify_invocation_with_replay(&request, replay),
            None => ucan::verify_invocation(&request),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use ucan::AudienceDid;
    use ucan::CapabilitySet;
    use ucan::ED25519_SECRET_KEY_BYTES;
    use ucan::Ed25519InMemorySigner;
    use ucan::KeyResolutionContext;
    use ucan::ProofCollection;
    use ucan::ProofReference;
    use ucan::ReplayAdmissionError;
    use ucan::TokenSigner;
    use ucan::TokenTimeBounds;
    use ucan::VerificationTime;
    use ucan::issue_token_with_signer;

    use super::*;

    const ISSUER_KEY_BYTE: u8 = 11;
    const AUDIENCE_KEY_BYTE: u8 = 17;
    const CHILD_KEY_BYTE: u8 = 19;
    const NOT_BEFORE_SECONDS: u64 = 1_000;
    const EXPIRES_AT_SECONDS: u64 = 2_000;
    const VERIFY_AT_SECONDS: u64 = 1_500;
    const RESOURCE: &str = "clankers:file:///workspace/project";
    const CHILD_RESOURCE: &str = "clankers:file:///workspace/project/src/lib.rs";
    const READ_ABILITY: &str = "file/read";
    const WRITE_ABILITY: &str = "file/write";

    struct NoCaveatPolicies;

    impl CaveatPolicySet for NoCaveatPolicies {
        fn policy_for(&self, _caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
            None
        }
    }

    static NO_CAVEAT_POLICIES: NoCaveatPolicies = NoCaveatPolicies;

    #[derive(Default)]
    struct TestReplayAdmission {
        seen: RefCell<HashSet<(ProofReference, String, String)>>,
    }

    impl ReplayAdmission for TestReplayAdmission {
        fn admit_invocation(
            &self,
            token_reference: &ProofReference,
            resource: &str,
            ability: &str,
        ) -> Result<(), ReplayAdmissionError> {
            let key = (token_reference.clone(), resource.to_owned(), ability.to_owned());
            if self.seen.borrow_mut().insert(key) {
                Ok(())
            } else {
                Err(ReplayAdmissionError::Duplicate {
                    reference: token_reference.clone(),
                })
            }
        }
    }

    fn token_for(ability: &str) -> (CompactToken, VerificationContext) {
        let issuer = Ed25519InMemorySigner::from_seed_bytes([ISSUER_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let audience_key = Ed25519InMemorySigner::from_seed_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let audience = AudienceDid::from(audience_key.issuer().expect("audience did"));
        let capabilities = CapabilitySet::single(RESOURCE, ability).expect("capability set");
        let token = issue_token_with_signer(
            &issuer,
            &audience,
            &capabilities,
            &ProofCollection::empty(),
            TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("time bounds"),
        )
        .expect("token");
        let context = VerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            KeyResolutionContext::new(vec![issuer.verification_key().expect("verification key")]),
            ProofCollection::empty(),
        );
        (token, context)
    }

    fn parent_token_for_child(ability: &str, child: &Ed25519InMemorySigner) -> (CompactToken, Ed25519InMemorySigner) {
        let parent = Ed25519InMemorySigner::from_seed_bytes([ISSUER_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let audience = AudienceDid::from(child.issuer().expect("child issuer"));
        let capabilities = CapabilitySet::single(RESOURCE, ability).expect("capability set");
        let token = issue_token_with_signer(
            &parent,
            &audience,
            &capabilities,
            &ProofCollection::empty(),
            TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("time bounds"),
        )
        .expect("parent token");
        (token, parent)
    }

    fn delegated_context(
        parent: &Ed25519InMemorySigner,
        child: &Ed25519InMemorySigner,
        proof: CompactToken,
    ) -> VerificationContext {
        VerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            KeyResolutionContext::new(vec![
                parent.verification_key().expect("parent verification key"),
                child.verification_key().expect("child verification key"),
            ]),
            ProofCollection::from_tokens(vec![proof]),
        )
    }

    fn authorizer<'a>(token: &'a CompactToken, context: &'a VerificationContext) -> PublicUcanAuthorizer<'a> {
        PublicUcanAuthorizer::new(token, context, &NO_CAVEAT_POLICIES)
    }

    #[test]
    fn adapter_allows_matching_public_ucan_invocation() {
        let (token, context) = token_for("file/*");
        let invocation = EffectInvocation::new(CHILD_RESOURCE, READ_ABILITY).expect("invocation");

        let result = authorizer(&token, &context).authorize(&invocation);

        assert!(result.is_allowed(), "expected allowed result, got {result:?}");
    }

    #[test]
    fn adapter_denies_unmatched_ability() {
        let (token, context) = token_for(READ_ABILITY);
        let invocation = EffectInvocation::new(CHILD_RESOURCE, WRITE_ABILITY).expect("invocation");

        let result = authorizer(&token, &context).authorize(&invocation);

        assert!(!result.is_allowed(), "expected denial, got {result:?}");
    }

    #[test]
    fn adapter_rejects_wildcard_invocation_requests() {
        let error = EffectInvocation::new(CHILD_RESOURCE, "file/*").expect_err("wildcard request denied");

        assert!(error.to_string().contains("malformed UCAN invocation"));
    }

    #[test]
    fn adapter_uses_public_replay_admission() {
        let (token, context) = token_for("file/*");
        let replay = TestReplayAdmission::default();
        let authorizer = authorizer(&token, &context).with_replay(&replay);
        let invocation = EffectInvocation::new(CHILD_RESOURCE, READ_ABILITY).expect("invocation");

        let first = authorizer.authorize(&invocation);
        let second = authorizer.authorize(&invocation);

        assert!(first.is_allowed(), "first invocation should pass: {first:?}");
        assert!(!second.is_allowed(), "duplicate invocation should be denied: {second:?}");
    }

    #[test]
    fn delegated_child_grant_can_attenuate_parent_authority() {
        let child = Ed25519InMemorySigner::from_seed_bytes([CHILD_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let (parent_token, parent) = parent_token_for_child("file/*", &child);
        let parent_context = delegated_context(&parent, &child, parent_token.clone());
        let parent_authorizer = authorizer(&parent_token, &parent_context);
        let invocation = EffectInvocation::new(CHILD_RESOURCE, READ_ABILITY).expect("invocation");
        let session = Ed25519InMemorySigner::from_seed_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let child_audience = AudienceDid::from(session.issuer().expect("session audience"));

        let grant = delegate_child_grant(
            &parent_authorizer,
            &parent_token,
            &child,
            &child_audience,
            &invocation,
            TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("time bounds"),
        )
        .expect("delegated grant");
        let child_context = delegated_context(&parent, &child, parent_token.clone());
        let child_authorizer = authorizer(grant.token(), &child_context);

        assert_eq!(grant.parent_reference(), &proof_reference(&parent_token));
        let allowed = child_authorizer.authorize(&invocation);
        assert!(allowed.is_allowed(), "delegated child should be allowed: {allowed:?}");
        let widened = EffectInvocation::new(CHILD_RESOURCE, WRITE_ABILITY).expect("widened invocation");
        assert!(!child_authorizer.authorize(&widened).is_allowed());
    }

    #[test]
    fn delegated_child_grant_denies_capability_not_covered_by_parent() {
        let child = Ed25519InMemorySigner::from_seed_bytes([CHILD_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let (parent_token, parent) = parent_token_for_child(READ_ABILITY, &child);
        let parent_context = delegated_context(&parent, &child, parent_token.clone());
        let parent_authorizer = authorizer(&parent_token, &parent_context);
        let widened = EffectInvocation::new(CHILD_RESOURCE, WRITE_ABILITY).expect("widened invocation");
        let session = Ed25519InMemorySigner::from_seed_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
        let child_audience = AudienceDid::from(session.issuer().expect("session audience"));

        let error = delegate_child_grant(
            &parent_authorizer,
            &parent_token,
            &child,
            &child_audience,
            &widened,
            TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("time bounds"),
        )
        .expect_err("widening delegation should fail");

        assert!(matches!(error, AdapterError::DelegationDenied { .. }));
    }
}
