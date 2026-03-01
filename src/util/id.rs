//! 8-char hex ID generation

use rand::Rng;

/// Generate a random 8-character lowercase hexadecimal ID.
///
/// Uses cryptographically secure random number generation to produce
/// a unique identifier suitable for short-lived IDs like request IDs.
///
/// # Examples
///
/// ```
/// use clankers::util::id::generate_id;
///
/// let id = generate_id();
/// assert_eq!(id.len(), 8);
/// assert!(id.chars().all(|c| c.is_ascii_hexdigit() && c.is_lowercase() || c.is_ascii_digit()));
/// ```
pub fn generate_id() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 4] = rng.random();
    hex::encode(bytes)
}

/// Generate a UUID v4 string.
///
/// Returns a standard UUID (36 characters including hyphens) suitable
/// for globally unique identifiers.
///
/// # Examples
///
/// ```
/// use clankers::util::id::generate_uuid;
///
/// let uuid = generate_uuid();
/// assert_eq!(uuid.len(), 36);
/// assert!(uuid.contains('-'));
/// ```
pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = generate_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(id.chars().all(|c| c.is_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_uuid() {
        let uuid = generate_uuid();
        assert_eq!(uuid.len(), 36);
        assert!(uuid.contains('-'));
    }

    #[test]
    fn test_generate_uuid_uniqueness() {
        let uuid1 = generate_uuid();
        let uuid2 = generate_uuid();
        assert_ne!(uuid1, uuid2);
    }
}
