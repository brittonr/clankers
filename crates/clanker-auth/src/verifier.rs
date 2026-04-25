//! Token verification and authorization.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::RwLock;

use iroh::PublicKey;

use crate::Cap;
use crate::builder::bytes_to_sign;
use crate::constants::MAX_DELEGATION_DEPTH;
use crate::constants::MAX_REVOCATION_LIST_SIZE;
use crate::constants::MAX_REVOCATION_LIST_SIZE_USIZE;
use crate::constants::TOKEN_CLOCK_SKEW_SECS;
use crate::error::AuthError;
use crate::token::Audience;
use crate::token::CapabilityToken;
use crate::utils::current_time_secs;

/// Verifies capability tokens and checks authorization.
pub struct TokenVerifier<C: Cap> {
    /// Lock order: revoked before parent_cache if both are ever needed together.
    revoked: RwLock<HashSet<[u8; 32]>>,
    /// Lock order: acquire after revoked; current verification drops revoked before this lock.
    parent_cache: RwLock<HashMap<[u8; 32], CapabilityToken<C>>>,
    trusted_roots: Vec<PublicKey>,
    clock_skew_tolerance: u64,
    _marker: PhantomData<C>,
}

impl<C: Cap> TokenVerifier<C> {
    pub fn new() -> Self {
        Self {
            revoked: RwLock::new(HashSet::new()),
            parent_cache: RwLock::new(HashMap::new()),
            trusted_roots: Vec::new(),
            clock_skew_tolerance: TOKEN_CLOCK_SKEW_SECS,
            _marker: PhantomData,
        }
    }

    pub fn with_trusted_root(mut self, key: PublicKey) -> Self {
        self.trusted_roots.push(key);
        self
    }

    pub fn with_clock_skew_tolerance(mut self, seconds: u64) -> Self {
        self.clock_skew_tolerance = seconds;
        self
    }

    pub fn register_parent_token(&self, token: CapabilityToken<C>) -> Result<(), AuthError> {
        let hash = token.hash()?;
        let mut cache = self.parent_cache.write().map_err(|_| AuthError::InternalError {
            reason: "parent cache lock poisoned".to_string(),
        })?;
        cache.insert(hash, token);
        Ok(())
    }

    pub fn clear_parent_cache(&self) -> Result<(), AuthError> {
        self.parent_cache
            .write()
            .map_err(|_| AuthError::InternalError {
                reason: "parent cache lock poisoned".to_string(),
            })?
            .clear();
        Ok(())
    }

    /// Verify token signature and validity.
    pub fn verify(&self, token: &CapabilityToken<C>, presenter: Option<&PublicKey>) -> Result<(), AuthError> {
        self.verify_internal(token, presenter, 0)
    }

    fn verify_internal(
        &self,
        token: &CapabilityToken<C>,
        presenter: Option<&PublicKey>,
        chain_depth: u8,
    ) -> Result<(), AuthError> {
        if chain_depth > MAX_DELEGATION_DEPTH {
            return Err(AuthError::DelegationTooDeep {
                depth: chain_depth,
                max: MAX_DELEGATION_DEPTH,
            });
        }

        assert!(token.version > 0, "token version must be positive");
        assert!(token.expires_at >= token.issued_at, "expires_at must be >= issued_at");

        if token.delegation_depth == 0 {
            assert!(token.proof.is_none(), "root token (depth 0) must not have proof");
        }

        // Signature
        let sign_bytes = bytes_to_sign(token)?;
        let signature = iroh::Signature::from_bytes(&token.signature);
        token.issuer.verify(&sign_bytes, &signature).map_err(|_| AuthError::InvalidSignature)?;

        // Expiration
        let now = current_time_secs();
        if token.expires_at + self.clock_skew_tolerance < now {
            return Err(AuthError::TokenExpired {
                expired_at: token.expires_at,
                now,
            });
        }

        // Not from the future
        if token.issued_at > now + self.clock_skew_tolerance {
            return Err(AuthError::TokenFromFuture {
                issued_at: token.issued_at,
                now,
            });
        }

        // r[impl auth.verify.audience]
        // Audience (leaf token only)
        if chain_depth == 0 {
            match &token.audience {
                Audience::Key(expected) => {
                    if let Some(actual) = presenter {
                        if expected != actual {
                            return Err(AuthError::WrongAudience {
                                expected: expected.to_string(),
                                actual: actual.to_string(),
                            });
                        }
                    } else {
                        return Err(AuthError::AudienceRequired);
                    }
                }
                Audience::Bearer => {}
            }
        }

        // r[impl auth.verify.revocation]
        // Revocation
        let hash = token.hash()?;
        let revoked_guard = self.revoked.read().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        if revoked_guard.contains(&hash) {
            return Err(AuthError::TokenRevoked);
        }
        drop(revoked_guard);

        // r[impl auth.verify.chain-complete]
        // r[impl auth.delegation.transitivity]
        // Trusted roots + delegation chain
        if !self.trusted_roots.is_empty() {
            if let Some(parent_hash) = token.proof {
                let cache = self.parent_cache.read().map_err(|_| AuthError::InternalError {
                    reason: "parent cache lock poisoned".to_string(),
                })?;
                if let Some(parent) = cache.get(&parent_hash) {
                    let parent = parent.clone();
                    drop(cache);
                    self.verify_internal(&parent, None, chain_depth + 1)?;
                } else {
                    return Err(AuthError::ParentTokenRequired);
                }
            } else if !self.trusted_roots.contains(&token.issuer) {
                return Err(AuthError::UntrustedRoot);
            }
        }

        Ok(())
    }

    /// Verify token with an explicit chain of parent tokens.
    pub fn verify_with_chain(
        &self,
        token: &CapabilityToken<C>,
        chain: &[CapabilityToken<C>],
        presenter: Option<&PublicKey>,
    ) -> Result<(), AuthError> {
        let chain_map: HashMap<[u8; 32], &CapabilityToken<C>> =
            chain.iter().map(|t| Ok((t.hash()?, t))).collect::<Result<_, AuthError>>()?;
        self.verify_with_chain_internal(token, &chain_map, presenter, 0)
    }

    fn verify_with_chain_internal(
        &self,
        token: &CapabilityToken<C>,
        chain_map: &HashMap<[u8; 32], &CapabilityToken<C>>,
        presenter: Option<&PublicKey>,
        chain_depth: u8,
    ) -> Result<(), AuthError> {
        if chain_depth > MAX_DELEGATION_DEPTH {
            return Err(AuthError::DelegationTooDeep {
                depth: chain_depth,
                max: MAX_DELEGATION_DEPTH,
            });
        }

        let sign_bytes = bytes_to_sign(token)?;
        let signature = iroh::Signature::from_bytes(&token.signature);
        token.issuer.verify(&sign_bytes, &signature).map_err(|_| AuthError::InvalidSignature)?;

        let now = current_time_secs();
        if token.expires_at + self.clock_skew_tolerance < now {
            return Err(AuthError::TokenExpired {
                expired_at: token.expires_at,
                now,
            });
        }
        if token.issued_at > now + self.clock_skew_tolerance {
            return Err(AuthError::TokenFromFuture {
                issued_at: token.issued_at,
                now,
            });
        }

        if chain_depth == 0 {
            match &token.audience {
                Audience::Key(expected) => {
                    if let Some(actual) = presenter {
                        if expected != actual {
                            return Err(AuthError::WrongAudience {
                                expected: expected.to_string(),
                                actual: actual.to_string(),
                            });
                        }
                    } else {
                        return Err(AuthError::AudienceRequired);
                    }
                }
                Audience::Bearer => {}
            }
        }

        let hash = token.hash()?;
        let revoked_guard = self.revoked.read().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        if revoked_guard.contains(&hash) {
            return Err(AuthError::TokenRevoked);
        }
        drop(revoked_guard);

        if !self.trusted_roots.is_empty() {
            if let Some(parent_hash) = token.proof {
                if let Some(parent) = chain_map.get(&parent_hash) {
                    self.verify_with_chain_internal(parent, chain_map, None, chain_depth + 1)?;
                } else {
                    let cache = self.parent_cache.read().map_err(|_| AuthError::InternalError {
                        reason: "parent cache lock poisoned".to_string(),
                    })?;
                    if let Some(parent) = cache.get(&parent_hash) {
                        let parent = parent.clone();
                        drop(cache);
                        self.verify_with_chain_internal(&parent, chain_map, None, chain_depth + 1)?;
                    } else {
                        return Err(AuthError::ParentTokenRequired);
                    }
                }
            } else if !self.trusted_roots.contains(&token.issuer) {
                return Err(AuthError::UntrustedRoot);
            }
        }

        Ok(())
    }

    /// Check if token authorizes the given operation.
    pub fn authorize(
        &self,
        token: &CapabilityToken<C>,
        operation: &C::Operation,
        presenter: Option<&PublicKey>,
    ) -> Result<(), AuthError> {
        assert!(!token.capabilities.is_empty(), "token must have at least one capability");
        self.verify(token, presenter)?;

        for cap in &token.capabilities {
            if cap.authorizes(operation) {
                return Ok(());
            }
        }

        Err(AuthError::Unauthorized {
            operation: format!("{:?}", operation),
        })
    }

    pub fn revoke(&self, token_hash: [u8; 32]) -> Result<(), AuthError> {
        let mut revoked = self.revoked.write().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        if revoked.len() >= MAX_REVOCATION_LIST_SIZE_USIZE {
            if revoked.contains(&token_hash) {
                return Ok(());
            }
            return Err(AuthError::InternalError {
                reason: format!("revocation list full: {} entries (max {})", revoked.len(), MAX_REVOCATION_LIST_SIZE),
            });
        }
        revoked.insert(token_hash);
        Ok(())
    }

    pub fn revoke_token(&self, token: &CapabilityToken<C>) -> Result<(), AuthError> {
        self.revoke(token.hash()?)
    }

    pub fn is_revoked(&self, token_hash: &[u8; 32]) -> Result<bool, AuthError> {
        let guard = self.revoked.read().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        Ok(guard.contains(token_hash))
    }

    pub fn clear_revocations(&self) -> Result<(), AuthError> {
        self.revoked
            .write()
            .map_err(|_| AuthError::InternalError {
                reason: "revocation lock poisoned".to_string(),
            })?
            .clear();
        Ok(())
    }

    pub fn revocation_count(&self) -> Result<u32, AuthError> {
        let guard = self.revoked.read().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        Ok(guard.len() as u32)
    }

    pub fn load_revoked(&self, hashes: &[[u8; 32]]) -> Result<(), AuthError> {
        let mut revoked = self.revoked.write().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        revoked.extend(hashes.iter().copied());
        Ok(())
    }

    pub fn get_all_revoked(&self) -> Result<Vec<[u8; 32]>, AuthError> {
        let guard = self.revoked.read().map_err(|_| AuthError::InternalError {
            reason: "revocation lock poisoned".to_string(),
        })?;
        Ok(guard.iter().copied().collect())
    }
}

impl<C: Cap> Default for TokenVerifier<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Cap> std::fmt::Debug for TokenVerifier<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenVerifier")
            .field("trusted_roots", &self.trusted_roots.len())
            .field("clock_skew_tolerance", &self.clock_skew_tolerance)
            .finish()
    }
}
