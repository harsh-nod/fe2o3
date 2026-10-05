use super::layout_tests::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

#[test]
fn original_recursive_staging_panic_refunds_unpublished_payload_exactly_once() {
    instances(|instances, budget| {
        let floor = budget.storage();
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[], budget).unwrap();
        let retained = layouts.lease.persistent.get();
        let credit = layouts
            .capture_emission_credit(instances.owner(), budget)
            .unwrap();
        let reached = std::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                let layouts = root.arena.layouts;
                let mut graph = SourceStorageOriginalGraphV29::default();
                graph.discover(layouts, RowKey::ty(RECURSIVE), budget)?;
                graph.containment(layouts, budget)?;
                let components = graph.components(layouts, budget)?;
                assert_eq!(components.ranges, [0..2]);
                let mut order = layouts.lease.vector(2, budget)?;
                for &node in &components.members {
                    order.push((graph.nodes[node].depth, graph.nodes[node].key, node));
                }
                sort_rows(&mut order, &layouts.lease, budget)?;
                let mut staged = layouts.lease.vector(2, budget)?;
                layouts.original_stage_group(&mut graph, &order, &mut staged, budget)?;
                assert_eq!(
                    staged.iter().map(|row| row.backing).sum::<usize>(),
                    size_of::<StorageFieldV1>()
                );
                assert_eq!(layouts.lease.persistent.get(), retained);
                assert!(layouts.physical.borrow().rows.is_empty());
                reached.set(true);
                panic!("after source-bound staging, before persistent publication");
                #[allow(unreachable_code)]
                Ok(())
            })
        }));
        assert!(reached.get());
        let error = result
            .expect("source scope must consume its staging callback panic")
            .unwrap_err();
        assert!(matches!(
            error,
            Error::Unsupported {
                detail: "source reference callback panicked",
                ..
            }
        ));
        assert_eq!(layouts.lease.persistent.get(), retained);
        assert!(layouts.physical.borrow().rows.is_empty());
        assert!(layouts.physical.borrow().original_extensions.is_empty());
        let scratch = credit.into_root_credit(&layouts, budget).unwrap();
        assert_eq!(scratch, SourceStorageEmissionCreditV29::headers().unwrap());
        assert!(layouts.permits_root_emission_refund(instances.owner(), scratch, budget));
        budget.release_storage(scratch).unwrap();
        assert_eq!(layouts.lease.owned.get(), layouts.lease.persistent.get());
        assert!(layouts.release(budget).is_err());
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn original_recursive_closure_keeps_published_credit_across_roots_and_callback_exits() {
    for exit in 0..3 {
        instances(|instances, budget| {
            let floor = budget.storage();
            budget.reserve_storage(269).unwrap();
            let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[], budget).unwrap();
            let mut previous = None;
            for round in 0..2 {
                let entry = budget.storage();
                let retained = layouts.lease.persistent.get();
                let credit = layouts
                    .capture_emission_credit(instances.owner(), budget)
                    .unwrap();
                let reached = std::cell::Cell::new(false);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_source_storage_root_v29(
                        &mut layouts,
                        instances,
                        budget,
                        |_, root, budget| {
                            let schema = root.arena.layouts.select_original_closure(
                                instances.owner(),
                                RECURSIVE,
                                budget,
                            )?;
                            if let Some(previous) = previous {
                                assert_eq!(previous, schema);
                            }
                            budget.reserve_storage(19)?;
                            reached.set(true);
                            if round == 1 && exit == 1 {
                                return Err(error("after original recursive publication").into());
                            }
                            if round == 1 && exit == 2 {
                                panic!("after original recursive publication");
                            }
                            Ok(schema)
                        },
                    )
                }));
                assert!(reached.get());
                let result = result.expect("source scope must consume the callback panic");
                if round == 1 && exit != 0 {
                    assert!(result.is_err());
                } else {
                    previous = Some(result.unwrap());
                }
                assert_eq!(layouts.physical.borrow().rows.len(), 2);
                assert_eq!(layouts.physical.borrow().original_extensions.len(), 2);
                if round == 1 {
                    assert_eq!(layouts.lease.persistent.get(), retained);
                }
                let scratch = credit.into_root_credit(&layouts, budget).unwrap();
                assert_eq!(
                    scratch,
                    SourceStorageEmissionCreditV29::headers().unwrap() + 19
                );
                assert!(layouts.permits_root_emission_refund(instances.owner(), scratch, budget));
                budget.release_storage(scratch).unwrap();
                assert_eq!(
                    budget.storage(),
                    entry + layouts.lease.persistent.get() - retained
                );
                assert_eq!(layouts.lease.owned.get(), layouts.lease.persistent.get());
            }
            assert_eq!(layouts.release(budget).is_err(), exit != 0);
            assert_eq!(budget.storage(), floor + 269);
            budget.release_storage(269).unwrap();
        });
    }
}

#[test]
fn original_recursive_group_refusal_drops_staged_payload_but_keeps_prior_live_rows() {
    instances(|instances, budget| {
        let floor = budget.storage();
        let mut limits = ProductionSemanticKirLimitsV1::default().storage_layout_limits();
        limits.rows = 2;
        let mut layouts =
            SourceStorageLayoutsV29::new_with_limits(instances.owner(), &[], limits, budget)
                .unwrap();
        let credit = layouts
            .capture_emission_credit(instances.owner(), budget)
            .unwrap();
        let retained = std::cell::Cell::new(0);
        let result =
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                root.arena
                    .layouts
                    .select_original_leaf_schema(instances.owner(), WORD, budget)?;
                retained.set(root.arena.layouts.lease.persistent.get());
                Ok(root.arena.layouts.select_original_closure(
                    instances.owner(),
                    RECURSIVE,
                    budget,
                )?)
            });
        assert!(result.is_err());
        assert_eq!(layouts.lease.persistent.get(), retained.get());
        assert_eq!(layouts.physical.borrow().rows.len(), 1);
        assert!(layouts.physical.borrow().original_extensions.is_empty());
        let scratch = credit.into_root_credit(&layouts, budget).unwrap();
        assert_eq!(scratch, SourceStorageEmissionCreditV29::headers().unwrap());
        assert!(layouts.permits_root_emission_refund(instances.owner(), scratch, budget));
        budget.release_storage(scratch).unwrap();
        assert_eq!(layouts.lease.owned.get(), layouts.lease.persistent.get());
        assert!(layouts.release(budget).is_err());
        assert_eq!(budget.storage(), floor);
    });
}

pub(super) fn instances<R>(
    consume: impl FnOnce(&ExecutionInstancesV29<'_>, &mut Budget<'_>) -> R,
) -> R {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consume(
                instances, budget,
            ))
        },
    )
    .unwrap()
}

fn is_accounting(error: &Error) -> bool {
    matches!(
        error,
        Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)
    )
}

#[test]
fn root_emission_credit_excludes_persistent_growth_across_repeated_roots() {
    instances(|instances, budget| {
        let before = budget.storage();
        let mut layouts =
            SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, DESCRIPTOR], budget).unwrap();
        let mut previous = None;
        for output in [17, 23] {
            let entry = budget.storage();
            let original = layouts.lease.persistent.get();
            let credit = layouts
                .capture_emission_credit(instances.owner(), budget)
                .unwrap();
            let schema =
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    let element = root
                        .arena
                        .layouts
                        .row_for(instances.owner(), WORD, budget)?;
                    let id = root.arena.layouts.select_schema(
                        instances.owner(),
                        DESCRIPTOR,
                        SourceStorageSelectionV29::Slice {
                            element,
                            value_space: AddressSpace::Global,
                            access: AccessMode::ReadOnly,
                        },
                        budget,
                    )?;
                    budget.reserve_storage(output)?;
                    Ok(id)
                })
                .unwrap();
            if let Some(previous) = previous {
                assert_eq!(schema, previous);
                assert_eq!(layouts.lease.persistent.get(), original);
            }
            previous = Some(schema);
            let headers = SourceStorageEmissionCreditV29::headers().unwrap();
            let scratch = credit.into_root_credit(&layouts, budget).unwrap();
            assert_eq!(scratch, headers + output);
            assert!(layouts.permits_root_emission_refund(instances.owner(), scratch, budget));
            budget.release_storage(scratch).unwrap();
            assert_eq!(
                budget.storage(),
                entry + layouts.lease.persistent.get() - original
            );
            assert_eq!(layouts.lease.owned.get(), layouts.lease.persistent.get());
        }
        layouts.release(budget).unwrap();
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn root_emission_credit_keeps_partial_schema_rows_on_error_and_panic() {
    for panic_after_schema in [false, true] {
        instances(|instances, budget| {
            let before = budget.storage();
            let mut layouts = SourceStorageLayoutsV29::new_with_limits(
                instances.owner(),
                &[PAIR, DESCRIPTOR],
                fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                    rows: if panic_after_schema { 8 } else { 7 },
                    edges: 10,
                    containment_depth: 2,
                    object_bytes: 16,
                },
                budget,
            )
            .unwrap();
            let original = layouts.lease.persistent.get();
            let credit = layouts
                .capture_emission_credit(instances.owner(), budget)
                .unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    budget.reserve_storage(31)?;
                    let element = root
                        .arena
                        .layouts
                        .row_for(instances.owner(), WORD, budget)?;
                    let _ = root.arena.layouts.select_schema(
                        instances.owner(),
                        DESCRIPTOR,
                        SourceStorageSelectionV29::Slice {
                            element,
                            value_space: AddressSpace::Global,
                            access: AccessMode::ReadOnly,
                        },
                        budget,
                    )?;
                    if panic_after_schema {
                        panic!("after persistent schema publication");
                    }
                    Ok(())
                })
            }));
            assert!(layouts.lease.persistent.get() > original);
            let first = layouts.lease.failure.observation();
            let failure = result
                .expect("source scope must consume its callback panic")
                .unwrap_err();
            if panic_after_schema {
                assert!(matches!(
                    failure,
                    Error::Unsupported {
                        detail: "source reference callback panicked",
                        ..
                    }
                ));
            }
            let scratch = credit.into_root_credit(&layouts, budget).unwrap();
            assert_eq!(
                scratch,
                SourceStorageEmissionCreditV29::headers().unwrap() + 31
            );
            assert!(layouts.permits_root_emission_refund(instances.owner(), scratch, budget));
            budget.release_storage(scratch).unwrap();
            assert_eq!(layouts.lease.owned.get(), layouts.lease.persistent.get());
            assert_eq!(
                format!("{:?}", layouts.lease.failure.observation()),
                format!("{first:?}")
            );
            assert!(layouts.release(budget).is_err());
            assert_eq!(budget.storage(), before);
        });
    }
}

#[test]
fn root_emission_credit_capture_has_independent_header_and_work_bounds() {
    #[allow(dead_code)]
    struct Fields {
        layouts: usize,
        source: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        slot: usize,
        entry: usize,
        persistent: usize,
    }
    let headers =
        size_of::<Fields>() + size_of::<Option<Fields>>() + size_of::<Result<Fields, Error>>();
    assert_eq!(
        size_of::<Fields>(),
        size_of::<SourceStorageEmissionCreditV29>()
    );
    assert_eq!(headers, SourceStorageEmissionCreditV29::headers().unwrap());
    for short in [false, true] {
        let owner = owner();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(113).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[WORD], &mut budget).unwrap();
        // One owner check plus twelve prepaid capture/cleanup groups.
        budget
            .charge_work(usize::MAX - budget.work() - (13 - usize::from(short)))
            .unwrap();
        let before = budget.storage();
        let result = layouts.capture_emission_credit(&owner, &mut budget);
        if short {
            assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
            assert_eq!(budget.storage(), before);
            assert!(layouts.release(&mut budget).is_err());
        } else {
            let credit = result.unwrap().into_root_credit(&layouts, &budget).unwrap();
            assert_eq!(credit, headers);
            assert!(layouts.permits_root_emission_refund(&owner, credit, &budget));
            budget.release_storage(credit).unwrap();
            layouts.release(&mut budget).unwrap();
        }
        assert_eq!(budget.storage(), 113);
    }
}

#[test]
fn root_emission_credit_capture_preserves_exact_storage_and_prior_work_history() {
    const LIMIT: usize = 64 * 1024 * 1024;
    for short in [false, true] {
        let owner = owner();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(131).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[WORD], &mut budget).unwrap();
        let headers = SourceStorageEmissionCreditV29::headers().unwrap();
        let pressure = LIMIT - budget.storage() - headers + usize::from(short);
        budget.reserve_storage(pressure).unwrap();
        assert!(matches!(
            budget.charge_work(usize::MAX),
            Err(ArgumentResourceV1::Work(_))
        ));
        let before = budget.storage();
        let result = layouts.capture_emission_credit(&owner, &mut budget);
        if short {
            assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                ))
            ));
            assert_eq!(budget.storage(), before);
            assert!(layouts.release(&mut budget).is_err());
        } else {
            let credit = result.unwrap();
            assert_eq!(credit.root_credit(&layouts, &budget), Some(headers));
            assert_eq!(credit.into_root_credit(&layouts, &budget), Some(headers));
            assert!(layouts.permits_root_emission_refund(&owner, headers, &budget));
            budget.release_storage(headers).unwrap();
            layouts.release(&mut budget).unwrap();
        }
        assert_eq!(budget.storage(), 131 + pressure);
        budget.release_storage(pressure).unwrap();
        assert_eq!(budget.storage(), 131);
        drop(budget);
        assert_eq!(work.failed_work(), Some(usize::MAX));
    }
}

#[test]
fn source_storage_root_success_restores_exact_table_credit_and_preserves_output_credit() {
    instances(|instances, budget| {
        let before = budget.storage();
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
        let table = budget.storage();
        let owned = layouts.lease.owned.get();
        for _ in 0..2 {
            let value = with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, root, budget| {
                    assert!(plan.storage_root.is_some());
                    let checkpoint = root.arena.layouts.lease.root.get().unwrap();
                    let retained = checkpoint.retained.unwrap();
                    let plan_bytes = retained.floor - table - retained.delta;
                    assert!(plan_bytes > 0 && retained.delta > 0);
                    let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
                    let path = root.root_path(PAIR, budget)?;
                    root.mutate(
                        state,
                        path,
                        SourceStorageRootMutationV29::Initialize,
                        budget,
                    )?;
                    assert!(root.is_initialized(state, path, budget)?);
                    let final_delta = root.arena.layouts.lease.owned.get() - owned;
                    assert!(final_delta > retained.delta);
                    assert_eq!(budget.storage(), table + plan_bytes + final_delta);
                    budget.reserve_storage(37)?;
                    Ok(23_u32)
                },
            )
            .unwrap();
            assert_eq!(value, 23);
            assert_eq!(layouts.lease.owned.get(), owned);
            assert!(layouts.lease.root.get().is_none());
            assert_eq!(budget.storage(), table + 37);
            budget.release_storage(37).unwrap();
        }
        layouts.release(budget).unwrap();
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn source_storage_module_table_survives_successive_original_instance_scopes() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let mut layouts = SourceStorageLayoutsV29::new(&owner, &[PAIR], &mut budget).unwrap();
    let owned = layouts.lease.owned.get();
    let rows = layouts.rows(&owner, &mut budget).unwrap().as_ptr();
    for _ in 0..2 {
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let before = budget.storage();
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
                    let path = root.root_path(PAIR, budget)?;
                    root.mutate(
                        state,
                        path,
                        SourceStorageRootMutationV29::Initialize,
                        budget,
                    )?;
                    Ok(())
                })
                .unwrap();
                assert_eq!(budget.storage(), before);
                assert_eq!(layouts.lease.owned.get(), owned);
                assert_eq!(layouts.rows(&owner, budget).unwrap().as_ptr(), rows);
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
    }
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_storage_root_ignored_semantic_mutation_failure_poison_is_shared_with_c1() {
    instances(|instances, budget| {
        let before = budget.storage();
        let mut layouts =
            SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, WORD], budget).unwrap();
        let table = budget.storage();
        let mut expected = None;
        let result = with_source_storage_root_v29(&mut layouts, instances, budget, |plan, root, budget| {
            let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
            let wrong = root.root_path(WORD, budget)?;
            assert!(root.mutate(state, wrong, SourceStorageRootMutationV29::Initialize, budget).is_err());
            let first = plan.failure.first_error().unwrap();
            let Error::Unsupported { detail, .. } = first else { panic!("semantic error lost") };
            expected = Some(detail);
            let work = budget.work();
            assert!(matches!(plan.check_owner(instances, budget), Err(Error::Unsupported { detail: found, .. }) if found == detail));
            assert!(root.root_path(PAIR, budget).is_err());
            assert_eq!(budget.work(), work);
            Ok(())
        }).unwrap_err();
        assert!(matches!(result, Error::Unsupported { detail, .. } if Some(detail) == expected));
        assert_eq!(budget.storage(), table);
        let work = budget.work();
        let second: Result<(), Error> =
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, _, _| {
                panic!("poisoned table reached a second root")
            });
        assert!(second.is_err());
        assert_eq!(budget.work(), work);
        assert!(layouts.release(budget).is_err());
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn source_storage_root_original_owned_error_survives_observations_and_later_denial() {
    instances(|instances, budget| {
        for propagated in [false, true] {
            let before = budget.storage();
            let mut retained = Vec::with_capacity(2);
            retained.push((17_u32, 23_u32, "original owned diagnostic"));
            let pointer = retained.as_ptr();
            let bytes = retained.capacity() * size_of::<(u32, u32, &'static str)>();
            budget.reserve_storage(bytes).unwrap();
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
            let table = budget.storage();
            let error = Error::RetainedLocalStorage {
                function: 31,
                retained_locals: retained,
                retained_count: 47,
            };
            let result = with_source_storage_root_v29(&mut layouts, instances, budget, |plan, root, budget| {
                let denied: Result<(), _> = root.arena.record(Err(error));
                assert!(denied.is_err());
                let work = budget.work();
                let observation = plan.check_owner(instances, budget).unwrap_err();
                assert!(matches!(observation, Error::Unsupported { detail: "source storage root is poisoned by an earlier owned diagnostic", .. }));
                assert!(root.root_path(PAIR, budget).is_err());
                let later = budget.reserve_storage(usize::MAX).unwrap_err();
                plan.failure.record_resource(later);
                assert_eq!(budget.work(), work);
                if propagated { return Err(observation.into()); }
                Ok(())
            }).unwrap_err();
            let Error::RetainedLocalStorage {
                function,
                retained_locals,
                retained_count,
            } = result
            else {
                panic!("owned original replaced")
            };
            assert_eq!((function, retained_count), (31, 47));
            assert_eq!(retained_locals.as_ptr(), pointer);
            assert_eq!(retained_locals, [(17, 23, "original owned diagnostic")]);
            assert_eq!(budget.storage(), table);
            assert!(layouts.lease.failure.first_error().is_some());
            drop(retained_locals);
            assert!(layouts.release(budget).is_err());
            assert_eq!(budget.storage(), before + bytes);
            budget.release_storage(bytes).unwrap();
        }
    });
}

#[test]
fn source_storage_root_callback_error_and_panic_clean_scratch_without_losing_diagnostic() {
    instances(|instances, budget| {
        for panic in [false, true] {
            let before = budget.storage();
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
            let table = budget.storage();
            let result: Result<(), Error> =
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    let _ = root.new_state(instances.root(), local_for(PAIR), budget)?;
                    if panic {
                        panic!("root callback panic")
                    }
                    Err(error("root callback selected error").into())
                });
            let result = result.unwrap_err();
            assert!(
                matches!(result, Error::Unsupported { detail, .. } if detail == if panic { "source reference callback panicked" } else { "root callback selected error" })
            );
            assert_eq!(budget.storage(), table);
            assert!(layouts.release(budget).is_err());
            assert_eq!(budget.storage(), before);
        }
    });
}

#[test]
fn source_storage_root_foreign_budget_and_undercut_cannot_refund() {
    instances(|instances, budget| {
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
        let original = budget.storage();
        let mut foreign_work = work();
        let mut foreign = Budget::new(&mut foreign_work, 64 * 1024 * 1024);
        foreign.reserve_storage(original).unwrap();
        let error =
            with_source_storage_root_v29(&mut layouts, instances, &mut foreign, |_, _, _| Ok(()))
                .unwrap_err();
        assert!(is_accounting(&error));
        assert_eq!(foreign.storage(), original);
        assert_eq!(foreign.work(), 0);
        assert_eq!(budget.storage(), original);
        assert!(layouts.release(budget).is_err());
    });
    instances(|instances, budget| {
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], budget).unwrap();
        let table = budget.storage();
        let owned = layouts.lease.owned.get();
        let error =
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                let _ = root.new_state(instances.root(), local_for(PAIR), budget)?;
                budget.release_storage(1)?;
                Ok(())
            })
            .unwrap_err();
        assert!(is_accounting(&error));
        assert!(budget.storage() > table);
        assert!(layouts.lease.owned.get() > owned);
        let retained = budget.storage();
        assert!(layouts.release(budget).is_err());
        assert_eq!(budget.storage(), retained);
    });
}

#[test]
fn source_storage_root_independent_fixed_entry_work_is_atomic_and_constructor_failure_settles() {
    // Two existing entry visits + initial destructor + four retries + the three
    // root entry/build/consuming custody checks, all admitted before callbacks.
    const ENTRY: usize = 2 + 1 + 4 + 3;
    assert_eq!(
        SOURCE_REFERENCE_ENTRY_WORK_V29 + SourceStorageRootCheckpointV29::additional_work(),
        ENTRY
    );
    instances(|instances, _| {
        for remaining in [ENTRY - 1, ENTRY] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], &mut budget).unwrap();
            let table = budget.storage();
            budget
                .charge_work(usize::MAX - budget.work() - remaining)
                .unwrap();
            let before = budget.work();
            let result: Result<(), Error> =
                with_source_storage_root_v29(&mut layouts, instances, &mut budget, |_, _, _| {
                    panic!("insufficient construction budget reached callback")
                });
            assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
            assert_eq!(
                budget.work(),
                before + if remaining == ENTRY { ENTRY } else { 0 }
            );
            assert_eq!(budget.storage(), table);
            assert!(layouts.lease.root.get().is_none());
            assert!(layouts.release(&mut budget).is_err());
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn source_storage_root_header_and_first_arena_admission_denials_restore_the_table_floor() {
    instances(|instances, _| {
        for short in [false, true] {
            let mut work = work();
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[PAIR], &mut budget).unwrap();
            let table = budget.storage();
            // The two legacy independent header formulas also check this exact
            // common header. Leave exactly that much caller capacity, or one less.
            let headers = source_reference_headers_v29::<()>().unwrap();
            let padding = budget.storage_limit() - table - headers + usize::from(short);
            budget.reserve_storage(padding).unwrap();
            let floor = budget.storage();
            let result: Result<(), Error> =
                with_source_storage_root_v29(&mut layouts, instances, &mut budget, |_, _, _| {
                    panic!("header-only capacity reached callback")
                });
            assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                ))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(layouts.lease.root.get().is_none());
            // Optional immutable snapshots now have the largest success slot;
            // retain both protected-operation and recorded-observation envelopes.
            let arena = size_of::<SourceStorageRootArenaV29<'_, '_>>()
                + size_of::<Result<SourceStorageRootArenaV29<'_, '_>, Error>>()
                + 2 * size_of::<Result<Option<SourceStorageSnapshotV29<'_>>, Error>>()
                + 2 * size_of::<
                    Result<Option<SourceStorageSnapshotV29<'_>>, RecordedStorageFailureV29<'_>>,
                >()
                + size_of::<std::cell::RefMut<'_, Vec<SourceStorageStateV29<'_, '_>>>>()
                + size_of::<std::cell::RefMut<'_, SourceStorageSnapshotIndexV29>>()
                + size_of::<Vec<Vec<SourceStorageSnapshotBucketV29>>>()
                + size_of::<Vec<SourceStorageSnapshotBucketV29>>()
                + size_of::<SourceStorageSnapshotBucketV29>()
                + size_of::<std::cell::Ref<'_, Vec<SourceStorageSubobjectV29<'_, '_>>>>()
                + 2 * size_of::<usize>();
            assert_eq!(source_storage_arena_headers_v29().unwrap(), arena);
            assert_eq!(
                budget.failed_storage(),
                Some(floor + headers + if short { 0 } else { arena })
            );
            assert!(layouts.release(&mut budget).is_err());
            assert_eq!(budget.storage(), padding);
            budget.release_storage(padding).unwrap();
        }
    });
}

#[test]
fn source_storage_root_rejected_output_destructor_does_not_replace_the_selected_error() {
    struct Rejected<'a>(&'a Cell<usize>);
    impl Drop for Rejected<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected root output destructor");
        }
    }
    instances(|instances, budget| {
        let mut layouts =
            SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, WORD], budget).unwrap();
        let table = budget.storage();
        let drops = Cell::new(0);
        let mut selected = None;
        let result =
            with_source_storage_root_v29(&mut layouts, instances, budget, |plan, root, budget| {
                let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
                let wrong = root.root_path(WORD, budget)?;
                assert!(
                    root.mutate(
                        state,
                        wrong,
                        SourceStorageRootMutationV29::Initialize,
                        budget
                    )
                    .is_err()
                );
                let Error::Unsupported { detail, .. } = plan.failure.first_error().unwrap() else {
                    panic!("wrong error")
                };
                selected = Some(detail);
                Ok(Rejected(&drops))
            });
        assert!(
            matches!(result, Err(Error::Unsupported { detail, .. }) if Some(detail) == selected)
        );
        assert_eq!(drops.get(), 1);
        assert_eq!(budget.storage(), table);
        assert!(layouts.release(budget).is_err());
    });
}
