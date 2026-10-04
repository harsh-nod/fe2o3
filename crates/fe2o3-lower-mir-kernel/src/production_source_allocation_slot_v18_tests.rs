include!("production_source_allocation_frames_v32_tests.rs");
include!("production_source_object_endpoints_v39_tests.rs");

#[test]
fn source_allocation_slot_queries_retain_actual_original_and_optimized_backing() {
    let mut kinds = [0usize; 2];
    for factory in [
        scalar_payload_owner_v18 as fn() -> ProductionSemanticSsaOwnerV1,
        stored_physical_owner_v29,
    ] {
        let completed = std::cell::Cell::new(false);
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            // These borrowed query headers belong to this unit-result scope,
            // not to the optimizer callback's zero-retained-storage receipt.
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let roots = original.source.root_count(budget)?;
                let mut visited = 0;
                for root in 0..roots {
                    let owner = original.source.root_row(root)?;
                    for (ordinal, slot) in owner.source_slots.slots.iter().enumerate() {
                        assert_eq!(
                            owner
                                .source_slots
                                .slots
                                .iter()
                                .filter(|other| other.origin.pointer == slot.origin.pointer)
                                .count(),
                            1
                        );
                        let input = scoped_raw_admission_v29::source_slot_input_v18(
                            original, root, ordinal, budget,
                        )?;
                        let scanned = original.retained_allocation(root, input, budget)?.unwrap();
                        let direct = original
                            .retained_allocation_for_slot_v18(root, ordinal, input, budget)?;
                        assert_eq!(
                            (direct.instance, direct.row),
                            (scanned.instance, scanned.row)
                        );
                        assert!(std::ptr::eq(direct.slot, scanned.slot));
                        assert!(std::ptr::eq(direct.slot, slot));
                        let scanned = optimized.allocation(root, input, budget)?.unwrap();
                        let direct =
                            optimized.allocation_for_slot_v18(root, ordinal, input, budget)?;
                        assert_eq!(
                            (
                                direct.instance(),
                                direct.input(),
                                direct.output(),
                                direct.pointer(),
                                direct.count()
                            ),
                            (
                                scanned.instance(),
                                scanned.input(),
                                scanned.output(),
                                scanned.pointer(),
                                scanned.count()
                            )
                        );
                        assert!(direct.output().is_some());
                        kinds[usize::from(matches!(
                            slot.representation,
                            ScopedSlotRepresentationV29::Object { .. }
                        ))] += 1;
                        visited += 1;
                    }
                }
                assert!(visited > 0);
                let floor = budget.storage();
                assert_eq!(
                    original.check_optimized_source_currentness_v18(optimized, budget)?,
                    roots
                );
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok(())
            })
        });
        assert!(completed.get());
    }
    assert!(
        kinds.into_iter().all(|count| count > 0),
        "both authentic representations required: {kinds:?}"
    );
}

#[test]
fn source_allocation_slot_query_unit_scope_restores_selected_error_and_unwind() {
    for panics in [false, true] {
        let reached = std::cell::Cell::new(false);
        run_production_optimized_consumer_v18(
            stored_physical_owner_v29,
            |original, optimized, budget| {
                let floor = budget.storage();
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    source_scalar_normalization_scratch_v18(
                        original.source.cleanup,
                        budget,
                        0,
                        |budget| {
                            let input = scoped_raw_admission_v29::source_slot_input_v18(
                                original, 0, 0, budget,
                            )?;
                            assert_eq!(
                                original
                                    .retained_allocation_for_slot_v18(0, 0, input, budget)?
                                    .row,
                                0
                            );
                            assert!(
                                optimized
                                    .allocation_for_slot_v18(0, 0, input, budget)?
                                    .output()
                                    .is_some()
                            );
                            assert!(budget.storage() > floor);
                            reached.set(true);
                            if panics {
                                std::panic::panic_any("allocation query selected panic");
                            }
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "allocation query selected error",
                            ))
                        },
                    )
                }));
                assert!(reached.get());
                if panics {
                    let payload =
                        caught.expect_err("the selected panic must resume after scratch cleanup");
                    assert_eq!(
                        payload.downcast_ref::<&'static str>(),
                        Some(&"allocation query selected panic")
                    );
                } else {
                    assert!(matches!(
                        caught,
                        Ok(Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "allocation query selected error"
                        )))
                    ));
                }
                assert_eq!(budget.storage(), floor);
                // The selected local exit is not a failed source query. Genuine
                // query failures in the separate hostile controls remain sticky.
                source_scalar_normalization_scratch_v18(
                    original.source.cleanup,
                    budget,
                    0,
                    |budget| {
                        let input = scoped_raw_admission_v29::source_slot_input_v18(
                            original, 0, 0, budget,
                        )?;
                        assert!(
                            optimized
                                .allocation_for_slot_v18(0, 0, input, budget)?
                                .output()
                                .is_some()
                        );
                        Ok(())
                    },
                )?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        );
        assert!(reached.get());
    }
}

#[test]
fn source_allocation_slot_queries_reject_other_slot_instance_root_and_missing_coordinates() {
    for optimized_query in [false, true] {
        for fault in 0..5 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared =
                retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget).unwrap();
            let reached = std::cell::Cell::new(false);
            let result = with_production_optimizer_result_v18(
                prepared,
                &mut budget,
                |original, optimized, budget| {
                    source_scalar_normalization_scratch_v18(
                        original.source.cleanup,
                        budget,
                        0,
                        |budget| {
                            let owner = original.source.root_row(1)?;
                            let (ordinal, slot) = owner
                                .source_slots
                                .slots
                                .iter()
                                .enumerate()
                                .find(|(_, slot)| {
                                    owner.source_slots.slots.iter().any(|other| {
                                        other.instance == slot.instance
                                            && other.origin.pointer != slot.origin.pointer
                                    })
                                })
                                .unwrap();
                            let input = scoped_raw_admission_v29::source_slot_input_v18(
                                original, 1, ordinal, budget,
                            )?;
                            assert!(std::ptr::eq(
                                original
                                    .retained_allocation_for_slot_v18(1, ordinal, input, budget)?
                                    .slot,
                                slot
                            ));
                            assert!(
                                optimized
                                    .allocation_for_slot_v18(1, ordinal, input, budget)?
                                    .output()
                                    .is_some()
                            );
                            let (root, selected, actual) = match fault {
                                0 | 1 => {
                                    let other = owner
                                        .source_slots
                                        .slots
                                        .iter()
                                        .enumerate()
                                        .find(|(_, other)| {
                                            other.origin.pointer != slot.origin.pointer
                                                && if fault == 0 {
                                                    other.instance == slot.instance
                                                } else {
                                                    other.instance != slot.instance
                                                }
                                        })
                                        .unwrap()
                                        .0;
                                    let other_input =
                                        scoped_raw_admission_v29::source_slot_input_v18(
                                            original, 1, other, budget,
                                        )?;
                                    (1, ordinal, other_input)
                                }
                                2 => (0, ordinal, input),
                                3 => (1, owner.source_slots.slots.len(), input),
                                4 => {
                                    let mut absent = input;
                                    absent.operation = u32::MAX;
                                    (1, ordinal, absent)
                                }
                                _ => unreachable!(),
                            };
                            let error = if optimized_query {
                                match optimized
                                    .allocation_for_slot_v18(root, selected, actual, budget)
                                {
                                    Err(error) => error,
                                    Ok(_) => panic!("substituted optimized slot admitted"),
                                }
                            } else {
                                match original.retained_allocation_for_slot_v18(
                                    root, selected, actual, budget,
                                ) {
                                    Err(error) => error,
                                    Ok(_) => panic!("substituted original slot admitted"),
                                }
                            };
                            assert!(
                                matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)),
                                "{error:?}"
                            );
                            let stopped = budget.work();
                            assert!(
                                original
                                    .retained_allocation_for_slot_v18(1, ordinal, input, budget)
                                    .is_err()
                            );
                            assert!(
                                optimized
                                    .allocation_for_slot_v18(1, ordinal, input, budget)
                                    .is_err()
                            );
                            assert_eq!(budget.work(), stopped);
                            reached.set(true);
                            Err(error)
                        },
                    )
                },
            );
            assert!(
                reached.get(),
                "fault {fault}, optimized {optimized_query}: {result:?}"
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                ))
            ));
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn source_allocation_slot_query_rejects_duplicate_or_substituted_attachment_rows() {
    for fault in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget).unwrap();
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                    let owner = source.root_row(1)?;
                    let slot = &owner.source_slots.slots[0];
                    let input = scoped_raw_admission_v29::source_slot_input_v18(original, 1, 0, budget)?;
                    assert!(std::ptr::eq(original.retained_allocation_for_slot_v18(1, 0, input, budget)?.slot, slot));
                    let floor = budget.storage();
                    budget.reserve_storage(size_of::<Vec<SourceAttachmentV18>>()
                        + size_of::<ProductionSourceCorrespondenceV18<'_>>())?;
                    let mut rows = source_attachments_v18(source, inventory, budget)?;
                    let target = rows.iter().position(|row| row.key.root == 1
                        && row.key.family == TileAttachmentFamilyV29::SourceSlot
                        && row.key.instance == slot.instance.index()
                        && row.key.field == TileAttachmentFieldV29::SlotAllocation
                        && matches!(row.location, TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point))
                            if point.function == input.block.function.0 as usize
                                && point.block == input.block.block as usize && point.operation == input.operation as usize)).unwrap();
                    if fault == 0 {
                        let extra = (target + 1) % rows.len();
                        rows[extra] = rows[target];
                        rows[extra].key.part = 1;
                    } else if fault == 1 {
                        rows[target].location = rows.iter().enumerate().find(|(index, row)| *index != target
                            && row.key.root == 1 && row.key.family == TileAttachmentFamilyV29::SourceSlot
                            && row.key.field == TileAttachmentFieldV29::SlotAllocation
                            && row.location != rows[target].location).unwrap().1.location;
                    } else {
                        rows[target].location = TileAttachmentLocationV29::NoOutput;
                    }
                    private_array_heapsort_v1(&mut rows, |row| source_attachment_key_v18(row.key),
                        &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
                    let tampered = ProductionSourceCorrespondenceV18 {
                        source, inventory, attachments: &rows, slot: std::ptr::from_ref(budget) as usize,
                        ledger: budget.work_ledger_identity_v1(), floor: budget.storage(),
                    };
                    let error = match tampered.retained_allocation_for_slot_v18(1, 0, input, budget) {
                        Err(error) => error,
                        Ok(_) => panic!("ambiguous or substituted allocation attachment admitted"),
                    };
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                    let stopped = budget.work();
                    assert!(original.retained_allocation_for_slot_v18(1, 0, input, budget).is_err());
                    assert_eq!(budget.work(), stopped);
                    reached.set(true);
                    drop(tampered);
                    drop(rows);
                    budget.release_storage(budget.storage() - floor)?;
                    Err(error)
                })
            }))
        });
        assert!(reached.get(), "fault {fault}: {result:?}");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn source_allocation_slot_resource_run_v18(
    typed: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut count = 0;
    let result = (|| {
        let prepared = if typed {
            physical_prepared_result_v29(&mut budget)?
        } else {
            retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget)?
        };
        prepared.with_source_consumer_v18(
            &mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            |original, budget| {
                                for root in 0..source.root_count(budget)? {
                                    let owner = source.root_row(root)?;
                                    for (ordinal, slot) in
                                        owner.source_slots.slots.iter().enumerate()
                                    {
                                        let input =
                                            scoped_raw_admission_v29::source_slot_input_v18(
                                                original, root, ordinal, budget,
                                            )?;
                                        let backing = original.retained_allocation_for_slot_v18(
                                            root, ordinal, input, budget,
                                        )?;
                                        assert_eq!(backing.row, ordinal);
                                        assert!(std::ptr::eq(backing.slot, slot));
                                        count += 1;
                                    }
                                }
                                Ok(())
                            },
                        )
                    })
                })
            },
        )
    })();
    let completed = result.is_ok();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (
        result,
        budget.work(),
        budget.peak_storage(),
        count,
        completed,
    )
}

#[test]
fn source_allocation_slot_queries_have_exact_and_one_short_owned_transactions() {
    for typed in [false, true] {
        let (result, work, storage, count, completed) = source_allocation_slot_resource_run_v18(
            typed,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        assert!(completed && count > 0);
        let (exact, actual_work, actual_storage, actual_count, completed) =
            source_allocation_slot_resource_run_v18(typed, work, storage);
        exact.unwrap();
        assert!(completed);
        assert_eq!(
            (actual_work, actual_storage, actual_count),
            (work, storage, count)
        );
        for (work_limit, storage_limit, expect_work) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let (short, _, _, _, completed) =
                source_allocation_slot_resource_run_v18(typed, work_limit, storage_limit);
            assert!(!completed);
            let error = match short {
                Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => error,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                )) => error,
                other => panic!("exact source-owned resource denial required: {other:?}"),
            };
            match (expect_work, error) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > error.limit());
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > error.limit());
                }
                _ => panic!("wrong first resource refusal: {error:?}"),
            }
        }
    }
}

#[test]
fn source_allocation_slot_fixed_headers_have_independent_exact_and_short_equations() {
    use fe2o3_kernel_analysis::CanonicalKirOutputUseV1 as OutputUse;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirOperationCoordinateV1 as Coordinate, CanonicalKirUseCoordinateV1 as Usage,
    };
    let shape = size_of::<()>()
        + 2 * size_of::<SourceOwnedResultV18<()>>()
        + size_of::<&ScopedSourceSlotV29>()
        + size_of::<&Operation>()
        + size_of::<&Option<ValueId>>()
        + size_of::<&Type>()
        + size_of::<&u32>()
        + size_of::<(
            ScopedAllocationIdentityV29,
            ScopedAllocationSourceV29,
            ScopedSlotRepresentationV29,
        )>()
        + size_of::<ScopedScalarArraySlotV29>()
        + size_of::<Result<ScopedScalarArraySlotV29, ProductionSemanticKirErrorV1>>()
        + size_of::<SourceOwnedResultV18<ScopedScalarArraySlotV29>>()
        + size_of::<Option<&(ValueId, PrivateArrayPhysicalLocationV1)>>()
        + size_of::<&(ValueId, PrivateArrayPhysicalLocationV1)>()
        + size_of::<&ValueId>()
        + size_of::<Option<ValueId>>()
        + size_of::<SourceCorrespondenceWorkV18<'_, '_>>()
        + size_of::<SourceOwnedResultV18<bool>>()
        + size_of::<Result<(), ProductionSemanticKirErrorV1>>();
    let slot = size_of::<SourcePhysicalBackingV18<'_>>()
        + 2 * size_of::<SourceOwnedResultV18<SourcePhysicalBackingV18<'_>>>()
        + size_of::<&ScopedModuleRootV29>()
        + size_of::<SourceOwnedResultV18<&ScopedModuleRootV29>>()
        + size_of::<Option<&ScopedSourceSlotV29>>()
        + size_of::<&ScopedSourceSlotV29>()
        + size_of::<SourceOwnedResultV18<&ScopedSourceSlotV29>>()
        + size_of::<Coordinate>()
        + size_of::<SourceOwnedResultV18<Coordinate>>()
        + size_of::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()
        + size_of::<SourceOwnedResultV18<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>()
        + size_of::<&Operation>()
        + size_of::<SourceOwnedResultV18<()>>();
    let transport = size_of::<SourcePhysicalBackingV18<'_>>()
        + size_of::<ProductionOptimizedSourceAllocationV18<'_>>()
        + 2 * size_of::<SourceOwnedResultV18<ProductionOptimizedSourceAllocationV18<'_>>>()
        + 2 * size_of::<Coordinate>()
        + size_of::<Option<Coordinate>>()
        + size_of::<SourceOwnedResultV18<Option<Coordinate>>>()
        + size_of::<usize>()
        + size_of::<SourceOwnedResultV18<usize>>()
        + size_of::<&Operation>()
        + size_of::<&[ValueDef]>()
        + size_of::<&ValueDef>()
        + size_of::<&Type>()
        + size_of::<&Option<ValueId>>()
        + size_of::<&ValueId>()
        + size_of::<&u32>()
        + size_of::<ScopedSlotRepresentationV29>()
        + size_of::<ScopedScalarArraySlotV29>()
        + size_of::<SourceCorrespondenceWorkV18<'_, '_>>()
        + size_of::<SourceOwnedResultV18<bool>>()
        + size_of::<Result<(), ProductionSemanticKirErrorV1>>()
        + size_of::<SourceOwnedResultV18<()>>()
        + 2 * size_of::<Definition>()
        + size_of::<Option<Definition>>()
        + size_of::<(
            Option<(ValueId, PrivateArrayPhysicalLocationV1)>,
            &Option<ValueId>,
        )>()
        + size_of::<OutputUse>()
        + size_of::<SourceOwnedResultV18<OutputUse>>()
        + size_of::<Option<OutputUse>>()
        + size_of::<Usage>();
    for (actual, expected) in [
        (source_allocation_shape_headers_v18().unwrap(), shape),
        (source_allocation_slot_headers_v18().unwrap(), slot),
        (
            ProductionOptimizedSourceCorrespondenceV18::allocation_transport_headers_v18().unwrap(),
            transport,
        ),
    ] {
        assert_eq!(actual, expected);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget =
                ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + expected - usize::from(short));
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let result = budget.reserve_storage(actual);
            if short {
                let Err(ArgumentResourceV1::Storage(error)) = result else {
                    panic!("header denial required");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (MODULE_FLOOR + expected, MODULE_FLOOR + expected - 1)
                );
                assert_eq!(budget.storage(), MODULE_FLOOR);
            } else {
                result.unwrap();
                assert_eq!(budget.storage(), MODULE_FLOOR + expected);
            }
            assert_eq!(budget.work(), 0);
        }
    }
}

#[test]
fn source_allocation_slot_query_retains_the_first_header_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget).unwrap();
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                let input = scoped_raw_admission_v29::source_slot_input_v18(original, 1, 0, budget)?;
                assert_eq!(original.retained_allocation_for_slot_v18(1, 0, input, budget)?.row, 0);
                let floor = budget.storage();
                let padding = budget.storage_limit() - floor - (source_allocation_slot_headers_v18()? - 1);
                budget.reserve_storage(padding)?;
                let held = budget.storage();
                let error = match original.retained_allocation_for_slot_v18(1, 0, input, budget) {
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) => error,
                    other => panic!("slot fixed header must refuse: {:?}", other.as_ref().err()),
                };
                assert_eq!((error.actual(), error.limit()), (budget.storage_limit() + 1, budget.storage_limit()));
                assert_eq!(budget.storage(), held);
                let stopped = budget.work();
                assert!(matches!(original.retained_allocation_for_slot_v18(1, 0, input, budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(replayed))) if replayed == error));
                assert_eq!((budget.work(), budget.storage()), (stopped, held));
                budget.release_storage(padding)?;
                assert_eq!(budget.storage(), floor);
                reached.set(true);
                Ok(())
            })
        }))
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_allocation_slot_query_rejects_foreign_ledger_before_debit_and_denies_refund() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget).unwrap();
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(
        &mut budget,
        |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                        let input = scoped_raw_admission_v29::source_slot_input_v18(
                            original, 1, 0, budget,
                        )?;
                        assert_eq!(
                            original
                                .retained_allocation_for_slot_v18(1, 0, input, budget)?
                                .row,
                            0
                        );
                        let mut foreign_work =
                            CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                        assert!(matches!(
                            original.retained_allocation_for_slot_v18(1, 0, input, &mut foreign),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                        let stopped = (budget.work(), budget.storage());
                        assert!(matches!(
                            original.retained_allocation_for_slot_v18(1, 0, input, budget),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!((budget.work(), budget.storage()), stopped);
                        reached.set(true);
                        Ok(())
                    })
                })
            })
        },
    );
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(
        budget.storage() > MODULE_FLOOR,
        "foreign custody must not refund the authentic owner"
    );
}

thread_local! {
    static ALLOCATION_DUPLICATE_ORIGIN_OBSERVED_V18: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn source_allocation_duplicate_origin_v18(
    original: &[ScopedSlotOriginV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Vec<ScopedSlotOriginV29>>, ProductionSemanticKirErrorV1> {
    type Output = Option<Vec<ScopedSlotOriginV29>>;
    budget.reserve_storage(
        size_of::<Output>()
            + 2 * size_of::<Result<Output, ProductionSemanticKirErrorV1>>()
            + size_of::<Option<usize>>()
            + size_of::<Option<(usize, usize)>>(),
    )?;
    let mut first = None;
    let mut pair = None;
    for (ordinal, origin) in original.iter().enumerate() {
        budget.charge_work(2)?;
        if !matches!(
            origin.identity,
            ScopedAllocationIdentityV29::OriginalObject { .. }
        ) {
            continue;
        }
        if let Some(previous) = first {
            pair = Some((previous, ordinal));
            break;
        }
        first = Some(ordinal);
    }
    let Some((first, second)) = pair else {
        return Ok(None);
    };
    let mut forged = emission_vec_v1(original.len(), budget)?;
    budget.charge_work(original.len())?;
    forged.extend_from_slice(original);
    assert_ne!(forged[first].identity, forged[second].identity);
    assert_ne!(forged[first].pointer, forged[second].pointer);
    // Only inert origins are changed: the second real Alloca result remains
    // distinct, so exact result authentication must reject before uniqueness.
    forged[second].pointer = forged[first].pointer;
    ALLOCATION_DUPLICATE_ORIGIN_OBSERVED_V18
        .set(ALLOCATION_DUPLICATE_ORIGIN_OBSERVED_V18.get() + 1);
    Ok(Some(forged))
}

#[test]
fn source_allocation_constructor_rejects_duplicate_original_pointer_before_owner_publication() {
    let (positive, _, _, count, completed) = source_allocation_slot_resource_run_v18(
        true,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    positive.unwrap();
    assert!(completed && count > 1);
    struct Restore(Option<ScopedObjectOriginMutationV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_OBJECT_ORIGIN_MUTATION_V29.set(self.0);
        }
    }
    let _restore = Restore(
        SCOPED_OBJECT_ORIGIN_MUTATION_V29.replace(Some(source_allocation_duplicate_origin_v18)),
    );
    ALLOCATION_DUPLICATE_ORIGIN_OBSERVED_V18.set(0);
    let (result, _, _, count, completed) = source_allocation_slot_resource_run_v18(
        true,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(ALLOCATION_DUPLICATE_ORIGIN_OBSERVED_V18.get() > 0);
    assert!(!completed);
    assert_eq!(
        count, 0,
        "no borrowed slot query may observe an invalid original roster"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        detail: "typed allocation identity or representation requires its exact source contract",
                        ..
                    }
                )
            ))
        ),
        "{result:?}"
    );
}

fn optimized_allocation_slot_resource_run_v18(
    work_limit: usize,
    storage_limit: usize,
) -> (ProductionOptimizerTestResultV18, usize, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut count = 0;
    let result = (|| {
        let prepared = physical_prepared_result_v29(&mut budget)?;
        with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                source_scalar_normalization_scratch_v18(
                    original.source.cleanup,
                    budget,
                    0,
                    |budget| {
                        let roots = original.source.root_count(budget)?;
                        for root in 0..roots {
                            let owner = original.source.root_row(root)?;
                            for (ordinal, slot) in owner.source_slots.slots.iter().enumerate() {
                                let input = scoped_raw_admission_v29::source_slot_input_v18(
                                    original, root, ordinal, budget,
                                )?;
                                let allocation = optimized
                                    .allocation_for_slot_v18(root, ordinal, input, budget)?;
                                assert_eq!(allocation.instance(), slot.instance.index());
                                assert!(allocation.output().is_some());
                                count += 1;
                            }
                        }
                        let floor = budget.storage();
                        assert_eq!(
                            original.check_optimized_source_currentness_v18(optimized, budget)?,
                            roots
                        );
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    },
                )
            },
        )
    })();
    let completed = result.is_ok();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (
        result,
        budget.work(),
        budget.peak_storage(),
        count,
        completed,
    )
}

#[test]
fn source_allocation_slot_optimized_currentness_transactions_have_exact_and_one_short_limits() {
    let (result, work, storage, count, completed) =
        optimized_allocation_slot_resource_run_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(completed && count > 0);
    let (exact, used, peak, actual_count, completed) =
        optimized_allocation_slot_resource_run_v18(work, storage);
    exact.unwrap();
    assert!(completed);
    assert_eq!((used, peak, actual_count), (work, storage, count));
    for (work_limit, storage_limit, expect_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, _, completed) =
            optimized_allocation_slot_resource_run_v18(work_limit, storage_limit);
        assert!(!completed);
        let error = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => error,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("exact optimized resource refusal required: {other:?}"),
        };
        match (expect_work, error) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > error.limit());
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > error.limit());
            }
            _ => panic!("wrong optimized resource refusal: {error:?}"),
        }
    }
}

#[test]
fn source_allocation_constructor_unique_pointer_guard_checks_authentic_roster() {
    let reached = std::cell::Cell::new(false);
    run_production_optimized_consumer_v18(stored_physical_owner_v29, |original, _, budget| {
        source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
            let root = original.source.root_row(0)?;
            let slots = &root.source_slots.slots;
            assert!(slots.len() >= 2);
            // Every genuine append accepts the prefix before its own row.
            for (ordinal, slot) in slots.iter().enumerate() {
                budget.charge_work(ordinal)?;
                assert!(scoped_slot_pointer_is_unique_v29(
                    &slots[..ordinal],
                    slot.origin.pointer
                ));
            }
            // The same paid guard refuses an already published original pointer.
            budget.charge_work(slots.len())?;
            assert!(!scoped_slot_pointer_is_unique_v29(
                slots,
                slots[0].origin.pointer
            ));
            // The inert direct predicate result cannot poison or publish an owner.
            let input = scoped_raw_admission_v29::source_slot_input_v18(original, 0, 0, budget)?;
            assert_eq!(
                original
                    .retained_allocation_for_slot_v18(0, 0, input, budget)?
                    .row,
                0
            );
            reached.set(true);
            Ok(())
        })
    });
    assert!(reached.get());
}
