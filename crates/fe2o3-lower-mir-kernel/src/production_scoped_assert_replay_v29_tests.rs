fn with_assert_module(
    kind: ModuleFixture,
    test: impl FnOnce(
        &mut PendingScopedModuleV29,
        &ExecutionLifecycleSourceV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    with_module_fixture(kind, &mut budget, |source, budget| {
        let floor = budget.storage();
        let mut pending = admit_pending_scoped_module_v29(
            source,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
        .unwrap();
        let retained = pending.retained_storage;
        test(&mut pending, source, budget);
        assert_eq!(budget.storage(), floor + retained);
        drop(pending);
        budget.release_storage(retained).unwrap();
    })
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scoped_module_assertions_use_final_function_coordinates_and_semantic_roots() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        with_assert_module(kind, |pending, source, budget| {
            let floor = budget.storage();
            let rows = collect_scoped_module_assertions_v29(
                pending.graph.module(),
                &pending.roots,
                source,
                budget,
            )
            .unwrap();
            let ordinals = if matches!(kind, ModuleFixture::Ordinary) {
                [0, 1]
            } else {
                [0, 2]
            };
            assert_eq!(rows.len(), 2);
            for (row, ordinal) in rows.iter().zip(ordinals) {
                assert_eq!(row.binding.block().function.0, ordinal);
                assert!(row.binding.expected());
                assert_eq!(
                    row.site.correspondence_owner,
                    source.launch.roots()[ordinal as usize].selected_root(),
                );
                assert_eq!(row.instance.index(), 0);
            }
            if !matches!(kind, ModuleFixture::Ordinary) {
                assert_eq!(rows[1].site.correspondence_owner.index(), 4);
            }
            assert_origin_drop_v1(rows, budget).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn scoped_module_assertion_collection_refuses_changed_root_and_capture_metadata() {
    for fault in 0..6 {
        with_assert_module(ModuleFixture::Mixed, |pending, source, budget| {
            match fault {
                0 => pending.roots[2].function_ordinal = 0,
                1 => pending.roots[2].function_ordinal = pending.graph.module().functions.len(),
                2 => pending.roots[2].coordinates.root = pending.roots[0].coordinates.root,
                3 => pending.roots[2].sidecars.rows[0]
                    .instance_assert_origins
                    .as_mut()
                    .unwrap()
                    .records
                    .clear(),
                4 => {
                    pending.roots[2].sidecars.rows[0]
                        .instance_assert_origins
                        .as_mut()
                        .unwrap()
                        .records[0]
                        .expected = false
                }
                5 => {
                    let sibling = pending.roots[0].sidecars.rows[0]
                        .instance_assert_origins
                        .as_ref()
                        .unwrap()
                        .records[0]
                        .site;
                    pending.roots[2].sidecars.rows[0]
                        .instance_assert_origins
                        .as_mut()
                        .unwrap()
                        .records[0]
                        .site = sibling;
                }
                _ => unreachable!(),
            }
            let floor = budget.storage();
            assert!(
                collect_scoped_module_assertions_v29(
                    pending.graph.module(),
                    &pending.roots,
                    source,
                    budget,
                )
                .is_err()
            );
            assert_eq!(
                budget.storage(),
                floor,
                "fault {fault} leaked scratch or earlier rows"
            );
        });
    }
}

#[test]
fn scoped_module_assertion_subject_refuses_local_index_and_conflicting_insertions() {
    for fault in 0..4 {
        with_assert_module(ModuleFixture::Mixed, |pending, source, budget| {
            let floor = budget.storage();
            let module = pending.graph.module();
            let root = &pending.roots[2];
            let functions = if fault == 0 {
                &module.functions[2..3]
            } else {
                &module.functions
            };
            let graph = AssertGraphIndexV1::build_functions(functions, true, budget).unwrap();
            let capture = root.sidecars.rows[0]
                .instance_assert_origins
                .as_ref()
                .unwrap();
            let recorded = &capture.records[0];
            let PendingAssertOutcomeV1::Emitted { failure, .. } = recorded.outcome else {
                panic!("expected an emitted assertion")
            };
            let mut insertion = pending.roots[1].insertions[0];
            let conflict = if fault == 2 { failure } else { recorded.block };
            insertion.before.block = conflict;
            insertion.after.block = conflict;
            let insertions = [insertion];
            let mut visits = 0;
            let result = production_call_instances_v1::with_production_call_instances_v1(
                source.owner,
                source.launch.roots()[2].selected_root(),
                budget,
                |instances, budget| {
                    Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                        replay_instance_asserts_in_functions_v1(
                            InstanceAssertReplaySubjectV1 {
                                functions: &module.functions,
                                function_ordinal: root.function_ordinal,
                                sidecars: &root.sidecars,
                                coordinates: &root.coordinates,
                                slot_relocation: root.slot_relocation.as_ref(),
                                insertions: if matches!(fault, 1 | 2) {
                                    &insertions
                                } else {
                                    &root.insertions
                                },
                            },
                            instances,
                            &graph,
                            budget,
                            &mut |_, _| {
                                visits += 1;
                                Err(assert_origin_invalid_v1(None, "collector refused").into())
                            },
                        ),
                    )
                },
            )
            .unwrap();
            assert!(result.is_err());
            assert_eq!(visits, usize::from(fault == 3));
            if fault == 3 {
                assert!(format!("{:?}", result.unwrap_err()).contains("collector refused"));
            }
            graph.release(budget).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}
