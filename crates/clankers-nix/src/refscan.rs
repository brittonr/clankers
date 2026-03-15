//! Store reference scanning in arbitrary text.
//!
//! Scans tool output for `/nix/store/...` paths and produces compact
//! annotations for agent context.

use crate::store_path::{NixPath, extract_store_paths};

/// Maximum output size to scan (skip for very large outputs).
const MAX_SCAN_SIZE: usize = 1_024 * 1_024; // 1 MB

/// Scan arbitrary text for nix store path references.
///
/// Returns deduplicated, parsed store paths found in the text.
/// Skips scanning for inputs larger than 1 MB.
pub fn scan_store_refs(text: &str) -> Vec<NixPath> {
    if text.len() > MAX_SCAN_SIZE {
        return Vec::new();
    }
    extract_store_paths(text)
}

/// Produce a compact annotation summarizing store paths found in text.
///
/// Returns `None` if no store paths are found.
///
/// Format: `[nix refs: glibc-2.38, gcc-13.3.0, hello-2.12.1 (3 store paths)]`
pub fn annotate_store_refs(text: &str) -> Option<String> {
    let refs = scan_store_refs(text);
    if refs.is_empty() {
        return None;
    }

    let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
    let count = names.len();
    let summary = names.join(", ");

    Some(format!("[nix refs: {summary} ({count} store paths)]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_paths() {
        let text = "error: collision between /nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-foo-1.0/bin/x \
                    and /nix/store/vxjiwkjkn7x4079qvh1jkl5pn05j2aw0-bar-2.0/bin/x";
        let refs = scan_store_refs(text);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "foo-1.0");
        assert_eq!(refs[1].name, "bar-2.0");
    }

    #[test]
    fn scan_empty() {
        let refs = scan_store_refs("");
        assert!(refs.is_empty());
    }

    #[test]
    fn scan_no_paths() {
        let refs = scan_store_refs("just some regular text");
        assert!(refs.is_empty());
    }

    #[test]
    fn scan_deduplicates() {
        let text = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello\n\
                     /nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello";
        let refs = scan_store_refs(text);
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn scan_skips_huge_input() {
        let text = "x".repeat(2_000_000);
        let refs = scan_store_refs(&text);
        assert!(refs.is_empty());
    }

    #[test]
    fn annotate_some() {
        let text = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-glibc-2.38 \
                     /nix/store/vxjiwkjkn7x4079qvh1jkl5pn05j2aw0-hello-2.12.1";
        let annotation = annotate_store_refs(text).unwrap();
        assert!(annotation.starts_with("[nix refs:"));
        assert!(annotation.contains("glibc-2.38"));
        assert!(annotation.contains("hello-2.12.1"));
        assert!(annotation.contains("2 store paths"));
    }

    #[test]
    fn annotate_none() {
        assert!(annotate_store_refs("no paths here").is_none());
    }

    #[test]
    fn annotate_empty() {
        assert!(annotate_store_refs("").is_none());
    }

    #[test]
    fn scan_handles_many_paths() {
        // Build text with 10 unique store paths (varying hash + name)
        // Valid nixbase32 hashes (alphabet: 0-9 a-d f-n p-s v-z)
        let hashes = [
            "0369cgjmqvy147adhknrwz258bfilpsx",
            "7adhknrwz258bfilpsx0369cgjmqvy14",
            "filpsx0369cgjmqvy147adhknrwz258b",
            "mqvy147adhknrwz258bfilpsx0369cgj",
            "wz258bfilpsx0369cgjmqvy147adhknr",
            "369cgjmqvy147adhknrwz258bfilpsx0",
            "adhknrwz258bfilpsx0369cgjmqvy147",
            "ilpsx0369cgjmqvy147adhknrwz258bf",
            "qvy147adhknrwz258bfilpsx0369cgjm",
            "z258bfilpsx0369cgjmqvy147adhknrw",
        ];
        let mut text = String::new();
        for (i, hash) in hashes.iter().enumerate() {
            text.push_str(&format!("/nix/store/{hash}-pkg-{i} "));
        }
        let refs = scan_store_refs(&text);
        assert_eq!(refs.len(), 10);
    }
}
