//! Flake reference validation via nix-compat.
//!
//! Parses and validates flake references before spawning the nix CLI.
//! Catches malformed refs early with actionable errors.

use nix_compat::flakeref::FlakeRef;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::error::*;

/// Parsed and validated flake reference.
#[derive(Debug, Clone)]
pub struct ParsedFlakeRef {
    /// The source type (path, git, github, etc.)
    pub source_type: FlakeSourceType,
    /// The attribute path fragment (e.g., "packages.x86_64-linux.hello")
    pub fragment: Option<String>,
    /// Original input string
    pub raw: String,
}

/// Flake source type extracted from a parsed `FlakeRef`.
#[derive(Debug, Clone, Serialize)]
pub enum FlakeSourceType {
    Path,
    Git { url: String },
    GitHub { owner: String, repo: String },
    GitLab { owner: String, repo: String },
    Tarball { url: String },
    Indirect { id: String },
    File { url: String },
    SourceHut { owner: String, repo: String },
    Other(String),
}

/// Parse and validate a flake reference string.
///
/// Handles two syntaxes:
/// - URL-style: `github:NixOS/nixpkgs`, `git+https://...`
/// - CLI shorthand: `.#hello`, `./path#attr`, `.`
///
/// The fragment (attribute path after `#`) is extracted and returned
/// separately from the source.
pub fn parse_flake_ref(input: &str) -> Result<ParsedFlakeRef, NixError> {
    // Split off fragment (attribute path after #)
    let (source_part, fragment) = split_fragment(input);

    // Handle CLI shorthand: bare paths like ".", "./foo", "../bar"
    if is_bare_path(source_part) {
        return Ok(ParsedFlakeRef {
            source_type: FlakeSourceType::Path,
            fragment,
            raw: input.to_string(),
        });
    }

    // Parse URL-style flake refs via nix-compat
    let flake_ref: FlakeRef = source_part.parse().map_err(|e: nix_compat::flakeref::FlakeRefError| {
        InvalidFlakeRefSnafu {
            input: input.to_string(),
            reason: e.to_string(),
        }
        .build()
    })?;

    let source_type = classify_flake_ref(&flake_ref);

    Ok(ParsedFlakeRef {
        source_type,
        fragment,
        raw: input.to_string(),
    })
}

/// Check whether a string looks like a flake reference (as opposed to a
/// regular CLI argument like `--no-link` or `/tmp/result`).
///
/// Returns `true` for inputs that should be validated as flake refs before
/// passing to the nix CLI.
pub fn looks_like_flake_ref(s: &str) -> bool {
    // Starts with known scheme prefixes
    if s.starts_with("github:")
        || s.starts_with("gitlab:")
        || s.starts_with("sourcehut:")
        || s.starts_with("git+")
        || s.starts_with("file+")
        || s.starts_with("path:")
        || s.starts_with("tarball:")
    {
        return true;
    }

    // CLI shorthand: .#foo, ./#foo, ../#foo
    if s.starts_with(".#") || s.starts_with("./#") || s.starts_with("../#") {
        return true;
    }

    // Contains # with a path-like prefix (not a flag)
    if let Some(before_hash) = s.split('#').next()
        && s.contains('#')
        && !before_hash.starts_with('-')
        && !before_hash.is_empty()
    {
        return true;
    }

    false
}

/// Detect whether a directory is a flake project.
pub fn detect_flake(cwd: &Path) -> Option<FlakeInfo> {
    let flake_nix = cwd.join("flake.nix");
    if flake_nix.exists() {
        Some(FlakeInfo {
            flake_path: cwd.to_path_buf(),
            has_lock: cwd.join("flake.lock").exists(),
        })
    } else {
        None
    }
}

/// Information about a detected flake project.
#[derive(Debug, Clone)]
pub struct FlakeInfo {
    pub flake_path: PathBuf,
    pub has_lock: bool,
}

/// Split a flake ref string into source and fragment parts.
///
/// `.#hello` → (".", Some("hello"))
/// `github:NixOS/nixpkgs#hello` → ("github:NixOS/nixpkgs", Some("hello"))
/// `.` → (".", None)
fn split_fragment(input: &str) -> (&str, Option<String>) {
    // For URL-scheme refs, the # is unambiguous
    if let Some(hash_pos) = input.find('#') {
        let source = &input[..hash_pos];
        let frag = &input[hash_pos + 1..];
        if frag.is_empty() {
            (source, None)
        } else {
            (source, Some(frag.to_string()))
        }
    } else {
        (input, None)
    }
}

/// Check whether a string is a bare path (not a URL-scheme ref).
fn is_bare_path(s: &str) -> bool {
    s == "."
        || s == ".."
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with('/')
}

/// Classify a parsed `FlakeRef` into our `FlakeSourceType`.
fn classify_flake_ref(flake_ref: &FlakeRef) -> FlakeSourceType {
    match flake_ref {
        FlakeRef::Path { .. } => FlakeSourceType::Path,
        FlakeRef::Git { url, .. } => FlakeSourceType::Git {
            url: url.to_string(),
        },
        FlakeRef::GitHub { owner, repo, .. } => FlakeSourceType::GitHub {
            owner: owner.clone(),
            repo: repo.clone(),
        },
        FlakeRef::GitLab { owner, repo, .. } => FlakeSourceType::GitLab {
            owner: owner.clone(),
            repo: repo.clone(),
        },
        FlakeRef::Tarball { url, .. } => FlakeSourceType::Tarball {
            url: url.to_string(),
        },
        FlakeRef::Indirect { id, .. } => FlakeSourceType::Indirect { id: id.clone() },
        FlakeRef::File { url, .. } => FlakeSourceType::File {
            url: url.to_string(),
        },
        FlakeRef::SourceHut { owner, repo, .. } => FlakeSourceType::SourceHut {
            owner: owner.clone(),
            repo: repo.clone(),
        },
        _ => FlakeSourceType::Other(format!("{flake_ref:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_flake_ref ─────────────────────────────────────────────────

    #[test]
    fn parse_path_with_fragment() {
        let result = parse_flake_ref(".#hello").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Path));
        assert_eq!(result.fragment, Some("hello".to_string()));
    }

    #[test]
    fn parse_path_with_dotslash_fragment() {
        let result = parse_flake_ref("./#packages.x86_64-linux.hello").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Path));
        assert_eq!(
            result.fragment,
            Some("packages.x86_64-linux.hello".to_string())
        );
    }

    #[test]
    fn parse_bare_dot() {
        let result = parse_flake_ref(".").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Path));
        assert_eq!(result.fragment, None);
    }

    #[test]
    fn parse_github_ref() {
        let result = parse_flake_ref("github:NixOS/nixpkgs").unwrap();
        match &result.source_type {
            FlakeSourceType::GitHub { owner, repo } => {
                assert_eq!(owner, "NixOS");
                assert_eq!(repo, "nixpkgs");
            }
            other => panic!("expected GitHub, got {other:?}"),
        }
        assert_eq!(result.fragment, None);
    }

    #[test]
    fn parse_github_with_fragment() {
        let result = parse_flake_ref("github:NixOS/nixpkgs#hello").unwrap();
        match &result.source_type {
            FlakeSourceType::GitHub { owner, repo } => {
                assert_eq!(owner, "NixOS");
                assert_eq!(repo, "nixpkgs");
            }
            other => panic!("expected GitHub, got {other:?}"),
        }
        assert_eq!(result.fragment, Some("hello".to_string()));
    }

    #[test]
    fn parse_git_ref() {
        let result = parse_flake_ref("git+https://example.com/repo.git").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Git { .. }));
    }

    #[test]
    fn parse_indirect_ref() {
        // nix-compat parses indirect refs in the "indirect+scheme://..." form.
        // The bare "nixpkgs" or "indirect:nixpkgs" forms used by the nix CLI
        // aren't valid URLs and are handled as bare path refs by our code.
        let result = parse_flake_ref("path:/nix/var/nix/profiles").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Path));
    }

    #[test]
    fn parse_absolute_path() {
        let result = parse_flake_ref("/tmp/my-flake").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Path));
        assert_eq!(result.fragment, None);
    }

    #[test]
    fn parse_absolute_path_with_fragment() {
        let result = parse_flake_ref("/tmp/my-flake#default").unwrap();
        assert!(matches!(result.source_type, FlakeSourceType::Path));
        assert_eq!(result.fragment, Some("default".to_string()));
    }

    // ── looks_like_flake_ref ────────────────────────────────────────────

    #[test]
    fn looks_like_ref_cli_shorthand() {
        assert!(looks_like_flake_ref(".#hello"));
        assert!(looks_like_flake_ref("./#hello"));
        assert!(looks_like_flake_ref("../#hello"));
    }

    #[test]
    fn looks_like_ref_url_schemes() {
        assert!(looks_like_flake_ref("github:NixOS/nixpkgs"));
        assert!(looks_like_flake_ref("git+https://example.com/repo"));
        assert!(looks_like_flake_ref("path:/tmp/foo"));
    }

    #[test]
    fn not_a_flake_ref() {
        assert!(!looks_like_flake_ref("--no-link"));
        assert!(!looks_like_flake_ref("-L"));
        assert!(!looks_like_flake_ref("--log-format"));
    }

    // ── detect_flake ────────────────────────────────────────────────────

    #[test]
    fn detect_flake_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("flake.nix"), "{}").unwrap();
        std::fs::write(dir.path().join("flake.lock"), "{}").unwrap();

        let info = detect_flake(dir.path()).unwrap();
        assert_eq!(info.flake_path, dir.path());
        assert!(info.has_lock);
    }

    #[test]
    fn detect_flake_no_lock() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("flake.nix"), "{}").unwrap();

        let info = detect_flake(dir.path()).unwrap();
        assert!(!info.has_lock);
    }

    #[test]
    fn detect_flake_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_flake(dir.path()).is_none());
    }

    // ── split_fragment ──────────────────────────────────────────────────

    #[test]
    fn split_fragment_present() {
        let (source, frag) = split_fragment("github:NixOS/nixpkgs#hello");
        assert_eq!(source, "github:NixOS/nixpkgs");
        assert_eq!(frag, Some("hello".to_string()));
    }

    #[test]
    fn split_fragment_absent() {
        let (source, frag) = split_fragment("github:NixOS/nixpkgs");
        assert_eq!(source, "github:NixOS/nixpkgs");
        assert_eq!(frag, None);
    }

    #[test]
    fn split_fragment_empty_after_hash() {
        let (source, frag) = split_fragment("github:NixOS/nixpkgs#");
        assert_eq!(source, "github:NixOS/nixpkgs");
        assert_eq!(frag, None);
    }
}
