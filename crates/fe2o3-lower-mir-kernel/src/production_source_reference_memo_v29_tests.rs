use super::*;
use production_call_instances_v1::with_production_call_instances_v1;
use std::cell::Cell;

fn with_builder(
    inspect: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>),
) {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        inspect(&mut builder, budget);
        // Adversarial cases deliberately deny cleanup. Do not normalize their
        // ledgers with a test-only whole-builder refund.
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

fn map_credit(count: usize) -> usize {
    std::mem::size_of::<BTreeMap<usize, usize>>()
        + (0..count)
            .map(|n| execution_cfg_map_entry_storage_v29::<usize, usize>(n).unwrap())
            .sum::<usize>()
}

#[test]
fn memo_credit_fixed_headers_and_empty_scope_work_are_independently_reconstructed() {
    use std::mem::{align_of_val, size_of, size_of_val};
    type Output = Result<Option<usize>, ProductionSemanticKirErrorV1>;
    with_builder(|builder, budget| {
        let captured = [11u64; 7];
        let run = move |_: &mut SourceReferenceBuilderV29<'_, '_, '_>,
                        _: &mut SourceReferenceMemoV29,
                        _: &mut ArgumentBudgetV1<'_>| {
            assert_eq!(captured[3], 11);
            Ok(None)
        };
        let owner = size_of::<BTreeMap<usize, usize>>() + size_of::<usize>();
        let callback = 2 * size_of_val(&run) + 2 * align_of_val(&run);
        let captures = 2 * (3 * size_of::<&mut usize>() + 2 * size_of::<&Option<usize>>());
        let root_and_ledger =
            size_of::<Option<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
                + size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
                + size_of::<ArgumentLedgerV1>();
        let control = 12 * size_of::<usize>()
            + 3 * size_of::<Option<usize>>()
            + 2 * size_of::<Output>()
            + size_of::<std::thread::Result<Output>>()
            + size_of::<&std::thread::Result<Output>>()
            + size_of::<Option<ProductionSemanticKirErrorV1>>()
            + size_of::<&ProductionSemanticKirErrorV1>()
            + size_of::<Box<dyn std::any::Any + Send>>()
            + size_of::<bool>();
        let refund = size_of::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            ArgumentLedgerV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>() + size_of::<Result<(), ProductionSemanticKirErrorV1>>();
        let expected = owner + callback + captures + root_and_ledger + control + refund;
        assert_eq!(source_reference_memo_headers_v29(&run).unwrap(), expected);
        let before = (budget.storage(), budget.work());
        builder.with_memo_v29(budget, run).unwrap();
        assert_eq!(
            budget.storage() - before.0,
            expected - size_of::<BTreeMap<usize, usize>>()
        );
        assert_eq!(budget.work() - before.1, 5 + 12 + 5);
    });
}

#[test]
fn memo_credit_releases_only_destroyed_map_and_keeps_published_rows_paid() {
    with_builder(|builder, budget| {
        let retained = Cell::new(0);
        let before = budget.storage();
        let result = builder
            .with_memo_v29(budget, |builder, memo, budget| {
                for index in 0..20 {
                    let before = budget.work();
                    memo.insert(index, index + 1, budget)?;
                    assert_eq!(
                        budget.work() - before,
                        1 + 16 * (index.checked_ilog2().unwrap_or(0) as usize + 2)
                    );
                }
                let node = builder.plain(WORD, budget)?;
                emission_push_v1(&mut builder.plan.children, node, budget)?;
                retained.set(budget.storage());
                Ok(Some(node))
            })
            .unwrap()
            .unwrap();
        assert_eq!(budget.storage(), retained.get() - map_credit(20));
        assert!(budget.storage() > before);
        assert_eq!(builder.plan.children.last(), Some(&result));
        assert_eq!(
            builder.plan.nodes[result].kind,
            SourceReferenceNodeKindV29::Plain(None)
        );
        let first = budget.storage();
        builder
            .with_memo_v29(budget, |_, memo, budget| {
                assert_eq!(memo.len(), 0);
                memo.insert(0, result, budget)?;
                retained.set(budget.storage());
                Ok(Some(result))
            })
            .unwrap();
        assert_eq!(budget.storage(), retained.get() - map_credit(1));
        assert!(
            budget.storage() > first,
            "live caller/catch headers are not map credit"
        );
    });
}

#[test]
fn memo_credit_selected_error_keeps_rows_and_refunds_only_accepted_map_charges() {
    with_builder(|builder, budget| {
        let retained = Cell::new(0);
        let node = Cell::new(usize::MAX);
        let result = builder.with_memo_v29(budget, |builder, memo, budget| {
            memo.insert(1, 2, budget)?;
            node.set(builder.plain(WORD, budget)?);
            retained.set(budget.storage());
            Err(source_reference_error_v29("memo selected callback error"))
        });
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "memo selected callback error",
                ..
            })
        ));
        assert_eq!(budget.storage(), retained.get() - map_credit(1));
        assert_eq!(builder.plan.nodes[node.get()].ty, WORD);
    });
}

#[test]
fn memo_credit_raw_unwind_keeps_identity_and_published_rows() {
    with_builder(|builder, budget| {
        let retained = Cell::new(0);
        let payload: Box<dyn std::any::Any + Send> = Box::new(731usize);
        let identity = (&*payload as *const dyn std::any::Any) as *const () as usize;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            builder.with_memo_v29(budget, |builder, memo, budget| {
                memo.insert(1, 2, budget)?;
                builder.plain(WORD, budget)?;
                retained.set(budget.storage());
                std::panic::resume_unwind(payload)
            })
        }))
        .unwrap_err();
        assert_eq!(
            (&*caught as *const dyn std::any::Any) as *const () as usize,
            identity
        );
        assert_eq!(*caught.downcast::<usize>().unwrap(), 731);
        assert_eq!(budget.storage(), retained.get() - map_credit(1));
        assert_eq!(builder.plan.nodes.last().unwrap().ty, WORD);
    });
}

#[test]
fn memo_credit_rejects_entry_floor_undercut_even_above_old_plan_floor() {
    with_builder(|builder, budget| {
        budget.reserve_storage(4096).unwrap();
        let after = Cell::new(0);
        let result = builder.with_memo_v29(budget, |builder, memo, budget| {
            memo.insert(1, 2, budget)?;
            budget.release_storage(1)?;
            assert!(budget.storage() - map_credit(1) > builder.plan.retained_floor);
            after.set(budget.storage());
            Ok(None)
        });
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            budget.storage(),
            after.get(),
            "failed custody receives no map refund"
        );
    });
}

#[test]
fn memo_credit_foreign_slot_and_ledger_receive_no_refund_or_work() {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let accepted = Cell::new(0);
        let retained = Cell::new(0);
        let result = builder.with_memo_v29(budget, |_, memo, budget| {
            memo.insert(1, 2, budget)?;
            let full = budget.storage();
            foreign.reserve_storage(full)?;
            accepted.set(budget.work());
            retained.set(full);
            std::mem::swap(budget, &mut foreign);
            Ok(None)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), retained.get());
        assert_eq!(budget.work(), 0);
        assert_eq!(foreign.storage(), retained.get());
        assert_eq!(foreign.work(), accepted.get());
        std::mem::swap(budget, &mut foreign);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

#[test]
fn memo_credit_same_ledger_wrong_slot_is_denied_before_callback_or_charge() {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let mut other = ArgumentBudgetV1::new(&mut other_work, usize::MAX);
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let before = (budget.storage(), budget.work());
        std::mem::swap(budget, &mut other);
        let result = builder.with_memo_v29(&mut other, |_, _, _| panic!("wrong-slot callback ran"));
        assert!(result.is_err());
        assert_eq!((other.storage(), other.work()), before);
        assert_eq!((budget.storage(), budget.work()), (0, 0));
        std::mem::swap(budget, &mut other);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

fn late_insert_probe(limit: usize, expect_failure: bool) -> usize {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, limit);
    let last_required = Cell::new(0);
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let retained = Cell::new(0);
        let accepted = Cell::new(0);
        let published = Cell::new(usize::MAX);
        let result = builder.with_memo_v29(budget, |builder, memo, budget| {
            published.set(builder.plain(WORD, budget)?);
            for index in 0..512 {
                last_required.set(
                    budget.storage() + execution_cfg_map_entry_storage_v29::<usize, usize>(index)?,
                );
                let before = (budget.storage(), memo.owned);
                let insertion = memo.insert(index, index, budget);
                if insertion.is_err() {
                    assert_eq!(
                        (budget.storage(), memo.owned),
                        before,
                        "rejected reserve never becomes credit"
                    );
                    assert_eq!(memo.len(), index);
                    retained.set(budget.storage());
                    return insertion.map(|_| None);
                }
                accepted.set(index + 1);
            }
            retained.set(budget.storage());
            Ok(None)
        });
        assert_eq!(
            budget.storage(),
            retained.get() - map_credit(accepted.get())
        );
        assert_eq!(builder.plan.nodes[published.get()].ty, WORD);
        if expect_failure {
            assert_eq!(accepted.get(), 511);
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage { .. }
                    )
                )
            ));
            assert!(matches!(
                builder.plan.failure.first_error(),
                Some(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage { .. }
                    )
                )
            ));
        } else {
            result.unwrap();
        }
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    last_required.get()
}

#[test]
fn memo_credit_late_one_short_insert_preserves_published_node_and_accepted_only_credit() {
    let last = late_insert_probe(usize::MAX, false);
    assert_eq!(late_insert_probe(last - 1, true), last);
}

fn resource_probe(work_limit: usize, storage_limit: usize) -> (bool, usize, usize) {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let result = (|| {
                let mut builder = SourceReferenceBuilderV29::new(instances, budget)?;
                builder.with_memo_v29(budget, |builder, memo, budget| {
                    for node in 0..512 {
                        memo.insert(node, node, budget)?;
                    }
                    builder.plain(WORD, budget).map(Some)
                })
            })();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        });
    (
        result.is_ok_and(|inner| inner.is_ok()),
        budget.work(),
        budget.peak_storage(),
    )
}

#[test]
fn memo_credit_exact_and_one_short_work_and_storage_are_independent() {
    let (success, work, storage) = resource_probe(usize::MAX, usize::MAX);
    assert!(success);
    assert!(resource_probe(work, storage).0);
    assert!(!resource_probe(work - 1, storage).0);
    assert!(!resource_probe(work, storage - 1).0);
}

fn observed_discriminant(
    builder: &mut SourceReferenceBuilderV29<'_, '_, '_>,
    local: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> usize {
    let instance = builder.plan.root;
    let index = builder.plan.enum_observations.len();
    emission_push_v1(
        &mut builder.plan.enum_observations,
        SourceReferenceEnumObservationV29 {
            site: SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(0),
                statement: Some(0),
            },
            source: 0,
            instance,
            local: SemanticLocalIdV1::from_index(local),
            generation: 0,
            ty: WORD,
            first: 0,
            count: 0,
        },
        budget,
    )
    .unwrap();
    builder
        .node(
            WORD,
            SourceReferenceNodeKindV29::Discriminant(index),
            budget,
        )
        .unwrap()
}

#[test]
fn memo_credit_real_discriminant_entry_shares_dag_and_optional_result_but_not_calls() {
    with_builder(|builder, budget| {
        let first = observed_discriminant(builder, 1, budget);
        let second = observed_discriminant(builder, 2, budget);
        let start = builder.plan.children.len();
        for _ in 0..2 {
            emission_push_v1(&mut builder.plan.children, first, budget).unwrap();
        }
        let aggregate = builder
            .node(
                WORD,
                SourceReferenceNodeKindV29::Aggregate {
                    first: start,
                    count: 2,
                },
                budget,
            )
            .unwrap();
        let mut state = source_reference_scratch_v29(2, budget).unwrap();
        for _ in 0..2 {
            state.push(SourceReferenceLocalV29 {
                node: Some(aggregate),
                ..Default::default()
            });
        }
        let state_id = builder.plan.states.len();
        emission_push_v1(&mut builder.plan.states, state, budget).unwrap();
        builder.frames[0] = Some(state_id);
        let nodes = builder.plan.nodes.len();
        let result = builder
            .invalidate_discriminant_values(
                Some(builder.plan.root),
                Some(SemanticLocalIdV1::from_index(1)),
                Some(aggregate),
                budget,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            builder.plan.nodes.len(),
            nodes + 2,
            "one rebuilt leaf and one shared aggregate"
        );
        assert_eq!(builder.plan.states[state_id][0].node, Some(result));
        assert_eq!(builder.plan.states[state_id][1].node, Some(result));
        let SourceReferenceNodeKindV29::Aggregate { first, count: 2 } =
            builder.plan.nodes[result].kind
        else {
            panic!("aggregate lost")
        };
        assert_eq!(
            builder.plan.children[first],
            builder.plan.children[first + 1]
        );
        let result = builder
            .invalidate_discriminant_values(
                Some(builder.plan.root),
                Some(SemanticLocalIdV1::from_index(2)),
                Some(second),
                budget,
            )
            .unwrap()
            .unwrap();
        assert_ne!(
            result, second,
            "optional return outside all frames is visited in a fresh memo"
        );
        assert_eq!(
            builder.plan.nodes[result].kind,
            SourceReferenceNodeKindV29::Plain(None)
        );
    });
}

#[test]
fn memo_credit_empty_entrypoints_do_not_allocate_or_charge() {
    with_builder(|builder, budget| {
        let before = (budget.storage(), budget.work());
        assert_eq!(
            builder
                .expire_addresses(builder.plan.root, None, Some(42), budget)
                .unwrap(),
            Some(42)
        );
        assert_eq!(
            builder
                .invalidate_discriminant_values(None, None, Some(42), budget)
                .unwrap(),
            Some(42)
        );
        assert_eq!((budget.storage(), budget.work()), before);
    });
}

#[test]
fn memo_credit_recursive_depth_check_still_precedes_hit() {
    with_builder(|builder, budget| {
        let result = builder.with_memo_v29(budget, |builder, memo, budget| {
            memo.insert(usize::MAX, 0, budget)?;
            assert!(
                builder
                    .expire_address_node(usize::MAX, builder.plan.root, None, 256, memo, budget)
                    .is_err()
            );
            assert!(
                builder
                    .invalidate_discriminant_node(usize::MAX, None, None, 256, memo, budget)
                    .is_err()
            );
            Ok(None)
        });
        assert!(result.is_ok());
    });
}

#[test]
fn memo_credit_real_address_entry_keeps_expired_sets_and_rebuilt_children_paid() {
    with_builder(|builder, budget| {
        let instance = builder.plan.root;
        let local = SemanticLocalIdV1::from_index(1);
        emission_push_v1(
            &mut builder.plan.raw_origins,
            SourceReferenceRawOriginV29 {
                site: SourceReferenceSiteV29 {
                    instance,
                    block: SemanticBlockIdV1::from_index(0),
                    statement: Some(0),
                },
                source: 0,
                formation: SourceReferenceRawFormationV29::AddressOf,
                instance,
                local,
                generation: 0,
                ty: WORD,
                pointer_type: REFERENCE,
                first: 0,
                count: 0,
                parent: None,
                mutable: false,
            },
            budget,
        )
        .unwrap();
        let source = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
            origin: 0,
            expired: false,
        });
        let set = builder.raw_set(source, source, false, budget).unwrap();
        let leaf = builder
            .node(REFERENCE, SourceReferenceNodeKindV29::Address(set), budget)
            .unwrap();
        let first = builder.plan.children.len();
        for _ in 0..2 {
            emission_push_v1(&mut builder.plan.children, leaf, budget).unwrap();
        }
        let aggregate = builder
            .node(
                CAPTURE,
                SourceReferenceNodeKindV29::Aggregate { first, count: 2 },
                budget,
            )
            .unwrap();
        let mut state = source_reference_scratch_v29(2, budget).unwrap();
        for _ in 0..2 {
            state.push(SourceReferenceLocalV29 {
                node: Some(aggregate),
                ..Default::default()
            });
        }
        let state_id = builder.plan.states.len();
        emission_push_v1(&mut builder.plan.states, state, budget).unwrap();
        builder.frames[0] = Some(state_id);
        let nodes = builder.plan.nodes.len();
        let retained = budget.storage();
        assert_eq!(
            builder
                .expire_addresses(
                    instance,
                    Some(SemanticLocalIdV1::from_index(2)),
                    Some(aggregate),
                    budget
                )
                .unwrap(),
            Some(aggregate)
        );
        assert_eq!(builder.plan.nodes.len(), nodes);
        let result = builder
            .expire_addresses(instance, Some(local), Some(aggregate), budget)
            .unwrap()
            .unwrap();
        assert_eq!(builder.plan.nodes.len(), nodes + 2);
        assert_eq!(builder.plan.states[state_id][0].node, Some(result));
        assert_eq!(builder.plan.states[state_id][1].node, Some(result));
        let SourceReferenceNodeKindV29::Aggregate { first, count: 2 } =
            builder.plan.nodes[result].kind
        else {
            panic!("aggregate lost")
        };
        let rebuilt = builder.plan.children[first];
        assert_eq!(rebuilt, builder.plan.children[first + 1]);
        let SourceReferenceNodeKindV29::Address(set) = builder.plan.nodes[rebuilt].kind else {
            panic!("address lost")
        };
        let row = builder.plan.raw_sets[set];
        assert!(
            builder
                .plan
                .raw_choice(SourceReferenceRawChoicesV29::Retained(row), 0, budget)
                .unwrap()
                .unwrap()
                .expired
        );
        assert!(budget.storage() > retained);
    });
}

#[test]
fn memo_credit_integrated_root_growth_stays_paid_and_owned_error_survives_lost_custody() {
    for lose_custody in [false, true] {
        let owner = owner(Case::Shared);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
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
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let last = Cell::new(0);
            let reached = Cell::new(false);
            let result = source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, root, budget| {
                    let mut builder = SourceReferenceBuilderV29::new_with_root(
                        plan.instances,
                        SourceReferenceStorageV29::ScalarCells,
                        Some(root),
                        budget,
                    )?;
                    let result = builder.with_memo_v29(budget, |builder, memo, budget| {
                        let entry = budget.storage();
                        memo.insert(1, 2, budget)?;
                        let root = builder.storage_root.as_ref().unwrap();
                        let old = root
                            .snapshot_statistics(budget)
                            .map_err(|error| builder.storage_error(error))?;
                        let snapshot = root
                            .snapshot_local(
                                plan.root,
                                SemanticLocalIdV1::from_index(1),
                                true,
                                budget,
                            )
                            .map_err(|error| builder.storage_error(error))?
                            .unwrap();
                        let snapshot = root
                            .snapshot_lifetime(snapshot, false, budget)
                            .map_err(|error| builder.storage_error(error))?;
                        let next = root
                            .snapshot_statistics(budget)
                            .map_err(|error| builder.storage_error(error))?;
                        assert_ne!(old, next, "fixture must retain real C2 snapshot growth");
                        builder.retain_storage_snapshot(snapshot, budget)?;
                        if lose_custody {
                            // Undercut the entry-plus-retained-growth floor, not
                            // merely the root's ordinary-query poison bit.
                            let required = entry
                                + (next.1 - old.1)
                                + execution_cfg_map_entry_storage_v29::<usize, usize>(0)?;
                            budget.release_storage(budget.storage() - required + 1)?;
                        }
                        last.set(budget.storage());
                        reached.set(true);
                        if lose_custody {
                            Err(source_reference_error_v29("memo original owned failure"))
                        } else {
                            Ok(None)
                        }
                    });
                    if lose_custody {
                        assert_eq!(budget.storage(), last.get());
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                function: 0,
                                block: None,
                                statement: None,
                                detail: "memo original owned failure",
                            })
                        ));
                        assert!(builder.plan.failure.first_error().is_none());
                        assert!(
                            !builder
                                .plan
                                .storage_root
                                .as_ref()
                                .unwrap()
                                .permits_cleanup_refund(
                                    builder.plan.instances,
                                    &builder.plan.failure,
                                    0,
                                    budget,
                                ),
                            "root poison remains observable"
                        );
                        assert!(builder.plan.failure.first_error().is_none());
                    } else {
                        assert_eq!(budget.storage(), last.get() - map_credit(1));
                        assert!(!builder.plan.storage_snapshots.is_empty());
                    }
                    result.map(|_| ()).map_err(Into::into)
                },
            );
            assert!(reached.get());
            if lose_custody {
                assert!(
                    matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "memo original owned failure",
                            ..
                        })
                    ),
                    "outer checkpoint must retain the original owned diagnostic: {result:?}"
                );
                assert_eq!(
                    budget.storage(),
                    last.get(),
                    "poisoned active root also receives no refund"
                );
            } else {
                result.unwrap();
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        })
        .unwrap();
    }
}
