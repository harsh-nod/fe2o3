use super::*;

#[test]
fn exact_constant_remainders_are_queryable_without_changing_fact_shape() {
    for (lhs, rhs, expected) in [(8, 3, 2), (0, 2, 0), (u64::MAX, 2, 1)] {
        let fact = derive_binary(
            Some(IndexBinaryKindAttr::Remainder),
            SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(lhs)),
            SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(rhs)),
            &[64],
        );
        assert!(matches!(fact, SparseIndexFactV1::Remainder { .. }));
        assert_eq!(fact.constant_value(), Some(expected));
        assert_eq!(fact.evaluate(&[0]), Some(expected));
        assert_eq!(fact.evaluate(&[63]), Some(expected));
    }
}

#[test]
fn remainder_query_rejects_zero_unknown_and_invocation_dependent_values() {
    let constant = || SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(8));
    let zero = derive_binary(
        Some(IndexBinaryKindAttr::Remainder),
        constant(),
        SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(0)),
        &[64],
    );
    assert_eq!(zero, SparseIndexFactV1::Unknown);
    assert_eq!(zero.constant_value(), None);
    let explicit_zero = SparseIndexFactV1::Remainder {
        dividend: SparseAffineIndexV1::constant(8),
        modulus: 0,
    };
    assert_eq!(explicit_zero.constant_value(), None);
    for (lhs, rhs) in [
        (constant(), SparseIndexFactV1::Unknown),
        (SparseIndexFactV1::Unknown, constant()),
    ] {
        assert_eq!(
            derive_binary(Some(IndexBinaryKindAttr::Remainder), lhs, rhs, &[64]).constant_value(),
            None
        );
    }
    let mut dividend = SparseAffineIndexV1::constant(8);
    dividend.coefficients[0] = 1;
    assert_eq!(
        SparseIndexFactV1::Remainder {
            dividend,
            modulus: 3
        }
        .constant_value(),
        None
    );
}

#[test]
fn concrete_overflow_is_not_hidden_by_a_remainder_constant_query() {
    let overflow = derive_binary(
        Some(IndexBinaryKindAttr::Add),
        SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(u64::MAX)),
        SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(1)),
        &[1],
    );
    assert!(matches!(overflow, SparseIndexFactV1::MachineOverflow(_)));
    assert_eq!(overflow.constant_value(), None);
    let remainder = derive_binary(
        Some(IndexBinaryKindAttr::Remainder),
        overflow,
        SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(1)),
        &[1],
    );
    assert!(matches!(remainder, SparseIndexFactV1::MachineOverflow(_)));
    assert_eq!(remainder.constant_value(), None);
}
