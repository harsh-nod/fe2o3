use crate::{
    AccessMode, AddressSpace, MemoryAccess, MemoryEffect, Operation, OperationKind,
    StorageCopyOverlapV1, StorageLayoutIdV1, StorageOperationV1, StorageProjectionV1, Type,
    ValueId,
};

fn operation(storage: StorageOperationV1) -> Operation {
    // These fixtures exercise raw visitors, not semantic or memory admission.
    Operation::new(Vec::new(), OperationKind::Storage(storage))
}

fn access(space: AddressSpace, volatile: bool) -> MemoryAccess {
    MemoryAccess {
        address_space: space,
        alignment: 4,
        volatile,
    }
}

fn operand_cases() -> Vec<(StorageOperationV1, Vec<ValueId>)> {
    let a = ValueId(7);
    let b = ValueId(11);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    vec![
        (
            StorageOperationV1::Project {
                base: a,
                step: StorageProjectionV1::Field(2),
            },
            vec![a],
        ),
        (
            StorageOperationV1::Project {
                base: a,
                step: StorageProjectionV1::ArrayIndex(b),
            },
            vec![a, b],
        ),
        (
            StorageOperationV1::Project {
                base: a,
                step: StorageProjectionV1::Variant { index: 1, access },
            },
            vec![a],
        ),
        (
            StorageOperationV1::ReadValue { address: a, access },
            vec![a],
        ),
        (
            StorageOperationV1::WriteValue {
                address: a,
                value: b,
                access,
            },
            vec![a, b],
        ),
        (
            StorageOperationV1::CopyObject {
                source: a,
                destination: b,
                source_access: access,
                destination_access: access,
                overlap: StorageCopyOverlapV1::NonOverlapping,
            },
            vec![a, b],
        ),
        (
            StorageOperationV1::CopyObject {
                source: a,
                destination: a,
                source_access: access,
                destination_access: access,
                overlap: StorageCopyOverlapV1::MayOverlap,
            },
            vec![a, a],
        ),
    ]
}

#[test]
fn storage_shared_operand_visitors_preserve_every_endpoint_and_duplicate() {
    for (storage, expected) in operand_cases() {
        let operation = operation(storage);
        assert_eq!(operation.kind.operand_count(), expected.len());
        assert_eq!(operation.operands(), expected);
        let mut borrowed = Vec::new();
        operation.visit_operands(|value| borrowed.push(value));
        assert_eq!(borrowed, expected);
    }
}

#[test]
fn storage_shared_operand_visitor_stops_at_each_rejected_endpoint() {
    for (storage, expected) in operand_cases() {
        let operation = operation(storage);
        for stop in 0..expected.len() {
            let mut visited = Vec::new();
            let result = operation.kind.try_visit_operands(|value| {
                visited.push(value);
                if visited.len() == stop + 1 {
                    Err(stop)
                } else {
                    Ok(())
                }
            });
            assert_eq!(result, Err(stop));
            assert_eq!(visited, expected[..=stop]);
        }
    }
}

fn check_effects(storage: StorageOperationV1, expected: Vec<MemoryEffect>) {
    let operation = operation(storage);
    assert_eq!(operation.memory_effects(), expected);
    let mut borrowed = Vec::new();
    operation
        .try_visit_local_memory_effects_v1(|effect| {
            borrowed.push(effect.to_owned());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(borrowed, expected);
    for stop in 0..expected.len() {
        let mut visited = Vec::new();
        let result = operation.try_visit_local_memory_effects_v1(|effect| {
            visited.push(effect.to_owned());
            if visited.len() == stop + 1 {
                Err(stop)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(stop));
        assert_eq!(visited, expected[..=stop]);
    }
}

#[test]
fn storage_shared_effects_keep_tag_reads_and_volatile_value_accesses() {
    let base = ValueId(2);
    for space in [
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Global,
        AddressSpace::Constant,
        AddressSpace::Generic,
    ] {
        for volatile in [false, true] {
            let access = access(space, volatile);
            let read = if volatile {
                MemoryEffect::VolatileRead(space)
            } else {
                MemoryEffect::Read(space)
            };
            let write = if volatile {
                MemoryEffect::VolatileWrite(space)
            } else {
                MemoryEffect::Write(space)
            };
            check_effects(
                StorageOperationV1::Project {
                    base,
                    step: StorageProjectionV1::Variant { index: 0, access },
                },
                vec![read.clone()],
            );
            check_effects(
                StorageOperationV1::ReadValue {
                    address: base,
                    access,
                },
                vec![read],
            );
            check_effects(
                StorageOperationV1::WriteValue {
                    address: base,
                    value: ValueId(3),
                    access,
                },
                vec![write],
            );
        }
    }
    for step in [
        StorageProjectionV1::Field(0),
        StorageProjectionV1::ArrayIndex(ValueId(3)),
    ] {
        check_effects(StorageOperationV1::Project { base, step }, Vec::new());
    }
}

#[test]
fn storage_shared_copy_effects_preserve_endpoint_spaces_order_and_volatility() {
    for overlap in [
        StorageCopyOverlapV1::MayOverlap,
        StorageCopyOverlapV1::NonOverlapping,
    ] {
        for source_volatile in [false, true] {
            for destination_volatile in [false, true] {
                let read = if source_volatile {
                    MemoryEffect::VolatileRead(AddressSpace::Global)
                } else {
                    MemoryEffect::Read(AddressSpace::Global)
                };
                let write = if destination_volatile {
                    MemoryEffect::VolatileWrite(AddressSpace::Workgroup)
                } else {
                    MemoryEffect::Write(AddressSpace::Workgroup)
                };
                check_effects(
                    StorageOperationV1::CopyObject {
                        source: ValueId(4),
                        destination: ValueId(5),
                        source_access: access(AddressSpace::Global, source_volatile),
                        destination_access: access(AddressSpace::Workgroup, destination_volatile),
                        overlap,
                    },
                    vec![read, write],
                );
            }
        }
    }
}

#[test]
fn storage_object_is_a_pointee_descriptor_not_a_first_class_ssa_value() {
    let object = Type::StorageObject(StorageLayoutIdV1(3));
    assert!(!object.is_storable());
    assert!(!object.contains_execution_role_v15());
    assert!(object.as_scalar().is_none());
    let pointer = Type::pointer(object.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    assert!(pointer.is_storable());
    assert!(!pointer.contains_execution_role_v15());
    let slice = Type::slice(object, AddressSpace::Global, AccessMode::ReadOnly);
    assert!(!slice.is_storable());
    assert!(!slice.contains_execution_role_v15());
}

#[test]
fn storage_shared_capability_visitor_has_no_implicit_target_or_source_authority() {
    for (storage, _) in operand_cases() {
        let operation = operation(storage);
        assert_eq!(operation.required_capability_visitation_work_v1(), Some(1));
        let mut visits = 0;
        operation
            .try_visit_required_capabilities_v1(|_| {
                visits += 1;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(visits, 0);
        assert!(operation.required_capabilities().is_empty());
    }
}
