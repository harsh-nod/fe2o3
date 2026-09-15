mod checked_control_tests {
    use super::*;
    use crate::production_ranked_projection_v1::checked_control_v1;
    use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1, SemanticSwitchTargetsV1};

    fn branch_source(explicit: u128, condition: bool, duplicate: bool) -> SemanticFunctionDeclV1 {
        let first = assertion_ranked_write_statements(1, 2);
        let mut second = first.clone();
        second[0] = typed_assignment(
            2,
            A_U32,
            SemanticRvalueKindV1::Use(typed_constant(A_U32, 7, 4)),
        );
        assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    211,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_constant(A_BOOL, u128::from(condition), 1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                explicit,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(
                                SemanticEdgeRoleV1::SwitchOtherwise,
                                if duplicate { 1 } else { 2 },
                            ),
                        )
                        .unwrap(),
                    },
                ),
                block(212, first, SemanticTerminatorKindV1::Return),
                block(213, second, SemanticTerminatorKindV1::Return),
            ],
            false,
        )
    }

    fn stores(owner: &Owner) -> usize {
        owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(operation.kind, fe2o3_kernel_ir::OperationKind::Store { .. })
            })
            .count()
    }

    fn wrapped_branch_source(condition: bool) -> ProductionPreRankedKirOwnerV1 {
        let seed = wrapped_literal_assertion_ssa();
        let semantic = seed.source_semantic();
        let source = &semantic.functions()[1];
        let branch = branch_source(1, condition, false);
        let mut first = branch.blocks()[1].statements().to_vec();
        first.extend_from_slice(source.blocks()[1].statements());
        let mut second = branch.blocks()[2].statements().to_vec();
        second.extend_from_slice(source.blocks()[1].statements());
        let body = SemanticFunctionDeclV1::new(
            source.identity(),
            source.role(),
            source.item_definition_identity(),
            source.monomorphization_identity(),
            source.generic_type_arguments_identity(),
            source.const_generic_arguments_identity(),
            source.source(),
            source.abi().clone(),
            source.locals().to_vec(),
            source.entry(),
            vec![
                branch.blocks()[0].clone(),
                block(212, first, SemanticTerminatorKindV1::Return),
                block(213, second, SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        materialize_ranked_fixture_v1(
            assertion_ssa_functions(
                semantic.types().to_vec(),
                vec![semantic.functions()[0].clone(), body],
            ),
            &[ranked_root_input_1d(A_NAME, 247, 64)],
        )
        .unwrap()
    }

    #[test]
    fn actual_selected_control_covers_whole_dormant_writes_and_keeps_edge_occurrences() {
        for explicit in [0, 1] {
            for condition in [false, true] {
                for duplicate in [false, true] {
                    let source =
                        assertion_materialized(branch_source(explicit, condition, duplicate));
                    with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
                        if !duplicate {
                            assert_eq!(stores(source.executable()), 2);
                        }
                        assert_eq!(stores(checked.owner()), 1);
                        with_checked_output_assertions_budget_v1(&source, bound, checked,
                            Profile::Gfx942, budget, |session| {
                                let semantic = source.semantic_ssa().source_semantic();
                                let function = &semantic.functions()[0];
                                let mut facts = session.for_source(ROOT, ROOT);
                                facts.with_checked_control_scope_v1(|facts| {
                                    let frame = checked_control_v1::prepare(semantic.types(), function,
                                        semantic.callables(), facts)?.expect("actual partial control");
                                    let ordinal = u32::from(u128::from(condition) != explicit);
                                    let selected = frame.selected_for_test(0).unwrap();
                                    assert_eq!(selected.semantic_ordinal(), ordinal);
                                    assert_eq!(selected.input().successor, u32::from(!condition));
                                    let live = if duplicate { 1 } else { 1 + ordinal as usize };
                                    let dead = 3 - live;
                                    assert!(frame.projects_block(0));
                                    assert!(frame.projects_block(live));
                                    assert!(!frame.projects_block(dead));
                                    let coverage = frame.coverage_for_test(dead);
                                    assert_eq!(coverage.source_statements(), function.blocks()[dead].statements().len());
                                    if !duplicate {
                                        assert!(coverage.original_operations().is_some_and(|n| n > 0));
                                    }
                                    assert!(matches!(coverage.disposition(),
                                        ProductionSourceOutputBlockV1::Materialized { executable: false, .. }
                                        | ProductionSourceOutputBlockV1::NotMaterialized));
                                    let site = fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                                        block: fe2o3_mir_model::SsaBlockIdV1::new(live as u32), statement: 1,
                                    };
                                    assert_eq!(facts.private_array_constant_index(site,
                                        fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination)?,
                                        Some(if live == 1 { 0 } else { 7 }));
                                    Ok(())
                                })
                            }).unwrap();
                    });
                }
            }
        }
    }

    #[test]
    fn actual_live_failed_assert_is_not_erased_by_its_dead_access_target() {
        let source = assertion_materialized(assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    201,
                    vec![],
                    assertion_terminator(typed_constant(A_BOOL, 0, 1), true, 1),
                ),
                block(
                    202,
                    assertion_ranked_write_statements(1, 2),
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        ));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            assert_eq!(stores(checked.owner()), 0);
            let result = with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let semantic = source.semantic_ssa().source_semantic();
                    let mut facts = session.for_source(ROOT, ROOT);
                    facts.with_checked_control_scope_v1(|facts| {
                        checked_control_v1::prepare(
                            semantic.types(),
                            &semantic.functions()[0],
                            semantic.callables(),
                            facts,
                        )
                        .map(|_| ())
                    })
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::UnprovenAssert { block: 0, .. })
            ));
            let called = std::cell::Cell::new(false);
            let result = with_projected_checked_output_roots_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                &[ranked_root_input_1d(A_NAME, 247, 64)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |_, _, _| {
                    called.set(true);
                    Ok(())
                },
            );
            assert!(!called.get());
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::UnprovenAssert { block: 0, .. })
            ));
        });
    }

    #[test]
    fn real_partial_root_projects_only_the_live_ranked_write() {
        for condition in [false, true] {
            let source = assertion_materialized(branch_source(1, condition, false));
            with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
                let (roots, ()) = with_projected_checked_output_roots_v1(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    &[ranked_root_input_1d(A_NAME, 247, 64)],
                    &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                    budget,
                    |roots, view, _| {
                        assert_eq!(roots.len(), 1);
                        assert_eq!(roots[0].access_sources.len(), 1);
                        assert!(roots[0].executable_effect_sources.is_empty());
                        assert!(roots[0].bounds_are_clean());
                        assert!(roots[0].all_kernel_checks_are_clean());
                        assert_eq!(stores(view.output()), 1);
                        assert!(!view.grants_authority());
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(roots.len(), 1);
            });
        }
    }

    #[test]
    fn actual_partial_control_keeps_wrapped_root_and_body_axes() {
        for condition in [false, true] {
            let source = wrapped_branch_source(condition);
            with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
                let (roots, ()) = with_projected_checked_output_roots_v1(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    &[ranked_root_input_1d(A_NAME, 247, 64)],
                    &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                    budget,
                    |roots, _, _| {
                        assert_eq!(roots.len(), 1);
                        assert_eq!(roots[0].semantic_root(), ROOT);
                        assert_eq!(
                            roots[0].semantic_u32_induction.function(),
                            SemanticFunctionIdV1::from_index(1)
                        );
                        assert_eq!(roots[0].access_sources.len(), 1);
                        Ok(())
                    },
                )
                .unwrap();
                assert!(roots[0].all_kernel_checks_are_clean());
            });
        }
    }

    #[test]
    fn checked_source_coverage_rejects_foreign_function_and_missing_block() {
        let source = assertion_materialized(branch_source(1, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let mut facts = session.for_source(ROOT, SemanticFunctionIdV1::from_index(99));
                    assert!(facts.checked_block_coverage_v1(0).is_err());
                    drop(facts);
                    let mut facts = session.for_source(ROOT, ROOT);
                    assert!(facts.checked_block_coverage_v1(99).is_err());
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn actual_failed_bounds_condition_is_validated_at_the_guard() {
        let source = assertion_materialized(assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    201,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_constant(A_BOOL, 0, 1),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: typed_constant(A_U64, 8, 8),
                            index: typed_constant(A_U64, 8, 8),
                        },
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    202,
                    assertion_ranked_write_statements(1, 2),
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        ));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            assert_eq!(stores(checked.owner()), 0);
            let result = with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let semantic = source.semantic_ssa().source_semantic();
                    let mut facts = session.for_source(ROOT, ROOT);
                    facts.with_checked_control_scope_v1(|facts| {
                        checked_control_v1::prepare(
                            semantic.types(),
                            &semantic.functions()[0],
                            semantic.callables(),
                            facts,
                        )
                        .map(|_| ())
                    })
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                    block: 0,
                    kind: "bounds-check",
                    ..
                })
            ));
        });
    }

    #[test]
    fn all_checked_dormant_effects_do_not_authorize_an_empty_ranked_root() {
        let source = assertion_materialized(assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    211,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_constant(A_BOOL, 1, 1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                1,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(212, vec![], SemanticTerminatorKindV1::Return),
                block(
                    213,
                    assertion_ranked_write_statements(1, 2),
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        ));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            assert_eq!(stores(source.executable()), 1);
            assert_eq!(stores(checked.owner()), 0);
            let called = std::cell::Cell::new(false);
            let result = with_projected_checked_output_roots_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                &[ranked_root_input_1d(A_NAME, 247, 64)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |_, _, _| {
                    called.set(true);
                    Ok(())
                },
            );
            assert!(!called.get());
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a kernel without a statically ranked indexed memory access"
                ))
            ));
        });
    }

    #[test]
    fn checked_control_public_query_component_has_independent_source_bounds() {
        let source = assertion_materialized(branch_source(1, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    session.with_output_occurrences_v1(|view, inherited| {
                        // One source function, three materialized blocks: the
                        // source-function lookup costs 4+1; sorted block lookup
                        // visits are 2,1,2, each costing 1+3. Coverage adds 12
                        // plus block's final read1: 26,22,26. The selected-edge
                        // public query is 29+5+8=42; its mapping16 is a subset.
                        // This public-query group excludes adapter and frame work.
                        const HISTORY: usize = 7;
                        const PUBLIC_QUERIES: usize = 26 + 22 + 26 + 42;
                        assert_eq!(PUBLIC_QUERIES, 116);
                        let floor = inherited.storage();
                        for (repeats, limit, accepted, denied) in [
                            (1, HISTORY + PUBLIC_QUERIES, 123, None),
                            (2, HISTORY + 2 * PUBLIC_QUERIES, 239, None),
                            (2, HISTORY + 2 * PUBLIC_QUERIES - 1, 237, Some(239)),
                        ] {
                            let mut work = Work::new(limit);
                            let mut query = Budget::new(&mut work, floor);
                            query.charge_work(HISTORY).unwrap();
                            query.reserve_storage(floor).unwrap();
                            let result = (|| -> Result<(), ProductionSourceOutputErrorV1> {
                                for _ in 0..repeats {
                                    for block in 0..3 {
                                        let coverage = view.block_coverage(
                                            ROOT,
                                            ROOT,
                                            SemanticBlockIdV1::from_index(block),
                                            &mut query,
                                        )?;
                                        assert_eq!(
                                            coverage.source_statements(),
                                            if block == 0 { 0 } else { 2 }
                                        );
                                        assert!(coverage.original_operations().is_some());
                                    }
                                    let selected = view
                                        .selected_successor(
                                            ROOT,
                                            ROOT,
                                            SemanticBlockIdV1::from_index(0),
                                            &mut query,
                                        )?
                                        .unwrap();
                                    assert_eq!(selected.semantic_ordinal(), 0);
                                    assert_eq!(selected.semantic_target().index(), 1);
                                }
                                Ok(())
                            })();
                            assert_eq!(result.is_ok(), denied.is_none());
                            assert_eq!(query.work(), accepted);
                            assert_eq!(query.storage(), floor);
                            assert_eq!(query.peak_storage(), floor);
                            assert_eq!(query.failed_storage(), None);
                            drop(query);
                            assert_eq!(work.failed_work(), denied);
                        }
                        Ok(())
                    })
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn checked_control_frame_bounds_preserve_history_prefixes_and_scope_floor() {
        let source = assertion_materialized(branch_source(1, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    // These are fixed-fixture construction bounds, not measured
                    // caps and not bounds for source/B/O admission or the root.
                    // Coverage adapters: (26+1)+(22+1)+(26+1)=77.
                    // Selected adapter: public42+conversion1=43.
                    // Frame-local work: entry4+rows12+grammar18+switch10+
                    // queue3+reach12+agreement6=65. prepare=77+43+65=185.
                    // into_cfg adds dispatch4+three row visits9=13.
                    const HISTORY: usize = 7;
                    const PREPARE: usize = 185;
                    const CONVERT: usize = 13;
                    let floor =
                        session.with_output_occurrences_v1(|_, budget| Ok(budget.storage()))?;
                    let row_bytes =
                        std::mem::size_of::<checked_control_v1::CheckedControlFrameV1>()
                            + 3 * checked_control_v1::CONTROL_ROW_LAYOUT_BYTES_FOR_TEST;
                    let prepare_bytes = row_bytes
                        + std::mem::size_of::<Vec<usize>>()
                        + 3 * std::mem::size_of::<usize>();
                    let convert_bytes = std::mem::size_of::<Vec<ProjectedCfgTerminatorV1>>()
                        + std::mem::size_of::<Vec<bool>>()
                        + 3 * (std::mem::size_of::<ProjectedCfgTerminatorV1>()
                            + std::mem::size_of::<bool>());
                    let semantic = source.semantic_ssa().source_semantic();
                    assert_eq!(semantic.functions().len(), 1);
                    assert_eq!(semantic.functions()[0].blocks().len(), 3);
                    // The projection skeleton is a borrowed test input for CFG
                    // conversion only; no live-effect correspondence is asserted.
                    let projected: [ProjectedSemanticBlockV1; 3] =
                        std::array::from_fn(|_| ProjectedSemanticBlockV1 { items: Vec::new() });
                    for (
                        convert,
                        work_limit,
                        storage_limit,
                        accepted,
                        denied_work,
                        denied_storage,
                        peak,
                    ) in [
                        (
                            false,
                            HISTORY + PREPARE,
                            floor + prepare_bytes,
                            192,
                            None,
                            None,
                            floor + prepare_bytes,
                        ),
                        (
                            false,
                            HISTORY + PREPARE - 1,
                            floor + prepare_bytes,
                            190,
                            Some(192),
                            None,
                            floor + prepare_bytes,
                        ),
                        // Queue reservation follows frame prefix167, history7.
                        (
                            false,
                            HISTORY + PREPARE,
                            floor + prepare_bytes - 1,
                            174,
                            None,
                            Some(floor + prepare_bytes),
                            floor + row_bytes,
                        ),
                        (
                            true,
                            HISTORY + PREPARE + CONVERT,
                            floor + prepare_bytes + convert_bytes,
                            205,
                            None,
                            None,
                            floor + prepare_bytes + convert_bytes,
                        ),
                        (
                            true,
                            HISTORY + PREPARE + CONVERT - 1,
                            floor + prepare_bytes + convert_bytes,
                            202,
                            Some(205),
                            None,
                            floor + prepare_bytes + convert_bytes,
                        ),
                        // Conversion reserves after prepare185+dispatch4+history7.
                        (
                            true,
                            HISTORY + PREPARE + CONVERT,
                            floor + prepare_bytes + convert_bytes - 1,
                            196,
                            None,
                            Some(floor + prepare_bytes + convert_bytes),
                            floor + prepare_bytes,
                        ),
                    ] {
                        let mut work = Work::new(work_limit);
                        let mut query = Budget::new(&mut work, storage_limit);
                        query.charge_work(HISTORY).unwrap();
                        query.reserve_storage(floor).unwrap();
                        let result = {
                            let mut facts =
                                session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT);
                            facts.with_checked_control_scope_v1(|facts| {
                                let frame = checked_control_v1::prepare(
                                    semantic.types(),
                                    &semantic.functions()[0],
                                    semantic.callables(),
                                    facts,
                                )?
                                .unwrap();
                                assert!(frame.projects_block(1));
                                if convert {
                                    let (terminators, reached) =
                                        frame.into_cfg(&projected, facts)?;
                                    assert_eq!(reached.as_slice(), &[true, true, false]);
                                    assert!(matches!(
                                        terminators[0],
                                        ProjectedCfgTerminatorV1::Branch(1)
                                    ));
                                    assert!(matches!(
                                        terminators[2],
                                        ProjectedCfgTerminatorV1::AbsentMaterialized
                                    ));
                                    drop((terminators, reached));
                                } else {
                                    drop(frame);
                                }
                                Ok(())
                            })
                        };
                        assert_eq!(
                            result.is_ok(),
                            denied_work.is_none() && denied_storage.is_none()
                        );
                        assert_eq!(query.work(), accepted);
                        assert_eq!(query.storage(), floor);
                        assert_eq!(query.peak_storage(), peak);
                        assert_eq!(query.failed_storage(), denied_storage);
                        drop(query);
                        assert_eq!(work.failed_work(), denied_work);
                    }
                    for (limit, accepted, denied) in [(377, 377, None), (376, 375, Some(377))] {
                        let mut work = Work::new(limit);
                        let mut query = Budget::new(&mut work, floor + prepare_bytes);
                        query.charge_work(HISTORY).unwrap();
                        query.reserve_storage(floor).unwrap();
                        let mut result = Ok(());
                        for _ in 0..2 {
                            let mut facts =
                                session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT);
                            result = facts.with_checked_control_scope_v1(|facts| {
                                let _frame = checked_control_v1::prepare(
                                    semantic.types(),
                                    &semantic.functions()[0],
                                    semantic.callables(),
                                    facts,
                                )?
                                .unwrap();
                                Ok(())
                            });
                            if result.is_err() {
                                break;
                            }
                        }
                        assert_eq!(result.is_ok(), denied.is_none());
                        assert_eq!(query.work(), accepted);
                        assert_eq!(query.storage(), floor);
                        assert_eq!(query.peak_storage(), floor + prepare_bytes);
                        drop(query);
                        assert_eq!(work.failed_work(), denied);
                    }
                    for panic in [false, true] {
                        let mut work = Work::new(HISTORY + PREPARE);
                        let mut query = Budget::new(&mut work, floor + prepare_bytes);
                        query.charge_work(HISTORY).unwrap();
                        query.reserve_storage(floor).unwrap();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let mut facts =
                                session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT);
                            facts.with_checked_control_scope_v1(|facts| {
                                let _frame = checked_control_v1::prepare(
                                    semantic.types(),
                                    &semantic.functions()[0],
                                    semantic.callables(),
                                    facts,
                                )?
                                .unwrap();
                                if panic {
                                    panic!("control callback unwind");
                                }
                                Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                                    "control callback failure",
                                ))
                            })
                        }));
                        assert_eq!(result.is_err(), panic);
                        if let Ok(result) = result {
                            assert!(result.is_err());
                        }
                        assert_eq!(query.storage(), floor);
                        assert_eq!(query.work(), HISTORY + PREPARE);
                        assert_eq!(query.peak_storage(), floor + prepare_bytes);
                        drop(query);
                        assert_eq!(work.failed_work(), None);
                    }
                    Ok(())
                },
            )
            .unwrap();
        });
    }
}
