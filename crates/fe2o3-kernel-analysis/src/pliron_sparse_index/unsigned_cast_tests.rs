use super::*;

#[test]
fn unsigned_casts_retain_in_range_affine_indices_and_exact_wrapping_values() {
    let source = SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(0));
    for bits in [8, 16, 32, 64] {
        let mask = u64::MAX >> (64 - bits);
        assert_eq!(
            derive_unsigned_cast(source.clone(), Some(mask), &[64]),
            source
        );
        for value in [0, mask, mask.saturating_add(1), u64::MAX] {
            let constant = SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(value));
            let cast = derive_unsigned_cast(constant, Some(mask), &[]);
            assert_eq!(cast.evaluate(&[]), Some(value & mask));
            assert!(matches!(cast, SparseIndexFactV1::Affine(_)));
        }
    }
    let cast = derive_unsigned_cast(source, Some(255), &[300]);
    for lane in [0, 255, 256, 299] {
        assert_eq!(cast.evaluate(&[lane]), Some(lane & 255));
    }
    let dynamic = derive_unsigned_cast(
        SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(0)),
        Some(255),
        &[0],
    );
    assert!(matches!(dynamic, SparseIndexFactV1::Remainder { .. }));
    assert_eq!(dynamic.evaluate(&[257]), Some(1));
}

#[test]
fn casts_do_not_turn_overflow_unknown_or_malformed_width_into_a_value() {
    let overflow = SparseIndexFactV1::MachineOverflow(SparseMachineOverflowV1 {
        operation: IndexBinaryKindAttr::Add,
        invocation: vec![1],
        lhs: u64::MAX,
        rhs: 1,
    });
    for mask in [255, u64::MAX] {
        assert_eq!(
            derive_unsigned_cast(overflow.clone(), Some(mask), &[64]),
            overflow
        );
        assert_eq!(
            derive_unsigned_cast(SparseIndexFactV1::Unknown, Some(mask), &[64]),
            SparseIndexFactV1::Unknown
        );
    }
    assert_eq!(
        derive_unsigned_cast(
            SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(1)),
            None,
            &[]
        ),
        SparseIndexFactV1::Unknown
    );
}
