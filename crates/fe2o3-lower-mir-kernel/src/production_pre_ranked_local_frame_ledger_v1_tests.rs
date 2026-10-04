use super::*;

const INNER_PANIC: usize = 0x3671;

fn replace_inner_budget<'w>(
    view: &mut ProductionCallViewV1<'_, 'w>,
    other: &mut ArgumentBudgetV1<'w>,
    recorded: &mut Option<(usize, usize, ArgumentLedgerV1)>,
    surplus: usize,
    mode: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Private view access deliberately exercises cleanup below the public scopes.
    let active = view.entry.budget.storage();
    *recorded = Some((
        active,
        view.entry.budget.work(),
        view.entry.budget.work_ledger_identity_v1(),
    ));
    other.reserve_storage(active + surplus)?;
    let slot = view.entry.budget as *const ArgumentBudgetV1<'_>;
    std::mem::swap(view.entry.budget, other);
    assert_eq!(view.entry.budget as *const ArgumentBudgetV1<'_>, slot);
    match mode {
        0 => Ok(()),
        1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
        _ => std::panic::panic_any(INNER_PANIC),
    }
}

#[test]
fn private_inner_view_replacement_does_not_debit_parameter_or_outer_foreign_storage() {
    for joined in [false, true] {
        for surplus in [0, 7] {
            for mode in 0..3 {
                let mut original_work = Work::new(usize::MAX);
                let mut replacement_work = Work::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut original_work, usize::MAX);
                let mut other = ArgumentBudgetV1::new(&mut replacement_work, usize::MAX);
                budget.reserve_storage(FLOOR).unwrap();
                let owner = materialize(Fixture::Literal(true), true, &mut budget);
                let retained = owner.retained_analysis_storage_v1();
                budget.reserve_storage(retained).unwrap();
                let (inventory, receipt) =
                    CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();
                let mut recorded = None;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if joined {
                        owner.with_checked_canonical_calls_v1(
                            &inventory,
                            &mut budget,
                            |calls, budget| {
                                let (root, call) = calls.sites().next().unwrap();
                                calls.with_call_local_frame_v1(root, call, budget, |view, local| {
                                    assert!(local.is_none());
                                    replace_inner_budget(
                                        view,
                                        &mut other,
                                        &mut recorded,
                                        surplus,
                                        mode,
                                    )
                                })
                            },
                        )
                    } else {
                        let source = owner
                            .correspondence
                            .call_returns
                            .iter()
                            .find(|row| {
                                matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. })
                            })
                            .unwrap();
                        owner.with_checked_call_v1(
                            source.correspondence_owner,
                            source.semantic_function,
                            source.semantic_block,
                            &mut budget,
                            |view| {
                                replace_inner_budget(view, &mut other, &mut recorded, surplus, mode)
                            },
                        )
                    }
                }));
                let (active, prefix, original_ledger) = recorded.expect("genuine view entered");
                assert!(active > floor, "actual call and parameter scratch was live");
                if !joined && mode == 2 {
                    // The legacy owner/parameter scopes retain Result-only cleanup.
                    let panic = result.unwrap_err();
                    assert_eq!(panic.downcast_ref::<usize>().copied(), Some(INNER_PANIC));
                } else {
                    assert!(accounting(
                        result
                            .expect("joined Accounting must suppress the panic")
                            .unwrap_err()
                    ));
                }
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (active + surplus, active + surplus)
                );
                assert_eq!(budget.failed_storage(), None);
                assert_eq!(budget.work(), 0, "all cleanup checks were prepaid");
                assert_eq!((other.storage(), other.work()), (active, prefix));
                assert!(other.work_ledger_identity_v1() == original_ledger);
                assert!(budget.work_ledger_identity_v1() != original_ledger);

                // The test, not a production scope, restores the displaced ledger.
                std::mem::swap(&mut budget, &mut other);
                budget.release_storage(active - floor).unwrap();
                other.release_storage(active + surplus).unwrap();
                owner
                    .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
                        let (root, call) = calls.sites().next().unwrap();
                        calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                            assert!(local.is_none());
                            Ok(())
                        })
                    })
                    .unwrap();
                assert_eq!((budget.storage(), other.storage()), (floor, 0));
                drop(inventory);
                budget.release_storage(receipt.retained_storage()).unwrap();
                drop(owner);
                budget.release_storage(retained).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
}

#[test]
fn canonical_scratch_identity_check_has_prepaid_two_one_boundary() {
    for limit in [1, 2] {
        let mut work = Work::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            entered = true;
            budget.reserve_storage(7)?;
            Ok(())
        });
        if limit == 1 {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error)
                )) if error.actual() == 2 && error.limit() == 1
            ));
            assert!(!entered);
            assert_eq!((budget.work(), budget.peak_storage()), (0, FLOOR));
        } else {
            result.unwrap();
            assert!(entered);
            assert_eq!((budget.work(), budget.peak_storage()), (2, FLOOR + 7));
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn helper_getter_identity_check_has_five_four_boundary_before_lookup() {
    with_source(true, |owner, inventory, budget| {
        let helper = owner
            .correspondence
            .lowered_functions
            .iter()
            .position(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            .unwrap();
        owner
            .with_checked_helper_memory_v1(inventory, budget, |memory, budget| {
                for limit in [4, 5] {
                    let mut work = Work::new(limit);
                    let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                    let floor = budget.storage();
                    foreign.reserve_storage(floor)?;
                    let result = memory.local_frame(helper, &mut foreign);
                    if limit == 4 {
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(error)
                            )) if error.actual() == 5 && error.limit() == 4
                        ));
                        assert_eq!(foreign.work(), 0);
                    } else {
                        assert!(accounting(result.err().unwrap()));
                        assert_eq!(foreign.work(), 5);
                    }
                    assert_eq!((foreign.storage(), foreign.peak_storage()), (floor, floor));
                    assert_eq!(foreign.failed_storage(), None);
                }
                assert!(memory.local_frame(helper, budget)?.is_none());
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn calls_identity_header_and_first_preparation_charge_have_literal_boundaries() {
    with_source(true, |owner, inventory, original| {
        let rows = &owner.correspondence;
        let count = rows.lowered_functions.len();
        let first = 6 * count
            + rows.call_returns.len()
            + rows.terminator_operation_spans.len()
            + rows.parameter_bindings.len()
            + rows.parameter_component_bindings.len()
            + rows.ignored_parameter_bindings.len();
        let header = std::mem::size_of::<ProductionCanonicalCallsV1<'_>>();
        assert_eq!(
            header,
            5 * std::mem::size_of::<usize>() + 2 * std::mem::size_of::<Vec<()>>()
        );
        let reserved = header
            + count * std::mem::size_of::<CanonicalCallGroupV1<'_>>()
            + rows.call_returns.len() * std::mem::size_of::<CanonicalCallBindingV1<'_>>();
        // Scratch capture/postflight2, inventory identity1, exact index census.
        let prefix = 2 + 1 + first;
        let floor = original.storage();
        for storage_failure in [false, true] {
            let limit = if storage_failure { prefix } else { prefix - 1 };
            let storage = if storage_failure {
                floor + reserved - 1
            } else {
                usize::MAX
            };
            let mut work = Work::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage);
            budget.reserve_storage(floor).unwrap();
            let mut entered = false;
            let result = owner.with_checked_canonical_calls_v1(inventory, &mut budget, |_, _| {
                entered = true;
                Ok(())
            });
            assert!(!entered);
            if storage_failure {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
                assert_eq!(budget.work(), prefix);
                assert_eq!(budget.failed_storage(), Some(floor + reserved));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error)
                    )) if error.actual() == prefix && error.limit() == prefix - 1
                ));
                assert_eq!(budget.work(), 3);
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
        }
    });
}
