use super::storage_tests_v1::*;
use super::storage_views_tests_v1::{invocation, memory};
use super::*;

fn overlapping(overlap: StorageCopyOverlapV1) -> Module {
    let mut layouts = rows();
    layouts.push(StorageLayoutV1 {
        size: 12,
        alignment: 4,
        kind: StorageLayoutKindV1::Record(
            vec![StorageFieldV1 {
                offset: 4,
                layout: StorageLayoutIdV1(2),
            }]
            .into_boxed_slice(),
        ),
    });
    layouts.push(StorageLayoutV1 {
        size: 12,
        alignment: 4,
        kind: StorageLayoutKindV1::Union(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(2),
                },
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(3),
                },
            ]
            .into_boxed_slice(),
        ),
    });
    let space = AddressSpace::Private;
    module(
        layouts,
        vec![
            allocate(1, 4, space),
            value(2, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
            value(3, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
            project(4, 2, 1, StorageProjectionV1::Field(0), space),
            project(5, 3, 1, StorageProjectionV1::Field(1), space),
            project(6, 2, 5, StorageProjectionV1::Field(0), space),
            project(7, 0, 4, StorageProjectionV1::ArrayIndex(ValueId(2)), space),
            project(8, 0, 4, StorageProjectionV1::ArrayIndex(ValueId(3)), space),
            constant(9, 11),
            constant(10, 22),
            write(7, 9, space),
            write(8, 10, space),
            Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::CopyObject {
                    source: ValueId(4),
                    destination: ValueId(6),
                    source_access: MemoryAccess::new(space, 4),
                    destination_access: MemoryAccess::new(space, 4),
                    overlap,
                }),
            ),
            project(11, 0, 6, StorageProjectionV1::ArrayIndex(ValueId(3)), space),
            read(12, 11, space),
            output(12),
        ],
    )
}

#[test]
fn storage_actual_overlapping_copy_uses_snapshot_and_read_before_write_events() {
    let module = overlapping(StorageCopyOverlapV1::MayOverlap);
    let mut events = Events::default();
    assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 22);
    let copy: Vec<_> = events
        .0
        .iter()
        .filter(|e| e.site.operation == Some(12))
        .filter_map(|e| match e.kind {
            SimulationEventKindV1::MemoryRead { offset, bytes, .. } => Some((false, offset, bytes)),
            SimulationEventKindV1::MemoryWrite { offset, bytes, .. } => Some((true, offset, bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(copy, [(false, 0, 8), (true, 4, 8)]);
}

#[test]
fn storage_actual_nonoverlap_contract_refuses_before_copy_observations() {
    let mut events = Events::default();
    assert!(
        run(
            &overlapping(StorageCopyOverlapV1::NonOverlapping),
            &mut events
        )
        .is_err()
    );
    assert!(!events.0.iter().any(|e| e.site.operation == Some(12)
        && matches!(
            e.kind,
            SimulationEventKindV1::MemoryRead { .. } | SimulationEventKindV1::MemoryWrite { .. }
        )));
}

#[test]
fn storage_copy_preserves_uninitialized_padding_and_invalidates_tag_guard() {
    let (mut memory, source) = memory(
        vec![1, 2, 3, 4, 0, 0, 0, 0],
        vec![true, true, true, true, false, false, false, false],
        AddressSpace::Private,
    );
    let snapshot = memory
        .storage_snapshot_v1(&source, 8, invocation())
        .unwrap();
    let guard = memory.storage_register_guard_v1(&source, 0, 4).unwrap();
    memory.storage_prepare_copy_v1(&source, &snapshot).unwrap();
    memory.storage_commit_copy_v1(&source, &snapshot, None);
    snapshot.release(&memory.storage_accounting);
    assert_eq!(
        &memory.allocations[&source.pointer.allocation].initialized,
        &[true, true, true, true, false, false, false, false]
    );
    let mut viewed = source.pointer.clone();
    viewed.storage_guard = Some(guard);
    assert!(memory.allocation(&viewed).is_err());
}

#[test]
fn storage_actual_copy_preserves_complete_pointer_relocation_and_live_referent() {
    let mut module = super::storage_values_tests_v1::pointer_program(false);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let space = AddressSpace::Private;
    operations.insert(6, allocate(8, 3, space));
    operations.insert(
        7,
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::CopyObject {
                source: ValueId(4),
                destination: ValueId(8),
                source_access: MemoryAccess::new(space, 4),
                destination_access: MemoryAccess::new(space, 4),
                overlap: StorageCopyOverlapV1::NonOverlapping,
            }),
        ),
    );
    let OperationKind::Storage(StorageOperationV1::ReadValue { address, .. }) =
        &mut operations[8].kind
    else {
        panic!("pointer read")
    };
    *address = ValueId(8);
    let mut events = Events::default();
    assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 77);
    let copy: Vec<_> = events
        .0
        .iter()
        .filter(|event| event.site.operation == Some(7))
        .filter_map(|event| match event.kind {
            SimulationEventKindV1::MemoryRead {
                allocation,
                bytes: 4,
                ..
            } => Some((false, allocation)),
            SimulationEventKindV1::MemoryWrite {
                allocation,
                bytes: 4,
                ..
            } => Some((true, allocation)),
            _ => None,
        })
        .collect();
    assert_eq!(copy.len(), 2);
    assert!(!copy[0].0 && copy[1].0);
    assert_ne!(copy[0].1, copy[1].1);
}

#[test]
fn storage_snapshot_refuses_partial_or_dead_pointer_before_snapshot_reservation() {
    let (mut memory, source) = memory(vec![0; 8], vec![true; 8], AddressSpace::Private);
    let StorageLayoutKindV1::Pointer(representation) =
        super::storage_values_tests_v1::pointer_row().kind
    else {
        unreachable!()
    };
    let allocation = memory
        .allocations
        .get_mut(&source.pointer.allocation)
        .unwrap();
    storage_reserve_v1(
        &mut allocation.storage.relocations,
        1,
        &memory.storage_accounting,
    )
    .unwrap();
    let mut target = source.pointer.clone();
    target.byte_offset = 4;
    target.lower_bound = 4;
    target.element = ScalarType::U32;
    allocation.storage.relocations.push(StorageRelocationV1 {
        start: 0,
        end: 4,
        representation,
        value: StoragePointerPayloadV1::Scalar(target),
    });
    allocation.storage.relocation_bytes = 4;
    let held = memory.storage_accounting.held();
    assert!(matches!(
        memory.storage_snapshot_v1(&source, 2, invocation()),
        Err(SimulationExecutionErrorKindV1::StorageViolation {
            reason: "copy cuts through a symbolic pointer representation"
        })
    ));
    assert_eq!(memory.storage_accounting.held(), held);
    let StoragePointerPayloadV1::Scalar(target) = &mut memory
        .allocations
        .get_mut(&source.pointer.allocation)
        .unwrap()
        .storage
        .relocations[0]
        .value
    else {
        unreachable!()
    };
    target.allocation = u64::MAX;
    assert!(matches!(
        memory.storage_snapshot_v1(&source, 4, invocation()),
        Err(SimulationExecutionErrorKindV1::DanglingPointer {
            allocation: u64::MAX
        })
    ));
    assert_eq!(memory.storage_accounting.held(), held);
}
