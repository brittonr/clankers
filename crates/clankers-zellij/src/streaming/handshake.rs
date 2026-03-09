//! Zellij streaming authentication handshake
//! PSK + NodeId verification.

use rand::Rng;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

/// Generate a 32-byte pre-shared key
pub fn generate_psk() -> [u8; 32] {
    let mut psk = [0u8; 32];
    rand::rng().fill(&mut psk);
    psk
}

/// Format PSK as hex for display
pub fn psk_to_hex(psk: &[u8; 32]) -> String {
    hex::encode(psk)
}

/// Parse PSK from hex string
pub fn psk_from_hex(hex_str: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(hex_str).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut psk = [0u8; 32];
    psk.copy_from_slice(&bytes);
    Some(psk)
}

/// Host: verify PSK from incoming connection
pub async fn verify_psk<S: AsyncReadExt + Unpin>(stream: &mut S, expected_psk: &[u8; 32]) -> std::io::Result<bool> {
    let mut received = [0u8; 32];
    stream.read_exact(&mut received).await?;
    Ok(received == *expected_psk)
}

/// Guest: send PSK to host
pub async fn send_psk<S: AsyncWriteExt + Unpin>(stream: &mut S, psk: &[u8; 32]) -> std::io::Result<()> {
    stream.write_all(psk).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_psk_is_random() {
        let psk1 = generate_psk();
        let psk2 = generate_psk();
        assert_ne!(psk1, psk2);
        assert_eq!(psk1.len(), 32);
    }

    #[test]
    fn test_psk_hex_roundtrip() {
        let psk = generate_psk();
        let hex = psk_to_hex(&psk);
        assert_eq!(hex.len(), 64); // 32 bytes = 64 hex chars
        let decoded = psk_from_hex(&hex).expect("failed to decode PSK from hex");
        assert_eq!(decoded, psk);
    }

    #[test]
    fn test_psk_from_hex_invalid() {
        assert!(psk_from_hex("not_hex").is_none());
        assert!(psk_from_hex("aabb").is_none()); // too short
        assert!(psk_from_hex("").is_none());
    }

    #[test]
    fn test_psk_from_hex_wrong_length() {
        // Valid hex but wrong length (16 bytes instead of 32)
        let short = "00".repeat(16);
        assert!(psk_from_hex(&short).is_none());
    }

    #[tokio::test]
    async fn test_send_and_verify_psk() {
        let psk = generate_psk();
        let mut buf = Vec::new();

        // Send PSK to buffer
        send_psk(&mut buf, &psk).await.expect("failed to send PSK");
        assert_eq!(buf.len(), 32);

        // Verify with correct PSK
        let mut cursor = std::io::Cursor::new(buf.clone());
        assert!(verify_psk(&mut cursor, &psk).await.expect("failed to verify correct PSK"));

        // Verify with wrong PSK
        let wrong_psk = generate_psk();
        let mut cursor = std::io::Cursor::new(buf);
        assert!(!verify_psk(&mut cursor, &wrong_psk).await.expect("failed to verify wrong PSK"));
    }
}
