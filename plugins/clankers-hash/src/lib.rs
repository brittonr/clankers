//! clankers-hash plugin — Cryptographic hash and encoding utilities.
//!
//! Provides two tools for the clankers agent:
//!
//! - **`hash_text`** — Compute SHA-256 or MD5 digests of arbitrary text.
//! - **`encode_text`** — Encode / decode text with Base64 or Hex.
//!
//! All algorithms are implemented in pure Rust with zero external crypto
//! dependencies, so the module compiles cleanly to `wasm32-unknown-unknown`.

use clanker_plugin_sdk::prelude::*;

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest functions
// ═══════════════════════════════════════════════════════════════════════

/// Dispatch a tool call to the appropriate handler.
#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("hash_text", handle_hash_text),
        ("encode_text", handle_encode_text),
    ])
}

/// Handle a plugin lifecycle event.
#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "clankers-hash", &[
        ("agent_start", |_| "clankers-hash plugin initialized".to_string()),
        ("agent_end", |_| "clankers-hash plugin shutting down".to_string()),
    ])
}

/// Return plugin metadata as JSON.
#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new("clankers-hash", "0.1.0", &[
        ("hash_text", "Compute a cryptographic hash (SHA-256 or MD5) of text"),
        ("encode_text", "Encode or decode text with Base64 or Hex"),
    ], &[])))
}

// ═══════════════════════════════════════════════════════════════════════
//  Tool implementations
// ═══════════════════════════════════════════════════════════════════════

/// Process the `hash_text` tool call.
///
/// Args:
/// - `text` (required) — the string to hash.
/// - `algorithm` (optional) — `"sha256"` (default) or `"md5"`.
fn handle_hash_text(args: &Value) -> Result<String, String> {
    let text = args.require_str("text")?;
    let algorithm = args.get_str_or("algorithm", "sha256");

    match algorithm {
        "sha256" => Ok(hex_encode(&sha256(text.as_bytes()))),
        "md5" => Ok(hex_encode(&md5(text.as_bytes()))),
        other => Err(format!("unsupported algorithm: {other} (use sha256 or md5)")),
    }
}

/// Process the `encode_text` tool call.
///
/// Args:
/// - `text` (required) — the string to encode or decode.
/// - `encoding` (optional) — `"base64"` (default) or `"hex"`.
/// - `direction` (optional) — `"encode"` (default) or `"decode"`.
fn handle_encode_text(args: &Value) -> Result<String, String> {
    let text = args.require_str("text")?;
    let encoding = args.get_str_or("encoding", "base64");
    let direction = args.get_str_or("direction", "encode");

    match (encoding, direction) {
        ("base64", "encode") => Ok(base64_encode(text.as_bytes())),
        ("base64", "decode") => {
            let bytes = base64_decode(text)?;
            String::from_utf8(bytes)
                .map_err(|e| format!("decoded bytes are not valid UTF-8: {e}"))
        }
        ("hex", "encode") => Ok(hex_encode(text.as_bytes())),
        ("hex", "decode") => {
            let bytes = hex_decode(text)?;
            String::from_utf8(bytes)
                .map_err(|e| format!("decoded bytes are not valid UTF-8: {e}"))
        }
        (enc, dir) => Err(format!(
            "unsupported encoding/direction: {enc}/{dir} \
             (use base64|hex with encode|decode)"
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Pure-Rust SHA-256 implementation (FIPS 180-4)
// ═══════════════════════════════════════════════════════════════════════

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7)
                ^ w[i - 15].rotate_right(18)
                ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17)
                ^ w[i - 2].rotate_right(19)
                ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g; g = f; f = e;
            e = d.wrapping_add(temp1);
            d = c; c = b; b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut digest = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        digest[4 * i..4 * i + 4].copy_from_slice(&val.to_be_bytes());
    }
    digest
}

// ═══════════════════════════════════════════════════════════════════════
//  Pure-Rust MD5 implementation (RFC 1321)
// ═══════════════════════════════════════════════════════════════════════

const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const MD5_T: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
    0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
    0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
    0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
    0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
    0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
    0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
    0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
    0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

fn md5(data: &[u8]) -> [u8; 16] {
    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    for block in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };

            let temp = a
                .wrapping_add(f)
                .wrapping_add(MD5_T[i])
                .wrapping_add(m[g]);
            a = d; d = c; c = b;
            b = b.wrapping_add(temp.rotate_left(MD5_S[i]));
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut digest = [0u8; 16];
    digest[0..4].copy_from_slice(&a0.to_le_bytes());
    digest[4..8].copy_from_slice(&b0.to_le_bytes());
    digest[8..12].copy_from_slice(&c0.to_le_bytes());
    digest[12..16].copy_from_slice(&d0.to_le_bytes());
    digest
}

// ═══════════════════════════════════════════════════════════════════════
//  Hex encoding / decoding
// ═══════════════════════════════════════════════════════════════════════

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("hex string must have even length".to_string());
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = hex_val(pair[0])?;
            let lo = hex_val(pair[1])?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex character: '{}'", c as char)),
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Base64 encoding / decoding (RFC 4648, standard alphabet, with padding)
// ═══════════════════════════════════════════════════════════════════════

const B64_CHARS: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for c in s.bytes() {
        let val = b64_val(c)?;
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

fn b64_val(c: u8) -> Result<u8, String> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(format!("invalid base64 character: '{}'", c as char)),
    }
}
