use super::*;
use crate::CanonicalKirInventoryV18;
use crate::canonical_kir_inventory_v1::v18_tests::{admit, storage_module, with_inventory};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionRoleV15 as ExecutionRole, Function, Kernel, LaunchDomain, LaunchExtent, Module,
    Operation, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};

const LIMIT: usize = 10_000_000;

#[test]
fn v18_discriminant_read_cannot_drop_ordering_or_trap_obligations() {
    use crate::canonical_kir_inventory_v1::v18_tests::storage_discriminant_module;
    with_inventory(&storage_discriminant_module(AddressSpace::Private), |inventory, budget| {
        let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        for ordinal in [18, 20] {
            let index = candidate.rows().iter().position(|row| row.subject == Subject::Operation(ordinal)).unwrap();
            let row = &candidate.rows()[index];
            assert_eq!(row.role, Role::Operation(OperationClass::StorageReadDiscriminant));
            for obligation in [Obligation::Ordering, Obligation::TrapBehavior, Obligation::Initialization,
                Obligation::Bounds, Obligation::Provenance, Obligation::Lifetime, Obligation::RaceFreedom,
                Obligation::ExactScalarSemantics, Obligation::ReferenceRefinement] {
                assert!(row.obligations.contains(obligation));
            }
            let mut weakened = candidate.rows().to_vec();
            weakened[index].role = Role::Operation(OperationClass::StorageRead);
            assert!(check_rows(inventory, &metadata, weakened, budget).is_err());
        }
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

fn candidate<'i, 'g, 'm>(
    inventory: &'i CanonicalKirInventoryV18<'g>,
    metadata: &'i CanonicalRankedMetadataV18<'g, 'm>,
    budget: &mut Budget<'_>,
) -> (CanonicalRankedCandidateV18<'i, 'g, 'm>, usize) {
    let metadata_bytes = metadata.storage_extent(budget).unwrap();
    budget.reserve_storage(metadata_bytes).unwrap();
    let floor = budget.storage();
    let (candidate, receipt) =
        build_canonical_ranked_candidate_v18(inventory, metadata, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    (candidate, metadata_bytes + receipt.retained_storage())
}

#[test]
fn v18_ranked_storage_layouts_operations_and_effects_keep_original_owner() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        for volatile in [false, true] {
            with_inventory(&storage_module(space, volatile), |inventory, budget| {
                let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
                let (candidate, bytes) = candidate(inventory, &metadata, budget);
                let floor = budget.storage();
                for index in 0..4 {
                    let row = &candidate.rows()[index + 1];
                    assert_eq!(row.subject, Subject::StorageLayout(index));
                    assert_eq!(row.role, Role::StorageLayout);
                    for obligation in [
                        Obligation::ReferenceRefinement,
                        Obligation::Initialization,
                        Obligation::Bounds,
                        Obligation::Provenance,
                    ] {
                        assert!(row.obligations.contains(obligation));
                    }
                }
                let expected = [
                    (3, OperationClass::StorageProject),
                    (4, OperationClass::StorageProject),
                    (5, OperationClass::StorageWrite),
                    (6, OperationClass::StorageRead),
                    (8, OperationClass::StorageCopy),
                    (10, OperationClass::StorageProject),
                    (11, OperationClass::StorageProject),
                    (12, OperationClass::StorageWrite),
                    (13, OperationClass::StorageSetDiscriminant),
                    (14, OperationClass::StorageProject),
                    (15, OperationClass::StorageProject),
                    (16, OperationClass::StorageRead),
                ];
                for (ordinal, class) in expected {
                    let row = candidate
                        .rows()
                        .iter()
                        .find(|row| row.subject == Subject::Operation(ordinal))
                        .unwrap();
                    assert_eq!(row.role, Role::Operation(class));
                    for obligation in [
                        Obligation::ReferenceRefinement,
                        Obligation::Initialization,
                        Obligation::Bounds,
                        Obligation::Provenance,
                        Obligation::Lifetime,
                        Obligation::TrapBehavior,
                        Obligation::ExactScalarSemantics,
                    ] {
                        assert!(row.obligations.contains(obligation));
                    }
                    if volatile && matches!(ordinal, 5 | 6 | 8 | 12 | 14 | 16) {
                        assert!(row.obligations.contains(Obligation::Ordering));
                    }
                }
                with_checked_canonical_ranked_view_v18(
                    inventory,
                    &metadata,
                    &candidate,
                    budget,
                    |view, budget| {
                        assert!(std::ptr::eq(view.inventory(budget)?, inventory));
                        assert_eq!(
                            view.inventory(budget)?.identity_v18(),
                            *inventory.owner().identity()
                        );
                        for index in 0..4 {
                            assert!(std::ptr::eq(
                                view.storage_layout(index, budget)?,
                                &inventory.owner().module().storage_layouts[index]
                            ));
                        }
                        for index in 0..inventory.operations().len() {
                            assert!(std::ptr::eq(
                                view.operation(index, budget)?,
                                inventory.operations()[index].operation
                            ));
                        }
                        let target = view.edge(0, budget)?.target_id;
                        let arguments = view.edge(0, budget)?.arguments;
                        assert_eq!(target, view.edge(1, budget)?.target_id);
                        assert_ne!(arguments, view.edge(1, budget)?.arguments);
                        Ok::<_, Error>(())
                    },
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
                drop(candidate);
                budget.release_storage(bytes).unwrap();
            });
        }
    }
}

fn check_rows(
    inventory: &CanonicalKirInventoryV18<'_>,
    metadata: &CanonicalRankedMetadataV18<'_, '_>,
    rows: Vec<Row>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let candidate = CanonicalRankedCandidateV18::from_rows(inventory, metadata, rows);
    let bytes = candidate.retained_storage().unwrap();
    budget.reserve_storage(bytes).unwrap();
    let floor = budget.storage();
    let result =
        with_checked_canonical_ranked_view_v18(inventory, metadata, &candidate, budget, |_, _| {
            Ok(())
        });
    assert_eq!(budget.storage(), floor);
    drop(candidate);
    budget.release_storage(bytes).unwrap();
    result
}

#[test]
fn v18_ranked_every_missing_duplicated_reordered_and_weakened_row_rejects() {
    with_inventory(
        &storage_module(AddressSpace::Private, false),
        |inventory, budget| {
            let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
            let (candidate, bytes) = candidate(inventory, &metadata, budget);
            let original = candidate.rows();
            assert!(check_rows(inventory, &metadata, original.to_vec(), budget).is_ok());
            for i in 0..original.len() {
                let mut missing = original.to_vec();
                missing.remove(i);
                assert!(check_rows(inventory, &metadata, missing, budget).is_err());
                let mut duplicate = original.to_vec();
                duplicate.insert(i, original[i]);
                assert!(check_rows(inventory, &metadata, duplicate, budget).is_err());
                if i + 1 < original.len() {
                    let mut reordered = original.to_vec();
                    reordered.swap(i, i + 1);
                    assert!(check_rows(inventory, &metadata, reordered, budget).is_err());
                }
                if original[i].obligations != Obligations::NONE {
                    let mut weakened = original.to_vec();
                    weakened[i].obligations = Obligations::NONE;
                    assert!(matches!(
                        check_rows(inventory, &metadata, weakened, budget),
                        Err(Error::MismatchedRow { .. })
                    ));
                }
            }
            drop(candidate);
            budget.release_storage(bytes).unwrap();
        },
    );
}

#[test]
fn v18_ranked_same_bytes_do_not_authorize_foreign_owner_inventory_or_metadata() {
    let module = storage_module(AddressSpace::Private, false);
    let (foreign, foreign_bytes) = admit(&module);
    with_inventory(&module, |inventory, budget| {
        budget.reserve_storage(foreign_bytes).unwrap();
        assert_eq!(inventory.owner().identity(), foreign.identity());
        let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let foreign_metadata = CanonicalRankedMetadataV18::new(&foreign, &[]);
        assert!(matches!(
            build_canonical_ranked_candidate_v18(inventory, &foreign_metadata, budget),
            Err(Error::ForeignMetadata)
        ));
        let duplicate_metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
        assert_eq!(
            with_checked_canonical_ranked_view_v18(
                inventory,
                &duplicate_metadata,
                &candidate,
                budget,
                |_, _| Ok::<_, Error>(())
            ),
            Err(Error::ForeignMetadata)
        );
        let (other_inventory, other_receipt) =
            CanonicalKirInventoryV18::derive_v18(inventory.owner(), budget).unwrap();
        budget
            .reserve_storage(other_receipt.retained_storage())
            .unwrap();
        assert_eq!(
            with_checked_canonical_ranked_view_v18(
                &other_inventory,
                &metadata,
                &candidate,
                budget,
                |_, _| Ok::<_, Error>(())
            ),
            Err(Error::ForeignInventory)
        );
        drop(other_inventory);
        budget
            .release_storage(other_receipt.retained_storage())
            .unwrap();
        drop(candidate);
        budget.release_storage(bytes).unwrap();
        drop(foreign);
        budget.release_storage(foreign_bytes).unwrap();
    });
}

#[test]
fn v18_ranked_layout_metadata_is_range_checked_but_never_proof() {
    with_inventory(
        &storage_module(AddressSpace::Private, false),
        |inventory, budget| {
            for index in [0, 4, usize::MAX] {
                let rows = [CanonicalRankedMetadataRowV1 {
                    subject: Subject::StorageLayout(index),
                    kind: CanonicalRankedMetadataKindV1::Memory,
                    facts: &[Fact::Identity([17; 32])],
                }];
                let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &rows);
                let (candidate, bytes) = candidate(inventory, &metadata, budget);
                let result = with_checked_canonical_ranked_view_v18(
                    inventory,
                    &metadata,
                    &candidate,
                    budget,
                    |view, budget| {
                        let last = view.row_count(budget)? - 1;
                        assert!(
                            view.row(last, budget)?
                                .obligations
                                .contains(Obligation::SourceMetadata)
                        );
                        assert!(
                            view.row(last, budget)?
                                .obligations
                                .contains(Obligation::Initialization)
                        );
                        Ok::<_, Error>(())
                    },
                );
                assert_eq!(
                    result,
                    if index == 0 {
                        Ok(())
                    } else {
                        Err(Error::MetadataSubject { ordinal: 0 })
                    }
                );
                drop(candidate);
                budget.release_storage(bytes).unwrap();
            }
        },
    );
}

#[test]
fn v18_ranked_build_and_scope_exact_limits_and_one_short_preserve_input() {
    with_inventory(
        &storage_module(AddressSpace::Private, false),
        |inventory, outer| {
            let identity = inventory.identity_v18();
            let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
            let (candidate, bytes) = candidate(inventory, &metadata, outer);
            let floor = outer.storage();
            let build_at = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, floor + storage_limit);
                budget.reserve_storage(floor).unwrap();
                let result =
                    build_canonical_ranked_candidate_v18(inventory, &metadata, &mut budget).map(
                        |(candidate, receipt)| {
                            let rows = candidate.rows().to_vec();
                            drop(candidate);
                            (rows, receipt.retained_storage())
                        },
                    );
                assert_eq!(budget.storage(), floor);
                (result, budget.work(), budget.peak_storage() - floor)
            };
            let (built, work, peak) = build_at(LIMIT, LIMIT);
            assert_eq!(build_at(work, peak), (built.clone(), work, peak));
            assert!(matches!(
                build_at(work - 1, peak).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                build_at(work, peak - 1).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
            let check_at = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, floor + storage_limit);
                budget.reserve_storage(floor).unwrap();
                let result = with_checked_canonical_ranked_view_v18(
                    inventory,
                    &metadata,
                    &candidate,
                    &mut budget,
                    |view, budget| {
                        assert_eq!(
                            view.storage_layout(0, budget)?,
                            &inventory.owner().module().storage_layouts[0]
                        );
                        view.row_count(budget)
                    },
                );
                assert_eq!(budget.storage(), floor);
                (result, budget.work(), budget.peak_storage() - floor)
            };
            let (checked, work, peak) = check_at(LIMIT, LIMIT);
            assert_eq!(check_at(work, peak), (checked.clone(), work, peak));
            assert!(matches!(
                check_at(work - 1, peak).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                check_at(work, peak - 1).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(inventory.identity_v18(), identity);
            drop(candidate);
            outer.release_storage(bytes).unwrap();
        },
    );
}

#[test]
fn v18_ranked_ignored_invalid_and_foreign_queries_poison_scope() {
    with_inventory(
        &storage_module(AddressSpace::Private, false),
        |inventory, budget| {
            let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
            let (candidate, bytes) = candidate(inventory, &metadata, budget);
            let floor = budget.storage();
            let result = with_checked_canonical_ranked_view_v18(
                inventory,
                &metadata,
                &candidate,
                budget,
                |view, budget| {
                    let _ = view.storage_layout(usize::MAX, budget);
                    Ok::<_, Error>(())
                },
            );
            assert_eq!(
                result,
                Err(Error::InvalidCoordinate(Subject::StorageLayout(usize::MAX)))
            );
            assert_eq!(budget.storage(), floor);
            let mut foreign_work = Work::new(LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, LIMIT);
            foreign.reserve_storage(31).unwrap();
            let result = with_checked_canonical_ranked_view_v18(
                inventory,
                &metadata,
                &candidate,
                budget,
                |view, _| {
                    let _ = view.storage_layout(0, &mut foreign);
                    Ok::<_, Error>(())
                },
            );
            assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
            assert_eq!(foreign.storage(), 31);
            assert_eq!(foreign.work(), 0);
            assert_eq!(budget.storage(), floor);
            let result: Result<()> = with_checked_canonical_ranked_view_v18(
                inventory,
                &metadata,
                &candidate,
                budget,
                |_, _| panic!("ranked V18 callback"),
            );
            assert_eq!(result, Err(Error::Panicked));
            assert_eq!(budget.storage(), floor);
            drop(candidate);
            budget.release_storage(bytes).unwrap();
        },
    );
}

#[test]
fn v18_ranked_undercut_backing_cannot_refund_or_mint_storage() {
    let (owner, owner_bytes) = admit(&storage_module(AddressSpace::Private, false));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(23 + owner_bytes).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let metadata = CanonicalRankedMetadataV18::new(&owner, &[]);
    let (candidate, _) = candidate(&inventory, &metadata, &mut budget);
    let result = with_checked_canonical_ranked_view_v18(
        &inventory,
        &metadata,
        &candidate,
        &mut budget,
        |view, budget| {
            budget.release_storage(budget.storage()).unwrap();
            let _ = view.storage_layout(0, budget);
            Ok::<_, Error>(())
        },
    );
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), 0);
    drop(candidate);
    drop(metadata);
    drop(inventory);
    drop(owner);
}

#[test]
fn v18_ranked_legacy_empty_table_rows_work_and_storage_are_unchanged() {
    let module = crate::canonical_ranked_view_v1::tests::mixed();
    let (owner12, bytes12) = crate::canonical_ranked_view_v1::tests::admit(&module);
    let (owner18, bytes18) = admit(&module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(11 + bytes12 + bytes18).unwrap();
    let mut observed = Vec::new();
    macro_rules! measure {
        ($owner:expr, $derive:path, $build:path, $check:path) => {{
            let (inventory, receipt) = $derive($owner, &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let metadata = Metadata::new($owner, &[]);
            let metadata_bytes = metadata.storage_extent(&mut budget).unwrap();
            budget.reserve_storage(metadata_bytes).unwrap();
            let start = budget.work();
            let (candidate, candidate_receipt) =
                $build(&inventory, &metadata, &mut budget).unwrap();
            budget
                .reserve_storage(candidate_receipt.retained_storage())
                .unwrap();
            let build_work = budget.work() - start;
            let start = budget.work();
            $check(
                &inventory,
                &metadata,
                &candidate,
                &mut budget,
                |view, budget| view.row_count(budget),
            )
            .unwrap();
            observed.push((
                candidate.rows().to_vec(),
                candidate_receipt.retained_storage(),
                build_work,
                budget.work() - start,
            ));
            drop(candidate);
            drop(metadata);
            drop(inventory);
            budget
                .release_storage(
                    candidate_receipt.retained_storage()
                        + metadata_bytes
                        + receipt.retained_storage(),
                )
                .unwrap();
        }};
    }
    measure!(
        &owner12,
        Inventory::derive,
        build_canonical_ranked_candidate_v1,
        with_checked_canonical_ranked_view_v1
    );
    measure!(
        &owner18,
        CanonicalKirInventoryV18::derive_v18,
        build_canonical_ranked_candidate_v18,
        with_checked_canonical_ranked_view_v18
    );
    assert_eq!(observed[0], observed[1]);
    drop((owner12, owner18));
    budget.release_storage(bytes12 + bytes18).unwrap();
    assert_eq!(budget.storage(), 11);
}

fn execution_module() -> Module {
    let role = |id, role| vec![ValueDef::new(ValueId(id), Type::Execution(role))];
    let operations = vec![
        Operation::new(
            role(10, ExecutionRole::Context),
            Op::Execution(Execution::ContextIssue),
        ),
        Operation::new(
            role(11, ExecutionRole::Workgroup),
            Op::Execution(Execution::WorkgroupDerive {
                context: ValueId(10),
            }),
        ),
        Operation::new(
            role(
                12,
                ExecutionRole::MaskedTileU32 {
                    lanes: 64,
                    elements: 1,
                },
            ),
            Op::Execution(Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 64,
                elements: 1,
            }),
        ),
        Operation::new(
            role(
                13,
                ExecutionRole::LaneFragmentU32 {
                    lanes: 64,
                    elements: 1,
                },
            ),
            Op::Execution(Execution::TileIntoFragmentU32 {
                tile: ValueId(12),
                lanes: 64,
                elements: 1,
            }),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(21), Type::BOOL),
            ],
            Op::Execution(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(13),
                lanes: 64,
                elements: 1,
            }),
        ),
        Operation::new(
            vec![],
            Op::Execution(Execution::ScopeEnd {
                workgroup: ValueId(11),
                discarded: vec![],
            }),
        ),
    ];
    let mut module = Module::new("ranked-execution");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

#[test]
fn v18_ranked_actual_execution_lifecycle_keeps_all_pending_obligations() {
    with_inventory(&execution_module(), |inventory, budget| {
        let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let classes = [
            OperationClass::ExecutionContext,
            OperationClass::ExecutionWorkgroup,
            OperationClass::ExecutionTileLoad,
            OperationClass::ExecutionTileIntoFragment,
            OperationClass::ExecutionFragmentIntoParts,
            OperationClass::ExecutionScopeEnd,
        ];
        for (i, class) in classes.into_iter().enumerate() {
            let row = candidate
                .rows()
                .iter()
                .find(|row| row.subject == Subject::Operation(i))
                .unwrap();
            assert_eq!(row.role, Role::Operation(class));
            for obligation in [
                Obligation::Lifetime,
                Obligation::Ordering,
                Obligation::Contract,
                Obligation::Launch,
                Obligation::ReferenceRefinement,
            ] {
                assert!(row.obligations.contains(obligation));
            }
        }
        with_checked_canonical_ranked_view_v18(inventory, &metadata, &candidate, budget, |_, _| {
            Ok::<_, Error>(())
        })
        .unwrap();
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn v18_ranked_unused_layout_rows_change_identity_and_cannot_be_omitted() {
    let mut module = storage_module(AddressSpace::Private, false);
    let (original, original_bytes) = admit(&module);
    module
        .storage_layouts
        .push(fe2o3_kernel_ir::StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::U32),
        });
    with_inventory(&module, |inventory, budget| {
        budget.reserve_storage(original_bytes).unwrap();
        assert_eq!(
            original.module().functions,
            inventory.owner().module().functions
        );
        assert_ne!(original.identity(), inventory.owner().identity());
        let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let extra = candidate
            .rows()
            .iter()
            .position(|row| row.subject == Subject::StorageLayout(4))
            .unwrap();
        let mut omitted = candidate.rows().to_vec();
        omitted.remove(extra);
        assert!(matches!(
            check_rows(inventory, &metadata, omitted, budget),
            Err(Error::MismatchedRow { .. })
        ));
        let foreign = CanonicalRankedMetadataV18::new(&original, &[]);
        assert!(matches!(
            build_canonical_ranked_candidate_v18(inventory, &foreign, budget),
            Err(Error::ForeignMetadata)
        ));
        drop(candidate);
        budget.release_storage(bytes).unwrap();
        drop(original);
        budget.release_storage(original_bytes).unwrap();
    });
}

#[test]
fn v18_ranked_ordered_gpu_operations_are_covered_without_safety_or_purity_claims() {
    use fe2o3_kernel_ir::{
        AssemblySourceIdentity, Gfx942OrderedProgramRegistersV1, Gfx942OrderedProgramV1,
        Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1, Gfx942ProgramDestinationV1,
        Gfx942ProgramInstructionV1, Gfx942ProgramRoleV1, Gfx942U32ProgramV1,
    };
    let source = AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]);
    let inputs = [ValueId(0), ValueId(1), ValueId(2)];
    let operations = [
        (
            OperationClass::OrderedRegion,
            Op::Gfx942OrderedRegion(
                Gfx942OrderedRegionV1::new(
                    source,
                    Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
                    inputs,
                )
                .unwrap(),
            ),
        ),
        (
            OperationClass::OrderedProgram,
            Op::Gfx942OrderedProgram(
                Gfx942OrderedProgramV1::new(
                    source,
                    Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
                    inputs,
                    Gfx942U32ProgramV1::from_instructions(&[Gfx942ProgramInstructionV1::Move {
                        destination: Gfx942ProgramDestinationV1::Output,
                        source: Gfx942ProgramRoleV1::Input0,
                    }])
                    .unwrap(),
                )
                .unwrap(),
            ),
        ),
    ];
    for (class, kind) in operations {
        let ty = Type::Scalar(ScalarType::U32);
        let mut module = Module::new("ranked-ordered");
        module.functions.push(Function::definition(
            "ordered",
            Signature::new(vec![ty.clone(); 3], vec![ty.clone()]),
            inputs.to_vec(),
            vec![BasicBlock {
                id: BlockId(17),
                parameters: vec![],
                operations: vec![Operation::new(vec![ValueDef::new(ValueId(3), ty)], kind)],
                terminator: Some(Terminator::Return {
                    values: vec![ValueId(3)],
                }),
            }],
        ));
        with_inventory(&module, |inventory, budget| {
            let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
            let (candidate, bytes) = candidate(inventory, &metadata, budget);
            let row = candidate
                .rows()
                .iter()
                .find(|row| row.subject == Subject::Operation(0))
                .unwrap();
            assert_eq!(row.role, Role::Operation(class));
            for obligation in [
                Obligation::ReferenceRefinement,
                Obligation::Assembly,
                Obligation::ExactScalarSemantics,
                Obligation::Ordering,
                Obligation::Convergence,
                Obligation::Target,
                Obligation::TrapBehavior,
                Obligation::Initialization,
            ] {
                assert!(row.obligations.contains(obligation));
            }
            with_checked_canonical_ranked_view_v18(
                inventory,
                &metadata,
                &candidate,
                budget,
                |view, budget| {
                    assert!(std::ptr::eq(
                        view.operation(0, budget)?,
                        inventory.operations()[0].operation
                    ));
                    Ok::<_, Error>(())
                },
            )
            .unwrap();
            drop(candidate);
            budget.release_storage(bytes).unwrap();
        });
    }
}
