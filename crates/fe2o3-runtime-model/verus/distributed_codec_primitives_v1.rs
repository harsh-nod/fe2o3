// Actual private byte/cursor bodies, not a complete wire codec refinement.
// Existing declarations and structural equality retain their qualified bridge.
// Slice indexing/copying, checked arithmetic, Rust and pinned vstd remain the
// standard-library/compiler trust boundary; this unit adds no trusted adapter.
// Writer capacity is an internal caller obligation, not a decoder premise.
include!("distributed_publication_construction_v1.rs");
include!("../src/distributed_publication_contract/codec_primitives_body.rs");

verus! {

spec fn readable(bytes: Seq<u8>, offset: usize, count: usize) -> bool {
    offset as int + count as int <= bytes.len()
}

fn take<'a>(bytes: &'a [u8], offset: &mut usize, count: usize)
    -> (result: Result<&'a [u8], E>)
    ensures
        result.is_ok() == readable(bytes@, *old(offset), count),
        match result {
            Ok(value) => value@ == bytes@.subrange(
                    *old(offset) as int, *old(offset) as int + count as int)
                && *final(offset) as int == *old(offset) as int + count as int,
            Err(error) => error == E::WrongLength && *final(offset) == *old(offset),
        },
{
    proof {
        vstd::slice::axiom_spec_len(bytes);
    }
    distributed_codec_take_body_v1!(verus_exec_expr, bytes, offset, count)
}

fn fixed<const N: usize>(bytes: &[u8], offset: &mut usize) -> (result: Result<[u8; N], E>)
    ensures
        result.is_ok() == readable(bytes@, *old(offset), N),
        match result {
            Ok(value) => value@ == bytes@.subrange(
                    *old(offset) as int, *old(offset) as int + N as int)
                && *final(offset) as int == *old(offset) as int + N as int,
            Err(error) => error == E::WrongLength && *final(offset) == *old(offset),
        },
{
    distributed_codec_fixed_body_v1!(verus_exec_expr, bytes, offset, N)
}

fn put(bytes: &mut [u8], offset: &mut usize, value: &[u8])
    requires
        *old(offset) <= old(bytes)@.len(),
        value@.len() <= old(bytes)@.len() - *old(offset) as int,
    ensures
        *final(offset) as int == *old(offset) as int + value@.len(),
        final(bytes)@.len() == old(bytes)@.len(),
        final(bytes)@.subrange(*old(offset) as int, *final(offset) as int) == value@,
        forall|i: int| 0 <= i < old(bytes)@.len()
            && !( *old(offset) as int <= i < *final(offset) as int)
            ==> #[trigger] final(bytes)@[i] == old(bytes)@[i],
{
    distributed_codec_put_body_v1!(verus_exec_expr, bytes, offset, value)
}

fn finish(bytes: &[u8], offset: usize) -> (result: Result<(), E>)
    ensures result == if offset == bytes@.len() { Ok(()) } else { Err(E::WrongLength) },
{
    distributed_codec_finish_body_v1!(verus_exec_expr, bytes, offset)
}

spec fn little_u16(value: u16) -> [u8; 2] {
    [value as u8, (value >> 8) as u8]
}

spec fn value_u16(bytes: [u8; 2]) -> u16 {
    (bytes[0] as u16) | ((bytes[1] as u16) << 8)
}

spec fn little_u64(value: u64) -> [u8; 8] {
    [value as u8, (value >> 8) as u8, (value >> 16) as u8, (value >> 24) as u8,
        (value >> 32) as u8, (value >> 40) as u8, (value >> 48) as u8, (value >> 56) as u8]
}

spec fn value_u64(bytes: [u8; 8]) -> u64 {
    (bytes[0] as u64) | ((bytes[1] as u64) << 8) | ((bytes[2] as u64) << 16)
        | ((bytes[3] as u64) << 24) | ((bytes[4] as u64) << 32)
        | ((bytes[5] as u64) << 40) | ((bytes[6] as u64) << 48) | ((bytes[7] as u64) << 56)
}

fn u16_le(value: u16) -> (result: [u8; 2])
    ensures result == little_u16(value),
{
    distributed_codec_u16_le_body_v1!(verus_exec_expr, value)
}

fn u16_from_le(bytes: [u8; 2]) -> (result: u16)
    ensures result == value_u16(bytes),
{
    distributed_codec_u16_from_le_body_v1!(verus_exec_expr, bytes)
}

fn u64_le(value: u64) -> (result: [u8; 8])
    ensures result == little_u64(value),
{
    distributed_codec_u64_le_body_v1!(verus_exec_expr, value)
}

fn u64_from_le(bytes: [u8; 8]) -> (result: u64)
    ensures result == value_u64(bytes),
{
    distributed_codec_u64_from_le_body_v1!(verus_exec_expr, bytes)
}

fn u16_value_roundtrip(value: u16) -> (result: u16)
    ensures result == value,
{
    let encoded = u16_le(value);
    let result = u16_from_le(encoded);
    proof {
        assert(((value as u8) as u16) | ((((value >> 8) as u8) as u16) << 8) == value)
            by (bit_vector);
    }
    result
}

fn u16_bytes_roundtrip(bytes: [u8; 2]) -> (result: [u8; 2])
    ensures result == bytes,
{
    let value = u16_from_le(bytes);
    let result = u16_le(value);
    proof {
        let a = bytes[0];
        let b = bytes[1];
        assert((((a as u16) | ((b as u16) << 8)) as u8) == a) by (bit_vector);
        assert(((((a as u16) | ((b as u16) << 8)) >> 8) as u8) == b) by (bit_vector);
        assert(result@ == bytes@);
    }
    result
}

fn u64_value_roundtrip(value: u64) -> (result: u64)
    ensures result == value,
{
    let encoded = u64_le(value);
    let result = u64_from_le(encoded);
    proof {
        assert(((value as u8) as u64)
            | ((((value >> 8) as u8) as u64) << 8)
            | ((((value >> 16) as u8) as u64) << 16)
            | ((((value >> 24) as u8) as u64) << 24)
            | ((((value >> 32) as u8) as u64) << 32)
            | ((((value >> 40) as u8) as u64) << 40)
            | ((((value >> 48) as u8) as u64) << 48)
            | ((((value >> 56) as u8) as u64) << 56) == value) by (bit_vector);
    }
    result
}

fn u64_bytes_roundtrip(bytes: [u8; 8]) -> (result: [u8; 8])
    ensures result == bytes,
{
    let value = u64_from_le(bytes);
    let result = u64_le(value);
    proof {
        let a = bytes[0];
        let b = bytes[1];
        let c = bytes[2];
        let d = bytes[3];
        let e = bytes[4];
        let f = bytes[5];
        let g = bytes[6];
        let h = bytes[7];
        let joined = (a as u64) | ((b as u64) << 8) | ((c as u64) << 16) | ((d as u64) << 24)
            | ((e as u64) << 32) | ((f as u64) << 40) | ((g as u64) << 48) | ((h as u64) << 56);
        assert(joined == ((a as u64) | ((b as u64) << 8) | ((c as u64) << 16) | ((d as u64) << 24)
            | ((e as u64) << 32) | ((f as u64) << 40) | ((g as u64) << 48) | ((h as u64) << 56))
            ==> (joined as u8 == a
            && (joined >> 8) as u8 == b && (joined >> 16) as u8 == c
            && (joined >> 24) as u8 == d && (joined >> 32) as u8 == e
            && (joined >> 40) as u8 == f && (joined >> 48) as u8 == g
            && (joined >> 56) as u8 == h)) by (bit_vector);
        assert(result@ == bytes@);
    }
    result
}

}
