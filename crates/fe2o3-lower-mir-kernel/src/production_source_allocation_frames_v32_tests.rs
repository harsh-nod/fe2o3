#[test]
fn source_allocation_frames_visit_exact_declaring_slots_and_relocated_allocations() {
    let mut representations = [0usize; 2];
    for factory in [
        scalar_payload_owner_v18 as fn() -> ProductionSemanticSsaOwnerV1,
        stored_physical_owner_v29,
    ] {
        let completed = std::cell::Cell::new(false);
        run_production_optimized_consumer_v18(factory, |original, _, budget| {
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let mut visited = 0;
                for root in 0..original.source.root_count(budget)? {
                    let owner = original.source.root_row(root)?;
                    let mut ordinal = 0;
                    original.visit_allocation_frames_v32(root, budget, |row, budget| {
                        let slot = &owner.source_slots.slots[ordinal];
                        assert_eq!(row.root(), root);
                        assert_eq!(row.instance(), slot.instance.index());
                        assert_eq!(row.local(), slot.origin.identity.original_local().unwrap());
                        assert_eq!(row.semantic_type(), slot.origin.semantic_type);
                        assert_eq!(
                            row.function(),
                            original.source.instance(root, row.instance(), budget)?.0
                        );
                        assert_eq!(
                            row.allocation(),
                            scoped_raw_admission_v29::source_slot_input_v18(
                                original, root, ordinal, budget
                            )?
                        );
                        assert_eq!(
                            row.allocation().block.function.0 as usize,
                            owner.function_ordinal
                        );
                        let source_generation = match slot.origin.identity {
                            ScopedAllocationIdentityV29::LegacyLocal(_) => None,
                            ScopedAllocationIdentityV29::OriginalObject { generation, .. } => {
                                Some(generation)
                            }
                            _ => panic!("unexpected retained allocation identity"),
                        };
                        assert_eq!(row.source_generation(), source_generation);
                        match slot.representation {
                            ScopedSlotRepresentationV29::ScalarArray(value) => {
                                representations[0] += 1;
                                assert_eq!(
                                    (row.bytes(), row.alignment()),
                                    (value.bytes, value.element.alignment)
                                );
                            }
                            ScopedSlotRepresentationV29::Object {
                                schema,
                                bytes,
                                alignment,
                            } => {
                                representations[1] += 1;
                                assert_eq!(
                                    (row.layout(), row.bytes(), row.alignment()),
                                    (Some(schema), bytes, alignment)
                                );
                            }
                        }
                        ordinal += 1;
                        visited += 1;
                        Ok(())
                    })?;
                    assert_eq!(ordinal, owner.source_slots.slots.len());
                }
                assert!(visited > 0);
                completed.set(true);
                Ok(())
            })
        });
        assert!(completed.get());
    }
    assert!(representations.into_iter().all(|count| count > 0));
}

#[test]
fn source_allocation_frames_keep_multiple_roots_and_repeated_helper_locals_distinct() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let completed = std::cell::Cell::new(false);
    with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
        source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
            let roots = original.source.root_count(budget)?;
            assert!(roots > 1);
            let mut all = Vec::new();
            for root in 0..roots {
                original.visit_allocation_frames_v32(root, budget, |row, _| {
                    assert_eq!(row.root(), root);
                    all.push(row);
                    Ok(())
                })?;
            }
            assert!(
                all.iter()
                    .any(|row| all.iter().any(|other| row.root() == other.root()
                        && row.instance() != other.instance()
                        && row.local() == other.local()))
            );
            for (index, row) in all.iter().enumerate() {
                assert!(
                    !all[..index]
                        .iter()
                        .any(|old| old.allocation() == row.allocation())
                );
            }
            completed.set(true);
            Ok(())
        })
    })
    .unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_allocation_frames_reject_wrong_root_and_preserve_sticky_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let reached = std::cell::Cell::new(false);
    let result =
        with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let root = original.source.root_count(budget)?;
                let error = original
                    .visit_allocation_frames_v32(root, budget, |_, _| {
                        panic!("foreign root reached visitor")
                    })
                    .unwrap_err();
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                ));
                let stopped = budget.work();
                assert!(
                    original
                        .visit_allocation_frames_v32(0, budget, |_, _| panic!(
                            "sticky refusal reached visitor"
                        ))
                        .is_err()
                );
                assert_eq!(budget.work(), stopped);
                reached.set(true);
                Err(error)
            })
        });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_allocation_frames_keep_callback_owned_index_credit_after_header_disposal() {
    run_production_optimized_consumer_v18(stored_physical_owner_v29, |original, _, budget| {
        source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
            let count = original.source.root_row(0)?.source_slots.slots.len();
            assert!(count > 0);
            let query_credit = allocation_query_credit_v39(original, count, budget)?;
            let floor = budget.storage();
            let mut rows = Vec::new();
            original.visit_allocation_frames_v32(0, budget, |row, budget| {
                if rows.capacity() == 0 {
                    rows = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
                }
                rows.push(row);
                Ok(())
            })?;
            assert_eq!(rows.len(), count);
            let credit = rows.capacity() * size_of::<ProductionSourceAllocationFrameV32>();
            assert_eq!(budget.storage(), floor + query_credit + credit);
            drop(rows);
            budget.release_storage(credit)?;
            assert_eq!(budget.storage(), floor + query_credit);
            Ok(())
        })
    });
}

// The visitor refunds its own fixed frame, not its caller-owned query phase.
// A direct census measures that separate charge without a visitor or callback.
fn allocation_query_credit_v39(
    original: &ProductionSourceCorrespondenceV18<'_>,
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let floor = budget.storage();
    let mut credit = 0;
    source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
        let query_floor = budget.storage();
        for ordinal in 0..count {
            let _ = original.allocation_frame_v32(0, ordinal, budget)?;
        }
        credit = budget.storage() - query_floor;
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    assert!(credit > 0);
    Ok(credit)
}

struct AllocationVisitorDropV32(std::rc::Rc<std::cell::Cell<usize>>);
impl Drop for AllocationVisitorDropV32 {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn source_allocation_frames_dispose_callback_and_headers_on_ordinary_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let drops = std::rc::Rc::new(std::cell::Cell::new(0));
    let completed = std::cell::Cell::new(false);
    let result =
        with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
            assert!(!original.source.root_row(0)?.source_slots.slots.is_empty());
            let captured = AllocationVisitorDropV32(drops.clone());
            let floor = budget.storage();
            let error = source_scalar_normalization_scratch_v18(
                original.source.cleanup,
                budget,
                0,
                |budget| {
                    original.visit_allocation_frames_v32(0, budget, move |_, _| {
                        let _ = &captured;
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "allocation visitor callback refusal",
                        ))
                    })
                },
            )
            .unwrap_err();
            assert_eq!(drops.get(), 1);
            assert_eq!(budget.storage(), floor);
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Binding("allocation visitor callback refusal")
            ));
            let before = (budget.work(), budget.storage());
            assert_eq!(
                original
                    .visit_allocation_frames_v32(0, budget, |_, _| panic!("sticky visitor called"))
                    .unwrap_err()
                    .to_string(),
                error.to_string()
            );
            assert_eq!((budget.work(), budget.storage()), before);
            completed.set(true);
            Err(error)
        });
    assert!(result.is_err());
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_allocation_frames_refuse_callback_loss_of_the_live_visitor_floor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let reached = std::cell::Cell::new(false);
    let completed = std::cell::Cell::new(false);
    let result =
        with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
            assert!(!original.source.root_row(0)?.source_slots.slots.is_empty());
            let error = original
                .visit_allocation_frames_v32(0, budget, |_, budget| {
                    budget.release_storage(1)?;
                    reached.set(true);
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(original.source.cleanup.is_denied());
            let before = (budget.work(), budget.storage());
            assert!(
                original
                    .visit_allocation_frames_v32(0, budget, |_, _| panic!(
                        "lost-floor visitor called"
                    ))
                    .is_err()
            );
            assert_eq!((budget.work(), budget.storage()), before);
            completed.set(true);
            Err(error)
        });
    assert!(result.is_err());
    assert!(completed.get());
    assert!(reached.get());
    assert!(budget.storage() > MODULE_FLOOR);
}

#[test]
fn source_allocation_frames_unwind_drops_callback_before_refunding_headers() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let drops = std::rc::Rc::new(std::cell::Cell::new(0));
    let reached = std::cell::Cell::new(false);
    let caught = std::cell::Cell::new(false);
    let result =
        with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
            // Query credit belongs to this scratch phase, not the visitor or
            // the outer optimizer's fixed retained-storage receipt.
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                assert!(!original.source.root_row(0)?.source_slots.slots.is_empty());
                let query_credit = allocation_query_credit_v39(original, 1, budget)?;
                let floor = budget.storage();
                let captured = AllocationVisitorDropV32(drops.clone());
                let reached = &reached;
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.visit_allocation_frames_v32(0, budget, move |_, _| {
                        let _ = &captured;
                        reached.set(true);
                        panic!("allocation visitor unwind")
                    })
                }))
                .unwrap_err();
                assert_eq!(
                    panic.downcast_ref::<&str>(),
                    Some(&"allocation visitor unwind")
                );
                assert_eq!(drops.get(), 1);
                assert_eq!(budget.storage(), floor + query_credit);
                assert!(!original.source.cleanup.is_denied());
                caught.set(true);
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "observed allocation visitor unwind",
                ))
            })
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Binding("observed allocation visitor unwind")
        ))
    ));
    assert!(caught.get());
    assert!(reached.get());
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn allocation_visitor_resource_cut_v32(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let observed = std::cell::Cell::new(None);
    let result =
        with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
            assert!(!original.source.root_row(0)?.source_slots.slots.is_empty());
            let floor = budget.storage();
            // Test-owned padding isolates this visitor's peak from earlier source
            // constructors, without changing either production resource limit.
            let padding = match cut {
                Some((false, remaining)) => MODULE_LIMIT - floor - remaining,
                _ => budget.peak_storage() + 1 - floor,
            };
            budget.reserve_storage(padding)?;
            assert_eq!(budget.peak_storage(), budget.storage());
            if let Some((true, remaining)) = cut {
                budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining)?;
            }
            let before = (budget.work(), budget.storage());
            let mut visited = 0;
            let result = source_scalar_normalization_scratch_v18(
                original.source.cleanup,
                budget,
                0,
                |budget| {
                    original.visit_allocation_frames_v32(0, budget, |_, _| {
                        visited += 1;
                        Ok(())
                    })
                },
            );
            assert_eq!(budget.storage(), before.1);
            let output = match result {
                Ok(()) => {
                    assert!(visited > 0);
                    observed.set(Some((
                        budget.work() - before.0,
                        budget.peak_storage() - before.1,
                        true,
                    )));
                    ProductionSourceOwnedViewErrorV18::Binding("allocation visitor resource stop")
                }
                Err(error) => {
                    match (cut, &error) {
                        (
                            Some((true, _)),
                            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                                _,
                            )),
                        )
                        | (
                            Some((false, _)),
                            ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Storage(_),
                            ),
                        ) => (),
                        _ => panic!("unexpected visitor resource boundary: {error:?}"),
                    }
                    let after = (budget.work(), budget.storage());
                    assert_eq!(
                        original
                            .visit_allocation_frames_v32(0, budget, |_, _| panic!(
                                "resource-refused visitor called"
                            ))
                            .unwrap_err()
                            .to_string(),
                        error.to_string()
                    );
                    assert_eq!((budget.work(), budget.storage()), after);
                    observed.set(Some((0, 0, false)));
                    error
                }
            };
            budget.release_storage(padding)?;
            assert_eq!(budget.storage(), floor);
            Err(output)
        });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
    observed.get().expect("visitor resource boundary reached")
}

#[test]
fn source_allocation_frames_exact_and_one_short_work_and_storage_are_sticky() {
    let (work, storage, passed) = allocation_visitor_resource_cut_v32(None);
    assert!(passed && work > 0 && storage > 0);
    assert!(allocation_visitor_resource_cut_v32(Some((true, work))).2);
    assert!(allocation_visitor_resource_cut_v32(Some((false, storage))).2);
    assert!(!allocation_visitor_resource_cut_v32(Some((true, work - 1))).2);
    assert!(!allocation_visitor_resource_cut_v32(Some((false, storage - 1))).2);
}
