//! Store path parsing via nix-compat.
//!
//! Wraps `nix_compat::store_path::StorePath` into an agent-friendly `NixPath`
//! that includes the human name, hash, and derivation flag.

use nix_compat::nixbase32;
use nix_compat::store_path::{StorePath, StorePathRef};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

use crate::error::*;

/// Regex matching `/nix/store/<32-char-nixbase32-hash>-<name>` in arbitrary text.
///
/// The nix store hash is always exactly 32 nixbase32 characters ([0-9a-df-np-sv-z]).
/// The name is one or more characters from the valid set.
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "compile-time constant regex pattern"))]
static STORE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/nix/store/([0-9a-df-np-sv-z]{32})-([a-zA-Z0-9+\-._?=][a-zA-Z0-9+\-._?=]*)").expect("static regex")
});

/// Agent-friendly representation of a nix store path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NixPath {
    /// Full absolute path (e.g., "/nix/store/abc123...-hello-2.12.1")
    pub path: String,
    /// The name component (e.g., "hello-2.12.1")
    pub name: String,
    /// The nixbase32-encoded hash (e.g., "ql5gvvahh5gnir9g8v25xd4dwqa4hcmp")
    pub store_hash: String,
    /// Whether this is a .drv file
    pub is_derivation: bool,
}

/// Parse a single absolute nix store path into a [`NixPath`].
///
/// Accepts paths like `/nix/store/ql5gvvahh5gnir9g8v25xd4dwqa4hcmp-hello-2.12.1`.
/// Returns `NixError::NotAStorePath` if the input doesn't start with `/nix/store/`.
/// Returns `NixError::InvalidStorePath` if the hash or name is malformed.
pub fn parse_store_path(path: &str) -> Result<NixPath, NixError> {
    // Strip any trailing path components — StorePath only covers the
    // direct child of /nix/store/.
    let store_part = strip_to_store_entry(path);

    let parsed: StorePathRef<'_> =
        StorePath::from_absolute_path(store_part.as_bytes()).map_err(|e| {
            if store_part.starts_with("/nix/store/") {
                InvalidStorePathSnafu {
                    path: path.to_string(),
                    reason: e.to_string(),
                }
                .build()
            } else {
                NotAStorePathSnafu {
                    path: path.to_string(),
                }
                .build()
            }
        })?;

    let name: String = (*parsed.name()).to_string();
    let store_hash = nixbase32::encode(parsed.digest());

    Ok(NixPath {
        path: store_part.to_string(),
        name: name.clone(),
        store_hash,
        is_derivation: name.ends_with(".drv"),
    })
}

/// Extract all store paths from a block of text.
///
/// Scans for `/nix/store/<hash>-<name>` patterns using regex and parses each
/// through nix-compat. Invalid matches (hash decoding failure, etc.) are
/// silently skipped. Results are deduplicated by path.
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "regex capture group 0 always exists when captures_iter yields"))]
pub fn extract_store_paths(text: &str) -> Vec<NixPath> {
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for caps in STORE_PATH_RE.captures_iter(text) {
        let full_match = caps.get(0).unwrap().as_str();
        let abs_path = full_match.to_string();

        if seen.contains(&abs_path) {
            continue;
        }

        if let Ok(nix_path) = parse_store_path(&abs_path) {
            seen.insert(abs_path);
            results.push(nix_path);
        }
    }

    results
}

/// Strip any subpath after the store entry.
///
/// `/nix/store/abc-hello/bin/hello` → `/nix/store/abc-hello`
fn strip_to_store_entry(path: &str) -> &str {
    let prefix = "/nix/store/";
    if let Some(rest) = path.strip_prefix(prefix) {
        // Find the next `/` after the store entry name
        if let Some(slash_pos) = rest.find('/') {
            &path[..prefix.len() + slash_pos]
        } else {
            path
        }
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_store_path() {
        let path = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-net-tools-1.60_p20170221182432";
        let result = parse_store_path(path).unwrap();
        assert_eq!(result.name, "net-tools-1.60_p20170221182432");
        assert_eq!(result.store_hash, "00bgd045z0d4icpbc2yyz4gx48ak44la");
        assert!(!result.is_derivation);
        assert_eq!(result.path, path);
    }

    #[test]
    fn parse_drv_path() {
        let path = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello-2.12.1.drv";
        let result = parse_store_path(path).unwrap();
        assert_eq!(result.name, "hello-2.12.1.drv");
        assert!(result.is_derivation);
    }

    #[test]
    fn parse_path_with_subpath() {
        let path = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello-2.12.1/bin/hello";
        let result = parse_store_path(path).unwrap();
        assert_eq!(result.name, "hello-2.12.1");
        // Subpath is stripped — only the store entry is returned
        assert_eq!(
            result.path,
            "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello-2.12.1"
        );
    }

    #[test]
    fn parse_not_a_store_path() {
        let result = parse_store_path("/home/user/project");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, NixError::NotAStorePath { .. }));
    }

    #[test]
    fn parse_malformed_hash() {
        // Hash too short
        let result = parse_store_path("/nix/store/abc-hello");
        assert!(result.is_err());
    }

    #[test]
    fn extract_from_multiline_text() {
        let text = r#"
building '/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-foo.drv'...
fetching path '/nix/store/vxjiwkjkn7x4079qvh1jkl5pn05j2aw0-bar-1.0'
some other output
/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-foo.drv
"#;
        let paths = extract_store_paths(text);
        assert_eq!(paths.len(), 2);
        assert!(paths[0].is_derivation);
        assert_eq!(paths[0].name, "foo.drv");
        assert_eq!(paths[1].name, "bar-1.0");
    }

    #[test]
    fn extract_deduplicates() {
        let text = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello\n\
                     /nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello";
        let paths = extract_store_paths(text);
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn extract_empty_text() {
        let paths = extract_store_paths("");
        assert!(paths.is_empty());
    }

    #[test]
    fn extract_no_store_paths() {
        let text = "just some regular text without any nix paths";
        let paths = extract_store_paths(text);
        assert!(paths.is_empty());
    }

    #[test]
    fn nixbase32_roundtrip() {
        // Verify we use nix-compat's nixbase32 (non-standard alphabet)
        let hash_str = "00bgd045z0d4icpbc2yyz4gx48ak44la";
        let bytes = nixbase32::decode(hash_str.as_bytes()).unwrap();
        let re_encoded = nixbase32::encode(&bytes);
        assert_eq!(hash_str, re_encoded);
    }
}
