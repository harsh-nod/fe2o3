use core::sync::atomic::Ordering;

use fe2o3_kernel_ir::{
    AddressSpace, AtomicKind, MemoryAccess, MemoryOrdering, StandardAtomicMappingError,
    SynchronizationScope, ValueId, map_scoped_core_atomic,
};

fn map(
    kind: AtomicKind,
    ordering: Ordering,
    failure: Option<Ordering>,
) -> Result<fe2o3_kernel_ir::Atomic, StandardAtomicMappingError> {
    map_scoped_core_atomic(
        kind,
        ValueId(0),
        (kind != AtomicKind::Load).then_some(ValueId(1)),
        (kind == AtomicKind::CompareExchange).then_some(ValueId(2)),
        MemoryAccess::new(AddressSpace::Global, 4),
        SynchronizationScope::Device,
        None,
        ordering,
        failure,
    )
}

#[test]
fn maps_every_ordering_at_explicit_device_scope() {
    let cases = [
        (Ordering::Relaxed, MemoryOrdering::Relaxed),
        (Ordering::Acquire, MemoryOrdering::Acquire),
        (Ordering::Release, MemoryOrdering::Release),
        (Ordering::AcqRel, MemoryOrdering::AcquireRelease),
        (Ordering::SeqCst, MemoryOrdering::SequentiallyConsistent),
    ];
    for (source, expected) in cases {
        let atomic = map(AtomicKind::Add, source, None).unwrap();
        assert_eq!(atomic.ordering, expected);
        assert_eq!(atomic.scope, SynchronizationScope::Device);
    }
}

#[test]
fn rejects_illegal_rust_ordering_combinations() {
    for (kind, success, failure) in [
        (AtomicKind::Load, Ordering::Release, None),
        (AtomicKind::Store, Ordering::Acquire, None),
        (AtomicKind::Add, Ordering::Relaxed, Some(Ordering::Relaxed)),
        (AtomicKind::CompareExchange, Ordering::AcqRel, None),
        (
            AtomicKind::CompareExchange,
            Ordering::AcqRel,
            Some(Ordering::Release),
        ),
        (
            AtomicKind::CompareExchange,
            Ordering::Release,
            Some(Ordering::Acquire),
        ),
    ] {
        assert!(matches!(
            map(kind, success, failure),
            Err(StandardAtomicMappingError::InvalidOrdering { .. })
        ));
    }
}

#[test]
fn admits_only_reviewed_address_space_and_scope_pairs() {
    for (address_space, scope) in [
        (AddressSpace::Global, SynchronizationScope::Workgroup),
        (AddressSpace::Global, SynchronizationScope::Device),
        (AddressSpace::Workgroup, SynchronizationScope::Workgroup),
    ] {
        map_scoped_core_atomic(
            AtomicKind::Add,
            ValueId(0),
            Some(ValueId(1)),
            None,
            MemoryAccess::new(address_space, 4),
            scope,
            None,
            Ordering::Relaxed,
            None,
        )
        .unwrap();
    }

    assert_eq!(
        map_scoped_core_atomic(
            AtomicKind::Load,
            ValueId(0),
            None,
            None,
            MemoryAccess::new(AddressSpace::Global, 4),
            SynchronizationScope::System,
            None,
            Ordering::Acquire,
            None,
        ),
        Err(StandardAtomicMappingError::MissingCoherentAllocation {
            pointer: ValueId(0),
        })
    );

    for (address_space, scope) in [
        (AddressSpace::Global, SynchronizationScope::Subgroup),
        (AddressSpace::Workgroup, SynchronizationScope::Device),
        (AddressSpace::Generic, SynchronizationScope::System),
        (AddressSpace::Private, SynchronizationScope::Invocation),
        (AddressSpace::Constant, SynchronizationScope::System),
    ] {
        assert_eq!(
            map_scoped_core_atomic(
                AtomicKind::Add,
                ValueId(0),
                Some(ValueId(1)),
                None,
                MemoryAccess::new(address_space, 4),
                scope,
                None,
                Ordering::Relaxed,
                None,
            ),
            Err(StandardAtomicMappingError::UnsupportedScope {
                address_space,
                scope,
            })
        );
    }
}
