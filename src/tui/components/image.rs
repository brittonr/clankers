//! Image display (Kitty/iTerm2/Sixel protocols)
//!
//! Renders images inline in the terminal using escape sequences specific to
//! the terminal emulator. Supported protocols:
//! - **Kitty** graphics protocol (kitty, WezTerm, Ghostty, and others)
//! - **iTerm2** inline image protocol (iTerm2, WezTerm)
//! - **Sixel** (xterm -ti 340, mlterm, foot, older terminals)
//!
//! The raw image bytes (PNG, JPEG, GIF, etc.) are base64-encoded and sent
//! via the appropriate escape sequence. The terminal decodes and displays them.

use std::fs::OpenOptions;
use std::io::Write;
use std::io::{self};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

/// Open the controlling terminal (`/dev/tty`) for writing.
///
/// Graphics escape sequences must be written directly to the terminal,
/// not to stdout, because stdout may be piped or captured by a TUI
/// framework. Falls back to stdout if `/dev/tty` is unavailable (e.g.
/// in CI or when there is no controlling terminal).
fn open_tty() -> io::Result<Box<dyn Write>> {
    match OpenOptions::new().write(true).open("/dev/tty") {
        Ok(f) => Ok(Box::new(f)),
        Err(_) => Ok(Box::new(io::stdout())),
    }
}

/// Detected terminal image protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Kitty graphics protocol — uses APC sequences.
    Kitty,
    /// iTerm2 inline image protocol — uses OSC 1337.
    ITerm2,
    /// Sixel protocol — not yet implemented, detected only.
    Sixel,
    /// No image protocol detected.
    None,
}

/// Detect the best available image protocol for the current terminal.
///
/// Checks environment variables in order of specificity:
/// 1. `TERM_PROGRAM` for known terminals (iTerm2, kitty, WezTerm, Ghostty)
/// 2. `KITTY_WINDOW_ID` for kitty
/// 3. Falls back to `None`
pub fn detect_protocol() -> ImageProtocol {
    // Check TERM_PROGRAM for known terminals
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        match term.as_str() {
            "iTerm.app" => return ImageProtocol::ITerm2,
            "WezTerm" | "kitty" | "Ghostty" => return ImageProtocol::Kitty,
            _ => {}
        }
    }
    // Check KITTY_WINDOW_ID
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return ImageProtocol::Kitty;
    }
    ImageProtocol::None
}

/// Render an image to the terminal using the specified protocol.
///
/// Writes escape sequences directly to the controlling terminal (`/dev/tty`),
/// bypassing stdout so they reach the terminal even when stdout is piped or
/// captured. Returns the estimated number of terminal lines the image
/// occupies, or `None` if rendering failed.
///
/// `data` should be raw image bytes (PNG, JPEG, GIF, etc.).
/// `max_width` and `max_height` are the maximum cell dimensions to use.
pub fn render_image_to_terminal(
    data: &[u8],
    protocol: &ImageProtocol,
    max_width: u16,
    max_height: u16,
) -> Option<usize> {
    if data.is_empty() {
        return None;
    }

    let result = match protocol {
        ImageProtocol::Kitty => render_kitty(data, max_width, max_height),
        ImageProtocol::ITerm2 => render_iterm2(data, max_width, max_height),
        ImageProtocol::Sixel => {
            // Sixel requires complex color quantization; show placeholder
            render_placeholder(data)
        }
        ImageProtocol::None => render_placeholder(data),
    };

    result.ok()
}

/// Render using the Kitty graphics protocol.
///
/// The Kitty protocol transmits base64-encoded image data in APC sequences:
///   `\x1b_Gkey=value,...;base64data\x1b\\`
///
/// For large images the payload is chunked (4096 bytes per chunk) with
/// `m=1` on continuation chunks and `m=0` on the final chunk.
///
/// Reference: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
fn render_kitty(data: &[u8], max_width: u16, max_height: u16) -> io::Result<usize> {
    let encoded = BASE64.encode(data);
    let mut tty = open_tty()?;

    // Chunk size for kitty protocol (4096 is the conventional limit)
    const CHUNK_SIZE: usize = 4096;
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(CHUNK_SIZE)
        .map(|c| {
            // SAFETY: base64 output is always valid ASCII/UTF-8
            std::str::from_utf8(c).expect("base64 is valid UTF-8")
        })
        .collect();

    let total_chunks = chunks.len();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == total_chunks - 1;
        let more = if is_last { 0 } else { 1 };

        if is_first {
            // First chunk: include format/action/size params
            // f=100 = auto-detect format, a=T = transmit and display
            // c/r = columns/rows to occupy
            write!(tty, "\x1b_Ga=T,f=100,m={more},c={c},r={r};{chunk}\x1b\\", c = max_width, r = max_height,)?;
        } else {
            // Continuation chunk
            write!(tty, "\x1b_Gm={more};{chunk}\x1b\\")?;
        }
    }

    // Move cursor below the image
    writeln!(tty)?;
    tty.flush()?;

    Ok(max_height as usize)
}

/// Render using the iTerm2 inline image protocol.
///
/// The iTerm2 protocol uses a single OSC 1337 sequence:
///   `\x1b]1337;File=key=value;key=value:base64data\x07`
///
/// Reference: <https://iterm2.com/documentation-images.html>
fn render_iterm2(data: &[u8], max_width: u16, _max_height: u16) -> io::Result<usize> {
    let encoded = BASE64.encode(data);
    let mut tty = open_tty()?;

    write!(
        tty,
        "\x1b]1337;File=inline=1;size={size};width={width}:{encoded}\x07",
        size = data.len(),
        width = max_width,
    )?;
    writeln!(tty)?;
    tty.flush()?;

    // iTerm2 auto-sizes; estimate ~half the max height
    let estimated_lines = (max_width as usize).max(1);
    Ok(estimated_lines.min(20))
}

/// Placeholder rendering for unsupported protocols.
fn render_placeholder(data: &[u8]) -> io::Result<usize> {
    let mut tty = open_tty()?;
    let size = data.len();
    if size >= 1024 * 1024 {
        writeln!(tty, "[Image: {:.1} MB]", size as f64 / (1024.0 * 1024.0))?;
    } else if size >= 1024 {
        writeln!(tty, "[Image: {:.1} KB]", size as f64 / 1024.0)?;
    } else {
        writeln!(tty, "[Image: {size} bytes]")?;
    }
    tty.flush()?;
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_protocol_none() {
        // In a test environment without terminal vars, should get None or whatever
        // the env happens to have. Just ensure it doesn't panic.
        let _protocol = detect_protocol();
    }

    #[test]
    fn test_render_empty_data_returns_none() {
        let result = render_image_to_terminal(&[], &ImageProtocol::None, 80, 24);
        assert_eq!(result, None);
    }

    #[test]
    fn test_render_placeholder_bytes() {
        let result = render_placeholder(&[0u8; 500]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_render_placeholder_kb() {
        let result = render_placeholder(&[0u8; 2048]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_render_placeholder_mb() {
        let data = vec![0u8; 1024 * 1024 + 1];
        let result = render_placeholder(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_render_none_protocol_uses_placeholder() {
        let result = render_image_to_terminal(&[1, 2, 3], &ImageProtocol::None, 80, 24);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_render_sixel_falls_back_to_placeholder() {
        let result = render_image_to_terminal(&[1, 2, 3], &ImageProtocol::Sixel, 80, 24);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_kitty_render_small_image() {
        // Just verify it doesn't panic/error with small data
        let data = b"fake png data for testing";
        let result = render_kitty(data, 40, 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);
    }

    #[test]
    fn test_kitty_render_chunked() {
        // Data large enough to require multiple chunks (>4096 bytes base64)
        let data = vec![0xABu8; 4000]; // ~5336 base64 chars -> 2 chunks
        let result = render_kitty(&data, 80, 24);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 24);
    }

    #[test]
    fn test_iterm2_render() {
        let data = b"fake image bytes";
        let result = render_iterm2(data, 40, 20);
        assert!(result.is_ok());
    }

    #[test]
    fn test_protocol_eq() {
        assert_eq!(ImageProtocol::Kitty, ImageProtocol::Kitty);
        assert_ne!(ImageProtocol::Kitty, ImageProtocol::ITerm2);
        assert_ne!(ImageProtocol::None, ImageProtocol::Sixel);
    }
}
