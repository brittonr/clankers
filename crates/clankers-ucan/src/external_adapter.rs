//! Adapter between Clankers effect requests and the sibling `ucan` crate.
//!
//! This module intentionally depends only on public exports from `ucan`.

use std::fmt;

pub use ucan::CaveatDecision;
pub use ucan::CaveatDocument;
pub use ucan::CaveatIdentifier;
pub use ucan::CaveatPolicy;
pub use ucan::CaveatPolicySet;
pub use ucan::CompactToken;
pub use ucan::InvocationRequest;
pub use ucan::InvocationResult;
pub use ucan::ReplayAdmission;
pub use ucan::VerificationContext;

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
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedInvocation { message } => {
                write!(formatter, "malformed UCAN invocation: {message}")
            }
        }
    }
}

impl std::error::Error for AdapterError {}

pub type AdapterResult<T> = Result<T, AdapterError>;

pub trait UcanAuthorizer {
    fn authorize(&self, invocation: &EffectInvocation) -> InvocationResult;
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
}
