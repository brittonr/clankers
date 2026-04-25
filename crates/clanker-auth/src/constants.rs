//! Tiger Style constants for capability-based authorization.
//!
//! These constants define fixed limits to prevent resource exhaustion
//! and ensure predictable token sizes.

/// Maximum number of capabilities per token (32).
///
/// Tiger Style: Bounded to prevent token bloat and DoS.
pub const MAX_CAPABILITIES_PER_TOKEN: u32 = 32;

/// Maximum number of capabilities per token as a platform-local collection bound.
pub const MAX_CAPABILITIES_PER_TOKEN_USIZE: usize = 32;

/// Maximum delegation chain depth (8 levels).
///
/// Tiger Style: Bounded to prevent unbounded proof chains.
/// Root -> Service -> User -> ... max 8 levels
pub const MAX_DELEGATION_DEPTH: u8 = 8;

/// Maximum delegation chain depth as a platform-local collection bound.
pub const MAX_DELEGATION_DEPTH_USIZE: usize = 8;

/// Maximum token size in bytes (8 KB).
///
/// Tiger Style: Bounded to prevent oversized tokens.
/// Typical token with 10 capabilities is ~500 bytes.
pub const MAX_TOKEN_SIZE: u32 = 8 * 1024;

/// Maximum token size as a platform-local collection bound.
pub const MAX_TOKEN_SIZE_USIZE: usize = 8 * 1024;

/// Maximum revocation list size (10,000 entries).
///
/// Tiger Style: Bounded to prevent unbounded memory growth.
/// Old revocations can be pruned after token expiry.
pub const MAX_REVOCATION_LIST_SIZE: u32 = 10_000;

/// Maximum revocation list size as a platform-local collection bound.
pub const MAX_REVOCATION_LIST_SIZE_USIZE: usize = 10_000;

/// Token clock skew tolerance (60 seconds).
///
/// Tiger Style: Fixed tolerance for clock drift between nodes.
pub const TOKEN_CLOCK_SKEW_SECS: u64 = 60;

// ============================================================================
// Compile-Time Constant Assertions
// ============================================================================

// Tiger Style: assert positive values
const _: () = assert!(MAX_CAPABILITIES_PER_TOKEN > 0);
const _: () = assert!(MAX_DELEGATION_DEPTH > 0);
const _: () = assert!(MAX_TOKEN_SIZE > 0);
const _: () = assert!(MAX_REVOCATION_LIST_SIZE > 0);
const _: () = assert!(TOKEN_CLOCK_SKEW_SECS > 0);

// Tiger Style: assert constant relationships
// A token with max capabilities must still fit within max token size.
// Each capability serializes to roughly ~100 bytes max, so 32 * 100 = 3200 bytes
// plus overhead (~200 bytes) must be under MAX_TOKEN_SIZE.
const _: () = assert!(MAX_TOKEN_SIZE >= 4096);
// Delegation depth must fit in a u8 and be reasonable
const _: () = assert!(MAX_DELEGATION_DEPTH <= 32);
// Clock skew tolerance must be less than 5 minutes to prevent replay attacks
const _: () = assert!(TOKEN_CLOCK_SKEW_SECS <= 300);
// Revocation list must be bounded but useful
const _: () = assert!(MAX_REVOCATION_LIST_SIZE <= 1_000_000);
