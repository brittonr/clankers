//! Zellij streaming framing protocol
//! Length-prefixed messages over QUIC streams.

use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

pub const ALPN: &[u8] = b"clankers/stream/1";

/// Handshake message sent from host to guest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_name: String,
    pub zellij_version: String,
    pub read_only: bool,
}

/// Write a length-prefixed JSON message to a stream
pub async fn write_message<W: AsyncWriteExt + Unpin, T: Serialize>(writer: &mut W, msg: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(msg).map_err(|e| std::io::Error::other(format!("serialize: {}", e)))?;
    let len = (bytes.len() as u32).to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-prefixed JSON message from a stream
pub async fn read_message<R: AsyncReadExt + Unpin, T: for<'de> Deserialize<'de>>(reader: &mut R) -> std::io::Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 10_000_000 {
        return Err(std::io::Error::other("message too large"));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| std::io::Error::other(format!("deserialize: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpn() {
        assert_eq!(ALPN, b"clankers/stream/1");
    }

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            session_name: "test-session".to_string(),
            zellij_version: "0.40.0".to_string(),
            read_only: false,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.session_name, "test-session");
        assert_eq!(deserialized.zellij_version, "0.40.0");
        assert!(!deserialized.read_only);
    }

    #[test]
    fn test_session_info_read_only() {
        let info = SessionInfo {
            session_name: "ro".to_string(),
            zellij_version: "0.40.0".to_string(),
            read_only: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert!(deserialized.read_only);
    }

    #[tokio::test]
    async fn test_write_read_message_roundtrip() {
        let info = SessionInfo {
            session_name: "roundtrip".to_string(),
            zellij_version: "0.40.0".to_string(),
            read_only: false,
        };

        let mut buf = Vec::new();
        write_message(&mut buf, &info).await.unwrap();

        // Buffer should have 4-byte length prefix + JSON
        assert!(buf.len() > 4);
        let expected_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert_eq!(expected_len, buf.len() - 4);

        let mut cursor = std::io::Cursor::new(buf);
        let result: SessionInfo = read_message(&mut cursor).await.unwrap();
        assert_eq!(result.session_name, "roundtrip");
    }

    #[tokio::test]
    async fn test_write_read_multiple_messages() {
        let mut buf = Vec::new();

        let msg1 = SessionInfo {
            session_name: "first".to_string(),
            zellij_version: "1.0".to_string(),
            read_only: false,
        };
        let msg2 = SessionInfo {
            session_name: "second".to_string(),
            zellij_version: "2.0".to_string(),
            read_only: true,
        };

        write_message(&mut buf, &msg1).await.unwrap();
        write_message(&mut buf, &msg2).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let r1: SessionInfo = read_message(&mut cursor).await.unwrap();
        let r2: SessionInfo = read_message(&mut cursor).await.unwrap();

        assert_eq!(r1.session_name, "first");
        assert_eq!(r2.session_name, "second");
    }
}
