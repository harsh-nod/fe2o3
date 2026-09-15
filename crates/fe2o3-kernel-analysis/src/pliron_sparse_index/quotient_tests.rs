use super::*;

fn constant(value: u64) -> SparseIndexFactV1 {
    SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(value))
}

fn divide(lhs: SparseIndexFactV1, rhs: SparseIndexFactV1, extents: &[u64]) -> SparseIndexFactV1 {
    derive_binary(Some(IndexBinaryKindAttr::Divide), lhs, rhs, extents)
}

#[test]
fn quotient_retains_exact_floor_semantics_without_becoming_affine() {
    let index = SparseAffineIndexV1::invocation(0);
    let quotient = divide(
        SparseIndexFactV1::Affine(index.clone()),
        constant(64),
        &[1024],
    );
    assert!(matches!(quotient, SparseIndexFactV1::Quotient { .. }));
    assert!(quotient.affine().is_none());
    for point in [0, 63, 64, 65, 1023] {
        assert_eq!(quotient.evaluate(&[point]), Some(point / 64));
    }
    assert_eq!(quotient.maximum(&[1024]), Some(15));
    let affine = index
        .checked_scale(3)
        .unwrap()
        .checked_add(&SparseAffineIndexV1::constant(5))
        .unwrap();
    let quotient = divide(SparseIndexFactV1::Affine(affine), constant(7), &[31]);
    for point in 0..31 {
        assert_eq!(quotient.evaluate(&[point]), Some((3 * point + 5) / 7));
    }
    assert_eq!(quotient.maximum(&[31]), Some(95 / 7));
    assert_eq!(
        derive_binary(
            Some(IndexBinaryKindAttr::Multiply),
            quotient,
            constant(16),
            &[31]
        ),
        SparseIndexFactV1::Unknown
    );
}

#[test]
fn quotient_rejects_unproved_divisors_and_referenced_domains() {
    let index = SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(0));
    for divisor in [constant(0), SparseIndexFactV1::Unknown, index.clone()] {
        assert_eq!(
            divide(index.clone(), divisor, &[1024]),
            SparseIndexFactV1::Unknown
        );
    }
    for extents in [&[][..], &[0][..]] {
        assert_eq!(
            divide(index.clone(), constant(64), extents),
            SparseIndexFactV1::Unknown
        );
    }
    assert_eq!(
        divide(SparseIndexFactV1::Unknown, constant(64), &[1024]),
        SparseIndexFactV1::Unknown
    );
    let second_axis = SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(1));
    assert_eq!(
        divide(second_axis.clone(), constant(64), &[1024]),
        SparseIndexFactV1::Unknown
    );
    assert!(matches!(
        divide(second_axis, constant(64), &[0, 1024]),
        SparseIndexFactV1::Quotient { .. }
    ));
    let unused_axis = divide(index, constant(64), &[1024, 0]);
    assert_eq!(unused_axis.evaluate(&[1023, u64::MAX]), Some(15));
    assert_eq!(unused_axis.maximum(&[1024, 0]), None);
    assert_eq!(unused_axis.maximum(&[1024, 1]), Some(15));
    let zero = SparseIndexFactV1::Quotient {
        dividend: SparseAffineIndexV1::invocation(0),
        divisor: 0,
    };
    assert_eq!(zero.evaluate(&[1]), None);
    assert_eq!(zero.maximum(&[1024]), None);
}

#[test]
fn quotient_never_erases_upstream_machine_overflow() {
    let sum = derive_binary(
        Some(IndexBinaryKindAttr::Add),
        constant(u64::MAX),
        constant(1),
        &[4],
    );
    let product = derive_binary(
        Some(IndexBinaryKindAttr::Multiply),
        constant(u64::MAX),
        SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(0)),
        &[4],
    );
    for overflow in [sum, product] {
        assert!(overflow.machine_overflow().is_some());
        assert_eq!(divide(overflow.clone(), constant(64), &[4]), overflow);
        assert_eq!(divide(overflow.clone(), constant(u64::MAX), &[4]), overflow);
        assert_eq!(divide(constant(0), overflow.clone(), &[4]), overflow);
    }
}

#[test]
fn quotient_constants_and_huge_domains_need_no_enumeration() {
    for (lhs, rhs) in [(0, 64), (17, 4), (u64::MAX, u64::MAX)] {
        assert_eq!(
            divide(constant(lhs), constant(rhs), &[0]).constant_value(),
            Some(lhs / rhs)
        );
    }
    let quotient = divide(
        SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(0)),
        constant(64),
        &[u64::MAX],
    );
    assert_eq!(quotient.maximum(&[u64::MAX]), Some((u64::MAX - 1) / 64));
    assert_eq!(
        quotient.evaluate(&[u64::MAX - 1]),
        Some((u64::MAX - 1) / 64)
    );
}
