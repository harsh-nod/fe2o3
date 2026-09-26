use super::instance_layout_tests::layout_root_only_owner;
use super::*;

const LAYOUT_LIMIT: usize = 20_000_000;

macro_rules! layout_scope {
    (|$plan:ident, $root:ident, $budget:ident| $body:block) => {{
        let owner = layout_root_only_owner();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LAYOUT_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LAYOUT_LIMIT);
        budget.reserve_storage(37).unwrap();
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)
                .unwrap();
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            &owner,
            demands.types(&owner, &mut budget).unwrap(),
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut budget,
        )
        .unwrap();
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let floor = budget.storage();
                let result = source_storage_v29::with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |$plan, $root, $budget| $body,
                );
                assert!(budget.storage() >= floor);
                let extra = budget.storage() - floor;
                if layouts.permits_root_emission_refund(&owner, extra, budget) {
                    budget.release_storage(extra).unwrap();
                }
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
            },
        )
        .unwrap();
        let settled = layouts.permits_root_emission_refund(&owner, 0, &budget);
        let cleanup = layouts.release(&mut budget);
        if settled {
            demands.discard(&mut budget).unwrap();
            assert_eq!(budget.storage(), 37);
        } else {
            drop(demands);
            assert!(budget.storage() > 37);
        }
        let result = result.and(cleanup);
        let storage_denial = budget.failed_storage();
        drop(budget);
        (result, work.failed_work(), storage_denial)
    }};
}

fn independent_layout_headers<R>() -> usize {
    use std::mem::size_of;
    type Owner = ExecutionInstanceLayoutsV29<'static, 'static>;
    type Error = ProductionSemanticKirErrorV1;
    type Panic = Box<dyn std::any::Any + Send>;
    // The owner retains one C1 borrow, source stamp, ledger, slot/floor,
    // dense original-ID vector, original-call locator and backing-cell index,
    // not a second graph.
    let payload = size_of::<&SourceReferencePlanV29<'_, '_>>()
        + size_of::<ExecutionCallSourceV29>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 2 * size_of::<usize>()
        + size_of::<Vec<Option<ExecutionInstanceLayoutV29>>>()
        + size_of::<BTreeMap<(usize, u32), ProductionCallInstanceIdV1>>()
        + size_of::<BTreeMap<(usize, u32, u32), SourceBackingAllocationV29>>();
    let alignment = [
        std::mem::align_of::<&SourceReferencePlanV29<'_, '_>>(),
        std::mem::align_of::<ExecutionCallSourceV29>(),
        std::mem::align_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        std::mem::align_of::<usize>(),
        std::mem::align_of::<Vec<Option<ExecutionInstanceLayoutV29>>>(),
        std::mem::align_of::<BTreeMap<(usize, u32), ProductionCallInstanceIdV1>>(),
        std::mem::align_of::<BTreeMap<(usize, u32, u32), SourceBackingAllocationV29>>(),
    ]
    .into_iter()
    .max()
    .unwrap();
    let fields = payload.div_ceil(alignment) * alignment;
    assert_eq!(fields, size_of::<Owner>());
    // Protected disposal pays the same fixed four retries as the shared helper.
    let cleanup = size_of::<[Option<Panic>; 2]>()
        + size_of::<std::panic::AssertUnwindSafe<[Option<Panic>; 2]>>()
        + 2 * size_of::<Panic>()
        + size_of::<std::panic::AssertUnwindSafe<Panic>>()
        + 2 * size_of::<Result<(), Panic>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    fields
        + 2 * size_of::<Result<Owner, Error>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + size_of::<Result<Result<R, Error>, Panic>>()
        + size_of::<Result<R, Error>>()
        + size_of::<Option<Error>>()
        + size_of::<Result<Result<R, Error>, Panic>>()
        + size_of::<std::panic::AssertUnwindSafe<Result<Result<R, Error>, Panic>>>()
        + cleanup
}

#[test]
fn original_layout_owner_has_independent_exact_and_one_short_boundaries() {
    for cut in 0..3 {
        for prior in [false, true] {
            let completed = std::cell::Cell::new(false);
            let (outer, work_denial, storage_denial) = layout_scope!(|plan, _root, budget| {
                assert_eq!(plan.instances.instances().len(), 1);
                assert!(plan.storage_root.is_some());
                // Custody5, initial protected drop1 plus four retries,
                // concrete growth checkpoint8,
                // constructor custody5, roster1, Vec3, root row3, source stamp20.
                let exact_work = 5 + 5 + 8 + 5 + 1 + 3 + 3 + 20;
                let exact_storage = independent_layout_headers::<()>()
                    + std::mem::size_of::<Option<ExecutionInstanceLayoutV29>>();
                let remaining_work = exact_work - usize::from(cut == 1);
                let remaining_storage = exact_storage - usize::from(cut == 2);
                budget.charge_work(LAYOUT_LIMIT - budget.work() - remaining_work)?;
                let padding = LAYOUT_LIMIT - budget.storage() - remaining_storage;
                budget.reserve_storage(padding)?;
                let floor = budget.storage();
                let start = budget.work();
                if prior {
                    assert!(budget.charge_work(remaining_work + 23).is_err());
                    assert!(budget.reserve_storage(remaining_storage + 29).is_err());
                }
                let result =
                    with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                        assert_eq!(layouts.rows.len(), 1);
                        assert_eq!(layouts.rows.capacity(), 1);
                        assert!(layouts.rows[0].is_none());
                        assert!(layouts.calls.is_empty());
                        assert_eq!(budget.storage(), floor + exact_storage);
                        Ok(())
                    });
                match cut {
                    0 => {
                        result?;
                        assert_eq!(budget.work(), start + exact_work);
                    }
                    1 => {
                        assert!(
                            matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(error))) if error.actual() == LAYOUT_LIMIT + 1)
                        );
                        assert_eq!(
                            budget.work(),
                            start + exact_work - 20,
                            "the final source-stamp prepayment is atomic"
                        );
                    }
                    2 => {
                        assert!(
                            matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(error))) if error.actual() == LAYOUT_LIMIT + 1)
                        );
                        assert_eq!(
                            budget.work(),
                            start + 5 + 5 + 8 + 5 + 1 + 3,
                            "row backing is paid before initialization and source binding"
                        );
                    }
                    _ => unreachable!(),
                }
                assert_eq!(budget.storage(), floor);
                assert_eq!(
                    budget.peak_storage(),
                    if cut == 2 {
                        floor + independent_layout_headers::<()>()
                    } else {
                        LAYOUT_LIMIT
                    }
                );
                budget.release_storage(padding)?;
                completed.set(true);
                Ok(())
            });
            assert!(
                completed.get(),
                "a caught panic cannot satisfy the boundary test"
            );
            assert_eq!(outer.is_err(), cut != 0);
            assert_eq!(
                work_denial,
                if prior {
                    Some(LAYOUT_LIMIT + 23)
                } else if cut == 1 {
                    Some(LAYOUT_LIMIT + 1)
                } else {
                    None
                }
            );
            assert_eq!(
                storage_denial,
                if prior {
                    Some(LAYOUT_LIMIT + 29)
                } else if cut == 2 {
                    Some(LAYOUT_LIMIT + 1)
                } else {
                    None
                }
            );
        }
    }
}

#[test]
fn layout_owner_preserves_selected_callback_errors_panics_and_disposable_output_credit() {
    for panic in [false, true] {
        let completed = std::cell::Cell::new(false);
        let (outer, _, _) = layout_scope!(|plan, _root, budget| {
            let floor = budget.storage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_execution_instance_layouts_v29(plan, budget, |_, budget| {
                    budget.reserve_storage(13)?;
                    if panic {
                        std::panic::panic_any(1227usize);
                    }
                    Err::<(), _>(execution_instance_layout_error_v29())
                })
            }));
            if panic {
                assert_eq!(result.unwrap_err().downcast_ref::<usize>(), Some(&1227));
            } else {
                assert!(
                    matches!(result.unwrap(), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "physical layout differs from its original call instance")
                );
            }
            assert_eq!(
                budget.storage(),
                floor + 13,
                "the layout scope releases only its own receipt, not arbitrary callback output credit"
            );
            budget.release_storage(13)?;
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        outer.unwrap();
    }
}

#[test]
fn layout_query_has_an_inclusive_independent_work_boundary_and_sticky_failure() {
    for short in [false, true] {
        let completed = std::cell::Cell::new(false);
        let (outer, work_denial, storage_denial) = layout_scope!(|plan, _root, budget| {
            let floor = budget.storage();
            let result = with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                // Check custody once, then again before each erased-budget charge.
                let exact = 5 + (5 + 4) + (5 + 5);
                budget.charge_work(LAYOUT_LIMIT - budget.work() - exact + usize::from(short))?;
                let start = budget.work();
                let current = budget.storage();
                let peak = budget.peak_storage();
                let result = layouts.row(plan.root, budget);
                if short {
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error))) if error.actual() == LAYOUT_LIMIT + 1)
                    );
                    assert_eq!(budget.work(), start + exact - 5);
                } else {
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                        if detail == "physical layout differs from its original call instance")
                    );
                    assert_eq!(budget.work(), start + exact);
                }
                assert_eq!(budget.storage(), current);
                assert_eq!(budget.peak_storage(), peak);
                Ok(())
            });
            assert_eq!(
                result.is_err(),
                short,
                "ignored resource denial is selected at postflight"
            );
            assert_eq!(budget.storage(), floor);
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        assert_eq!(outer.is_err(), short);
        assert_eq!(work_denial, short.then_some(LAYOUT_LIMIT + 1));
        assert_eq!(storage_denial, None);
    }
}

struct RejectedLayoutOutput<'a>(&'a std::cell::Cell<usize>);
impl Drop for RejectedLayoutOutput<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        std::panic::panic_any(1228usize);
    }
}

#[test]
fn caught_layout_query_failure_and_rejected_output_drop_cannot_become_success() {
    for lost_floor in [false, true] {
        let completed = std::cell::Cell::new(false);
        let dropped = std::cell::Cell::new(0);
        let (outer, _, _) = layout_scope!(|plan, root, budget| {
            let floor = budget.storage();
            let mut observed = 0;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                    observed = budget.storage();
                    if lost_floor {
                        budget.release_storage(1)?;
                        assert!(layouts.row(plan.root, budget).is_err());
                        budget.reserve_storage(1)?;
                    } else {
                        plan.failure.record_resource(ArgumentResourceV1::Arithmetic);
                        assert!(layouts.row(plan.root, budget).is_err());
                    }
                    Ok(RejectedLayoutOutput(&dropped))
                })
            }));
            let result = outcome.expect("the rejected destructor is protected");
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                if error == if lost_floor { ArgumentResourceV1::Accounting } else { ArgumentResourceV1::Arithmetic })
            );
            assert_eq!(dropped.get(), 1);
            assert!(observed > floor);
            assert_eq!(
                budget.storage(),
                observed,
                "denied cleanup does not refund paid owner envelopes"
            );
            assert!(!root.custody_view().retains_custody(plan.instances, &plan.failure, budget));
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        assert!(outer.is_err());
    }
}

#[test]
fn captured_root_growth_survives_layout_scope_and_nested_layout_owners() {
    let completed = std::cell::Cell::new(false);
    let (outer, _, _) = layout_scope!(|plan, root, budget| {
        let floor = budget.storage();
        let mut growth = 0;
        with_execution_instance_layouts_v29(plan, budget, |_, budget| {
            with_execution_instance_layouts_v29(plan, budget, |_, budget| {
                let before = budget.storage();
                let state = root
                    .new_state(plan.root, SemanticLocalIdV1::from_index(1), budget)
                    .unwrap();
                let _copy = root.copy_state(state, budget).unwrap();
                growth = budget.storage() - before;
                assert!(growth > 0);
                Ok(())
            })
        })?;
        assert_eq!(budget.storage(), floor + growth);
        assert!(root.custody_view().retains_custody(plan.instances, &plan.failure, budget));
        assert!(root.custody_view().retains_after_refund(0, budget));
        completed.set(true);
        Ok(())
    });
    assert!(completed.get());
    outer.unwrap();
}

#[test]
fn layout_owner_preserves_earlier_query_error_and_callback_error_before_cleanup_loss() {
    for earlier in [false, true] {
        let completed = std::cell::Cell::new(false);
        let (outer, _, _) = layout_scope!(|plan, _root, budget| {
            let floor = budget.storage();
            let result = with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                if earlier {
                    plan.failure.record_resource(ArgumentResourceV1::Arithmetic);
                    let work = budget.work();
                    assert!(matches!(
                        layouts.row(plan.root, budget),
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Arithmetic
                            )
                        )
                    ));
                    assert_eq!(budget.work(), work, "the first failure stops the retry");
                } else {
                    budget.release_storage(1)?;
                }
                Err::<(), _>(execution_instance_layout_error_v29())
            });
            if earlier {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Arithmetic
                        )
                    )
                ));
                assert_eq!(budget.storage(), floor);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "physical layout differs from its original call instance")
                );
                assert!(budget.storage() > floor, "no refund after owner loss");
            }
            completed.set(true);
            result.map_err(Into::into)
        });
        assert!(completed.get());
        if earlier {
            assert!(matches!(
                outer,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Arithmetic
                    )
                )
            ));
        } else {
            assert!(
                matches!(outer, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "physical layout differs from its original call instance")
            );
        }
    }
}

#[test]
fn generic_rejected_output_envelopes_have_independent_large_result_storage() {
    for short in [false, true] {
        let completed = std::cell::Cell::new(false);
        let entered = std::cell::Cell::new(false);
        let (outer, _, denial) = layout_scope!(|plan, _root, budget| {
            type Large = [u8; 4096];
            let headers = independent_layout_headers::<Large>();
            assert!(
                headers > independent_layout_headers::<()>(),
                "generic output cannot use unit cleanup headers"
            );
            assert_eq!(execution_instance_layout_headers_v29::<Large>()?, headers);
            let row = std::mem::size_of::<Option<ExecutionInstanceLayoutV29>>();
            let remaining = if short { headers - 1 } else { headers + row };
            let padding = LAYOUT_LIMIT - budget.storage() - remaining;
            budget.reserve_storage(padding)?;
            let floor = budget.storage();
            let work = budget.work();
            let result = with_execution_instance_layouts_v29(plan, budget, |_, budget| {
                entered.set(true);
                assert_eq!(budget.storage(), floor + headers + row);
                Ok([9u8; 4096])
            });
            if short {
                assert!(!entered.get());
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error))) if error.actual() == LAYOUT_LIMIT + 1)
                );
                assert_eq!(
                    budget.work(),
                    work + 5 + 5,
                    "only original custody and protected-drop prepayment precede the header"
                );
            } else {
                assert_eq!(result?, [9u8; 4096]);
                assert!(entered.get());
                assert_eq!(budget.work(), work + 5 + 5 + 8 + 5 + 1 + 3 + 3 + 20);
            }
            assert_eq!(budget.storage(), floor);
            budget.release_storage(padding)?;
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        assert_eq!(outer.is_err(), short);
        assert_eq!(denial, short.then_some(LAYOUT_LIMIT + 1));
    }
}
