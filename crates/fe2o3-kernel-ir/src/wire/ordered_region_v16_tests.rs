use super::super::{CanonicalKernelIrWorkBudgetV1, DecodeBudgetV12, WriterModeV1};
use super::*;

fn fixture() -> Gfx942OrderedRegionV1 {
    Gfx942OrderedRegionV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [
            ValueId(0x0102_0304),
            ValueId(0x1122_3344),
            ValueId(u32::MAX),
        ],
    )
    .unwrap()
}

fn encoded() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V16, None);
    encode(&mut writer, &fixture()).unwrap();
    assert_eq!(writer.length(), 146);
    writer.bytes
}

fn decode_exact(bytes: &[u8]) -> Result<Gfx942OrderedRegionV1, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V16;
    let region = decode(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(region)
}

#[test]
fn payload_has_literal_146_byte_layout_and_exact_identity_and_ssa_roundtrip() {
    let bytes = encoded();
    let mut expected = vec![0];
    for marker in [1, 2, 3, 4] {
        expected.extend_from_slice(&[marker; 32]);
    }
    expected.extend_from_slice(&[32, 33, 34, 35, 36]);
    expected.extend_from_slice(&[4, 3, 2, 1, 0x44, 0x33, 0x22, 0x11, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(bytes, expected);
    assert_eq!(decode_exact(&bytes), Ok(fixture()));
    for axis in 0..4 {
        let mut changed = bytes.clone();
        changed[1 + axis * 32 + 31] ^= 0x40;
        let decoded = decode_exact(&changed).unwrap();
        assert_ne!(decoded.source(), fixture().source());
        let mut writer = Writer::new(KERNEL_IR_VERSION_V16, None);
        encode(&mut writer, &decoded).unwrap();
        assert_eq!(writer.bytes, changed);
    }
}

#[test]
fn closed_profile_register_range_aliases_and_each_empty_source_axis_reject() {
    let bytes = encoded();
    for profile in 1..=u8::MAX {
        let mut bad = bytes.clone();
        bad[0] = profile;
        assert!(
            matches!(decode_exact(&bad), Err(KernelIrDecodeError::UnknownTag { tag, .. }) if tag == profile)
        );
    }
    for position in 129..134 {
        for invalid in [64, 127, 255] {
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
    for version in [0, 1, 11, 12, 13, 14, 15, 17, u16::MAX] {
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
        let region = Gfx942OrderedRegionV1::new(
            fixture().source(),
            Gfx942OrderedRegionRegistersV1::new(scratch, output, inputs).unwrap(),
            [ValueId(u32::MAX); 3],
        )
        .unwrap();
        let mut writer = Writer::new(KERNEL_IR_VERSION_V16, None);
        encode(&mut writer, &region).unwrap();
        assert_eq!(decode_exact(&writer.bytes), Ok(region));
    }
}

#[test]
fn counting_materialization_comparison_and_decode_charge_literal_bounded_work() {
    const ENCODE_WORK: usize = 160 + 146;
    const COUNT_WORK: usize = 160 + 13;
    for (limit, succeeds) in [(COUNT_WORK, true), (COUNT_WORK - 1, false)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut counter = Writer::counter(KERNEL_IR_VERSION_V16, &mut budget);
        assert_eq!(encode(&mut counter, &fixture()).is_ok(), succeeds);
        assert!(counter.bytes.is_empty());
        assert_eq!(counter.peak_auxiliary_bytes, 0);
        if succeeds {
            assert_eq!(counter.length(), 146);
        }
        assert_eq!(budget.work(), limit);
    }
    for (limit, succeeds) in [(ENCODE_WORK, true), (ENCODE_WORK - 1, false), (159, false)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut writer = Writer::new(KERNEL_IR_VERSION_V16, Some(&mut budget));
        assert_eq!(encode(&mut writer, &fixture()).is_ok(), succeeds);
        if succeeds {
            assert_eq!(writer.bytes, encoded());
        }
        if limit == 159 {
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
        reader.version = KERNEL_IR_VERSION_V16;
        assert_eq!(decode(&mut reader).is_ok(), succeeds);
        if succeeds {
            assert!(reader.is_finished());
            assert_eq!(budget.work(), ENCODE_WORK);
        }
    }
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(ENCODE_WORK + 13);
    let mut comparing = Writer::comparing(KERNEL_IR_VERSION_V16, &bytes, Some(&mut budget));
    encode(&mut comparing, &fixture()).unwrap();
    assert!(matches!(
        comparing.mode,
        WriterModeV1::Compare { matches: true, .. }
    ));
    assert_eq!(comparing.length(), 146);
    assert!(comparing.bytes.is_empty());
}

#[test]
fn cumulative_work_is_not_reset_between_fixed_payloads() {
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(306 + 305);
    let mut writer = Writer::new(KERNEL_IR_VERSION_V16, Some(&mut budget));
    encode(&mut writer, &fixture()).unwrap();
    assert!(matches!(
        encode(&mut writer, &fixture()),
        Err(KernelIrEncodeError::WorkLimit(_))
    ));
    assert!(writer.length() >= 146);
    let bytes = [encoded(), encoded()].concat();
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(306 + 305);
    let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Work(&mut budget)));
    reader.version = KERNEL_IR_VERSION_V16;
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
    let mut budget = FixedPayloadBudget(CanonicalKernelIrWorkBudgetV1::new(306));
    let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
    reader.version = KERNEL_IR_VERSION_V16;
    assert_eq!(decode(&mut reader), Ok(fixture()));
    assert!(reader.is_finished());
    assert_eq!(budget.0.work(), 306);
}
