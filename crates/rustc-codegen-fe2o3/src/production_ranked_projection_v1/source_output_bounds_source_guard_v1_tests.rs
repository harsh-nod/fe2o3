// Included inside bounds_cfg_tests. The original guard remains observable
// before a runtime-selected Return path, with and without partial coverage.
#[test]
fn actual_dynamic_bounds_failure_is_not_skipped_on_a_return_only_success_path() {
    let original =
        genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 1, true);
    let semantic = original.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    for partial in [false, true] {
        let mut blocks = vec![
            function.blocks()[0].clone(),
            function.blocks()[1].clone(),
            block(203, vec![], SemanticTerminatorKindV1::Return),
            block(
                204,
                vec![typed_assignment(
                    6,
                    A_BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Equal,
                        left: typed_operand(3, A_U32),
                        right: typed_constant(A_U32, 0, 4),
                    },
                )],
                zero_switch(6, A_BOOL, 1, 2),
            ),
        ];
        if partial {
            blocks.push(block(207, vec![], SemanticTerminatorKindV1::Return));
        }
        let ssa = assertion_ssa_functions(
            semantic.types().to_vec(),
            vec![replace_function(
                function,
                function.locals().to_vec(),
                blocks,
            )],
        );
        let source =
            materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 1)]).unwrap();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            // Optimizer byte equality is allowed: this is a control-retention
            // regression, separate from the genuine changed-O fixture tests.
            with_actual(&source, profile, |bound, checked, budget| {
                let floor = budget.storage();
                let result = with_projected_checked_output_roots_v1(
                    &source,
                    bound,
                    checked,
                    profile,
                    &[ranked_root_input_1d(A_NAME, 247, 1)],
                    &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                    budget,
                    |roots, _, _| {
                        let [root] = roots else {
                            panic!("one actual root")
                        };
                        let blocks = root.lowering.kernel().blocks();
                        let mut visited = vec![false; blocks.len()];
                        let mut pending = vec![0];
                        let mut guards = 0;
                        while let Some(block) = pending.pop() {
                            if visited[block] {
                                continue;
                            }
                            visited[block] = true;
                            use fe2o3_pliron::ProductionRankedTerminatorV1 as T;
                            match blocks[block].terminator() {
                                T::Return => panic!("return bypasses the source bounds assertion"),
                                T::Trap => {}
                                T::IndexLessThan { false_block, .. }
                                | T::IndexLessThanArgs { false_block, .. } => {
                                    assert!(matches!(
                                        blocks[*false_block as usize].terminator(),
                                        T::Trap
                                    ));
                                    guards += 1;
                                }
                                T::Branch { target }
                                | T::BranchArgs { target, .. }
                                | T::BranchArgsAdd { target, .. }
                                | T::BranchArgsAddAt { target, .. } => {
                                    pending.push(*target as usize)
                                }
                                T::AnalysisSplit {
                                    first_block,
                                    second_block,
                                    ..
                                }
                                | T::AnalysisSplitArgs {
                                    first_block,
                                    second_block,
                                    ..
                                } => {
                                    pending.extend([*first_block as usize, *second_block as usize])
                                }
                                T::IndexEqual {
                                    true_block,
                                    false_block,
                                    ..
                                }
                                | T::IndexEqualArgs {
                                    true_block,
                                    false_block,
                                    ..
                                } => pending.extend([*true_block as usize, *false_block as usize]),
                            }
                        }
                        assert_eq!(guards, 1);
                        Ok(())
                    },
                );
                assert_eq!(budget.storage(), floor);
                assert!(result.is_ok(), "partial {partial}");
            });
        }
    }
}

#[test]
fn actual_capacity_excess_is_recorded_before_later_denial() {
    let source = genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 1, true);
    with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
        with_checked_output_assertions_budget_v1(
            &source,
            bound,
            checked,
            Profile::Gfx942,
            budget,
            |session| {
                let floor = session.with_output_occurrences_v1(|_, budget| Ok(budget.storage()))?;
                const PREFIX: usize = 7;
                let requested = std::mem::size_of::<Vec<u8>>() + 4;
                let actual = std::mem::size_of::<Vec<u8>>() + 8;
                for storage_under in [false, true] {
                    for work_under in [false, true] {
                        let mut work = Work::new(PREFIX + 5 - usize::from(work_under));
                        let mut query =
                            Budget::new(&mut work, floor + actual - usize::from(storage_under));
                        query.charge_work(PREFIX).unwrap();
                        query.reserve_storage(floor).unwrap();
                        let result = {
                            let mut facts =
                                session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT);
                            facts.with_checked_control_scope_v1(|facts| {
                                facts.reserve_checked_control_storage_v1(requested)?;
                                facts.charge_private_array_work(4)?;
                                // Component-only allocator overcapacity: actual owned
                                // storage exceeds the prepaid request, with no fake O.
                                let values = Vec::<u8>::with_capacity(8);
                                assert_eq!(values.capacity(), 8);
                                bounds_cfg_v1::reconcile_capacity(&values, 4, facts)?;
                                facts.charge_private_array_work(1)?;
                                drop(values);
                                Ok(())
                            })
                        };
                        assert_eq!(result.is_ok(), !storage_under && !work_under);
                        assert_eq!(query.storage(), floor);
                        assert_eq!(
                            query.peak_storage(),
                            floor + if storage_under { requested } else { actual }
                        );
                        assert_eq!(
                            query.failed_storage(),
                            storage_under.then_some(floor + actual)
                        );
                        assert_eq!(
                            query.work(),
                            PREFIX + if storage_under || work_under { 4 } else { 5 }
                        );
                        assert_eq!(
                            work.failed_work(),
                            (!storage_under && work_under).then_some(PREFIX + 5)
                        );
                    }
                }
                Ok(())
            },
        )
        .unwrap();
    });
}
