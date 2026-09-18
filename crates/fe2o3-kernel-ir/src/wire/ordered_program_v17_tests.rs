//! Synthetic exact-payload controls, not source, native or owner authority.
use super::super::{CanonicalKernelIrWorkBudgetV1, DecodeBudgetV12, WriterModeV1};
use super::*;

fn words(active: &[u16]) -> [u16; 16] {
    let mut result = [0; 16];
    result[..active.len()].copy_from_slice(active);
    result
}
fn program() -> Gfx942U32ProgramV1 {
    Gfx942U32ProgramV1::from_descriptors(3, words(&[0x85, 0x133, 0x19d])).unwrap()
}

fn fixture() -> Gfx942OrderedProgramV1 {
    Gfx942OrderedProgramV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [
            ValueId(0x0102_0304),
            ValueId(0x1122_3344),
            ValueId(u32::MAX),
        ],
        program(),
    )
    .unwrap()
}

fn encoded() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V17, None);
    encode(&mut writer, &fixture()).unwrap();
    assert_eq!(writer.length(), 179);
    writer.bytes
}

fn decode_exact(bytes: &[u8]) -> Result<Gfx942OrderedProgramV1, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V17;
    let region = decode(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(region)
}

#[test]
fn payload_has_literal_179_byte_layout_and_exact_identity_and_ssa_roundtrip() {
    let bytes = encoded();
    let mut expected = vec![0];
    for marker in [1, 2, 3, 4] {
        expected.extend_from_slice(&[marker; 32]);
    }
    expected.extend_from_slice(&[32, 33, 34, 35, 36]);
    expected.extend_from_slice(&[4, 3, 2, 1, 0x44, 0x33, 0x22, 0x11, 0xff, 0xff, 0xff, 0xff]);
    expected.extend_from_slice(&[3, 0x85, 0, 0x33, 1, 0x9d, 1]);
    expected.extend_from_slice(&[0; 26]);
    assert_eq!(expected.len(), 179);
    assert_eq!(bytes, expected);
    assert_eq!(decode_exact(&bytes), Ok(fixture()));
    for axis in 0..4 {
        let mut changed = bytes.clone();
        changed[1 + axis * 32 + 31] ^= 0x40;
        let decoded = decode_exact(&changed).unwrap();
        assert_ne!(decoded.source(), fixture().source());
        let mut writer = Writer::new(KERNEL_IR_VERSION_V17, None);
        encode(&mut writer, &decoded).unwrap();
        assert_eq!(writer.bytes, changed);
    }
}

#[test]
fn closed_revision_register_range_aliases_and_each_empty_source_axis_reject() {
    let bytes = encoded();
    for revision in 1..=u8::MAX {
        let mut bad = bytes.clone();
        bad[0] = revision;
        assert!(
            matches!(decode_exact(&bad), Err(KernelIrDecodeError::UnknownTag { tag, .. }) if tag == revision)
        );
    }
    for position in 129..134 {
        for invalid in 64..=255 {
            let mut bad = bytes.clone();
            bad[position] = invalid;
            assert_eq!(decode_exact(&bad), Err(KernelIrDecodeError::NonCanonical));
        }
        for other in position + 1..134 {
            let mut bad = bytes.clone();
            bad[other] = bad[position];
            assert_eq!(decode_exact(&bad), Err(KernelIrDecodeError::NonCanonical));
        }
    }
    for axis in 0..4 {
        let mut bad = bytes.clone();
        bad[1 + axis * 32..1 + (axis + 1) * 32].fill(0);
        assert_eq!(decode_exact(&bad), Err(KernelIrDecodeError::NonCanonical));
    }
}

#[test]
fn every_truncated_prefix_and_trailing_payload_is_rejected_by_exact_boundary() {
    let bytes = encoded();
    for end in 0..bytes.len() {
        assert_eq!(
            decode_exact(&bytes[..end]),
            Err(KernelIrDecodeError::Truncated)
        );
    }
    let mut extra = bytes;
    extra.push(0);
    assert_eq!(
        decode_exact(&extra),
        Err(KernelIrDecodeError::TrailingBytes)
    );
}

#[test]
fn leaf_cannot_encode_or_decode_this_payload_under_another_version() {
    for version in [0, 1, 11, 12, 13, 14, 15, 16, 18, u16::MAX] {
        let mut writer = Writer::new(version, None);
        assert!(
            matches!(encode(&mut writer, &fixture()), Err(KernelIrEncodeError::UnsupportedInVersion { version: actual, .. }) if actual == version)
        );
        assert!(writer.bytes.is_empty());
        let bytes = encoded();
        let mut reader = Reader::new(&bytes, None);
        reader.version = version;
        assert_eq!(
            decode(&mut reader),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
        assert_eq!(reader.offset, 0);
    }
}

#[test]
fn boundary_register_bindings_and_repeated_ssa_inputs_roundtrip_without_aliasing_registers() {
    for (scratch, output, inputs) in [(0, 63, [1, 62, 2]), (63, 0, [61, 17, 1])] {
        let region = Gfx942OrderedProgramV1::new(
            fixture().source(),
            Gfx942OrderedProgramRegistersV1::new(scratch, output, inputs).unwrap(),
            [ValueId(u32::MAX); 3],
            program(),
        )
        .unwrap();
        let mut writer = Writer::new(KERNEL_IR_VERSION_V17, None);
        encode(&mut writer, &region).unwrap();
        assert_eq!(decode_exact(&writer.bytes), Ok(region));
    }
}

#[test]
fn counting_materialization_comparison_and_decode_charge_literal_bounded_work() {
    const ENCODE_WORK: usize = 2048 + 179;
    const COUNT_WORK: usize = 2048 + 30;
    for (limit, succeeds) in [(COUNT_WORK, true), (COUNT_WORK - 1, false)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut counter = Writer::counter(KERNEL_IR_VERSION_V17, &mut budget);
        assert_eq!(encode(&mut counter, &fixture()).is_ok(), succeeds);
        assert!(counter.bytes.is_empty());
        assert_eq!(counter.peak_auxiliary_bytes, 0);
        if succeeds {
            assert_eq!(counter.length(), 179);
        }
        assert_eq!(budget.work(), limit);
    }
    for (limit, succeeds) in [(ENCODE_WORK, true), (ENCODE_WORK - 1, false), (2047, false)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut writer = Writer::new(KERNEL_IR_VERSION_V17, Some(&mut budget));
        assert_eq!(encode(&mut writer, &fixture()).is_ok(), succeeds);
        if succeeds {
            assert_eq!(writer.bytes, encoded());
        }
        if limit == 2047 {
            assert!(writer.bytes.is_empty());
        }
        if succeeds {
            assert_eq!(budget.work(), ENCODE_WORK);
        }
    }
    let bytes = encoded();
    for (limit, succeeds) in [(ENCODE_WORK, true), (ENCODE_WORK - 1, false)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Work(&mut budget)));
        reader.version = KERNEL_IR_VERSION_V17;
        assert_eq!(decode(&mut reader).is_ok(), succeeds);
        if succeeds {
            assert!(reader.is_finished());
            assert_eq!(budget.work(), ENCODE_WORK);
        }
    }
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(ENCODE_WORK + 30);
    let mut comparing = Writer::comparing(KERNEL_IR_VERSION_V17, &bytes, Some(&mut budget));
    encode(&mut comparing, &fixture()).unwrap();
    assert!(matches!(
        comparing.mode,
        WriterModeV1::Compare { matches: true, .. }
    ));
    assert_eq!(comparing.length(), 179);
    assert!(comparing.bytes.is_empty());
}

#[test]
fn cumulative_work_is_not_reset_between_fixed_payloads() {
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(2227 + 2226);
    let mut writer = Writer::new(KERNEL_IR_VERSION_V17, Some(&mut budget));
    encode(&mut writer, &fixture()).unwrap();
    assert!(matches!(
        encode(&mut writer, &fixture()),
        Err(KernelIrEncodeError::WorkLimit(_))
    ));
    assert!(writer.length() >= 179);
    let bytes = [encoded(), encoded()].concat();
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(2227 + 2226);
    let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Work(&mut budget)));
    reader.version = KERNEL_IR_VERSION_V17;
    assert_eq!(decode(&mut reader), Ok(fixture()));
    assert!(matches!(
        decode(&mut reader),
        Err(KernelIrDecodeError::WorkLimit(_))
    ));
}

#[test]
fn resource_metered_payload_decode_never_requests_heap_storage() {
    struct FixedPayloadBudget(CanonicalKernelIrWorkBudgetV1);
    impl super::super::DecodeResourceBudgetV12 for FixedPayloadBudget {
        fn work_budget(&mut self) -> &mut CanonicalKernelIrWorkBudgetV1 {
            &mut self.0
        }
        fn work_limit(&self) -> usize {
            self.0.limit()
        }
        fn reserve(
            &mut self,
            _: usize,
        ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1> {
            panic!("fixed payload must not allocate; parent owns operation storage")
        }
        fn release(
            &mut self,
            _: usize,
        ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1> {
            panic!("fixed payload must not release parent-owned storage")
        }
    }
    let bytes = encoded();
    let mut budget = FixedPayloadBudget(CanonicalKernelIrWorkBudgetV1::new(2227));
    let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
    reader.version = KERNEL_IR_VERSION_V17;
    assert_eq!(decode(&mut reader), Ok(fixture()));
    assert!(reader.is_finished());
    assert_eq!(budget.0.work(), 2227);
}

// Independent arithmetic role/initialization checker, never a model decoder.
fn reference_valid(count: u8, descriptors: &[u16; 16]) -> bool {
    if !(1..=16).contains(&count) {
        return false;
    }
    let mut defined = [true, true, true, false, false];
    for (position, &word) in descriptors.iter().enumerate() {
        if position >= usize::from(count) {
            if word != 0 {
                return false;
            }
            continue;
        }
        if word / 1024 != 0 {
            return false;
        }
        let opcode = word % 8;
        let destination = 3 + usize::from((word / 8) % 2);
        let left = usize::from((word / 16) % 8);
        let right = usize::from((word / 128) % 8);
        if opcode > 5 || left > 4 || (opcode == 0 && right != 0) || (opcode != 0 && right > 4) {
            return false;
        }
        if !defined[left] || (opcode != 0 && !defined[right]) {
            return false;
        }
        defined[destination] = true;
    }
    defined[4]
}

fn mutate_descriptors(bytes: &mut [u8], count: u8, descriptors: [u16; 16]) {
    assert_eq!(bytes.len(), 179);
    bytes[146] = count;
    for (position, word) in descriptors.into_iter().enumerate() {
        bytes[147 + position * 2..149 + position * 2].copy_from_slice(&word.to_le_bytes());
    }
}

#[test]
fn all_counts_preserve_active_zero_moves_instead_of_requiring_original_count() {
    let bytes = encoded();
    for count in 0..=u8::MAX {
        let mut changed = bytes.clone();
        changed[146] = count;
        let result = decode_exact(&changed);
        assert_eq!(result.is_ok(), (3..=16).contains(&count), "count={count}");
        if let Ok(value) = result {
            assert_eq!(value.program().count(), count);
            assert_eq!(value.program().descriptors(), program().descriptors());
            let mut writer = Writer::new(KERNEL_IR_VERSION_V17, None);
            encode(&mut writer, &value).unwrap();
            assert_eq!(writer.bytes, changed);
        } else {
            assert_eq!(result, Err(KernelIrDecodeError::NonCanonical));
        }
    }
}

#[test]
fn all_descriptor_slots_and_bits_are_checked_including_inactive_padding() {
    let bytes = encoded();
    let baseline = *program().descriptors();
    for position in 0..16 {
        for bit in 0..16 {
            let mut descriptors = baseline;
            descriptors[position] ^= 1 << bit;
            let mut changed = bytes.clone();
            mutate_descriptors(&mut changed, 3, descriptors);
            let decoded = decode_exact(&changed);
            assert_eq!(
                decoded.is_ok(),
                reference_valid(3, &descriptors),
                "position={position} bit={bit}"
            );
            if let Ok(value) = decoded {
                assert_eq!(value.program().descriptors(), &descriptors);
                let mut writer = Writer::new(KERNEL_IR_VERSION_V17, None);
                encode(&mut writer, &value).unwrap();
                assert_eq!(writer.bytes, changed);
            } else {
                assert_eq!(decoded, Err(KernelIrDecodeError::NonCanonical));
            }
        }
    }
    for (count, descriptors) in [
        (1, words(&[0x0000])),                 // Scratch only: no output at exit.
        (1, words(&[0x0048])),                 // Output read before its first definition.
        (1, words(&[0x0039])),                 // Scratch read before its first definition.
        (1, words(&[0x0088])),                 // Move's unused second source is nonzero.
        (1, words(&[0x0058])),                 // First source role outside 0..=4.
        (1, words(&[0x0289])),                 // Second source role outside 0..=4.
        (1, words(&[0x000e])),                 // Reserved opcode6.
        (1, words(&[0x000f])),                 // Reserved opcode7.
        (1, words(&[0x0408])),                 // Reserved descriptor bit.
        (2, words(&[0x0030, 0x0008])),         // Self read is checked before the write.
        (3, words(&[0x0008, 0x0030, 0x0048])), // A later output cannot define scratch.
    ] {
        let mut changed = bytes.clone();
        mutate_descriptors(&mut changed, count, descriptors);
        assert!(!reference_valid(count, &descriptors));
        assert_eq!(
            decode_exact(&changed),
            Err(KernelIrDecodeError::NonCanonical)
        );
    }
}

#[test]
fn all_program_lengths_and_padding_bits_preserve_the_exact_wire_boundary() {
    let bytes = encoded();
    for count in 1..=16_u8 {
        let mut descriptors = [0; 16];
        descriptors[..usize::from(count)].fill(8);
        let mut exact = bytes.clone();
        mutate_descriptors(&mut exact, count, descriptors);
        let decoded = decode_exact(&exact).unwrap();
        assert_eq!(decoded.program().count(), count);
        assert_eq!(decoded.program().descriptors(), &descriptors);
        assert_eq!(decoded.program().instructions().len(), usize::from(count));
        for inactive in usize::from(count)..16 {
            for bit in 0..16 {
                let mut bad = exact.clone();
                let offset = 147 + inactive * 2 + bit / 8;
                bad[offset] |= 1 << (bit % 8);
                assert_eq!(decode_exact(&bad), Err(KernelIrDecodeError::NonCanonical));
            }
        }
    }
    // A valid old 146-byte pair payload is a truncated V17 payload, never a
    // program inferred from the old owner, a default count or missing padding.
    assert_eq!(
        decode_exact(&bytes[..146]),
        Err(KernelIrDecodeError::Truncated)
    );
}

#[test]
fn unused_and_repeated_steps_remain_distinct_even_when_logical_values_match() {
    let bytes = encoded();
    let descriptions = [(1, words(&[8])), (2, words(&[0x10, 8])), (16, [8; 16])];
    let mut prior = Vec::new();
    for (count, descriptors) in descriptions {
        let mut exact = bytes.clone();
        mutate_descriptors(&mut exact, count, descriptors);
        let decoded = decode_exact(&exact).unwrap();
        assert_eq!(decoded.program().evaluate([19, 23, 42]), 19);
        assert_eq!(decoded.program().descriptors(), &descriptors);
        for previous in &prior {
            assert_ne!(previous, &exact);
        }
        prior.push(exact);
    }
}

#[test]
fn comparison_detects_every_changed_byte_without_allocating_output() {
    let bytes = encoded();
    for position in 0..179 {
        let mut wrong = bytes.clone();
        wrong[position] ^= 1;
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(2257);
        let mut writer = Writer::comparing(KERNEL_IR_VERSION_V17, &wrong, Some(&mut budget));
        encode(&mut writer, &fixture()).unwrap();
        assert!(matches!(
            writer.mode,
            WriterModeV1::Compare { matches: false, .. }
        ));
        assert!(writer.bytes.is_empty());
        assert_eq!(writer.length(), 179);
        assert_eq!(budget.work(), 2257);
    }
}
