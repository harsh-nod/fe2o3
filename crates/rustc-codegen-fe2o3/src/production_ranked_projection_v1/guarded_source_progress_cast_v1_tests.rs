use super::*;

fn projected(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
) -> (
    ProjectedUniformInductionV1,
    Vec<ProductionRankedOperationV1>,
) {
    let constants = constant_locals(function).unwrap();
    let origins = local_stable_argument_origins(types, function).unwrap();
    let definitions = local_definition_counts(function);
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 0;
    let mut operations = Vec::new();
    let mut next_value = 0;
    let mut inductions = project_uniform_inductions_v1(
        &[],
        types,
        function,
        &constants,
        &origins,
        &definitions,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )
    .unwrap();
    assert_eq!(inductions.len(), 1);
    let raw = inductions[0].source_progress.ranked_bound;
    assert_eq!(inductions[0].bound, raw);
    reconcile_source_progress_and_emit_unsigned_casts_v1(
        types,
        function,
        &constants,
        &origins,
        &definitions,
        &arguments,
        &mut inductions,
        &mut operations,
        &mut next_value,
    )
    .unwrap();
    let induction = inductions.remove(0);
    assert_eq!(induction.source_progress.ranked_bound, raw);
    assert_ne!(induction.bound, raw);
    assert_eq!(induction.bound_cast.as_ref().unwrap().ranked_value, raw);
    assert!(operations.iter().any(|operation| matches!(operation,
        ProductionRankedOperationV1::IndexUnsignedCast { result, source, bit_width: 32 }
        if ProductionRankedValueV1::Local(*result) == induction.bound && *source == raw
    )));
    (induction, operations)
}

#[test]
fn genuine_reconciler_result_joins_without_relabeling_raw_source_bound() {
    with_candidate(|progress, facts, types, function| {
        let (induction, operations) = projected(types, function);
        let raw = induction.source_progress.ranked_bound;
        progress
            .with_source(
                0,
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                facts,
                |facts| {
                    facts.require_guarded_source_progress_v1(
                        types,
                        function,
                        &induction,
                        &operations,
                    )
                },
            )
            .unwrap();
        assert_eq!(induction.source_progress.ranked_bound, raw);
        assert_ne!(induction.bound, raw);
        assert!(progress.sites[0].consumed);
        progress.finish(facts.0).unwrap();
    });
}

fn dual_result(
    result: ProductionRankedValueIdV1,
    success: ProductionRankedValueIdV1,
) -> ProductionRankedOperationV1 {
    let value = ProductionRankedValueV1::Argument(0);
    ProductionRankedOperationV1::PredicatedCheckedRowStripedIndex2D {
        result,
        success,
        invocation: value,
        component: value,
        rows: value,
        columns: value,
        row_stride: value,
        physical_extent: value,
        lanes_per_row: 1,
        elements_per_lane: 1,
    }
}

#[test]
fn cast_transport_refuses_stale_recipe_missing_changed_and_duplicate_definitions() {
    for mode in 0..22 {
        with_candidate(|progress, facts, types, function| {
            let (mut induction, mut operations) = projected(types, function);
            let ProductionRankedValueV1::Local(result) = induction.bound else {
                unreachable!()
            };
            let cast_index = operations.iter().position(|operation| matches!(operation,
                ProductionRankedOperationV1::IndexUnsignedCast { result: actual, .. } if *actual == result
            )).unwrap();
            let other = ProductionRankedValueIdV1::new(987);
            match mode {
                0 => {
                    operations.remove(cast_index);
                }
                1 => {
                    operations[cast_index] = ProductionRankedOperationV1::IndexUnsignedCast {
                        result,
                        source: induction.source_progress.ranked_bound,
                        bit_width: 16,
                    }
                }
                2 => {
                    operations[cast_index] = ProductionRankedOperationV1::IndexUnsignedCast {
                        result,
                        source: ProductionRankedValueV1::Argument(9),
                        bit_width: 32,
                    }
                }
                3 => {
                    operations[cast_index] = ProductionRankedOperationV1::IndexUnsignedCast {
                        result: other,
                        source: induction.source_progress.ranked_bound,
                        bit_width: 32,
                    }
                }
                4 => operations[cast_index] = ProductionRankedOperationV1::IndexUnknown { result },
                5 => operations.push(operations[cast_index].clone()),
                6 => {
                    operations.push(ProductionRankedOperationV1::IndexConstant { result, value: 0 })
                }
                7 => operations.insert(0, ProductionRankedOperationV1::IndexUnknown { result }),
                8 => operations.push(dual_result(result, other)),
                9 => operations.push(dual_result(other, result)),
                10 => induction.bound_cast.as_mut().unwrap().header = 0,
                11 => induction.bound_cast.as_mut().unwrap().comparison_statement = 0,
                12 => induction.bound_cast.as_mut().unwrap().source_type = BOOL,
                13 => induction.bound_cast.as_mut().unwrap().source_operand = value(1, U32),
                14 => {
                    induction.bound_cast.as_mut().unwrap().ranked_value =
                        ProductionRankedValueV1::Argument(9)
                }
                15 => induction.bound_cast.as_mut().unwrap().bit_width = 64,
                16 => induction.bound = induction.source_progress.ranked_bound,
                17 => induction.bound = ProductionRankedValueV1::Argument(9),
                18 => induction.bound = ProductionRankedValueV1::Local(other),
                19 => induction.bound_cast = None,
                20 => {
                    induction.bound_cast.as_mut().unwrap().source_operand = SemanticOperandV1::Move(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], U32)
                            .unwrap(),
                    )
                }
                _ => induction.bound_cast.as_mut().unwrap().bit_width = 16,
            }
            let before = facts.0.storage();
            assert!(
                progress
                    .with_source(
                        0,
                        SemanticFunctionIdV1::from_index(0),
                        SemanticFunctionIdV1::from_index(0),
                        facts,
                        |facts| facts.require_guarded_source_progress_v1(
                            types,
                            function,
                            &induction,
                            &operations
                        ),
                    )
                    .is_err(),
                "cast mutation {mode}"
            );
            assert!(!progress.sites[0].consumed);
            assert!(progress.finish(facts.0).is_err());
            assert_eq!(facts.0.storage(), before);
        });
    }
}

#[test]
fn narrower_reconciled_width_transport_requires_its_exact_emitted_width() {
    // Isolated provenance checks, not newly issued source range proofs. The
    // production reconciler must already have established the retained width.
    for width in [8, 16, 32] {
        let mut induction = candidate(true);
        let raw = induction.bound;
        let result = ProductionRankedValueIdV1::new(2);
        induction.bound_cast = Some(ProjectedUnsignedCastCandidateV1 {
            header: induction.header,
            comparison_statement: induction.source_progress.header_statement,
            source_operand: induction.source_progress.bound_operand.clone(),
            source_type: U32,
            ranked_value: raw,
            bit_width: width,
        });
        induction.bound = ProductionRankedValueV1::Local(result);
        let mut work = Work::new(18);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        require_ranked_bound(
            &induction,
            &[ProductionRankedOperationV1::IndexUnsignedCast {
                result,
                source: raw,
                bit_width: width,
            }],
            &mut Facts(&mut budget),
        )
        .unwrap();
        assert_eq!((budget.work(), budget.storage()), (18, FLOOR));
    }
}

#[test]
fn exact_and_short_cast_scan_work_is_linear_and_has_no_storage_delta() {
    with_candidate(|_, _, types, function| {
        let (induction, base) = projected(types, function);
        for extra in [0, 64] {
            let mut operations = base.clone();
            for index in 0..extra {
                operations.push(ProductionRankedOperationV1::IndexUnknown {
                    result: ProductionRankedValueIdV1::new(1000 + index),
                });
            }
            let exact = 12 + 6 * operations.len();
            for short in [false, true] {
                let mut work = Work::new(exact - usize::from(short));
                let mut budget = Budget::new(&mut work, FLOOR);
                budget.reserve_storage(FLOOR).unwrap();
                let result = require_ranked_bound(&induction, &operations, &mut Facts(&mut budget));
                if short {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::Resource(Resource::Work(_))
                        ))
                    ));
                } else {
                    result.unwrap();
                    assert_eq!(budget.work(), exact);
                }
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    });
}

#[test]
fn historical_default_hook_has_no_new_work_or_storage() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let (ssa, _) = fixture::source(30);
    let source = ssa.source_semantic();
    Facts(&mut budget)
        .require_guarded_source_progress_v1(
            source.types(),
            &source.functions()[0],
            &candidate(false),
            &[],
        )
        .unwrap();
    assert_eq!((budget.work(), budget.storage()), (0, FLOOR));
}
