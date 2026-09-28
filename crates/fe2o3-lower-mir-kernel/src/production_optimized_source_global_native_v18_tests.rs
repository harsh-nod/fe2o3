#[test]
fn pending_global_source_native_pair_frames_have_an_independent_equation() {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirInventoryV18 as Inventory, CanonicalKirOperationRefV1,
    };
    type PairFrame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a mut ArgumentBudgetV1<'a>,
        &'a Inventory<'a>,
        &'a GlobalSourceAccessEndpointV18,
        std::array::IntoIter<SliceOperation, 4>,
        Option<SliceOperation>,
        SliceOperation,
        Option<&'a fe2o3_kernel_ir::Operation>,
        &'a fe2o3_kernel_ir::Operation,
        &'a CanonicalKirOperationRefV1<'a>,
        Result<&'a CanonicalKirOperationRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Option<&'a Terminator>,
        Option<(&'a Terminator, &'a Terminator)>,
        &'a CanonicalKirBlockRefV1<'a>,
        Result<&'a CanonicalKirBlockRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        (&'a Inventory<'a>,),
        &'a CanonicalKirDefinitionRefV1<'a>,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<ValueId, ProductionSourceOwnedViewErrorV18>,
        Result<&'a Inventory<'a>, ProductionSourceOwnedViewErrorV18>,
        (
            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        Option<ValueId>,
        [ValueId; 4],
        [&'a fe2o3_kernel_ir::Operation; 4],
        [&'a fe2o3_kernel_ir::OperationKind; 2],
        [&'a ValueId; 2],
        &'a fe2o3_kernel_ir::MemoryAccess,
        [bool; 2],
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        [usize; 3],
        Option<&'a GlobalSourceAccessPairV18>,
        Option<&'a GlobalSourceAccessPairV18>,
        &'a GlobalSourceAccessPairV18,
        SourceOwnedResultV18<Option<&'a GlobalSourceAccessPairV18>>,
        (&'a Option<&'a GlobalSourceAccessPairV18>, &'a Option<&'a GlobalSourceAccessPairV18>),
        (&'a GlobalSourceAccessPairV18, &'a GlobalSourceAccessPairV18),
        Result<(), ArgumentResourceV1>,
        SourceOwnedResultV18<()>,
        bool,
    );
    let expected = size_of::<PairFrame<'_>>()
        + 2 * size_of::<Result<PairFrame<'_>, PendingGlobalNativeErrorV18>>();
    assert_eq!(global_native_pair_headers_v18().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result = budget.reserve_storage(global_native_pair_headers_v18().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected && error.limit() == expected - 1));
            assert_eq!(budget.storage(), 0);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
        }
    }
}

fn with_native_global_test_view_v18(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    run: impl FnOnce(
        &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let output = optimized.output_inventory(budget)?;
    let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
    let metadata_storage = metadata.storage_extent(budget).unwrap();
    budget.reserve_storage(metadata_storage)?;
    let (candidate, receipt) =
        build_canonical_ranked_candidate_v18(output, &metadata, budget).unwrap();
    budget.reserve_storage(receipt.retained_storage())?;
    let layouts = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
        rows: 4096,
        edges: 16384,
        containment_depth: 128,
        object_bytes: 1 << 24,
    };
    with_checked_canonical_ranked_view_v18(
        output,
        &metadata,
        &candidate,
        budget,
        |checked, budget| {
            fe2o3_pliron::with_pending_canonical_ranked_source_roles_v18(
                checked,
                layouts,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        output.owner(),
                        budget,
                        |native, budget| {
                            run(native, budget);
                            Ok(())
                        },
                    )
                },
            )
            .unwrap();
            Ok::<_, CanonicalRankedViewErrorV1>(())
        },
    )
    .unwrap();
    drop(candidate);
    drop(metadata);
    budget.release_storage(metadata_storage + receipt.retained_storage())?;
    Ok(())
}

#[test]
fn pending_global_source_native_headers_have_independent_exact_and_one_short_boundaries() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, PendingGlobalNativeErrorV18>>()
    }
    let expected = global_native_pair_headers_v18().unwrap()
        + h::<PendingGlobalSourceNativeAccessV18<'_, '_>>()
        + h::<&GlobalSourceAccessPairV18>()
        + h::<Option<&GlobalSourceAccessPairV18>>()
        + h::<Option<&PendingGlobalSourceNativeAccessV18<'_, '_>>>()
        + h::<&fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>>()
        + h::<&fe2o3_kernel_ir::Operation>()
        + h::<Option<&fe2o3_kernel_ir::Operation>>()
        + h::<&Terminator>()
        + h::<Option<&Terminator>>()
        + h::<&CanonicalKirDefinitionRefV1<'_>>()
        + h::<[SliceOperation; 4]>()
        + h::<[ValueId; 3]>()
        + h::<[usize; 4]>()
        + h::<Result<(), PendingGlobalNativeErrorV18>>()
        + h::<std::thread::Result<Result<(), PendingGlobalNativeErrorV18>>>()
        + h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<(
            &PendingGlobalSourceNativeAccessV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>();
    assert_eq!(global_native_headers_v18(0).unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result = budget.reserve_storage(global_native_headers_v18(0).unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
            if error.actual() == expected && error.limit() == expected - 1));
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
        }
    }
}

pub(super) fn test_pending_global_native_positive_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    counts: &std::cell::Cell<[usize; 2]>,
    cross_descriptor: bool,
    rhs_pending: bool,
) -> SourceOwnedResultV18<()> {
    let input_root = original.source.root(0, budget)?.1;
    let input_function = &original.inventory.functions()[input_root];
    let input_operations = &original.inventory.operations()[input_function.operations.clone()];
    assert_eq!(
        input_operations
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::Load { .. }))
            .count(),
        1,
        "one genuine original Load"
    );
    assert_eq!(
        input_operations
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::Store { .. }))
            .count(),
        1,
        "one genuine original Store"
    );
    let output = optimized.output_inventory(budget)?;
    let output_function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
    let output_operations = &output.operations()[output_function.operations.clone()];
    let output_counts = [
        output_operations
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::Load { .. }))
            .count(),
        output_operations
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::Store { .. }))
            .count(),
    ];
    assert_eq!(
        output_counts,
        [1, 1],
        "actual optimized Load/Store opcode census"
    );
    if rhs_pending {
        for (inventory, operations) in [
            (original.inventory, input_operations),
            (output, output_operations),
        ] {
            let store = operations
                .iter()
                .find(|row| matches!(row.operation.kind, OperationKind::Store { .. }))
                .unwrap();
            let value_use = &inventory.uses()[store.operands.start + 1];
            let SliceDefinition::Result {
                operation,
                result: 0,
            } = inventory.definitions()[value_use.definition].coordinate
            else {
                panic!("Arithmetic Store must use its actual Binary result");
            };
            let producer = optimized_source_operation_row_v18(inventory, operation, budget)?;
            assert!(
                matches!(producer.operation.kind, OperationKind::Binary { .. }),
                "retained arithmetic RHS: {:?}",
                producer.operation.kind
            );
        }
    }
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        assert_eq!(source.roles.rows.len(), output_operations.len());
        if cross_descriptor {
            let read = source.roles.rows.iter().find_map(|row| row.global.as_ref()
                .filter(|pair| !pair.output.writing)).expect("genuine cross-descriptor read");
            let write = source.roles.rows.iter().find_map(|row| row.global.as_ref()
                .filter(|pair| pair.output.writing)).expect("genuine cross-descriptor write");
            for (pair, argument) in [(read, 0), (write, 1)] {
                assert!(matches!(pair.input.logical.root,
                    SliceDefinition::FunctionArgument { argument: actual, .. } if actual == argument));
                assert!(matches!(pair.output.logical.root,
                    SliceDefinition::FunctionArgument { argument: actual, .. } if actual == argument));
            }
            assert_ne!(read.input.logical.root, write.input.logical.root);
            assert_ne!(read.output.logical.root, write.output.logical.root);
        }
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            for (row, actual) in source.roles.rows.iter().zip(output_operations) {
                assert_eq!(row.output, actual.coordinate);
                let memory = match actual.operation.kind {
                    OperationKind::Load { .. } => Some(false),
                    OperationKind::Store { .. } => Some(true),
                    _ => None,
                };
                if let Some(writing) = memory {
                    let pair = row.global.as_ref().expect("actual memory has an exact source pair");
                    assert_eq!(pair.output.writing, writing);
                    assert_eq!(pair.output.logical.access.operation, actual.coordinate);
                    assert_eq!(row.write_recipe_pending, writing && rhs_pending);
                }
                let floor = budget.storage();
                source
                    .with_native_access_v18(native, row.output, budget, |view, _| {
                        assert_eq!(view.is_some(), memory.is_some_and(|writing| !writing || !rhs_pending),
                            "actual opcode {:?}, RHS pending {}", actual.operation.kind, row.write_recipe_pending);
                        if let Some(view) = view {
                            assert!(!view.grants_memory_or_launch_authority());
                            assert!(!view.native.memory_safety_is_complete());
                            assert_eq!(view.pair.output.logical.access.operation, row.output);
                            let mut count = counts.get();
                            count[usize::from(view.pair.output.writing)] += 1;
                            counts.set(count);
                        }
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(counts.get(), [output_counts[0], if rhs_pending { 0 } else { output_counts[1] }]);
        })
    })
}

pub(super) fn test_pending_global_native_mutations_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: usize,
    reached: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            let pair = source
                .roles
                .rows
                .iter()
                .find_map(|row| row.global.as_ref().filter(|pair| pair.output.writing))
                .unwrap();
            let mut changed = *pair;
            match fault {
                0 => changed.output.value = changed.output.pointer,
                1 => changed.output.pointer = changed.output.value,
                2 => changed.output.logical.index = changed.output.logical.root,
                3 => changed.output.logical.root = changed.output.logical.index,
                4 => changed.output.logical.guard_condition = changed.output.logical.index,
                5 => changed.output.writing = false,
                6 => changed.output.logical.guard_edge.successor = u32::MAX,
                _ => unreachable!(),
            }
            assert!(matches!(
                source.check_native_pair_v18(native, &changed, budget),
                Err(PendingGlobalNativeErrorV18::Source(_))
            ));
            reached.set(true);
        })
    })
}

pub(super) fn test_pending_global_native_foreign_owner_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    reached: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original
        .source
        .with_ranked_correspondence_v18(original.inventory, budget, |foreign, budget| {
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                optimized.pending_global_output_v18(foreign, budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "pending global substituted exact correspondence"
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), before);
            reached.set(true);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
}

pub(super) fn test_pending_global_native_accessor_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let before = (budget.work(), budget.storage());
    let output = optimized.pending_global_output_v18(original, budget)?;
    assert_eq!((budget.work(), budget.storage()), before);
    assert!(std::ptr::eq(output, optimized.output_inventory(budget)?));
    Ok(())
}

pub(super) fn test_pending_global_native_frame_retry_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    observed: &std::cell::Cell<bool>,
    entered: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
                with_native_global_test_view_v18(optimized, budget, |native, budget| {
                    let operation = source.roles.rows.iter()
                        .find(|row| row.global.is_some()).unwrap().output;
                    let callback = |_: Option<&PendingGlobalSourceNativeAccessV18<'_, '_>>,
                                    _: &mut ArgumentBudgetV1<'_>| {
                        entered.set(true);
                        Ok(())
                    };
                    let required = global_native_headers_v18(std::mem::size_of_val(&callback)).unwrap();
                    let padding = budget.storage_limit() - budget.storage() - required + 1;
                    budget.reserve_storage(padding).unwrap();
                    let first = source.with_native_access_v18(native, operation, budget, callback);
                    let Err(PendingGlobalNativeErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(first)),
                    )) = first else { panic!("expected authentic frame storage refusal"); };
                    assert_eq!(first.actual(), first.limit() + 1);
                    assert_eq!(budget.failed_storage(), Some(first.actual()));
                    budget.release_storage(padding).unwrap();
                    let before = (budget.work(), budget.storage());
                    let retry = source.with_native_access_v18(native, operation, budget, |_, _| {
                        entered.set(true);
                        Ok(())
                    });
                    assert!(matches!(retry, Err(PendingGlobalNativeErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)),
                    )) if error == first));
                    assert_eq!((budget.work(), budget.storage()), before);
                    assert!(!entered.get());
                    observed.set(true);
                })
            })
}

pub(super) fn test_pending_global_native_foreign_budget_v18(
    module_limit: usize,
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    extra: usize,
    disposition: usize,
    observed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            let operation = source
                .roles
                .rows
                .iter()
                .find(|row| row.global.is_some())
                .unwrap()
                .output;
            let foreign_floor = std::cell::Cell::new(0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source.with_native_access_v18(native, operation, budget, |_, budget| {
                    let expected = budget.storage() + extra;
                    // Bounded process-lifetime fixture meter, not production state.
                    let meter =
                        Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(module_limit)));
                    let mut foreign = ArgumentBudgetV1::new(meter, module_limit);
                    foreign.reserve_storage(expected)?;
                    foreign_floor.set(expected);
                    let old = std::mem::replace(budget, foreign);
                    assert_eq!(old.storage() + extra, expected);
                    drop(old);
                    match disposition {
                        0 => Ok(()),
                        1 => Err(PendingGlobalNativeErrorV18::Source(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "foreign replacement sentinel",
                            ),
                        )),
                        _ => std::panic::resume_unwind(Box::new(0x1651_u64)),
                    }
                })
            }));
            assert_eq!(budget.storage(), foreign_floor.get());
            assert_eq!(budget.work(), 0);
            assert!(source.roles.original.source.cleanup.is_denied());
            match disposition {
                0 => assert!(matches!(
                    result.unwrap(),
                    Err(PendingGlobalNativeErrorV18::Native(
                        fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                            ArgumentResourceV1::Accounting
                        )
                    ))
                )),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(PendingGlobalNativeErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding("foreign replacement sentinel")
                    ))
                )),
                _ => assert_eq!(*result.unwrap_err().downcast::<u64>().unwrap(), 0x1651),
            }
            observed.set(true);
        })
    })
}

pub(super) fn test_pending_global_native_shared_read_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    reads: &std::cell::Cell<usize>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            for row in source.roles.rows {
                source
                    .with_native_access_v18(native, row.output, budget, |view, _| {
                        if let Some(view) = view {
                            assert!(!view.pair.output.writing);
                            assert!(!view.grants_memory_or_launch_authority());
                            assert_ne!(view.pair.output.logical.index, view.pair.output.address_index,
                                "the real U64 source index has a distinct emitted Index operand");
                            reads.set(reads.get() + 1);
                        }
                        Ok(())
                    })
                    .unwrap();
            }
        })
    })
}

pub(super) fn test_pending_global_native_shared_index_mutation_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    physical: bool,
    reached: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            let pair = source.roles.rows.iter().find_map(|row| row.global.as_ref()).unwrap();
            let refused = source.with_native_access_v18(
                native, pair.output.logical.access.operation, budget, |view, budget| {
                    let view = view.expect("authentic assertion-backed native read");
                    let copied = *view.pair;
                    // An equal copy is harmless; both individual identities must
                    // remain active even though only one is the raw GEP operand.
                    source.check_native_pair_v18(native, &copied, budget)?;
                    let coordinate = if physical {
                        copied.output.address_index
                    } else {
                        copied.output.logical.index
                    };
                    let inventory = optimized.output_inventory(budget)?;
                    let exact = optimized_source_definition_row_v18(inventory, coordinate, budget)?;
                    let replacement = inventory.definitions().iter().find(|row| {
                        let function = match row.coordinate {
                            SliceDefinition::FunctionArgument { function, .. } => function,
                            SliceDefinition::BlockArgument { block, .. } => block.function,
                            SliceDefinition::Result { operation, .. } => operation.block.function,
                        };
                        function == copied.output.logical.access.operation.block.function
                            && row.coordinate != coordinate && row.ty == exact.ty && row.value.is_some()
                    }).unwrap_or_else(|| panic!(
                        "physical={physical}: genuine same-type sibling definition for {coordinate:?}/{:?}", exact.ty));
                    let mut changed = copied;
                    if physical {
                        changed.output.address_index = replacement.coordinate;
                    } else {
                        changed.output.logical.index = replacement.coordinate;
                    }
                    assert!(matches!(
                        source.check_native_pair_v18(native, &changed, budget),
                        Err(PendingGlobalNativeErrorV18::Source(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "pending global native substituted checked source pair"
                            )
                        ))
                    ));
                    reached.set(true);
                    Ok(())
                },
            );
            assert!(matches!(refused, Err(PendingGlobalNativeErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "pending global native substituted checked source pair"
                )
            ))));
        })
    })
}
