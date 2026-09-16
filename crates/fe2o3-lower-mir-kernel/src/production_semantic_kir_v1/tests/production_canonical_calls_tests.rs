use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[path = "production_canonical_call_fixtures.rs"]
mod fixtures;

fn inventory(owner: &ProductionPreRankedKirOwnerV1) -> CanonicalKirInventoryV1<'_> {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    CanonicalKirInventoryV1::derive(owner.executable(), &mut budget)
        .unwrap()
        .0
}

fn inspect_batch(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    result: ArgumentCallResult,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    owner.with_checked_canonical_calls_v1(inventory, budget, |calls, budget| {
        assert!(calls.belongs_to(inventory));
        assert_eq!(calls.call_count(), 4);
        budget.charge_work(calls.call_count())?;
        for (root, index) in calls.sites() {
            let floor = budget.storage();
            let mut costs = [0; 2];
            for cost in &mut costs {
                let work = budget.work();
                calls.with_call(root, index, budget, |view| {
                    assert_eq!(view.caller().correspondence_owner(), root);
                    assert!(std::ptr::eq(
                        view.operation(),
                        inventory.calls()[index].operation
                    ));
                    inspect(view, result)
                })?;
                *cost = budget.work() - work;
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(costs[0], costs[1]);
            let other_root =
                SemanticFunctionIdV1::from_index(if root.index() == 1 { 2 } else { 1 });
            match calls.with_call(other_root, index, budget, |_| -> Result<(), _> {
                panic!("foreign root reached consumer")
            }) {
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch) => {}
                Err(error) => return Err(error),
                Ok(()) => panic!("foreign root accepted"),
            }
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    })
}

#[test]
fn canonical_calls_bind_actual_occurrences_for_packed_expanded_and_all_destinations() {
    for expanded in [false, true] {
        for result in [
            ArgumentCallResult::Zero,
            ArgumentCallResult::Scalar,
            ArgumentCallResult::Retained,
            ArgumentCallResult::Projected,
        ] {
            let owner = materialize(call_owner(expanded, result));
            let inventory = inventory(&owner);
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(23).unwrap();
            inspect_batch(&owner, &inventory, result, &mut budget).unwrap();
            assert_eq!(budget.storage(), 23);
        }
    }
}

#[test]
fn canonical_calls_follow_stored_blocks_not_sorted_semantic_ids() {
    let owner = materialize(transport_tests::diamond_owner_from(call_owner(
        false,
        ArgumentCallResult::Scalar,
    )));
    assert!(
        owner
            .correspondence
            .terminator_operation_spans
            .windows(2)
            .any(|pair| {
                pair[0].correspondence_owner == pair[1].correspondence_owner
                    && pair[0].semantic_function == pair[1].semantic_function
                    && pair[0].semantic_block > pair[1].semantic_block
            }),
        "fixture must retain nonmonotonic stored blocks"
    );
    let inventory = inventory(&owner);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    inspect_batch(&owner, &inventory, ArgumentCallResult::Scalar, &mut budget).unwrap();
}

#[test]
fn canonical_calls_exact_and_exhausted_budgets_preserve_the_incoming_floor() {
    let owner = materialize(call_owner(false, ArgumentCallResult::Scalar));
    let inventory = inventory(&owner);
    let run = |budget: &mut ArgumentBudgetV1<'_>| {
        inspect_batch(&owner, &inventory, ArgumentCallResult::Scalar, budget)
    };
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    run(&mut budget).unwrap();
    let required = (budget.work(), budget.peak_storage());
    for (work_limit, storage_limit, accepted) in [
        (required.0, required.1, true),
        (required.0 - 1, required.1, false),
        (required.0, required.1 - 1, false),
        (0, required.1, false),
        (required.0, 23, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        let result = run(&mut budget);
        if accepted {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
            assert!(run(&mut budget).is_err());
        }
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn canonical_calls_reject_foreign_inventory_and_missing_root_anchor_before_callback() {
    let mut owner = materialize(call_owner(false, ArgumentCallResult::Zero));
    let foreign = materialize(call_owner(false, ArgumentCallResult::Zero));
    let foreign_inventory = inventory(&foreign);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let reject = |_: &ProductionCanonicalCallsV1<'_>,
                  _: &mut ArgumentBudgetV1<'_>|
     -> Result<(), _> { panic!("invalid binding reached consumer") };
    assert!(matches!(
        owner.with_checked_canonical_calls_v1(&foreign_inventory, &mut budget, reject),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    owner.correspondence.call_returns = owner
        .correspondence
        .call_returns
        .iter()
        .copied()
        .filter(|row| {
            !(row.correspondence_owner.index() == 2
                && row.semantic_function.index() == 2
                && row.semantic_block.index() == 0)
        })
        .collect();
    let inventory = inventory(&owner);
    assert!(matches!(
        owner.with_checked_canonical_calls_v1(&inventory, &mut budget, reject),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn canonical_calls_callback_errors_unwinds_and_imbalances_restore_storage() {
    let owner = materialize(call_owner(false, ArgumentCallResult::Scalar));
    let inventory = inventory(&owner);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    let error: Result<(), _> =
        owner.with_checked_canonical_calls_v1(&inventory, &mut budget, |_, _| {
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        });
    assert!(matches!(
        error,
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), 23);
    for fail in [false, true] {
        let result = owner.with_checked_canonical_calls_v1(&inventory, &mut budget, |_, budget| {
            budget.reserve_storage(1)?;
            if fail {
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(budget.storage(), 23);
    }
    owner
        .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
            let (root, index) = calls.sites().next().unwrap();
            let floor = budget.storage();
            let error: Result<(), _> = calls.with_call(root, index, budget, |_| {
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            });
            assert!(matches!(
                error,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(budget.storage(), floor);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                calls.with_call(root, index, budget, |_| -> Result<(), _> {
                    std::panic::panic_any(219_u32)
                })
            }))
            .unwrap_err();
            assert_eq!(panic.downcast_ref::<u32>(), Some(&219));
            assert_eq!(budget.storage(), floor);
            calls.with_call(root, index, budget, |view| {
                inspect(view, ArgumentCallResult::Scalar)
            })
        })
        .unwrap();
    assert_eq!(budget.storage(), 23);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner.with_checked_canonical_calls_v1(&inventory, &mut budget, |_, _| -> Result<(), _> {
            std::panic::panic_any(220_u32)
        })
    }))
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<u32>(), Some(&220));
    assert_eq!(budget.storage(), 23);
}

#[test]
fn canonical_calls_keep_shared_outgoing_sites_root_qualified() {
    let mut owner = materialize(fixtures::shared_outgoing());
    let inventory = inventory(&owner);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
            assert_eq!(inventory.calls().len(), 5);
            assert_eq!(calls.call_count(), 6);
            assert_eq!(calls.function_count(), 6);
            let mut associations = Vec::new();
            for binding in calls.functions() {
                let source = binding.source();
                let expected = inventory
                    .function_for_name(source.kernel_ir_function().as_str(), budget)
                    .map_err(canonical_call_inventory_error_v1)?
                    .unwrap();
                assert!(std::ptr::eq(binding.canonical(), expected));
                assert_eq!(
                    source.role() == SemanticKirFunctionRoleV1::KernelEntry,
                    source.semantic_function() == source.correspondence_owner()
                );
                associations.push((
                    source.correspondence_owner().index(),
                    source.semantic_function().index(),
                ));
            }
            associations.sort();
            assert_eq!(
                associations,
                [(1, 0), (1, 1), (1, 3), (2, 0), (2, 2), (2, 3)]
            );
            let mut shared = [None; 2];
            for (root, index) in calls.sites() {
                calls.with_call(root, index, budget, |view| {
                    assert!(std::ptr::eq(
                        view.operation(),
                        inventory.calls()[index].operation
                    ));
                    if view.caller().semantic_function().index() == 0 {
                        assert_eq!(view.callee().association().semantic_function().index(), 3);
                        shared[root.index() as usize - 1] = Some(index);
                    }
                    Ok(())
                })?;
            }
            assert!(shared[0].is_some());
            assert_eq!(shared[0], shared[1]);
            Ok(())
        })
        .unwrap();
    drop(inventory);
    owner.correspondence.call_returns = owner
        .correspondence
        .call_returns
        .iter()
        .copied()
        .filter(|row| {
            !(row.correspondence_owner.index() == 2
                && row.semantic_function.index() == 0
                && row.semantic_block.index() == 0)
        })
        .collect();
    let inventory = self::inventory(&owner);
    assert!(matches!(
        owner.with_checked_canonical_calls_v1(&inventory, &mut budget, |_, _| -> Result<(), _> {
            panic!("missing shared-root anchor reached consumer")
        }),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn canonical_call_queries_do_not_rescan_unrelated_correspondence() {
    let mut measurements = Vec::new();
    for extra in [0, 128] {
        let owner = materialize(fixtures::unrelated_statements(extra));
        let inventory = inventory(&owner);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let query = owner
            .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
                let (root, index) = calls.sites().find(|(root, _)| root.index() == 1).unwrap();
                let start = budget.work();
                calls.with_call(root, index, budget, |view| {
                    inspect(view, ArgumentCallResult::Zero)
                })?;
                Ok(budget.work() - start)
            })
            .unwrap();
        measurements.push((owner.correspondence.statement_operation_spans.len(), query));
    }
    assert_eq!(measurements[1].0 - measurements[0].0, 128);
    assert_eq!(measurements[1].1, measurements[0].1);
}

#[test]
fn canonical_call_late_resource_failure_restores_the_live_batch_floor() {
    let owner = materialize(call_owner(false, ArgumentCallResult::Scalar));
    let inventory = inventory(&owner);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
            let (root, index) = calls.sites().next().unwrap();
            let floor = budget.storage();
            let start = budget.work();
            calls.with_call(root, index, budget, |view| {
                inspect(view, ArgumentCallResult::Scalar)
            })?;
            let cost = budget.work() - start;
            budget.charge_work(usize::MAX - budget.work() - (cost - 1))?;
            assert!(matches!(
                calls.with_call(root, index, budget, |view| inspect(
                    view,
                    ArgumentCallResult::Scalar
                )),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
            assert_eq!(
                budget.storage(),
                floor,
                "batch cleanup must not hide query scratch"
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}
