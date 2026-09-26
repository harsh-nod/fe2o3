use super::*;

fn write_access(
    ordinal: usize,
    start: u64,
    end: u64,
    byte_offset: ByteExpression,
    byte_width: u64,
) -> FormalMemoryAccess {
    FormalMemoryAccess {
        location: FunctionOperationLocation::new(BlockId(0), ordinal),
        allocation: FormalAllocationIdentity { parameter_index: 0 },
        kind: FormalMemoryAccessKind::Write,
        address_space: AddressSpace::Global,
        byte_offset,
        byte_width,
        alignment: 1,
        invocations: InvocationRange1d::new(start, end).unwrap(),
        domain: FormalAccessDomainV1::LaunchEnvelope,
    }
}

#[test]
fn singleton_does_not_hide_another_invocation() {
    for offset in [ByteExpression::constant(0), ByteExpression::Unbounded] {
        let singleton = write_access(0, 0, 1, offset, 4);
        let unrestricted = write_access(1, 0, 2, offset, 4);
        assert!(!proves_distinct_invocation_disjointness(
            &singleton,
            &unrestricted
        ));
        assert!(!proves_distinct_invocation_disjointness(
            &unrestricted,
            &singleton
        ));
    }
}

#[test]
fn same_singleton_has_no_distinct_invocation_pair() {
    for index in [0, 1, 17, u64::MAX - 1] {
        let left = write_access(0, index, index + 1, ByteExpression::Unbounded, 4);
        let right = write_access(1, index, index + 1, ByteExpression::constant(0), 8);
        assert!(proves_distinct_invocation_disjointness(&left, &right));
        assert!(proves_distinct_invocation_disjointness(&right, &left));
    }
    let left = write_access(0, 0, 1, ByteExpression::constant(0), 4);
    let right = write_access(1, 1, 2, ByteExpression::constant(0), 4);
    assert!(!proves_distinct_invocation_disjointness(&left, &right));
    assert!(!proves_distinct_invocation_disjointness(&right, &left));
}

#[test]
fn address_disjointness_still_proves_mixed_ranges() {
    let offset = ByteExpression::invocation_affine(0, 4);
    let left = write_access(0, 0, 1, offset, 4);
    let right = write_access(1, 0, 4, offset, 4);
    assert!(proves_distinct_invocation_disjointness(&left, &right));
    assert!(proves_distinct_invocation_disjointness(&right, &left));
    let left = FormalMemoryAccess {
        byte_width: 8,
        ..left
    };
    let right = FormalMemoryAccess {
        byte_width: 8,
        ..right
    };
    assert!(!proves_distinct_invocation_disjointness(&left, &right));
    assert!(!proves_distinct_invocation_disjointness(&right, &left));

    let left = write_access(0, 0, 1, ByteExpression::constant(0), 4);
    let right = write_access(1, 0, 4, ByteExpression::constant(16), 4);
    assert!(proves_distinct_invocation_disjointness(&left, &right));
    assert!(proves_distinct_invocation_disjointness(&right, &left));
}

#[test]
fn conflict_report_retains_singleton_against_unrestricted() {
    let singleton = write_access(0, 0, 1, ByteExpression::constant(0), 4);
    let unrestricted = write_access(1, 0, 2, ByteExpression::constant(0), 4);
    let expected = [
        (singleton.location, unrestricted.location),
        (unrestricted.location, unrestricted.location),
    ];
    let conflicts =
        derive_inter_invocation_conflicts(&[singleton, unrestricted], &mut None).unwrap();
    assert_eq!(
        conflicts
            .iter()
            .map(|conflict| (conflict.left, conflict.right))
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn bounded_affine_proofs_are_sound_and_symmetric() {
    let mut samples = Vec::new();
    for start in 0..3 {
        for end in start + 1..=3 {
            for constant in 0..3 {
                for stride in 0..4 {
                    for width in 1..4 {
                        samples.push((
                            write_access(
                                0,
                                start,
                                end,
                                ByteExpression::invocation_affine(constant, stride),
                                width,
                            ),
                            constant,
                            stride,
                        ));
                    }
                }
            }
        }
    }
    assert_eq!(samples.len(), 216);
    for (left, left_constant, left_stride) in &samples {
        for (right, right_constant, right_stride) in &samples {
            let proved = proves_distinct_invocation_disjointness(left, right);
            assert_eq!(proved, proves_distinct_invocation_disjointness(right, left));
            if !proved {
                continue;
            }
            // Enumerate bytes independently of the production affine/envelope helpers.
            for i in left.invocations.start()..left.invocations.end_exclusive() {
                for j in right.invocations.start()..right.invocations.end_exclusive() {
                    if i == j {
                        continue;
                    }
                    let a = u128::from(*left_constant) + u128::from(*left_stride) * u128::from(i);
                    let b = u128::from(*right_constant) + u128::from(*right_stride) * u128::from(j);
                    assert!(
                        a + u128::from(left.byte_width) <= b
                            || b + u128::from(right.byte_width) <= a,
                        "unsound proof: {left:?} at {i}, {right:?} at {j}"
                    );
                }
            }
        }
    }
}

#[test]
fn equal_launch_ranges_preserve_existing_proofs() {
    for count in [1, 2, 3, 64, 129, u64::MAX] {
        for (left_offset, right_offset, width, expected) in [
            (
                ByteExpression::constant(0),
                ByteExpression::constant(0),
                4,
                count == 1,
            ),
            (
                ByteExpression::Unbounded,
                ByteExpression::constant(0),
                4,
                count == 1,
            ),
            (
                ByteExpression::invocation_affine(0, 4),
                ByteExpression::invocation_affine(0, 4),
                4,
                true,
            ),
            (
                ByteExpression::invocation_affine(0, 4),
                ByteExpression::invocation_affine(0, 4),
                8,
                count == 1,
            ),
            (
                ByteExpression::constant(0),
                ByteExpression::constant(16),
                4,
                true,
            ),
            (
                ByteExpression::invocation_affine(0, 4),
                ByteExpression::invocation_affine(4, 4),
                4,
                count == 1,
            ),
        ] {
            let left = write_access(0, 0, count, left_offset, width);
            let right = write_access(1, 0, count, right_offset, width);
            assert_eq!(
                proves_distinct_invocation_disjointness(&left, &right),
                expected
            );
            assert_eq!(
                proves_distinct_invocation_disjointness(&right, &left),
                expected
            );
        }
    }
}
