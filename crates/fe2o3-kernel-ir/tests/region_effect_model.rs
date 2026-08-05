use fe2o3_kernel_ir::{
    AddressSpace, AllocationId, AllocationIdentity, AtomicEffect, AtomicKind, ByteExpression,
    InvocationRange1d, MemoryOrdering, MemoryRegion, RegionEffect, RegionEffectKind,
    RegionValidationError, ScalarType, SynchronizationEpoch, SynchronizationScope,
};

fn region(offset: ByteExpression, length: ByteExpression) -> MemoryRegion {
    MemoryRegion::new(
        AllocationId::new(7).into(),
        AddressSpace::Global,
        offset,
        length,
    )
}

#[test]
fn affine_expressions_evaluate_with_checked_arithmetic() {
    let expression = ByteExpression::invocation_affine(8, 4);
    assert_eq!(expression.checked_evaluate(3), Ok(20));
    assert_eq!(
        ByteExpression::invocation_affine(u64::MAX, 1).checked_evaluate(1),
        Err(RegionValidationError::ExpressionOverflow {
            expression: ByteExpression::invocation_affine(u64::MAX, 1),
            invocation_index: 1,
        })
    );
    assert_eq!(
        ByteExpression::Unbounded.checked_evaluate(0),
        Err(RegionValidationError::UnboundedExpression)
    );
}

#[test]
fn invocation_ranges_are_non_empty_and_half_open() {
    assert_eq!(
        InvocationRange1d::new(2, 2),
        Err(RegionValidationError::EmptyInvocationRange {
            start: 2,
            end_exclusive: 2,
        })
    );

    let range = InvocationRange1d::new(2, 5).unwrap();
    assert!(range.contains(2));
    assert!(range.contains(4));
    assert!(!range.contains(5));
}

#[test]
fn region_validation_rejects_zero_length_and_end_overflow() {
    let invocation = InvocationRange1d::from_count(1).unwrap();
    assert_eq!(
        region(ByteExpression::constant(0), ByteExpression::constant(0)).validate(invocation),
        Err(RegionValidationError::ZeroByteLength {
            invocation_index: 0,
        })
    );
    assert_eq!(
        region(
            ByteExpression::constant(u64::MAX),
            ByteExpression::constant(1),
        )
        .validate(invocation),
        Err(RegionValidationError::RegionEndOverflow {
            byte_offset: u64::MAX,
            byte_length: 1,
            invocation_index: 0,
        })
    );
}

#[test]
fn effect_validation_checks_width_alignment_and_affine_offsets() {
    let invocation = InvocationRange1d::from_count(4).unwrap();
    let valid = RegionEffect::new(
        RegionEffectKind::Write,
        region(
            ByteExpression::invocation_affine(0, 4),
            ByteExpression::constant(4),
        ),
        4,
        4,
        SynchronizationEpoch::INITIAL,
    );
    assert_eq!(valid.validate(invocation), Ok(()));

    let invalid_alignment = RegionEffect {
        alignment: 3,
        ..valid.clone()
    };
    assert_eq!(
        invalid_alignment.validate(invocation),
        Err(RegionValidationError::InvalidAlignment { alignment: 3 })
    );

    let misaligned = RegionEffect {
        region: region(
            ByteExpression::invocation_affine(2, 4),
            ByteExpression::constant(4),
        ),
        ..valid.clone()
    };
    assert_eq!(
        misaligned.validate(invocation),
        Err(RegionValidationError::MisalignedAccess { alignment: 4 })
    );

    let too_wide = RegionEffect {
        access_width: 8,
        ..valid
    };
    assert_eq!(
        too_wide.validate(invocation),
        Err(RegionValidationError::AccessExceedsRegion {
            access_width: 8,
            byte_length: 4,
        })
    );
}

#[test]
fn unknown_allocation_provenance_is_explicit() {
    let region = MemoryRegion::new(
        AllocationIdentity::Unknown,
        AddressSpace::Generic,
        ByteExpression::Unbounded,
        ByteExpression::Unbounded,
    );
    assert_eq!(region.allocation, AllocationIdentity::Unknown);
}

#[test]
fn atomic_value_width_must_match_the_byte_access() {
    let atomic = RegionEffect::new(
        RegionEffectKind::Atomic(AtomicEffect {
            kind: AtomicKind::Add,
            value_type: ScalarType::U64,
            scope: SynchronizationScope::Device,
            ordering: MemoryOrdering::Relaxed,
        }),
        region(ByteExpression::constant(0), ByteExpression::constant(8)),
        4,
        4,
        SynchronizationEpoch::INITIAL,
    );
    assert_eq!(
        atomic.validate(InvocationRange1d::from_count(1).unwrap()),
        Err(RegionValidationError::AtomicWidthMismatch {
            value_type: ScalarType::U64,
            value_width_bits: 64,
            access_width: 4,
        })
    );
}
