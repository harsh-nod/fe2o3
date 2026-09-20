fn owning_source_fixture(
    kind: ModuleFixture,
    preexisting: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedSourceInputsV29, ScopedModuleErrorV29> {
    let mut owner = module_fixture_owner(kind);
    if preexisting {
        let receipt = owner
            .try_capture_occurrences_with_budget_v1(budget)
            .map_err(ScopedModuleErrorV29::Occurrences)?;
        budget.reserve_storage(receipt.retained_storage())?;
    }
    let (input, launch) = with_module_fixture_view(&owner, kind, budget, |source, budget| {
        OwnedExecutionInputV29::capture(source, budget)
    })?;
    Ok(ScopedSourceInputsV29 {
        owner,
        launch,
        input: input?,
    })
}

#[test]
fn scoped_source_owner_retains_actual_inputs_graph_capture_and_replayed_assertions() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let source = owning_source_fixture(kind, preexisting, &mut budget).unwrap();
            let identity = source.owner.identity();
            let input_storage = source.input.retained_storage;
            let old_capture = source
                .owner
                .occurrence_storage()
                .map_or(0, |row| row.retained_storage());
            let mut donor = Some(source);
            let owner = SourceOwnedScopedModuleV29::try_new(
                &mut donor,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .unwrap();
            assert!(donor.is_none());
            assert_eq!(owner.source.owner.identity(), identity);
            assert_eq!(owner.assertions.len(), 2);
            assert_eq!(owner.capture.preexisting_storage(), old_capture);
            assert_eq!(
                owner.capture.transferred_storage(),
                if preexisting {
                    0
                } else {
                    owner
                        .source
                        .owner
                        .occurrence_storage()
                        .unwrap()
                        .retained_storage()
                },
            );
            assert_eq!(
                owner.retained_storage,
                input_storage
                    + size_of::<SourceOwnedScopedModuleV29>()
                    + owner.pending.retained_storage
                    + owner.capture.transferred_storage()
                    + owner.assertions.capacity() * size_of::<ReplayedInstanceAssertV1>()
            );
            assert_eq!(
                budget.storage(),
                MODULE_FLOOR + old_capture + owner.retained_storage
            );
            let floor = budget.storage();
            let graph_bytes = owner.pending.graph.canonical_bytes().as_ptr();
            let graph_identity = *owner.pending.graph.identity();
            owner.replay(&mut budget).unwrap();
            assert_eq!(owner.pending.graph.canonical_bytes().as_ptr(), graph_bytes);
            assert_eq!(owner.pending.graph.identity(), &graph_identity);
            assert_eq!(budget.storage(), floor);
            let retained = owner.retained_storage;
            drop(owner);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), MODULE_FLOOR + old_capture);
            budget.release_storage(old_capture).unwrap();
        }
    }
}

#[test]
fn scoped_source_owner_preserves_donor_on_foreign_or_missing_input_reservation() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let source = owning_source_fixture(ModuleFixture::Mixed, false, &mut budget).unwrap();
    let retained = source.input.retained_storage;
    let mut donor = Some(source);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
    foreign.reserve_storage(retained).unwrap();
    assert!(
        SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut foreign,
        )
        .is_err()
    );
    assert!(donor.is_some());
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), retained);
    budget.release_storage(retained).unwrap();
    let work_before = budget.work();
    assert!(
        SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .is_err()
    );
    assert!(donor.is_some());
    assert_eq!(budget.work(), work_before);
    drop(donor);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn scoped_source_owner_input_reservation_cannot_mask_missing_preexisting_capture() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let source = owning_source_fixture(ModuleFixture::Mixed, true, &mut budget).unwrap();
    let capture = source
        .owner
        .occurrence_storage()
        .unwrap()
        .retained_storage();
    let input = source.input.retained_storage;
    assert!(capture > 0);
    budget.release_storage(capture).unwrap();
    let mut donor = Some(source);
    let before = budget.work();
    assert!(
        SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .is_err()
    );
    assert!(donor.is_some());
    assert_eq!(budget.storage(), input);
    assert_eq!(budget.work(), before);
    drop(donor);
    budget.release_storage(input).unwrap();
}

#[test]
fn scoped_source_owner_consuming_failures_restore_only_the_caller_floor() {
    for preexisting in [false, true] {
        for fault in 0..2 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let mut source =
                owning_source_fixture(ModuleFixture::Mixed, preexisting, &mut budget).unwrap();
            let capture = source
                .owner
                .occurrence_storage()
                .map_or(0, |row| row.retained_storage());
            if fault == 0 {
                source.input.semantic_sha256[0] ^= 1;
            }
            let limits = if fault == 1 {
                ProductionSemanticKirLimitsV1::new_with_max_operations(0, 128, 128, 1024)
            } else {
                ProductionSemanticKirLimitsV1::default()
            };
            let mut donor = Some(source);
            assert!(SourceOwnedScopedModuleV29::try_new(&mut donor, limits, &mut budget).is_err());
            assert!(donor.is_none());
            assert_eq!(budget.storage(), MODULE_FLOOR + capture);
            budget.release_storage(capture).unwrap();
        }
    }
}

#[test]
fn scoped_source_owner_replay_rejects_foreign_ledger_before_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let mut donor = Some(owning_source_fixture(ModuleFixture::Mixed, false, &mut budget).unwrap());
    let owner = SourceOwnedScopedModuleV29::try_new(
        &mut donor,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
    foreign.reserve_storage(owner.retained_storage).unwrap();
    assert!(owner.replay(&mut foreign).is_err());
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), owner.retained_storage);
    let retained = owner.retained_storage;
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn scoped_source_owner_replay_rejects_metadata_changes_with_the_same_graph() {
    for fault in 0..25 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let mut donor =
            Some(owning_source_fixture(ModuleFixture::Array, false, &mut budget).unwrap());
        let mut owner = SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let graph_identity = *owner.pending.graph.identity();
        let roots = &mut owner.pending.roots;
        match fault {
            0 => roots[0].function_ordinal = 2,
            1 => roots[0].coordinates.semantic_sha256[0] ^= 1,
            2 => roots[0].coordinates.seeds.rows[0].function_name.push('x'),
            3 => {
                assert!(roots[0].coordinates.spans.rows.pop().is_some());
            }
            4 => {
                assert!(roots[0].coordinates.controls.rows.pop().is_some());
            }
            5 => {
                assert!(roots[1].coordinates.anchors.rows.pop().is_some());
            }
            6 => {
                roots
                    .iter_mut()
                    .flat_map(|root| &mut root.source_slots.slots)
                    .next()
                    .unwrap()
                    .origin
                    .local += 1
            }
            7 => roots[0].source_slots.instances[0].slots.end += 1,
            8 => roots[1].insertions[0].after.first += 1,
            9 => roots[0].declarations[0].function_ordinal = 0,
            10 => roots[0].sidecars.rows[0].next_value += 1,
            11 => {
                let slots = roots
                    .iter_mut()
                    .flat_map(|root| &mut root.sidecars.rows)
                    .find_map(|row| {
                        row.scoped_slot_origins
                            .as_mut()
                            .filter(|rows| !rows.is_empty())
                    })
                    .unwrap();
                assert!(slots.pop().is_some());
            }
            12 => {
                roots[0].sidecars.rows[0]
                    .scoped_initialization
                    .as_mut()
                    .unwrap()
                    .blocks[0]
                    .initialized
                    .end += 1
            }
            13 => {
                let anchors = roots
                    .iter_mut()
                    .flat_map(|root| &mut root.sidecars.rows)
                    .find_map(|row| {
                        row.scoped_memory_anchors
                            .as_mut()
                            .filter(|rows| !rows.rows.is_empty())
                    })
                    .unwrap();
                anchors.rows[0].position += 1;
            }
            14 => {
                roots[0].sidecars.rows[0]
                    .instance_assert_origins
                    .as_mut()
                    .unwrap()
                    .failed = true
            }
            15 => {
                roots[0].sidecars.rows[0]
                    .instance_assert_origins
                    .as_mut()
                    .unwrap()
                    .records[0]
                    .argument_count += 1
            }
            16 => {
                let events = roots[1]
                    .sidecars
                    .rows
                    .iter_mut()
                    .find_map(|row| {
                        row.lifecycle_events
                            .as_mut()
                            .filter(|events| !events.rows.is_empty())
                    })
                    .unwrap();
                events.rows[0].original_gap += 1;
            }
            17 => roots[0].sidecars.rows[0].private_arrays.payload.occupied += 1,
            18 => {
                assert!(
                    roots[0].sidecars.rows[0]
                        .statement_operation_spans
                        .pop()
                        .is_some()
                );
            }
            19 => {
                assert!(
                    roots[0].sidecars.rows[0]
                        .terminator_operation_spans
                        .pop()
                        .is_some()
                );
            }
            20 => {
                assert!(
                    roots[0].sidecars.rows[0]
                        .synthetic_operation_spans
                        .pop()
                        .is_some()
                );
            }
            21 => {
                assert!(roots[0].sidecars.rows[0].parameter_bindings.pop().is_some());
            }
            22 => roots[0].sidecars.rows[0].emitted_operations += 1,
            23 => owner.assertions[0].site = owner.assertions[1].site,
            24 => owner.source.input.events.pop().map(|_| ()).unwrap(),
            _ => unreachable!(),
        }
        let floor = budget.storage();
        assert!(owner.replay(&mut budget).is_err(), "fault {fault}");
        assert_eq!(budget.storage(), floor, "fault {fault} leaked scratch");
        assert_eq!(owner.pending.graph.identity(), &graph_identity);
        let retained = owner.retained_storage;
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn scoped_source_owner_replay_rejects_another_genuine_verified_graph() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let mut first = Some(owning_source_fixture(ModuleFixture::Mixed, false, &mut budget).unwrap());
    let mut a = SourceOwnedScopedModuleV29::try_new(
        &mut first,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let mut second = Some(owning_source_fixture(ModuleFixture::Array, false, &mut budget).unwrap());
    let mut b = SourceOwnedScopedModuleV29::try_new(
        &mut second,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_ne!(a.pending.graph.identity(), b.pending.graph.identity());
    let floor = budget.storage();
    // Both authentic graph reservations remain live throughout the substitution.
    std::mem::swap(&mut a.pending.graph, &mut b.pending.graph);
    assert!(a.replay(&mut budget).is_err());
    assert!(b.replay(&mut budget).is_err());
    assert_eq!(budget.storage(), floor);
    std::mem::swap(&mut a.pending.graph, &mut b.pending.graph);
    a.replay(&mut budget).unwrap();
    b.replay(&mut budget).unwrap();
    let retained = a.retained_storage + b.retained_storage;
    drop((a, b));
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

thread_local! {
    static OWNING_REPLAY_ROOT_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn panic_during_owning_reconstruction(
    source: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    _: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(source.owner.occurrence_storage().is_some());
    let count = OWNING_REPLAY_ROOT_VISITS.get() + 1;
    OWNING_REPLAY_ROOT_VISITS.set(count);
    assert_ne!(count, 6, "late source replay panic");
    Ok(())
}

#[test]
fn scoped_source_owner_unwind_after_graph_admission_restores_capture_ownership() {
    for preexisting in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let source = owning_source_fixture(ModuleFixture::Mixed, preexisting, &mut budget).unwrap();
        let capture = source
            .owner
            .occurrence_storage()
            .map_or(0, |row| row.retained_storage());
        let mut donor = Some(source);
        OWNING_REPLAY_ROOT_VISITS.set(0);
        let previous = SCOPED_SLOT_OBSERVER_V29.replace(Some(panic_during_owning_reconstruction));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            SourceOwnedScopedModuleV29::try_new(
                &mut donor,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
        }));
        SCOPED_SLOT_OBSERVER_V29.set(previous);
        assert!(result.is_err());
        assert_eq!(OWNING_REPLAY_ROOT_VISITS.get(), 6);
        assert!(donor.is_none());
        assert_eq!(budget.storage(), MODULE_FLOOR + capture);
        budget.release_storage(capture).unwrap();
    }
}
