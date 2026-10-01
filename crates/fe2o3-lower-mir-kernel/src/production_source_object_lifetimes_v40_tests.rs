fn source_object_lifetimes_run_v40(
    case: PhysicalAddressCase,
    work: usize,
    storage: usize,
    fault: u8,
) -> (SourceOwnedResultV18<()>, usize, usize, [usize; 3]) {
    let mut ledger = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut ledger, storage);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut census = [0; 3];
    let mut refused_storage = None;
    let result = (|| {
        let projection = physical_address_owner(case);
        let owner = physical_address_owner(case);
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())?;
        let prepared = with_module_fixture_view(
            &projection,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            },
        )?
        .0?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        source_scalar_normalization_scratch_v18(source.cleanup, budget, 0, |budget| {
                            for root in 0..source.root_count(budget)? {
                                let count = relation.memory_object_lifetime_count_v40(root, budget)?;
                                assert!(count > 0, "complete source object roster");
                                let mut previous = None;
                                for ordinal in 0..count {
                                    let row = relation.memory_object_lifetime_at_v40(root, ordinal, budget)?;
                                    let identity = row.identity(budget)?;
                                    let key = (identity.1, identity.2.index(), identity.3);
                                    assert!(previous.is_none_or(|before| before < key));
                                    previous = Some(key);
                                    let keyed = relation.memory_object_lifetime_v40(root, identity.1, identity.2, identity.3, budget)?.expect("enumerated row is queryable");
                                    assert_eq!(keyed.identity(budget)?, identity);
                                    assert_eq!(keyed.original_backing(budget)?, row.original_backing(budget)?);
                                    assert_eq!(keyed.activation_count(budget)?, row.activation_count(budget)?);
                                    for member in 0..row.activation_count(budget)? {
                                        assert_eq!(keyed.activation(member, budget)?, row.activation(member, budget)?);
                                    }
                                }
                                for instance in 0..source.instance_count(root, budget)? {
                                    for ordinal in 0..source.memory_anchor_count(root, instance, budget)? {
                                        let Some(recipe) = relation.memory_object_recipe_v39(root, instance, ordinal, budget)? else { continue; };
                                        for position in 0..recipe.endpoint_count(budget)? {
                                            let endpoint = recipe.endpoint(position, budget)?;
                                            let Some((owner_instance, local, generation)) = endpoint.local_identity(budget)? else { continue; };
                                            let lifetime = relation.memory_object_lifetime_v40(root, owner_instance, local, generation, budget)?
                                                .expect("an actual emitted local object has its original activation recipe");
                                            census[0] += 1;
                                            if fault != 0 {
                                                let before = budget.storage();
                                                let error = match fault {
                                                    1 => {
                                                        let lost = before - lifetime.required + 1;
                                                        budget.release_storage(lost)?;
                                                        let error = lifetime.activation_count(budget).unwrap_err();
                                                        budget.reserve_storage(lost)?;
                                                        refused_storage = Some(before);
                                                        error
                                                    }
                                                    2 => {
                                                        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                                                        let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                                                        foreign.reserve_storage(before)?;
                                                        let error = lifetime.activation_count(&mut foreign).unwrap_err();
                                                        assert_eq!((foreign.work(), foreign.storage()), (0, before));
                                                        refused_storage = Some(before);
                                                        error
                                                    }
                                                    3 => lifetime.activation(usize::MAX, budget).unwrap_err(),
                                                    _ => unreachable!(),
                                                };
                                                if fault < 3 { assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))); }
                                                else { assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding("original object activation ordinal"))); }
                                                let stopped = budget.work();
                                                assert!(lifetime.identity(budget).is_err());
                                                assert!(lifetime.activation(0, budget).is_err());
                                                assert_eq!(budget.work(), stopped);
                                                return Err(error);
                                            }
                                            let (root_type, _) = endpoint.source_types(budget)?;
                                            assert_eq!(lifetime.identity(budget)?, (root, owner_instance, local, generation, root_type));
                                            let actual = lifetime.original_backing(budget)?;
                                            let frame = relation.retained_allocation(root, actual, budget)?.expect("exact original Alloca");
                                            assert_eq!(frame.row, lifetime.slot);
                                            assert_eq!(frame.instance, owner_instance);
                                            assert_eq!(frame.slot.origin.semantic_type, root_type);
                                            let function = source.instance(root, owner_instance, budget)?.0;
                                            let original = &source.source_semantic(budget)?.functions()[function.index() as usize];
                                            assert_eq!(original.locals()[local.index() as usize].ty(), root_type);
                                            let count = lifetime.activation_count(budget)?;
                                            assert!(count > 0);
                                            let mut previous = None;
                                            for member in 0..count {
                                                let (atom, site) = lifetime.activation(member, budget)?;
                                                assert!(previous.is_none_or(|value| value < atom));
                                                previous = Some(atom);
                                                match site {
                                                    ProductionSourceObjectActivationV40::Entry => {
                                                        assert_eq!(atom, 0);
                                                        census[1] += 1;
                                                    }
                                                    ProductionSourceObjectActivationV40::StorageLive { block, statement } => {
                                                        assert!(matches!(original.blocks()[block.index() as usize].statements()[statement].kind(), SemanticStatementKindV1::StorageLive(found) if *found == local));
                                                        let first = 1 + original.blocks()[..block.index() as usize].iter().map(|row| row.statements().len()).sum::<usize>();
                                                        assert_eq!(atom as usize, first + statement);
                                                        census[2] += 1;
                                                    }
                                                }
                                            }
                                            assert!(relation.memory_object_lifetime_v40(root, owner_instance, local, u32::MAX, budget)?.is_none());
                                        }
                                    }
                                }
                            }
                            assert!(census.into_iter().all(|count| count > 0), "nonempty original object, entry and StorageLive recipes: {census:?}");
                            Ok(())
                        })
                    })
                })
            })
        })
    })();
    assert_eq!(
        budget.storage(),
        refused_storage.unwrap_or(MODULE_FLOOR),
        "{result:?}"
    );
    (result, budget.work(), budget.peak_storage(), census)
}

#[test]
fn source_object_lifetimes_retain_original_entry_restart_and_exact_backing_recipes() {
    for case in [
        PhysicalAddressCase::Stored,
        PhysicalAddressCase::FreshRestart,
    ] {
        let result = source_object_lifetimes_run_v40(case, MODULE_LIMIT, MODULE_LIMIT, 0);
        result.0.unwrap();
        assert!(result.3.into_iter().all(|value| value > 0));
    }
}

#[test]
fn source_object_lifetimes_latch_restored_undercut_foreign_ledger_and_invalid_member() {
    for fault in 1..=3 {
        let result = source_object_lifetimes_run_v40(
            PhysicalAddressCase::Stored,
            MODULE_LIMIT,
            MODULE_LIMIT,
            fault,
        );
        assert_eq!(result.3[0], 1, "actual source lifetime must be reached");
        assert!(result.0.is_err());
    }
}

#[test]
fn source_object_lifetimes_have_exact_and_one_short_whole_resources() {
    let run = |work, storage| {
        source_object_lifetimes_run_v40(PhysicalAddressCase::FreshRestart, work, storage, 0)
    };
    let (result, work, storage, census) = run(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, storage, census));
    for (w, s, work_failure) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let result = run(w, s);
        match (
            work_failure,
            source_slot_tests::original_repeated_source_resource_v29(result.0.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (work, w))
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (storage, s))
            }
            other => panic!("exact original activation resource refusal: {other:?}"),
        }
    }
}
