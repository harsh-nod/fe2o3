//! Inert encoding/file controls, never an authenticated compiler-owner fixture.
use super::*;

fn header(session: u32) -> Header {
    let role = session % 2;
    Header {
        role,
        order: session / 2,
        session,
        source_sha256: [19; 32],
        canonical_sha256: [23; 32],
        root: 0,
        helper: if role == 0 { u32::MAX } else { 1 },
        call: if role == 0 { [u32::MAX; 2] } else { [7, 2] },
        matrix: [8, 3],
        store: [9, 4],
    }
}
fn backing<const N: usize, const I: usize>(
    id: u64,
    allocation: u64,
    offset: u64,
    length: u64,
) -> Backing<N, I> {
    Backing {
        input_id: id,
        allocation,
        byte_length: N as u32,
        init_bit_length: N as u32,
        view_offset: offset,
        view_length: length,
        bytes: [0; N],
        initialized: [255; I],
    }
}
fn positive(h: Header, ordinal: u32) -> Positive {
    let length = [64, 13, 0][ordinal as usize % 3];
    let mut matrix = [0; 256];
    let mut caller = [0; 256];
    for (i, v) in matrix.iter_mut().enumerate() {
        *v = i as u32 + 17;
    }
    let permutation = if h.role == 1 && h.order == 1 {
        [1, 0, 2, 3]
    } else {
        [0, 1, 2, 3]
    };
    for lane in 0..64 {
        for c in 0..4 {
            caller[(4 * (lane / 16) + c) * 16 + lane % 16] =
                matrix[(4 * (lane / 16) + permutation[c]) * 16 + lane % 16];
        }
    }
    let mut output = backing::<272, 34>(13, 103, 8, u64::from(length));
    for raw in output.bytes.chunks_exact_mut(4) {
        raw.copy_from_slice(&0x7f123456u32.to_le_bytes());
    }
    let mut writes = [[0; 8]; 64];
    for lane in 0..length as usize {
        let bits = caller[4 * (lane / 16) * 16 + lane % 16];
        output.bytes[8 + 4 * lane..12 + 4 * lane].copy_from_slice(&bits.to_le_bytes());
        writes[lane] = [lane as u32, 4, 8 + 4 * lane as u32, 0, bits, 103, 0, 0];
    }
    Positive {
        pattern: ordinal / 3,
        output_length: length,
        records: 99,
        steps: 123,
        matrix_mask: u64::MAX,
        caller_mask: u64::MAX,
        store_mask: match length {
            64 => u64::MAX,
            13 => (1 << 13) - 1,
            _ => 0,
        },
        allocations: [101, 102, 103],
        matrix,
        caller,
        a: backing(11, 101, 0, 256),
        b: backing(12, 102, 0, 256),
        output,
        write_count: length,
        writes,
    }
}
fn negative(ordinal: u32) -> Negative {
    Negative {
        control: ordinal - 18,
        classification: [1, 1, 2, 3, 2, 3, 2, 3, 2, 4, 5, 5, 6, 7, 7, 7][(ordinal - 18) as usize],
        matrix_mask: 0,
        caller_mask: 0,
        global_writes: 0,
        floor_restored: 1,
    }
}
fn complete(h: Header) -> Vec<u8> {
    let mut out = Vec::with_capacity(COMPLETE_BYTES);
    out.extend(encode_header(h).unwrap());
    for ordinal in 0..18 {
        out.extend(encode_positive(h, ordinal, &positive(h, ordinal)).unwrap());
    }
    for ordinal in 18..34 {
        out.extend(encode_negative(ordinal, negative(ordinal)).unwrap());
    }
    out.extend(footer(Sha256::digest(&out).into()).unwrap());
    assert_eq!(out.len(), COMPLETE_BYTES);
    out
}
fn repair_footer(bytes: &mut [u8]) {
    let start = COMPLETE_BYTES - FOOTER_BYTES;
    let value = footer(Sha256::digest(&bytes[..start]).into()).unwrap();
    bytes[start..].copy_from_slice(&value);
}
fn assert_shape_mutation(offset: usize, value: u8) {
    let h = header(3);
    let mut bytes = complete(h);
    bytes[offset] = value;
    repair_footer(&mut bytes);
    assert!(validate_complete(&bytes, h).is_err(), "offset {offset}");
}

#[test]
fn exact_shape_offsets_and_all_four_headers_roundtrip() {
    assert_eq!(HEADER_BYTES, 128);
    assert_eq!(
        POSITIVE_BYTES,
        16 + 72 + 2048 + (3 * 40 + 1296 + 162) + 4 + 64 * 32 + 10
    );
    assert_eq!(NEGATIVE_BYTES, 16 + 8 + 24 + 4 + 28);
    assert_eq!(COMPLETE_BYTES, 105440);
    assert!(COMPLETE_BYTES <= FILE_CAP);
    for session in 0..4 {
        let h = header(session);
        let bytes = complete(h);
        let result = validate_complete(&bytes, h).unwrap();
        assert_eq!(result.bytes, 105440);
        assert_eq!(result.rows, 34);
        assert_eq!(result.sha256, <[u8; 32]>::from(Sha256::digest(&bytes)));
        assert_eq!(decode_header(&bytes[..128]).unwrap(), h);
        assert_eq!(
            decode_positive(h, 0, &bytes[128..128 + POSITIVE_BYTES]).unwrap(),
            positive(h, 0)
        );
    }
}
#[test]
fn exact_length_rejects_every_short_prefix_and_extra_data() {
    let h = header(0);
    let bytes = complete(h);
    // Length check returns before parsing, so this is bounded linear test work.
    for length in 0..COMPLETE_BYTES {
        assert!(validate_complete(&bytes[..length], h).is_err());
    }
    let mut extra = bytes;
    extra.push(0);
    assert!(validate_complete(&extra, h).is_err());
}
#[test]
fn reserved_frame_padding_and_footer_bytes_refuse_even_with_repaired_hash() {
    assert_shape_mutation(HEADER_BYTES + 12, 1);
    assert_shape_mutation(HEADER_BYTES + POSITIVE_BYTES - 1, 1);
    let negative_start = HEADER_BYTES + 18 * POSITIVE_BYTES;
    assert_shape_mutation(negative_start + 12, 1);
    assert_shape_mutation(negative_start + NEGATIVE_BYTES - 1, 1);
    let h = header(3);
    let mut bytes = complete(h);
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(validate_complete(&bytes, h).is_err());
}
#[test]
fn init_length_and_initialization_bits_are_exact_not_all_on_a_short_slice() {
    // A backing starts at frame(16)+fixed(72)+two arrays(2048).
    let a = HEADER_BYTES + 16 + 72 + 2048;
    assert_shape_mutation(a + 20, 1); // 512 init bits -> 513, never inferred from bytes.
    assert_shape_mutation(a + 21, 1); // 512 init bits -> 256.
}
#[test]
fn backing_length_bits_and_unused_output_store_slots_refuse() {
    let h = header(1);
    let mut row = positive(h, 0);
    row.a.init_bit_length = 511;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = positive(h, 0);
    row.b.byte_length = 511;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = positive(h, 0);
    row.output.initialized[33] &= 0x7f;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = positive(h, 1);
    row.writes[13][0] = 13;
    assert!(encode_positive(h, 1, &row).is_err());
    let a = HEADER_BYTES + 16 + 72 + 2048;
    assert_shape_mutation(a + 21, 1);
    assert_shape_mutation(a + 40 + 512, 0xfe);
}
#[test]
fn write_count_order_allocation_offset_and_bits_mutations_refuse() {
    let h = header(1);
    let good = positive(h, 0);
    let mut row = good.clone();
    row.write_count = 63;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = good.clone();
    row.writes.swap(0, 1);
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = good.clone();
    row.writes[0][5] = 102;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = good.clone();
    row.writes[0][2] = 12;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = good.clone();
    row.writes[0][4] ^= 1;
    assert!(encode_positive(h, 0, &row).is_err());
    let mut row = good;
    row.allocations[1] = 101;
    assert!(encode_positive(h, 0, &row).is_err());
}
#[test]
fn selected_header_order_and_direct_checkpoint_kind_are_not_inferred_from_data() {
    let identity = header(1);
    let swap = header(3);
    assert!(encode_positive(swap, 0, &positive(identity, 0)).is_err());
    assert!(encode_positive(identity, 0, &positive(swap, 0)).is_err());
    // Both original sessions still use the same direct MFMA values.
    let mut a = positive(header(0), 0);
    let b = positive(header(2), 0);
    assert_eq!(a, b);
    a.caller.swap(0, 16);
    assert!(encode_positive(header(2), 0, &a).is_err());
    let bytes = complete(identity);
    assert!(validate_complete(&bytes, swap).is_err());
    let mut h = header(0);
    h.call = [1, 2];
    assert!(encode_header(h).is_err());
}
#[test]
fn exact_negative_roster_and_no_pretend_rollback_counters() {
    for ordinal in 18..34 {
        let n = negative(ordinal);
        let bytes = encode_negative(ordinal, n).unwrap();
        assert_eq!(decode_negative(ordinal, &bytes).unwrap(), n);
        let mut changed = n;
        changed.control = (changed.control + 1) % 16;
        assert!(encode_negative(ordinal, changed).is_err());
        let mut changed = n;
        changed.global_writes = 1;
        assert!(encode_negative(ordinal, changed).is_err());
        let mut changed = n;
        changed.floor_restored = 0;
        assert!(encode_negative(ordinal, changed).is_err());
    }
}
#[test]
fn field_writers_and_readers_refuse_boundaries_without_wrapping() {
    let mut bytes = [0; 3];
    let mut p = Put::new(&mut bytes);
    assert!(p.u32(1).is_err());
    assert_eq!(p.offset, 0);
    let mut g = Get::new(&[0; 3]);
    assert!(g.u32().is_err());
    assert_eq!(g.offset, 0);
    let mut bytes = [0; 1];
    let mut p = Put {
        bytes: &mut bytes,
        offset: usize::MAX,
    };
    assert!(matches!(p.raw(&[1]), Err(Error::Arithmetic)));
    let mut g = Get {
        bytes: &[],
        offset: usize::MAX,
    };
    assert!(matches!(g.raw::<1>(), Err(Error::Arithmetic)));
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let parent =
            std::env::temp_dir().join(format!("fe2o3-bf16-sidecar-{}-{id}", std::process::id()));
        std::fs::create_dir(&parent).unwrap();
        Self(parent)
    }
    fn file(&self) -> std::path::PathBuf {
        self.0.join("cpu.bin")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.file());
        let _ = std::fs::remove_dir(&self.0);
    }
}
#[test]
fn create_new_preserves_existing_file_and_incomplete_prefix_is_not_complete() {
    let temp = Temp::new();
    let h = header(0);
    let mut w = Writer::create_new(&temp.file(), h).unwrap();
    assert!(Writer::create_new(&temp.file(), h).is_err());
    w.positive(&positive(h, 0)).unwrap();
    assert_eq!(
        w.committed_prefix(),
        (HEADER_BYTES + POSITIVE_BYTES, 1, false)
    );
    assert!(w.negative(negative(18)).is_err()); // No skipped positive rows.
    assert!(w.positive(&positive(h, 1)).is_err()); // sticky, no retry.
    assert_eq!(
        w.committed_prefix(),
        (HEADER_BYTES + POSITIVE_BYTES, 1, true)
    );
    drop(w);
    let bytes = std::fs::read(temp.file()).unwrap();
    assert_eq!(bytes.len(), HEADER_BYTES + POSITIVE_BYTES);
    assert!(validate_complete(&bytes, h).is_err());
}
#[test]
fn complete_file_footer_means_only_cpu_completion_and_cap_refusals_are_sticky() {
    let temp = Temp::new();
    let h = header(3);
    let mut w = Writer::create_new(&temp.file(), h).unwrap();
    for ordinal in 0..18 {
        w.positive(&positive(h, ordinal)).unwrap();
    }
    for ordinal in 18..34 {
        w.negative(negative(ordinal)).unwrap();
    }
    let summary = w.finish().unwrap();
    let bytes = std::fs::read(temp.file()).unwrap();
    assert_eq!(summary.bytes, COMPLETE_BYTES);
    assert_eq!(summary.sha256, validate_complete(&bytes, h).unwrap().sha256);
    // The format contains no outer-postflight/ordinary-admission success flag.
    assert_eq!(bytes.len(), 105440);
    let other = Temp::new();
    let mut w = Writer::create_new(&other.file(), h).unwrap();
    w.committed_bytes = COMPLETE_BYTES;
    assert!(w.append(&[0]).is_err());
    assert!(w.failed);
}
