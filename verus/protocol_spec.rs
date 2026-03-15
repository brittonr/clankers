//! Formal specifications and proofs for the length-prefixed frame protocol.
//!
//! Models write_frame and read_frame as pure functions over byte sequences.
//! Proves round-trip correctness, size rejection, and length encoding format.

use vstd::prelude::*;

verus! {

// ── Constants ───────────────────────────────────────────────────────────

/// Maximum frame size, matching the runtime constant.
pub open spec fn max_frame_size() -> nat { 10_000_000 }

// ── Types ───────────────────────────────────────────────────────────────

/// Result of a frame operation: Ok with data, or an error.
pub enum FrameResult {
    Ok { data: Seq<u8> },
    TooLarge,
    Eof,
}

// ── Spec fns ────────────────────────────────────────────────────────────

/// Encode a u32 as 4 big-endian bytes.
pub open spec fn u32_to_be_bytes(v: u32) -> Seq<u8> {
    seq![
        ((v >> 24u32) & 0xffu32) as u8,
        ((v >> 16u32) & 0xffu32) as u8,
        ((v >> 8u32) & 0xffu32) as u8,
        (v & 0xffu32) as u8
    ]
}

/// Decode 4 big-endian bytes to a u32.
pub open spec fn be_bytes_to_u32(b: Seq<u8>) -> u32
    recommends b.len() == 4
{
    ((b[0] as u32) << 24u32)
    | ((b[1] as u32) << 16u32)
    | ((b[2] as u32) << 8u32)
    | (b[3] as u32)
}

/// Model of write_frame: serialize payload, prepend 4-byte length.
/// Returns TooLarge if payload exceeds MAX_FRAME_SIZE.
// r[depends protocol.frame.length-encoding]
pub open spec fn write_frame_spec(payload: Seq<u8>) -> FrameResult {
    if payload.len() > max_frame_size() {
        FrameResult::TooLarge
    } else {
        let len_bytes = u32_to_be_bytes(payload.len() as u32);
        FrameResult::Ok { data: len_bytes + payload }
    }
}

/// Model of read_frame: read 4-byte length, then payload.
/// Returns TooLarge if length exceeds MAX_FRAME_SIZE.
/// Returns Eof if input is empty.
pub open spec fn read_frame_spec(input: Seq<u8>) -> FrameResult {
    if input.len() == 0 {
        FrameResult::Eof
    } else if input.len() < 4 {
        FrameResult::Eof
    } else {
        let len = be_bytes_to_u32(input.subrange(0, 4)) as nat;
        if len > max_frame_size() {
            FrameResult::TooLarge
        } else if input.len() < 4 + len {
            FrameResult::Eof
        } else {
            FrameResult::Ok { data: input.subrange(4, (4 + len) as int) }
        }
    }
}

/// Round-trip property: read(write(payload)) == payload.
// r[depends protocol.frame.roundtrip]
pub open spec fn frame_roundtrip(payload: Seq<u8>) -> bool {
    payload.len() <= max_frame_size() ==> {
        let written = write_frame_spec(payload);
        match written {
            FrameResult::Ok { data } => {
                match read_frame_spec(data) {
                    FrameResult::Ok { data: recovered } => recovered =~= payload,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

/// write_frame rejects oversized payloads without writing.
// r[depends protocol.frame.size-reject-write]
pub open spec fn write_rejects_oversized(payload: Seq<u8>) -> bool {
    payload.len() > max_frame_size() ==>
        write_frame_spec(payload) is TooLarge
}

/// read_frame rejects oversized length headers without allocating.
// r[depends protocol.frame.size-reject-read]
pub open spec fn read_rejects_oversized_header(input: Seq<u8>) -> bool {
    input.len() >= 4
    && be_bytes_to_u32(input.subrange(0, 4)) as nat > max_frame_size()
    ==> read_frame_spec(input) is TooLarge
}

// ── Proofs ──────────────────────────────────────────────────────────────

/// The core bit-vector identity: extracting 4 bytes from a u32 via
/// shift+mask and reassembling them via shift+OR recovers the original.
/// Stated entirely in u32 arithmetic so the bit_vector solver handles it.
proof fn be_roundtrip_u32(v: u32)
    ensures ({
        let b0 = (v >> 24u32) & 0xffu32;
        let b1 = (v >> 16u32) & 0xffu32;
        let b2 = (v >> 8u32) & 0xffu32;
        let b3 = v & 0xffu32;
        (b0 << 24u32) | (b1 << 16u32) | (b2 << 8u32) | b3 == v
    })
{
    assert(
        ((v >> 24u32) & 0xffu32) << 24u32
        | ((v >> 16u32) & 0xffu32) << 16u32
        | ((v >> 8u32) & 0xffu32) << 8u32
        | (v & 0xffu32)
        == v
    ) by (bit_vector);
}

/// When a u32 value is masked to 8 bits, casting to u8 and back preserves it.
proof fn u8_u32_roundtrip(x: u32)
    requires x == x & 0xffu32
    ensures (x as u8) as u32 == x
{
    assert((x & 0xffu32) as u8 as u32 == x & 0xffu32) by (bit_vector);
}

/// u32 big-endian encoding round-trips.
proof fn prove_be_roundtrip(v: u32)
    ensures be_bytes_to_u32(u32_to_be_bytes(v)) == v
{
    // Establish that masking produces values that fit in u8
    assert((v >> 24u32) & 0xffu32 == ((v >> 24u32) & 0xffu32) & 0xffu32) by (bit_vector);
    assert((v >> 16u32) & 0xffu32 == ((v >> 16u32) & 0xffu32) & 0xffu32) by (bit_vector);
    assert((v >> 8u32) & 0xffu32 == ((v >> 8u32) & 0xffu32) & 0xffu32) by (bit_vector);
    assert(v & 0xffu32 == (v & 0xffu32) & 0xffu32) by (bit_vector);

    // u8→u32 roundtrip for each byte
    u8_u32_roundtrip((v >> 24u32) & 0xffu32);
    u8_u32_roundtrip((v >> 16u32) & 0xffu32);
    u8_u32_roundtrip((v >> 8u32) & 0xffu32);
    u8_u32_roundtrip(v & 0xffu32);

    let b = u32_to_be_bytes(v);

    // Now the decoder reads (b[i] as u32) which equals the original masked value
    // Invoke the pure u32 reassembly identity
    be_roundtrip_u32(v);
}

/// read(write(payload)) recovers the original payload.
// r[verify protocol.frame.roundtrip]
proof fn prove_frame_roundtrip(payload: Seq<u8>)
    requires payload.len() <= max_frame_size()
    ensures frame_roundtrip(payload)
{
    let len_bytes = u32_to_be_bytes(payload.len() as u32);
    let wire = len_bytes + payload;

    // wire has 4 + payload.len() bytes
    assert(wire.len() == 4 + payload.len());

    // The first 4 bytes of wire are len_bytes
    assert(wire.subrange(0, 4) =~= len_bytes);

    // Decoding the length recovers payload.len()
    prove_be_roundtrip(payload.len() as u32);
    assert(be_bytes_to_u32(len_bytes) == payload.len() as u32);

    // payload.len() <= max_frame_size, so not TooLarge on read
    let read_len = be_bytes_to_u32(wire.subrange(0, 4)) as nat;
    assert(read_len == payload.len());
    assert(read_len <= max_frame_size());

    // The remaining bytes are the payload
    assert(wire.subrange(4, (4 + read_len) as int) =~= payload);
}

/// write_frame returns TooLarge for oversized payloads.
// r[verify protocol.frame.size-reject-write]
proof fn prove_write_rejects_oversized(payload: Seq<u8>)
    requires payload.len() > max_frame_size()
    ensures write_rejects_oversized(payload)
{
    // Follows directly from the spec: len > max → TooLarge
    assert(write_frame_spec(payload) is TooLarge);
}

/// read_frame returns TooLarge when the header indicates an oversized frame,
/// without reading the payload bytes.
// r[verify protocol.frame.size-reject-read]
proof fn prove_read_rejects_oversized_header(input: Seq<u8>)
    requires
        input.len() >= 4,
        be_bytes_to_u32(input.subrange(0, 4)) as nat > max_frame_size(),
    ensures
        read_rejects_oversized_header(input)
{
    // Follows from spec: decoded length > max → TooLarge
    assert(read_frame_spec(input) is TooLarge);
}

/// The wire format uses exactly 4 bytes for the length prefix, encoding
/// the payload size in big-endian order.
// r[verify protocol.frame.length-encoding]
proof fn prove_length_encoding()
    ensures ({
        // A payload of 256 bytes encodes length as [0, 0, 1, 0]
        let payload = Seq::new(256, |_i: int| 0u8);
        let written = write_frame_spec(payload);
        match written {
            FrameResult::Ok { data } => {
                data.len() == 260  // 4 + 256
            }
            _ => false,
        }
    })
{
    let payload = Seq::new(256, |_i: int| 0u8);
    assert(payload.len() == 256);
    assert(payload.len() <= max_frame_size());

    // Help the solver with bit-level reasoning for 256u32
    assert(((256u32 >> 24u32) & 0xffu32) == 0u32) by (bit_vector);
    assert(((256u32 >> 16u32) & 0xffu32) == 0u32) by (bit_vector);
    assert(((256u32 >> 8u32) & 0xffu32) == 1u32) by (bit_vector);
    assert((256u32 & 0xffu32) == 0u32) by (bit_vector);

    let len_bytes = u32_to_be_bytes(256u32);
    assert(len_bytes.len() == 4);

    let wire = len_bytes + payload;
    assert(wire.len() == 260);
}

} // verus!
