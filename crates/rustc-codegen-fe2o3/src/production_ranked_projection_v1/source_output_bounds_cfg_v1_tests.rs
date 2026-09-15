mod bounds_cfg_tests {
    use super::*;
    use crate::production_ranked_projection_v1::bounds_cfg_v1;

    fn guard_rows(function: &SemanticFunctionDeclV1) -> Vec<ProjectedBoundsCheckV1> {
        let mut operations = Vec::new();
        let mut next = 0;
        project_rust_bounds_checks(function, 0, &[], &mut operations, &mut next)
            .unwrap()
            .checks
    }

    fn replace_function(
        function: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
            locals,
            function.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone())
    }

    #[test]
    fn actual_bounds_guard_crosses_selected_control_and_keeps_mandatory_race() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let source =
                genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 1, true);
            with_actual_dynamic_global_root_v1(&source, profile, 1, |root, _, _| {
                assert_eq!(root.access_sources.len(), 1);
                assert_eq!(root.access_sources[0].semantic_block(), 1);
                Ok(())
            })
            .unwrap();
            let source =
                genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 64, true);
            let called = std::cell::Cell::new(false);
            let result = with_actual_dynamic_global_root_v1(&source, profile, 64, |_, _, _| {
                called.set(true);
                Ok(())
            });
            assert!(
                matches!(result, Err(ProductionRankedProjectionErrorV1::Compile { error, .. })
                if matches!(*error, fe2o3_pliron::ProductionRankedCompileErrorV1::Session(
                    fe2o3_pliron::ProductionSessionErrorV1::RankedRace(_))))
            );
            assert!(!called.get());
        }
    }

    #[test]
    fn actual_source_ssa_guard_is_exact_and_success_gateway_must_dominate() {
        let source =
            genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 1, true);
        let ssa = source.semantic_ssa();
        let semantic = ssa.source_semantic();
        let function = &semantic.functions()[0];
        let original_rows = guard_rows(function);
        assert_eq!(original_rows.len(), 1);
        assert_eq!(original_rows[0].guard_block, 0);
        assert_eq!(original_rows[0].access_block, 3);
        // Preserve the exact comparison but give its false edge a path to the
        // access. Assert-block domination alone would incorrectly admit this.
        let mut blocks = function.blocks().to_vec();
        blocks[0] = block(
            201,
            function.blocks()[0].statements().to_vec(),
            zero_switch(9, A_BOOL, 1, 3),
        );
        let bypass = assertion_ssa_functions(
            semantic.types().to_vec(),
            vec![replace_function(
                function,
                function.locals().to_vec(),
                blocks,
            )],
        );
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    profile,
                    budget,
                    |session| {
                        let mut facts = session.for_source(ROOT, ROOT);
                        let mut rows = original_rows.clone();
                        bounds_cfg_v1::authenticate_invariants(
                            function,
                            ssa.plan_for_function(ROOT).unwrap().plan(),
                            &mut rows,
                            &mut facts,
                        )?;
                        assert!(rows[0].invariant_ssa);
                        let dominance =
                            SemanticEnumPayloadDominanceV1::analyze(function, semantic.types())
                                .unwrap();
                        assert!(bounds_cfg_v1::controls(rows[0], 1, Some(&dominance)));
                        assert!(
                            projected_bounds_check_with_dominance(
                                &rows,
                                1,
                                SemanticLocalIdV1::from_index(2),
                                rows[0].index_local,
                                Some(&dominance)
                            )
                            .is_err()
                        );
                        assert!(
                            projected_bounds_check_with_dominance(
                                &rows,
                                1,
                                rows[0].slice_local,
                                SemanticLocalIdV1::from_index(7),
                                Some(&dominance)
                            )
                            .is_err()
                        );
                        assert!(bounds_cfg_v1::deferred_guard(
                            &rows,
                            0,
                            SemanticBlockIdV1::from_index(3),
                            &mut facts
                        )?);
                        assert!(!bounds_cfg_v1::deferred_guard(
                            &rows,
                            0,
                            SemanticBlockIdV1::from_index(1),
                            &mut facts
                        )?);
                        assert!(!bounds_cfg_v1::deferred_guard(
                            &rows,
                            1,
                            SemanticBlockIdV1::from_index(3),
                            &mut facts
                        )?);
                        let bypass_function = &bypass.source_semantic().functions()[0];
                        let mut rows = guard_rows(bypass_function);
                        bounds_cfg_v1::authenticate_invariants(
                            bypass_function,
                            bypass.plan_for_function(ROOT).unwrap().plan(),
                            &mut rows,
                            &mut facts,
                        )?;
                        assert!(rows[0].invariant_ssa);
                        let dominance = SemanticEnumPayloadDominanceV1::analyze(
                            bypass_function,
                            semantic.types(),
                        )
                        .unwrap();
                        assert!(dominance.block_dominates(
                            SemanticBlockIdV1::from_index(0),
                            SemanticBlockIdV1::from_index(1)
                        ));
                        assert!(!bounds_cfg_v1::controls(rows[0], 1, Some(&dominance)));
                        assert!(
                            projected_bounds_check_with_dominance(
                                &rows,
                                1,
                                rows[0].slice_local,
                                rows[0].index_local,
                                Some(&dominance)
                            )
                            .is_err()
                        );
                        Ok(())
                    },
                )
                .unwrap();
            });
        }
    }

    #[test]
    fn actual_source_ssa_rejects_changed_base_index_metadata_and_projected_storage() {
        let source =
            genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 1, true);
        let semantic = source.semantic_ssa().source_semantic();
        let function = &semantic.functions()[0];
        let original = guard_rows(function);
        for mutation in 0..6 {
            let mut blocks = function.blocks().to_vec();
            let mut statements = blocks[3].statements().to_vec();
            let mut locals = function.locals().to_vec();
            statements.push(match mutation {
                0 => typed_assignment(
                    8,
                    A_U64,
                    SemanticRvalueKindV1::Use(typed_constant(A_U64, 1, 8)),
                ),
                1 => typed_assignment(
                    7,
                    A_U64,
                    SemanticRvalueKindV1::Use(typed_constant(A_U64, 9, 8)),
                ),
                2 => typed_assignment(
                    1,
                    G_POINTER,
                    SemanticRvalueKindV1::Use(typed_operand(2, G_POINTER)),
                ),
                3 => statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(8),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, A_U64)
                                .unwrap(),
                        ],
                        A_U64,
                    )
                    .unwrap(),
                    SemanticRvalueV1::new(
                        A_U64,
                        SemanticRvalueKindV1::Use(typed_constant(A_U64, 1, 8)),
                    ),
                ))),
                4 => {
                    locals.push(local(219, A_BOOL_PTR, SemanticLocalRoleV1::Temporary));
                    typed_assignment(
                        14,
                        A_BOOL_PTR,
                        SemanticRvalueKindV1::AddressOf {
                            mutability: SemanticMutabilityV1::Mutable,
                            place: typed_place(9, A_BOOL),
                        },
                    )
                }
                _ => typed_assignment(
                    8,
                    A_U64,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: typed_operand(8, A_U64),
                        right: typed_constant(A_U64, 1, 8),
                    },
                ),
            });
            let terminator = if mutation == 5 {
                zero_switch(6, A_BOOL, 1, 3)
            } else {
                blocks[3].terminator().kind().clone()
            };
            blocks[3] = block(204, statements, terminator);
            let mutated = assertion_ssa_functions(
                semantic.types().to_vec(),
                vec![replace_function(function, locals, blocks)],
            );
            with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
                with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    budget,
                    |session| {
                        // This component test supplies only the real canonical ledger;
                        // the independently admitted mutant plan grants no O authority.
                        let mut facts = session.for_source(ROOT, ROOT);
                        let mut rows = original.clone();
                        bounds_cfg_v1::authenticate_invariants(
                            &mutated.source_semantic().functions()[0],
                            mutated.plan_for_function(ROOT).unwrap().plan(),
                            &mut rows,
                            &mut facts,
                        )?;
                        assert!(!rows[0].invariant_ssa, "mutation {mutation}");
                        assert!(!bounds_cfg_v1::deferred_guard(
                            &rows,
                            0,
                            SemanticBlockIdV1::from_index(3),
                            &mut facts
                        )?);
                        // Existing immediate-success eligibility remains independent
                        // of the deliberately stronger cross-block SSA requirement.
                        assert!(bounds_cfg_v1::controls(rows[0], 3, None));
                        assert!(!bounds_cfg_v1::controls(rows[0], 1, None));
                        Ok(())
                    },
                )
                .unwrap();
            });
        }
    }

    #[test]
    fn actual_bounds_ssa_query_has_derived_exact_under_prefix_and_unwind_bounds() {
        let source =
            genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::Parameter, 1, true);
        let ssa = source.semantic_ssa();
        let function = &ssa.source_semantic().functions()[0];
        let plan = ssa.plan_for_function(ROOT).unwrap().plan();
        let original = guard_rows(function);
        assert_eq!(function.locals().len(), 14);
        assert_eq!(function.blocks().len(), 4);
        assert_eq!(plan.promoted_variables().len(), 14);
        assert_eq!(plan.entry_definitions().len(), 3);
        let events = (0..4)
            .map(|block| {
                plan.resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(block))
                    .unwrap()
                    .len()
            })
            .collect::<Vec<_>>();
        // Source grammar: guard 7 statement + 3 terminator events; two
        // indexed stores have base/index/value uses; selector define/use.
        assert_eq!(events, vec![10, 3, 3, 2]);
        assert_eq!(original.len(), 1);
        const PREFIX: usize = 7;
        const EXACT: usize = 4 + 4 + 14 + 2 * 14 + 3 * 3 + 2 * 4 + 4 * 18 + 12;
        let scratch =
            std::mem::size_of::<Vec<usize>>() + 14 * bounds_cfg_v1::INVARIANT_ROW_BYTES_FOR_TEST;
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let floor =
                        session.with_output_occurrences_v1(|_, budget| Ok(budget.storage()))?;
                    for repeats in [1, 2] {
                        for under in [false, true] {
                            let limit = PREFIX + repeats * EXACT - usize::from(under);
                            let mut work = Work::new(limit);
                            let mut query = Budget::new(&mut work, floor + scratch);
                            query.charge_work(PREFIX).unwrap();
                            query.reserve_storage(floor).unwrap();
                            for iteration in 0..repeats {
                                let mut rows = original.clone();
                                let mut facts =
                                    session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT);
                                let result = bounds_cfg_v1::authenticate_invariants(
                                    function, plan, &mut rows, &mut facts,
                                );
                                let success = !under || iteration + 1 < repeats;
                                assert_eq!(result.is_ok(), success);
                                assert_eq!(rows[0].invariant_ssa, success);
                            }
                            assert_eq!(query.storage(), floor);
                            assert_eq!(query.peak_storage(), floor + scratch);
                            assert_eq!(query.failed_storage(), None);
                            assert_eq!(
                                query.work(),
                                PREFIX + repeats * EXACT - if under { 12 } else { 0 }
                            );
                            assert_eq!(
                                work.failed_work(),
                                under.then_some(PREFIX + repeats * EXACT)
                            );
                        }
                    }
                    let mut work = Work::new(PREFIX + EXACT);
                    let mut query = Budget::new(&mut work, floor + scratch - 1);
                    query.charge_work(PREFIX).unwrap();
                    query.reserve_storage(floor).unwrap();
                    let mut rows = original.clone();
                    let result = bounds_cfg_v1::authenticate_invariants(
                        function,
                        plan,
                        &mut rows,
                        &mut session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT),
                    );
                    assert!(result.is_err());
                    assert_eq!(query.work(), PREFIX + 4);
                    assert_eq!(query.storage(), floor);
                    assert_eq!(query.peak_storage(), floor);
                    assert_eq!(query.failed_storage(), Some(floor + scratch));
                    // One guard dispatch, one Assign dispatch, destination place
                    // header3 + two projections*12, and Copy operand1 + base place3.
                    const STORE_PREFLIGHT: usize = 1 + 1 + 3 + 2 * 12 + 1 + 3;
                    for under in [false, true] {
                        let mut work = Work::new(PREFIX + STORE_PREFLIGHT - usize::from(under));
                        let mut query = Budget::new(&mut work, floor);
                        query.charge_work(PREFIX).unwrap();
                        query.reserve_storage(floor).unwrap();
                        let mut rows = original.clone();
                        // Cost-only eligibility flag, not a fake guard/root witness.
                        rows[0].invariant_ssa = true;
                        let result = bounds_cfg_v1::prepay_statement_lookups(
                            &rows,
                            function.blocks()[1].statements()[0].kind(),
                            &mut session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT),
                        );
                        assert_eq!(result.is_ok(), !under);
                        assert_eq!(query.storage(), floor);
                        assert_eq!(
                            query.work(),
                            PREFIX + STORE_PREFLIGHT - if under { 3 } else { 0 }
                        );
                        assert_eq!(
                            work.failed_work(),
                            under.then_some(PREFIX + STORE_PREFLIGHT)
                        );
                    }
                    for panic in [false, true] {
                        let mut work = Work::new(PREFIX + EXACT);
                        let mut query = Budget::new(&mut work, floor + scratch);
                        query.charge_work(PREFIX).unwrap();
                        query.reserve_storage(floor).unwrap();
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let mut facts =
                                    session.for_source_with_query_budget_v1(&mut query, ROOT, ROOT);
                                facts.with_checked_control_scope_v1(|facts| {
                                    let mut rows = original.clone();
                                    bounds_cfg_v1::authenticate_invariants(
                                        function, plan, &mut rows, facts,
                                    )?;
                                    assert!(rows[0].invariant_ssa);
                                    if panic {
                                        panic!("bounds SSA callback unwind");
                                    }
                                    Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                                        "bounds SSA callback failure",
                                    ))
                                })
                            }));
                        assert_eq!(outcome.is_err(), panic);
                        assert_eq!(query.storage(), floor);
                        assert_eq!(query.work(), PREFIX + EXACT);
                        assert_eq!(query.peak_storage(), floor + scratch);
                    }
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    include!("source_output_bounds_source_guard_v1_tests.rs");
}
