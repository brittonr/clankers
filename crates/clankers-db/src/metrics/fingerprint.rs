//! BLAKE3 fingerprinting for high-cardinality metric labels.

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint {
    pub digest: [u8; 16],
    pub kind: FingerprintKind,
    pub byte_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FingerprintKind {
    Path,
    Command,
    ToolInput,
    PluginPayload,
    ErrorText,
    Prompt,
}

impl Fingerprint {
    pub fn new(kind: FingerprintKind, raw: &str) -> Self {
        let normalized = normalize(raw);
        let hash = blake3::hash(normalized.as_bytes());
        let mut digest = [0u8; 16];
        digest.copy_from_slice(&hash.as_bytes()[..16]);
        Self {
            digest,
            kind,
            byte_len: raw.len() as u32,
        }
    }

    pub fn hex(&self) -> String {
        hex::encode(self.digest)
    }
}

fn normalize(s: &str) -> String {
    let trimmed = s.trim();
    let collapsed: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.to_lowercase()
}

mod hex {
    pub fn encode(bytes: [u8; 16]) -> String {
        let mut s = String::with_capacity(32);
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_fingerprint() {
        let a = Fingerprint::new(FingerprintKind::Command, "cargo build --release");
        let b = Fingerprint::new(FingerprintKind::Command, "cargo build --release");
        assert_eq!(a.digest, b.digest);
    }

    #[test]
    fn normalization_collapses_whitespace() {
        let a = Fingerprint::new(FingerprintKind::Command, "cargo  build   --release");
        let b = Fingerprint::new(FingerprintKind::Command, "cargo build --release");
        assert_eq!(a.digest, b.digest);
    }

    #[test]
    fn normalization_case_insensitive() {
        let a = Fingerprint::new(FingerprintKind::Path, "/Home/User/File.rs");
        let b = Fingerprint::new(FingerprintKind::Path, "/home/user/file.rs");
        assert_eq!(a.digest, b.digest);
    }

    #[test]
    fn normalization_trims() {
        let a = Fingerprint::new(FingerprintKind::Command, "  cargo build  ");
        let b = Fingerprint::new(FingerprintKind::Command, "cargo build");
        assert_eq!(a.digest, b.digest);
    }

    #[test]
    fn different_kinds_same_content_differ() {
        let a = Fingerprint::new(FingerprintKind::Command, "foo");
        let b = Fingerprint::new(FingerprintKind::Path, "foo");
        // Same digest (kind is metadata, not part of hash input) but different kind
        assert_eq!(a.digest, b.digest);
        assert_ne!(a.kind, b.kind);
    }

    #[test]
    fn different_content_differs() {
        let a = Fingerprint::new(FingerprintKind::Command, "cargo build");
        let b = Fingerprint::new(FingerprintKind::Command, "cargo test");
        assert_ne!(a.digest, b.digest);
    }

    #[test]
    fn byte_len_tracks_original() {
        let f = Fingerprint::new(FingerprintKind::Prompt, "hello world");
        assert_eq!(f.byte_len, 11);
    }

    #[test]
    fn hex_output_is_32_chars() {
        let f = Fingerprint::new(FingerprintKind::Command, "test");
        assert_eq!(f.hex().len(), 32);
    }
}
