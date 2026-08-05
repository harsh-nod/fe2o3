use fe2o3_kernel_ir::{
    AddressSpace, AllocationId, AllocationIdentity, AtomicEffect, AtomicKind, ByteExpression,
    ConflictIndeterminateReason, ConflictReason, EffectConflict, InvocationPairing,
    InvocationRange1d, MemoryOrdering, MemoryRegion, NoConflictReason, RegionAnalysisError,
    RegionEffect, RegionEffectKind, RegionIndeterminateReason, RegionOverlap,
    RegionValidationError, ScalarType, SynchronizationEpoch, SynchronizationScope,
    analyze_effect_conflict, analyze_region_overlap,
};

fn region(
    allocation: AllocationIdentity,
    offset: ByteExpression,
    length: ByteExpression,
) -> MemoryRegion {
    MemoryRegion::new(allocation, AddressSpace::Global, offset, length)
}

fn effect(kind: RegionEffectKind, region: MemoryRegion, epoch: u32) -> RegionEffect {
    RegionEffect::new(kind, region, 4, 1, SynchronizationEpoch::new(epoch))
}

fn allocation(value: u32) -> AllocationIdentity {
    AllocationId::new(value).into()
}

fn invocations(count: u64) -> InvocationRange1d {
    InvocationRange1d::from_count(count).unwrap()
}

fn atomic(ordering: MemoryOrdering) -> RegionEffectKind {
    atomic_with_scope(ordering, SynchronizationScope::Device)
}

fn atomic_with_scope(ordering: MemoryOrdering, scope: SynchronizationScope) -> RegionEffectKind {
    RegionEffectKind::Atomic(AtomicEffect {
        kind: AtomicKind::Add,
        value_type: ScalarType::U32,
        scope,
        ordering,
    })
}

#[test]
fn proves_vecadd_writes_disjoint_between_invocations() {
    let writes = effect(
        RegionEffectKind::Write,
        region(
            allocation(1),
            ByteExpression::invocation_affine(0, 4),
            ByteExpression::constant(4),
        ),
        0,
    );

    assert_eq!(
        analyze_region_overlap(
            &writes.region,
            invocations(1024),
            &writes.region,
            invocations(1024),
            InvocationPairing::DistinctInvocations,
        ),
        Ok(RegionOverlap::Disjoint)
    );
    assert_eq!(
        analyze_effect_conflict(
            &writes,
            invocations(1024),
            &writes,
            invocations(1024),
            InvocationPairing::DistinctInvocations,
        ),
        Ok(EffectConflict::NoConflict(
            NoConflictReason::DisjointRegions
        ))
    );
}

#[test]
fn detects_overlapping_writes_and_read_write_conflicts() {
    let overlapping = region(
        allocation(1),
        ByteExpression::invocation_affine(0, 2),
        ByteExpression::constant(4),
    );
    let write = effect(RegionEffectKind::Write, overlapping.clone(), 0);
    let read = effect(RegionEffectKind::Read, overlapping, 0);

    assert_eq!(
        analyze_effect_conflict(
            &write,
            invocations(8),
            &write,
            invocations(8),
            InvocationPairing::DistinctInvocations,
        ),
        Ok(EffectConflict::Conflict(
            ConflictReason::OverlappingNonAtomicWrite
        ))
    );
    assert_eq!(
        analyze_effect_conflict(
            &read,
            invocations(8),
            &write,
            invocations(8),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::Conflict(
            ConflictReason::OverlappingNonAtomicWrite
        ))
    );
}

#[test]
fn effect_analysis_rejects_affine_regions_narrower_than_the_access() {
    let undersized = effect(
        RegionEffectKind::Write,
        region(
            allocation(1),
            ByteExpression::invocation_affine(0, 2),
            ByteExpression::invocation_affine(2, 1),
        ),
        0,
    );
    let first = InvocationRange1d::new(0, 1).unwrap();
    let second = InvocationRange1d::new(1, 2).unwrap();

    assert_eq!(
        analyze_region_overlap(
            &undersized.region,
            first,
            &undersized.region,
            second,
            InvocationPairing::DistinctInvocations,
        ),
        Ok(RegionOverlap::Disjoint)
    );
    assert_eq!(
        analyze_effect_conflict(
            &undersized,
            first,
            &undersized,
            second,
            InvocationPairing::DistinctInvocations,
        ),
        Err(RegionAnalysisError::LeftRegion(
            RegionValidationError::AccessExceedsRegion {
                access_width: 4,
                byte_length: 2,
                invocation_index: 0,
            }
        ))
    );

    let valid_later = InvocationRange1d::new(2, 3).unwrap();
    assert_eq!(
        analyze_effect_conflict(
            &undersized,
            valid_later,
            &undersized,
            first,
            InvocationPairing::DistinctInvocations,
        ),
        Err(RegionAnalysisError::RightRegion(
            RegionValidationError::AccessExceedsRegion {
                access_width: 4,
                byte_length: 2,
                invocation_index: 0,
            }
        ))
    );
}

#[test]
fn permits_overlapping_shared_reads() {
    let shared = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    let read = effect(RegionEffectKind::Read, shared, 0);
    assert_eq!(
        analyze_effect_conflict(
            &read,
            invocations(8),
            &read,
            invocations(8),
            InvocationPairing::DistinctInvocations,
        ),
        Ok(EffectConflict::NoConflict(NoConflictReason::SharedReads))
    );
}

#[test]
fn finds_partial_byte_range_overlap_and_separated_ranges() {
    let left = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    let partial = region(
        allocation(1),
        ByteExpression::constant(2),
        ByteExpression::constant(4),
    );
    let separated = region(
        allocation(1),
        ByteExpression::constant(4),
        ByteExpression::constant(4),
    );

    assert_eq!(
        analyze_region_overlap(
            &left,
            invocations(1),
            &partial,
            invocations(1),
            InvocationPairing::AnyInvocations,
        ),
        Ok(RegionOverlap::MayOverlap)
    );
    assert_eq!(
        analyze_region_overlap(
            &left,
            invocations(1),
            &separated,
            invocations(1),
            InvocationPairing::AnyInvocations,
        ),
        Ok(RegionOverlap::Disjoint)
    );
}

#[test]
fn distinct_allocations_are_disjoint() {
    let left = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    let right = region(
        allocation(2),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    assert_eq!(
        analyze_region_overlap(
            &left,
            invocations(4),
            &right,
            invocations(4),
            InvocationPairing::AnyInvocations,
        ),
        Ok(RegionOverlap::Disjoint)
    );
}

#[test]
fn unknown_and_unbounded_regions_never_report_disjoint() {
    let known = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    let unknown = region(
        AllocationIdentity::Unknown,
        ByteExpression::constant(4096),
        ByteExpression::constant(4),
    );
    let unbounded = region(
        allocation(2),
        ByteExpression::Unbounded,
        ByteExpression::constant(4),
    );

    assert_eq!(
        analyze_region_overlap(
            &known,
            invocations(1),
            &unknown,
            invocations(1),
            InvocationPairing::AnyInvocations,
        ),
        Ok(RegionOverlap::Indeterminate(
            RegionIndeterminateReason::UnknownAllocation
        ))
    );
    assert_eq!(
        analyze_region_overlap(
            &known,
            invocations(1),
            &unbounded,
            invocations(1),
            InvocationPairing::AnyInvocations,
        ),
        Ok(RegionOverlap::Indeterminate(
            RegionIndeterminateReason::UnboundedByteExpression
        ))
    );
}

#[test]
fn arithmetic_overflow_is_an_explicit_analysis_error() {
    let overflowing = region(
        allocation(1),
        ByteExpression::invocation_affine(u64::MAX, 1),
        ByteExpression::constant(4),
    );
    let valid = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    assert_eq!(
        analyze_region_overlap(
            &overflowing,
            invocations(2),
            &valid,
            invocations(2),
            InvocationPairing::AnyInvocations,
        ),
        Err(RegionAnalysisError::LeftRegion(
            RegionValidationError::RegionEndOverflow {
                byte_offset: u64::MAX,
                byte_length: 4,
                invocation_index: 0,
            }
        ))
    );
}

#[test]
fn different_epochs_do_not_imply_ordering() {
    let shared = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    let first = effect(RegionEffectKind::Write, shared.clone(), 1);
    let second = effect(RegionEffectKind::Write, shared, 2);
    assert_eq!(
        analyze_effect_conflict(
            &first,
            invocations(2),
            &second,
            invocations(2),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::Indeterminate(
            ConflictIndeterminateReason::EpochOrderingNotEstablished {
                left: SynchronizationEpoch::new(1),
                right: SynchronizationEpoch::new(2),
            }
        ))
    );
}

#[test]
fn compatible_atomics_are_allowed_and_mismatches_fail_conservatively() {
    let shared = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    let relaxed = effect(atomic(MemoryOrdering::Relaxed), shared.clone(), 0);
    let acquire_release = effect(atomic(MemoryOrdering::AcquireRelease), shared.clone(), 0);
    let different_operation = effect(
        RegionEffectKind::Atomic(AtomicEffect {
            kind: AtomicKind::Exchange,
            value_type: ScalarType::U32,
            scope: SynchronizationScope::Device,
            ordering: MemoryOrdering::Relaxed,
        }),
        shared.clone(),
        0,
    );
    let partial_object = effect(
        atomic(MemoryOrdering::Relaxed),
        region(
            allocation(1),
            ByteExpression::constant(2),
            ByteExpression::constant(4),
        ),
        0,
    );
    let non_atomic = effect(RegionEffectKind::Write, shared, 0);

    assert_eq!(
        analyze_effect_conflict(
            &relaxed,
            invocations(8),
            &relaxed,
            invocations(8),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::NoConflict(
            NoConflictReason::CompatibleAtomics
        ))
    );
    assert_eq!(
        analyze_effect_conflict(
            &relaxed,
            invocations(8),
            &partial_object,
            invocations(8),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::Conflict(
            ConflictReason::IncompatibleAtomicOverlap
        ))
    );
    assert_eq!(
        analyze_effect_conflict(
            &relaxed,
            invocations(8),
            &different_operation,
            invocations(8),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::Conflict(
            ConflictReason::IncompatibleAtomicOverlap
        ))
    );
    assert_eq!(
        analyze_effect_conflict(
            &relaxed,
            invocations(8),
            &acquire_release,
            invocations(8),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::Conflict(
            ConflictReason::IncompatibleAtomicOverlap
        ))
    );
    assert_eq!(
        analyze_effect_conflict(
            &relaxed,
            invocations(8),
            &non_atomic,
            invocations(8),
            InvocationPairing::AnyInvocations,
        ),
        Ok(EffectConflict::Conflict(
            ConflictReason::AtomicNonAtomicOverlap
        ))
    );
}

#[test]
fn atomic_compatibility_requires_scope_coverage_for_the_analysis_domain() {
    let shared = region(
        allocation(1),
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );

    for scope in [
        SynchronizationScope::Invocation,
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
    ] {
        let scoped = effect(
            atomic_with_scope(MemoryOrdering::Relaxed, scope),
            shared.clone(),
            0,
        );
        assert_eq!(
            analyze_effect_conflict(
                &scoped,
                invocations(8),
                &scoped,
                invocations(8),
                InvocationPairing::DistinctInvocations,
            ),
            Ok(EffectConflict::Indeterminate(
                ConflictIndeterminateReason::AtomicScopeCoverageNotEstablished { scope }
            ))
        );
    }

    let system = effect(
        atomic_with_scope(
            MemoryOrdering::SequentiallyConsistent,
            SynchronizationScope::System,
        ),
        shared,
        0,
    );
    assert_eq!(
        analyze_effect_conflict(
            &system,
            invocations(8),
            &system,
            invocations(8),
            InvocationPairing::DistinctInvocations,
        ),
        Ok(EffectConflict::NoConflict(
            NoConflictReason::CompatibleAtomics
        ))
    );
}
