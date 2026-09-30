// Semantic access identity, not a byte-range, initialization, or lifetime proof.
// In particular, SetDiscriminant still needs its owning layout to distinguish
// an actual tag store from the untagged-niche physical no-op.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceAddressFootprintRoleV33 {
    ValueRead,
    ValueWrite,
    TagRead,
    TagWrite,
    CopyRead,
    CopyWrite,
    VariantRead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAddressFootprintV33 {
    ordinal: u32,
    operand: u32,
    pointer: ValueId,
    access: MemoryAccess,
    role: SourceAddressFootprintRoleV33,
    typed: bool,
}

fn source_address_footprint_v33(
    operation: &Operation,
    ordinal: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceAddressFootprintV33>, ProductionSemanticKirErrorV1> {
    use SourceAddressFootprintRoleV33 as Role;
    budget.charge_work(1)?;
    let (operand, pointer, access, role, typed) = match (&operation.kind, ordinal) {
        (OperationKind::Load { pointer, access }, 0) => {
            (0, *pointer, *access, Role::ValueRead, false)
        }
        (
            OperationKind::Store {
                pointer, access, ..
            },
            0,
        ) => (0, *pointer, *access, Role::ValueWrite, false),
        (OperationKind::Storage(operation), ordinal) => {
            let (operand, pointer, access, role) = match (*operation, ordinal) {
                (ScopedObjectOperationV29::ReadValue { address, access }, 0) => {
                    (0, address, access, Role::ValueRead)
                }
                (
                    ScopedObjectOperationV29::WriteValue {
                        address, access, ..
                    },
                    0,
                ) => (0, address, access, Role::ValueWrite),
                (ScopedObjectOperationV29::ReadDiscriminant { address, access }, 0) => {
                    (0, address, access, Role::TagRead)
                }
                (
                    ScopedObjectOperationV29::SetDiscriminant {
                        address, access, ..
                    },
                    0,
                ) => (0, address, access, Role::TagWrite),
                (
                    ScopedObjectOperationV29::Project {
                        base,
                        step: ScopedObjectProjectionV29::Variant { access, .. },
                    },
                    0,
                ) => (0, base, access, Role::VariantRead),
                (
                    ScopedObjectOperationV29::CopyObject {
                        source,
                        source_access,
                        ..
                    },
                    0,
                ) => (0, source, source_access, Role::CopyRead),
                (
                    ScopedObjectOperationV29::CopyObject {
                        destination,
                        destination_access,
                        ..
                    },
                    1,
                ) => (1, destination, destination_access, Role::CopyWrite),
                _ => return Ok(None),
            };
            (operand, pointer, access, role, true)
        }
        _ => return Ok(None),
    };
    Ok(Some(SourceAddressFootprintV33 {
        ordinal,
        operand,
        pointer,
        access,
        role,
        typed,
    }))
}

#[cfg(test)]
mod footprint_tests_v33 {
    use super::*;

    #[test]
    fn source_footprints_match_every_storage_role_and_preserve_copy_order() {
        use fe2o3_kernel_ir::StorageMemoryAccessKindV1::{Read, Write};
        let access = MemoryAccess::new(AddressSpace::Private, 4);
        let pointer = ValueId(7);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        for kind in [
            ScopedObjectOperationV29::Project {
                base: pointer,
                step: ScopedObjectProjectionV29::Field(0),
            },
            ScopedObjectOperationV29::Project {
                base: pointer,
                step: ScopedObjectProjectionV29::ArrayIndex(ValueId(9)),
            },
            ScopedObjectOperationV29::Project {
                base: pointer,
                step: ScopedObjectProjectionV29::Variant { index: 0, access },
            },
            ScopedObjectOperationV29::Project {
                base: pointer,
                step: ScopedObjectProjectionV29::VariantForWrite { index: 0 },
            },
            ScopedObjectOperationV29::ReadValue {
                address: pointer,
                access,
            },
            ScopedObjectOperationV29::WriteValue {
                address: pointer,
                value: ValueId(9),
                access,
            },
            ScopedObjectOperationV29::ReadDiscriminant {
                address: pointer,
                access,
            },
            ScopedObjectOperationV29::SetDiscriminant {
                address: pointer,
                variant: 0,
                access,
            },
            ScopedObjectOperationV29::CopyObject {
                source: pointer,
                destination: pointer,
                source_access: access,
                destination_access: access,
                overlap: fe2o3_kernel_ir::StorageCopyOverlapV1::MayOverlap,
            },
        ] {
            let operation = Operation {
                results: vec![],
                kind: OperationKind::Storage(kind),
            };
            let mut ordinal = 0;
            kind.try_visit_memory_accesses(|actual, access, effect| {
                let row = source_address_footprint_v33(&operation, ordinal, &mut budget)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    (row.ordinal, row.pointer, row.access, row.typed),
                    (ordinal, actual, access, true)
                );
                assert_eq!(
                    row.operand,
                    if matches!(kind, ScopedObjectOperationV29::CopyObject { .. }) {
                        ordinal
                    } else {
                        0
                    }
                );
                assert_eq!(
                    effect,
                    match row.role {
                        SourceAddressFootprintRoleV33::ValueRead
                        | SourceAddressFootprintRoleV33::TagRead
                        | SourceAddressFootprintRoleV33::CopyRead
                        | SourceAddressFootprintRoleV33::VariantRead => Read,
                        SourceAddressFootprintRoleV33::ValueWrite
                        | SourceAddressFootprintRoleV33::TagWrite
                        | SourceAddressFootprintRoleV33::CopyWrite => Write,
                    }
                );
                ordinal += 1;
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
            assert!(
                source_address_footprint_v33(&operation, ordinal, &mut budget)
                    .unwrap()
                    .is_none()
            );
            assert!(
                source_address_footprint_v33(&operation, u32::MAX, &mut budget)
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn source_footprint_identity_does_not_collapse_aliased_copy_roles() {
        let rows = [
            SourceAddressAccessV29 {
                block: BlockId(3),
                operation: 5,
                footprint: 0,
                slot: 11,
            },
            SourceAddressAccessV29 {
                block: BlockId(3),
                operation: 5,
                footprint: 1,
                slot: 11,
            },
        ];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        for ordinal in 0..2 {
            let row = SourceAddressMemoryV29::access_footprint(
                &rows,
                BlockId(3),
                5,
                ordinal,
                &mut budget,
            )
            .unwrap()
            .unwrap();
            assert_eq!((row.footprint, row.slot), (ordinal, 11));
        }
        assert!(
            SourceAddressMemoryV29::access_footprint(&rows, BlockId(3), 5, 2, &mut budget)
                .unwrap()
                .is_none()
        );
        assert!(SourceAddressMemoryV29::access(&rows, BlockId(3), 5, &mut budget).is_err());
        assert!(SourceAddressMemoryV29::access(&rows[1..], BlockId(3), 5, &mut budget).is_err());
        assert_eq!(
            SourceAddressMemoryV29::access(&rows[..1], BlockId(3), 5, &mut budget).unwrap(),
            Some(&rows[0])
        );
        assert!(
            SourceAddressMemoryV29::access_footprint(
                &[rows[0], rows[0]],
                BlockId(3),
                5,
                0,
                &mut budget,
            )
            .is_err()
        );
    }

    #[test]
    fn source_footprint_classification_has_independent_one_work_limit() {
        let operation = Operation {
            results: vec![],
            kind: OperationKind::Load {
                pointer: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        };
        for limit in [1, 0] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_address_footprint_v33(&operation, 0, &mut budget);
            assert_eq!(result.is_ok(), limit == 1);
            assert_eq!(budget.work(), limit);
            assert_eq!(budget.storage(), 0);
            assert!(source_address_footprint_v33(&operation, 0, &mut budget).is_err());
            assert_eq!(budget.work(), limit);
        }
    }
}
