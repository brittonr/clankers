//! Wire protocol frame I/O helpers
//!
//! All frames are length-prefixed JSON: `[4-byte big-endian length][JSON payload]`.

/// Write a length-prefixed frame to the send stream.
pub async fn write_frame(
    send: &mut iroh::endpoint::SendStream,
    data: &[u8],
) -> Result<(), crate::error::Error> {
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len).await.map_err(io_err)?;
    send.write_all(data).await.map_err(io_err)?;
    Ok(())
}

/// Read a length-prefixed frame from the recv stream.
pub async fn read_frame(
    recv: &mut iroh::endpoint::RecvStream,
) -> Result<Vec<u8>, crate::error::Error> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.map_err(io_err)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 10_000_000 {
        return Err(crate::error::Error::Provider {
            message: "Frame too large".to_string(),
        });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(io_err)?;
    Ok(data)
}

fn io_err(e: impl std::fmt::Display) -> crate::error::Error {
    crate::error::Error::Provider {
        message: format!("IO error: {}", e),
    }
}
