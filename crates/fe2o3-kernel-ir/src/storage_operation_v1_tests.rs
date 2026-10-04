use super::*;
use crate::AddressSpace;

#[test]
fn discriminant_reads_visit_the_actual_pointer_and_remain_memory_effects() {
    for space in [
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Global,
        AddressSpace::Generic,
        AddressSpace::Constant,
    ] {
        let access = MemoryAccess::new(space, 1);
        let operation = StorageOperationV1::ReadDiscriminant {
            address: ValueId(17),
            access,
        };
        assert_eq!(operation.operand_count(), 1);
        let mut operands = Vec::new();
        operation
            .try_visit_operands(|value| {
                operands.push(value);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(operands, [ValueId(17)]);
        assert_eq!(operation.try_visit_operands(|_| Err(19)), Err(19));
        let mut accesses = Vec::new();
        operation
            .try_visit_memory_accesses(|value, actual, kind| {
                accesses.push((value, actual, kind));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            accesses,
            [(ValueId(17), access, StorageMemoryAccessKindV1::Read)]
        );
        assert_eq!(
            operation.try_visit_memory_accesses(|_, _, _| Err(23)),
            Err(23)
        );
        let mut effects = Vec::new();
        operation
            .try_visit_memory_effects(|effect| {
                effects.push(effect);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(effects, [MemoryEffect::Read(space)]);
        assert_eq!(operation.try_visit_memory_effects(|_| Err(29)), Err(29));
    }
}

#[test]
fn storage_construction_projection_visits_only_its_address() {
    let operation = StorageOperationV1::Project {
        base: ValueId(7),
        step: StorageProjectionV1::VariantForWrite { index: 3 },
    };
    assert_eq!(operation.operand_count(), 1);
    let mut seen = Vec::new();
    operation
        .try_visit_operands(|value| {
            seen.push(value);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(seen, [ValueId(7)]);
    assert_eq!(operation.try_visit_operands(|_| Err(19)), Err(19));
    operation
        .try_visit_memory_accesses(|_, _, _| -> Result<(), ()> {
            panic!("construction projection must not access the tag")
        })
        .unwrap();
    operation
        .try_visit_memory_effects(|_| -> Result<(), ()> {
            panic!("construction projection must not manufacture a memory effect")
        })
        .unwrap();
}

#[test]
fn storage_set_discriminant_has_one_operand_and_conservative_write_effect() {
    let access = MemoryAccess::new(AddressSpace::Private, 1);
    let operation = StorageOperationV1::SetDiscriminant {
        address: ValueId(9),
        variant: 4,
        access,
    };
    assert_eq!(operation.operand_count(), 1);
    let mut operands = Vec::new();
    operation
        .try_visit_operands(|value| {
            operands.push(value);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(operands, [ValueId(9)]);
    assert_eq!(operation.try_visit_operands(|_| Err(23)), Err(23));
    let mut accesses = Vec::new();
    operation
        .try_visit_memory_accesses(|address, access, kind| {
            accesses.push((address, access, kind));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        accesses,
        [(ValueId(9), access, StorageMemoryAccessKindV1::Write)]
    );
    assert_eq!(
        operation.try_visit_memory_accesses(|_, _, _| Err(29)),
        Err(29)
    );
    let mut effects = Vec::new();
    operation
        .try_visit_memory_effects(|effect| {
            effects.push(effect);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(effects, [MemoryEffect::Write(AddressSpace::Private)]);
    assert_eq!(operation.try_visit_memory_effects(|_| Err(31)), Err(31));
}

fn access(space: AddressSpace, alignment: u32, volatile: bool) -> MemoryAccess {
    MemoryAccess {
        address_space: space,
        alignment,
        volatile,
    }
}

fn cases() -> Vec<(StorageOperationV1, Vec<ValueId>)> {
    let read = MemoryAccess::new(AddressSpace::Private, 4);
    vec![
        (
            StorageOperationV1::Project {
                base: ValueId(3),
                step: StorageProjectionV1::Field(7),
            },
            vec![ValueId(3)],
        ),
        (
            StorageOperationV1::Project {
                base: ValueId(3),
                step: StorageProjectionV1::ArrayIndex(ValueId(5)),
            },
            vec![ValueId(3), ValueId(5)],
        ),
        (
            StorageOperationV1::Project {
                base: ValueId(3),
                step: StorageProjectionV1::Variant {
                    index: 1,
                    access: read,
                },
            },
            vec![ValueId(3)],
        ),
        (
            StorageOperationV1::ReadValue {
                address: ValueId(3),
                access: read,
            },
            vec![ValueId(3)],
        ),
        (
            StorageOperationV1::WriteValue {
                address: ValueId(3),
                value: ValueId(5),
                access: read,
            },
            vec![ValueId(3), ValueId(5)],
        ),
        (
            StorageOperationV1::CopyObject {
                source: ValueId(3),
                destination: ValueId(5),
                source_access: read,
                destination_access: read,
                overlap: StorageCopyOverlapV1::NonOverlapping,
            },
            vec![ValueId(3), ValueId(5)],
        ),
        (
            StorageOperationV1::CopyObject {
                source: ValueId(3),
                destination: ValueId(5),
                source_access: read,
                destination_access: read,
                overlap: StorageCopyOverlapV1::MayOverlap,
            },
            vec![ValueId(3), ValueId(5)],
        ),
    ]
}

#[test]
fn storage_operation_operands_have_stable_counts_order_and_early_refusal() {
    for (operation, expected) in cases() {
        assert_eq!(operation.operand_count(), expected.len());
        let mut actual = Vec::new();
        operation
            .try_visit_operands(|value| {
                actual.push(value);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(actual, expected);
        for cutoff in 0..expected.len() {
            let mut visited = Vec::new();
            let result = operation.try_visit_operands(|value| {
                if visited.len() == cutoff {
                    return Err(cutoff);
                }
                visited.push(value);
                Ok(())
            });
            assert_eq!(result, Err(cutoff));
            assert_eq!(visited, expected[..cutoff]);
        }
    }
}

#[test]
fn storage_operation_only_variant_projection_reads_memory() {
    for step in [
        StorageProjectionV1::Field(2),
        StorageProjectionV1::ArrayIndex(ValueId(4)),
    ] {
        let operation = StorageOperationV1::Project {
            base: ValueId(1),
            step,
        };
        operation
            .try_visit_memory_accesses(|_, _, _| -> Result<(), ()> {
                panic!("pure storage address projection must not read memory")
            })
            .unwrap();
    }
    let tag = access(AddressSpace::Workgroup, 1, true);
    let operation = StorageOperationV1::Project {
        base: ValueId(1),
        step: StorageProjectionV1::Variant {
            index: 2,
            access: tag,
        },
    };
    let mut accesses = Vec::new();
    operation
        .try_visit_memory_accesses(|pointer, memory, kind| {
            accesses.push((pointer, memory, kind));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        accesses,
        [(ValueId(1), tag, StorageMemoryAccessKindV1::Read)]
    );
    let mut effects = Vec::new();
    operation
        .try_visit_memory_effects(|effect| {
            effects.push(effect);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        effects,
        [MemoryEffect::VolatileRead(AddressSpace::Workgroup)]
    );
}

#[test]
fn storage_operation_copy_preserves_both_accesses_and_overlap_modes() {
    for (source_volatile, destination_volatile) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let source = access(AddressSpace::Global, 8, source_volatile);
        let destination = access(AddressSpace::Private, 1, destination_volatile);
        for overlap in [
            StorageCopyOverlapV1::MayOverlap,
            StorageCopyOverlapV1::NonOverlapping,
        ] {
            let operation = StorageOperationV1::CopyObject {
                source: ValueId(2),
                destination: ValueId(9),
                source_access: source,
                destination_access: destination,
                overlap,
            };
            let mut accesses = Vec::new();
            operation
                .try_visit_memory_accesses(|pointer, memory, kind| {
                    accesses.push((pointer, memory, kind));
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(
                accesses,
                [
                    (ValueId(2), source, StorageMemoryAccessKindV1::Read),
                    (ValueId(9), destination, StorageMemoryAccessKindV1::Write),
                ]
            );
            let mut effects = Vec::new();
            operation
                .try_visit_memory_effects(|effect| {
                    effects.push(effect);
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(
                effects,
                [
                    if source_volatile {
                        MemoryEffect::VolatileRead(AddressSpace::Global)
                    } else {
                        MemoryEffect::Read(AddressSpace::Global)
                    },
                    if destination_volatile {
                        MemoryEffect::VolatileWrite(AddressSpace::Private)
                    } else {
                        MemoryEffect::Write(AddressSpace::Private)
                    },
                ]
            );
        }
    }
}

#[test]
fn storage_operation_memory_visitors_stop_before_later_endpoint() {
    let operation = StorageOperationV1::CopyObject {
        source: ValueId(2),
        destination: ValueId(9),
        source_access: MemoryAccess::new(AddressSpace::Global, 8),
        destination_access: MemoryAccess::new(AddressSpace::Private, 8),
        overlap: StorageCopyOverlapV1::MayOverlap,
    };
    let mut count = 0;
    assert_eq!(
        operation.try_visit_memory_accesses(|pointer, _, kind| {
            count += 1;
            assert_eq!(
                (pointer, kind),
                (ValueId(2), StorageMemoryAccessKindV1::Read)
            );
            Err("stop")
        }),
        Err("stop")
    );
    assert_eq!(count, 1);
    count = 0;
    assert_eq!(
        operation.try_visit_memory_effects(|effect| {
            count += 1;
            assert_eq!(effect, MemoryEffect::Read(AddressSpace::Global));
            Err("stop")
        }),
        Err("stop")
    );
    assert_eq!(count, 1);
}

#[test]
fn storage_operation_read_write_keep_holder_space_and_volatility() {
    let memory = access(AddressSpace::Workgroup, 2, true);
    for (operation, kind) in [
        (
            StorageOperationV1::ReadValue {
                address: ValueId(0),
                access: memory,
            },
            StorageMemoryAccessKindV1::Read,
        ),
        (
            StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: memory,
            },
            StorageMemoryAccessKindV1::Write,
        ),
    ] {
        let mut seen = 0;
        operation
            .try_visit_memory_accesses(|pointer, actual, actual_kind| {
                seen += 1;
                assert_eq!((pointer, actual, actual_kind), (ValueId(0), memory, kind));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(seen, 1);
        let mut effects = Vec::new();
        operation
            .try_visit_memory_effects(|effect| {
                effects.push(effect);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            effects,
            [match kind {
                StorageMemoryAccessKindV1::Read =>
                    MemoryEffect::VolatileRead(AddressSpace::Workgroup),
                StorageMemoryAccessKindV1::Write =>
                    MemoryEffect::VolatileWrite(AddressSpace::Workgroup),
            }]
        );
    }
}
