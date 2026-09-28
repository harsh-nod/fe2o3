fn run_read_guards(
    consume: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>) -> Result<(), Error>,
) {
    let owner = owner_with(extended_types());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
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
    .unwrap();
}

#[test]
fn payload_reads_require_active_direct_and_niche_variants_not_conditional_facts() {
    run_read_guards(|instances, budget| {
        let floor = budget.storage();
        for (ty, some_variant, empty_variant) in [(EMPTY_DIRECT, 0, 1), (NICHE, 1, 0)] {
            let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[ty], budget)?;
            let root = layouts.root_subobject(instances.owner(), ty, budget)?;
            let field = payload(&layouts, &root, some_variant, budget)?;
            let mut some = SourceStorageStateV29::new(
                &layouts,
                instances,
                instances.root(),
                local_for(ty),
                budget,
            )?;
            let mut empty = some.copy(budget)?;
            some.begin_variant(&root, some_variant, budget)?;
            some.initialize(&field, budget)?;
            some.set_discriminant(&root, some_variant, budget)?;
            empty.set_discriminant(&root, empty_variant, budget)?;
            assert!(some.is_readable(&field, budget)?);
            assert!(!empty.is_readable(&field, budget)?);
            let joined = some.join(&empty, budget)?;
            assert!(
                joined.is_initialized(&field, budget)?,
                "conditional payload facts must remain available"
            );
            assert!(
                !joined.is_readable(&field, budget)?,
                "neither edge alone authorizes a merged payload read"
            );
            assert!(
                joined.is_readable(&root, budget)?,
                "whole values do not assert a chosen variant"
            );
            let before = joined.copy(budget)?;
            let mut selected = joined.copy(budget)?;
            assert!(selected.restrict_discriminant(&root, &[some_variant], budget)?);
            assert!(selected.is_readable(&field, budget)?);
            assert!(
                joined.equivalent(&before, budget)?,
                "read/refinement cannot mutate another snapshot"
            );
            selected.deinitialize(&field, budget)?;
            selected.set_discriminant(&root, some_variant, budget)?;
            assert!(
                !selected.is_readable(&field, budget)?,
                "tag writes cannot repair moved payloads"
            );
            before.discard(budget)?;
            selected.discard(budget)?;
            joined.discard(budget)?;
            empty.discard(budget)?;
            some.discard(budget)?;
            field.discard(budget)?;
            root.discard(budget)?;
            layouts.release(budget)?;
        }
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn nested_payload_reads_discharge_outer_direct_and_inner_niche_selectors() {
    run_read_guards(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[OUTER], budget)?;
        let root = layouts.root_subobject(instances.owner(), OUTER, budget)?;
        let inner = payload(&layouts, &root, 0, budget)?;
        let field = payload(&layouts, &inner, 1, budget)?;
        let mut some = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(OUTER),
            budget,
        )?;
        let mut empty = some.copy(budget)?;
        some.begin_variant(&root, 0, budget)?;
        some.begin_variant(&inner, 1, budget)?;
        some.initialize(&field, budget)?;
        some.set_discriminant(&inner, 1, budget)?;
        some.set_discriminant(&root, 0, budget)?;
        empty.set_discriminant(&root, 1, budget)?;
        assert!(some.is_readable(&field, budget)?);
        let mut joined = some.join(&empty, budget)?;
        assert!(joined.is_initialized(&field, budget)?);
        assert!(
            !joined.is_readable(&field, budget)?,
            "the inner selector cannot discharge its enclosing selector"
        );
        assert!(joined.restrict_discriminant(&root, &[0], budget)?);
        assert!(joined.is_readable(&field, budget)?);
        joined.set_discriminant(&inner, 0, budget)?;
        assert!(!joined.is_readable(&field, budget)?);
        assert!(
            joined.bytes_initialized(field.range, budget)?,
            "initialized niche bits are not typed read authority"
        );
        joined.discard(budget)?;
        empty.discard(budget)?;
        some.discard(budget)?;
        field.discard(budget)?;
        inner.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn readable_payload_query_rejects_foreign_original_geometry_and_ended_lifetimes() {
    run_read_guards(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
        let other = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
        let root = layouts.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
        let field = payload(&layouts, &root, 0, budget)?;
        let foreign_root = other.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
        let foreign = payload(&other, &foreign_root, 0, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(EMPTY_DIRECT),
            budget,
        )?;
        state.begin_variant(&root, 0, budget)?;
        state.initialize(&field, budget)?;
        state.set_discriminant(&root, 0, budget)?;
        assert!(state.is_readable(&field, budget)?);
        assert!(state.is_readable(&foreign, budget).is_err());
        state.storage_dead(budget)?;
        assert!(state.is_readable(&field, budget).is_err());
        state.discard(budget)?;
        foreign.discard(budget)?;
        foreign_root.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        other.release(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn readable_payload_query_has_an_independent_exact_and_one_short_work_boundary() {
    for short in [false, true] {
        run_read_guards(|instances, budget| {
            let floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
            let root = layouts.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
            let field = payload(&layouts, &root, 0, budget)?;
            let mut state = SourceStorageStateV29::new(
                &layouts,
                instances,
                instances.root(),
                local_for(EMPTY_DIRECT),
                budget,
            )?;
            state.begin_variant(&root, 0, budget)?;
            state.initialize(&field, budget)?;
            state.set_discriminant(&root, 0, budget)?;
            assert_eq!(
                field.path,
                [SubobjectStep::Variant(0), SubobjectStep::Field(0)]
            );
            assert_eq!(state.enums.len(), 1);
            assert!(state.enums[0].place.path.is_empty());
            assert_eq!(state.enums[0].readable, [0]);
            assert!(state.facts.iter().all(|fact| fact.place.path.len() <= 2));
            let original_facts = state
                .facts
                .iter()
                .map(|fact| {
                    (
                        fact.place.path.clone(),
                        fact.initialized,
                        fact.guards.clone(),
                    )
                })
                .collect::<Vec<_>>();
            // One place check; three exact vectors; path copy2; two pushes2;
            // one task visit; one prefix comparison per original sparse fact;
            // path scan2; one root-selector visit1 and one enum-prefix lookup1.
            let comparisons = original_facts
                .iter()
                .map(|(path, _, _)| path.len() + 1)
                .sum::<usize>();
            let exact = 1 + 3 * 3 + 2 + 2 * 2 + 1 + comparisons + 2 + 1 + 1;
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before_work = budget.work();
            let before_storage = budget.storage();
            let result = state.is_readable(&field, budget);
            if short {
                let error = result.unwrap_err();
                assert!(matches!(
                    error,
                    Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))
                ));
                assert_eq!(budget.work() - before_work, exact - 1);
                let stopped = budget.work();
                assert!(state.is_readable(&field, budget).is_err());
                assert_eq!(
                    budget.work(),
                    stopped,
                    "the actual layout lease retains its first failure"
                );
            } else {
                assert!(result?);
                assert_eq!(budget.work() - before_work, exact);
            }
            assert_eq!(
                budget.storage(),
                before_storage,
                "all conditional-query scratch was released before guard evaluation"
            );
            assert_eq!(
                state
                    .facts
                    .iter()
                    .map(|fact| (
                        fact.place.path.clone(),
                        fact.initialized,
                        fact.guards.clone()
                    ))
                    .collect::<Vec<_>>(),
                original_facts
            );
            state.discard(budget)?;
            field.discard(budget)?;
            root.discard(budget)?;
            assert_eq!(layouts.release(budget).is_err(), short);
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    }
}

#[test]
fn readable_payload_query_has_an_independent_peak_storage_boundary() {
    #[allow(dead_code)]
    enum QueryTaskMirror<'a, 'b> {
        Query(SourceStorageSubobjectV29<'a, 'b>),
        Reduce {
            all: bool,
            empty: bool,
            count: usize,
        },
    }
    for short in [false, true] {
        run_read_guards(|instances, budget| {
            let floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
            let root = layouts.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
            let field = payload(&layouts, &root, 0, budget)?;
            let mut state = SourceStorageStateV29::new(
                &layouts,
                instances,
                instances.root(),
                local_for(EMPTY_DIRECT),
                budget,
            )?;
            state.begin_variant(&root, 0, budget)?;
            state.initialize(&field, budget)?;
            state.set_discriminant(&root, 0, budget)?;
            // One task and answer slot, two vector headers, one copied original
            // subobject and its two-step path. Guard discharge allocates nothing.
            let path_bytes = 2 * size_of::<SubobjectStep>();
            let exact = size_of::<Vec<QueryTaskMirror<'_, '_>>>()
                + size_of::<Vec<bool>>()
                + size_of::<QueryTaskMirror<'_, '_>>()
                + size_of::<bool>()
                + size_of::<SourceStorageSubobjectV29<'_, '_>>()
                + path_bytes;
            let filler = 64 * 1024 * 1024 - budget.storage() - exact + usize::from(short);
            budget.reserve_storage(filler)?;
            let before = budget.storage();
            let result = state.is_readable(&field, budget);
            if short {
                assert!(matches!(
                    result,
                    Err(Error::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage { .. }
                    ))
                ));
                assert_eq!(
                    budget.storage(),
                    before + exact - path_bytes,
                    "the copied path is the first allocation that does not fit"
                );
                assert!(state.is_readable(&field, budget).is_err());
            } else {
                assert!(result?);
                assert_eq!(budget.storage(), before);
                assert_eq!(budget.peak_storage(), before + exact);
            }
            state.discard(budget)?;
            field.discard(budget)?;
            root.discard(budget)?;
            assert_eq!(layouts.release(budget).is_err(), short);
            assert_eq!(budget.storage(), floor + filler);
            budget.release_storage(filler)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    }
}

#[test]
fn snapshot_read_queries_have_independent_exact_short_and_entry_denial_work() {
    for selected in [false, true] {
        for mode in 0..3 {
            run_read_guards(|instances, budget| {
                let floor = budget.storage();
                let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget)?;
                assert_eq!(layouts.keys, [RowKey::ty(WORD)]);
                let table = budget.storage();
                let persistent = layouts.lease.persistent.get();
                let reached = std::cell::Cell::new(false);
                let result = with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |plan, root, budget| {
                        let snapshot = root
                            .snapshot_local(instances.root(), local_for(WORD), true, budget)?
                            .unwrap();
                        {
                            let snapshots = root.arena.snapshots.borrow();
                            let state = &snapshots[snapshot.index];
                            assert!(state.physical_root.is_some());
                            assert_eq!(state.facts.len(), 1);
                            assert!(state.facts[0].place.path.is_empty());
                            assert!(state.facts[0].initialized);
                            assert!(!state.has_selections);
                            assert!(state.enums.is_empty());
                        }
                        // Arena owner1, optional original-plan check5, source
                        // row owner1+singleton lookup1; state check1; three
                        // vector admissions3 each; two pushes2 each; task1;
                        // empty-prefix fact comparison1. Empty path scan costs0.
                        let exact =
                            1 + usize::from(selected) * 5 + 1 + 1 + 1 + 3 * 3 + 2 * 2 + 1 + 1;
                        let available = match mode {
                            0 => 0,
                            1 => exact,
                            _ => exact - 1,
                        };
                        budget.charge_work(usize::MAX - budget.work() - available)?;
                        let before_work = budget.work();
                        let before_storage = budget.storage();
                        let query = if selected {
                            root.snapshot_selected_readable(snapshot, &[], plan, None, budget)
                        } else {
                            root.snapshot_readable(snapshot, &[], budget)
                        };
                        reached.set(true);
                        if mode == 1 {
                            assert!(query?);
                            assert_eq!(budget.work() - before_work, exact);
                            assert_eq!(budget.storage(), before_storage);
                        } else {
                            assert!(query.is_err());
                            // A one-short query denies the final atomic push2,
                            // while entry denial cannot allocate or visit state.
                            assert_eq!(
                                budget.work() - before_work,
                                if mode == 0 { 0 } else { exact - 2 }
                            );
                            if mode == 0 {
                                assert_eq!(budget.storage(), before_storage);
                            }
                            let stopped = budget.work();
                            assert!(root.snapshot_readable(snapshot, &[], budget).is_err());
                            assert_eq!(budget.work(), stopped);
                        }
                        let snapshots = root.arena.snapshots.borrow();
                        assert_eq!(snapshots[snapshot.index].facts.len(), 1);
                        assert!(snapshots[snapshot.index].facts[0].initialized);
                        Ok(())
                    },
                );
                assert!(reached.get());
                if mode == 1 {
                    result?;
                } else {
                    assert!(matches!(
                        result,
                        Err(Error::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        ))
                    ));
                }
                assert_eq!(layouts.lease.persistent.get(), persistent);
                assert_eq!(budget.storage(), table);
                assert_eq!(layouts.release(budget).is_err(), mode != 1);
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
        }
    }
}

#[test]
fn snapshot_read_queries_reject_foreign_missing_borrowed_and_dead_snapshots() {
    for selected in [false, true] {
        for mutation in 0..4 {
            run_read_guards(|instances, budget| {
                let floor = budget.storage();
                let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget)?;
                let table = budget.storage();
                let result = with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |plan, root, budget| {
                        let original = root
                            .snapshot_local(instances.root(), local_for(WORD), true, budget)?
                            .unwrap();
                        assert!(root.snapshot_readable(original, &[], budget)?);
                        let snapshot = match mutation {
                            0 => SourceStorageSnapshotV29 {
                                arena: original.arena.wrapping_add(1),
                                ..original
                            },
                            1 => SourceStorageSnapshotV29 {
                                index: usize::MAX,
                                ..original
                            },
                            3 => root.snapshot_lifetime(original, false, budget)?,
                            _ => original,
                        };
                        let guard = (mutation == 2).then(|| root.arena.snapshots.borrow_mut());
                        let query = if selected {
                            root.snapshot_selected_readable(snapshot, &[], plan, None, budget)
                        } else {
                            root.snapshot_readable(snapshot, &[], budget)
                        };
                        assert!(query.is_err());
                        drop(guard);
                        assert!(root.arena.snapshots.borrow()[original.index].live);
                        Ok(())
                    },
                );
                assert!(matches!(result, Err(Error::Unsupported { .. })));
                assert_eq!(budget.storage(), table);
                assert!(layouts.release(budget).is_err());
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
        }
    }
}
