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

use std::collections::HashMap;
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
    Iterm2,
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
            "iTerm.app" => return ImageProtocol::Iterm2,
            "WezTerm" | "kitty" | "Ghostty" => return ImageProtocol::Kitty,
            // foot terminal supports Sixel natively
            "foot" => return ImageProtocol::Sixel,
            _ => {}
        }
    }
    // Check KITTY_WINDOW_ID
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return ImageProtocol::Kitty;
    }
    // Explicit Sixel override via env var
    if std::env::var("CLANKERS_SIXEL").is_ok() {
        return ImageProtocol::Sixel;
    }
    // Check TERM for known Sixel-capable terminals
    if let Ok(term) = std::env::var("TERM")
        && (term.contains("mlterm") || term.contains("yaft"))
    {
        return ImageProtocol::Sixel;
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
        ImageProtocol::Iterm2 => render_iterm2(data, max_width, max_height),
        ImageProtocol::Sixel => render_sixel(data, max_width, max_height).or_else(|_| render_placeholder(data)),
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
        let more = i32::from(!is_last);

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
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "base64 output is always valid UTF-8")
)]
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

/// Render using the Sixel protocol.
///
/// Sixel encodes images as escape sequences where each "sixel row" represents
/// 6 vertical pixels. Colors are defined in a palette (max 255), then pixel
/// data is sent as characters encoding 6-bit column bitmaps.
///
/// Format: `DCS q <palette> <pixel data> ST`
///
/// Reference: <https://en.wikipedia.org/wiki/Sixel>
fn render_sixel(data: &[u8], max_width: u16, max_height: u16) -> io::Result<usize> {
    // Decode the image
    let img = image::load_from_memory(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Resize to fit within max cell dimensions.
    // Assume ~8px per cell width, ~16px per cell height (common terminal font metrics).
    let cell_width_px = 8u32;
    let cell_height_px = 16u32;
    let max_px_w = u32::from(max_width) * cell_width_px;
    let max_px_h = u32::from(max_height) * cell_height_px;

    let img = img.resize(max_px_w, max_px_h, image::imageops::FilterType::Triangle);
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    if width == 0 || height == 0 {
        return render_placeholder(data);
    }

    // Quantize to 255 colors using frequency-based palette
    let pixels: Vec<[u8; 3]> = rgba
        .pixels()
        .filter(|p| p.0[3] > 128) // skip mostly-transparent pixels
        .map(|p| [p.0[0], p.0[1], p.0[2]])
        .collect();

    let palette = build_palette(&pixels, 255);
    if palette.is_empty() {
        return render_placeholder(data);
    }
    let color_map = build_color_lookup(&palette);

    // Build the sixel output
    let mut sixel = String::with_capacity(width as usize * height as usize);

    // DCS with P1=0 (normal aspect), P2=1 (transparent bg), P3=0 (pixel size from device)
    sixel.push_str("\x1bP0;1;0q");

    // Raster attributes: "1;1;width;height
    sixel.push_str(&format!("\"1;1;{};{}", width, height));

    // Define palette entries: #n;2;r%;g%;b%
    for (i, color) in palette.iter().enumerate() {
        let r_pct = (u32::from(color[0]) * 100) / 255;
        let g_pct = (u32::from(color[1]) * 100) / 255;
        let b_pct = (u32::from(color[2]) * 100) / 255;
        sixel.push_str(&format!("#{};2;{};{};{}", i, r_pct, g_pct, b_pct));
    }

    // Encode pixel data in sixel rows (6 pixels high each)
    for sixel_row in (0..height).step_by(6) {
        // Pre-scan which colors appear in this 6-row band
        let mut colors_used: Vec<usize> = Vec::new();
        for y in sixel_row..std::cmp::min(sixel_row + 6, height) {
            for x in 0..width {
                let pixel = rgba.get_pixel(x, y);
                if pixel.0[3] > 128 {
                    let rgb = [pixel.0[0], pixel.0[1], pixel.0[2]];
                    let ci = nearest_color(&rgb, &palette, &color_map);
                    if !colors_used.contains(&ci) {
                        colors_used.push(ci);
                    }
                }
            }
        }
        colors_used.sort_unstable();

        for (color_pass, &ci) in colors_used.iter().enumerate() {
            // Select color
            sixel.push_str(&format!("#{}", ci));

            for x in 0..width {
                let mut sixel_bits: u8 = 0;
                for dy in 0..6u32 {
                    let y = sixel_row + dy;
                    if y < height {
                        let pixel = rgba.get_pixel(x, y);
                        if pixel.0[3] > 128 {
                            let rgb = [pixel.0[0], pixel.0[1], pixel.0[2]];
                            if nearest_color(&rgb, &palette, &color_map) == ci {
                                sixel_bits |= 1 << dy;
                            }
                        }
                    }
                }
                sixel.push((b'?' + sixel_bits) as char);
            }

            // `$` = carriage return (same row, next color pass)
            // `-` = newline (advance to next sixel row)
            if color_pass + 1 < colors_used.len() {
                sixel.push('$');
            }
        }
        sixel.push('-');
    }

    // String Terminator
    sixel.push_str("\x1b\\");

    // Write to terminal
    let mut tty = open_tty()?;
    write!(tty, "{}", sixel)?;
    writeln!(tty)?;
    tty.flush()?;

    // Estimate terminal lines used
    let lines_used = (f64::from(height) / f64::from(cell_height_px)).ceil() as usize;
    Ok(lines_used)
}

/// Build a color palette from pixel data using frequency-based quantization.
///
/// Reduces to 5-bit per channel (32k color space), then takes the most
/// frequent colors up to `max_colors`.
fn build_palette(pixels: &[[u8; 3]], max_colors: usize) -> Vec<[u8; 3]> {
    let mut freq: HashMap<[u8; 3], usize> = HashMap::new();
    for &px in pixels {
        // Reduce to 5 bits per channel (~32k colors)
        let key = [px[0] & 0xF8, px[1] & 0xF8, px[2] & 0xF8];
        *freq.entry(key).or_insert(0) += 1;
    }

    let mut sorted: Vec<_> = freq.into_iter().collect();
    sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    sorted.truncate(max_colors);

    sorted.into_iter().map(|(color, _)| color).collect()
}

/// Build a lookup table mapping quantized RGB keys to palette indices
fn build_color_lookup(palette: &[[u8; 3]]) -> HashMap<[u8; 3], usize> {
    let mut map = HashMap::new();
    for (i, &color) in palette.iter().enumerate() {
        map.insert(color, i);
    }
    map
}

/// Find the nearest palette color index for an RGB value.
///
/// Fast path: exact match on quantized key. Slow path: Euclidean distance.
fn nearest_color(rgb: &[u8; 3], palette: &[[u8; 3]], lookup: &HashMap<[u8; 3], usize>) -> usize {
    // Fast path: exact quantized match
    let key = [rgb[0] & 0xF8, rgb[1] & 0xF8, rgb[2] & 0xF8];
    if let Some(&idx) = lookup.get(&key) {
        return idx;
    }

    // Slow path: nearest by Euclidean distance
    let mut best_idx = 0;
    let mut best_dist = u32::MAX;
    for (i, color) in palette.iter().enumerate() {
        let dr = i32::from(rgb[0]) - i32::from(color[0]);
        let dg = i32::from(rgb[1]) - i32::from(color[1]);
        let db = i32::from(rgb[2]) - i32::from(color[2]);
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_idx = i;
        }
    }
    best_idx
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
    fn test_render_sixel_invalid_data_falls_back() {
        // Invalid image data → render_sixel fails → falls back to placeholder
        let result = render_image_to_terminal(&[1, 2, 3], &ImageProtocol::Sixel, 80, 24);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_render_sixel_valid_png() {
        // Create a minimal 2x2 red PNG
        let mut img = image::RgbaImage::new(2, 2);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }
        let mut bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();

        let result = render_sixel(&bytes, 40, 10);
        assert!(result.is_ok());
        assert!(result.unwrap() >= 1);
    }

    #[test]
    fn test_build_palette_limits_colors() {
        let pixels: Vec<[u8; 3]> = (0..=255).map(|i| [i, 0, 0]).collect();
        let palette = build_palette(&pixels, 10);
        assert!(palette.len() <= 10);
    }

    #[test]
    fn test_build_palette_empty_input() {
        let palette = build_palette(&[], 10);
        assert!(palette.is_empty());
    }

    #[test]
    fn test_nearest_color_exact_match() {
        let palette = vec![[0, 0, 0], [248, 0, 0], [0, 248, 0]];
        let lookup = build_color_lookup(&palette);
        // [255,0,0] quantizes to [248,0,0] which is palette index 1
        assert_eq!(nearest_color(&[255, 0, 0], &palette, &lookup), 1);
    }

    #[test]
    fn test_nearest_color_distance_fallback() {
        let palette = vec![[0, 0, 0], [200, 0, 0]];
        let lookup = build_color_lookup(&palette);
        // [180,0,0] quantizes to [176,0,0] — not in lookup, so falls back to distance
        assert_eq!(nearest_color(&[180, 0, 0], &palette, &lookup), 1);
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
        assert_ne!(ImageProtocol::Kitty, ImageProtocol::Iterm2);
        assert_ne!(ImageProtocol::None, ImageProtocol::Sixel);
    }
}
