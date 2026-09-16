mod canonical_assertion_graph_tests {
    use super::super::canonical_assertion_facts_v1::*;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    include!("canonical_assertion_fixtures_v1_tests.rs");
    include!("materialized_callable_effect_v1_tests.rs");
    include!("legacy_assertion_scope_v1_tests.rs");

    const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

    #[test]
    fn genuine_wrapped_root_uses_selected_body_assertion_coordinates() {
        let materialized = wrapped_literal_assertion();
        with_canonical_assertions_v1(&materialized, |session| {
            {
                let mut facts = session.for_source(ROOT, SemanticFunctionIdV1::from_index(1));
                assert!(facts.is_materialized_block(0)?);
                assert_eq!(
                    facts.condition(0, true, SemanticBlockIdV1::from_index(1))?,
                    ProjectedAssertionConditionV1::Bool(true)
                );
            }
            let mut wrong_body = session.for_source(ROOT, ROOT);
            assert!(matches!(
                wrong_body.condition(0, true, SemanticBlockIdV1::from_index(1)),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Origin(_)
                ))
            ));
            Ok(())
        })
        .unwrap();
        let program = assertion_project(materialized).unwrap();
        let report = &program.roots[0].semantic_u32_induction;
        assert_eq!(report.function(), SemanticFunctionIdV1::from_index(1));
    }

    #[test]
    fn genuine_literal_and_alias_assertions_use_exact_bool_polarity() {
        for alias in [false, true] {
            for expected in [false, true] {
                assertion_project(assertion_materialized(literal_assertion(
                    expected, expected, alias,
                )))
                .unwrap();
                assert!(matches!(
                    assertion_project(assertion_materialized(literal_assertion(
                        !expected, expected, alias
                    ))),
                    Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                        block: 0,
                        kind: "division-by-zero",
                        ..
                    })
                ));
            }
        }
    }

    #[test]
    fn genuine_opposing_graph_bool_overrides_a_claimed_source_proof() {
        let function = literal_assertion(false, true, false);
        let materialized = assertion_materialized(function.clone());
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            assert!(facts.is_materialized_block(0)?);
            assert!(matches!(
                projected_cfg_terminator(&function, 0, &[], true, &mut facts, &[], &[]),
                Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                    block: 0,
                    kind: "division-by-zero",
                    ..
                })
            ));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn genuine_dynamic_argument_is_not_a_proof() {
        let function = dynamic_assertion();
        let materialized = assertion_materialized(function);
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            assert_eq!(
                facts.condition(0, true, SemanticBlockIdV1::from_index(1))?,
                ProjectedAssertionConditionV1::Dynamic
            );
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            assertion_project(materialized),
            Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                block: 0,
                kind: "division-by-zero",
                ..
            })
        ));
    }

    #[test]
    fn genuine_escaped_constants_and_checked_results_do_not_supply_assertion_proofs() {
        for (checked, kind) in [(false, "division-by-zero"), (true, "arithmetic-overflow")] {
            let function = escaped_assertion(checked);
            assert!(!SemanticAssertProofsV1::analyze(&assertion_types(), &function).unwrap()[0]);
            let materialized = assertion_materialized(function);
            with_canonical_assertions_v1(&materialized, |session| {
                let mut facts = session.for_source(ROOT, ROOT);
                assert_eq!(
                    facts.condition(0, !checked, SemanticBlockIdV1::from_index(1))?,
                    ProjectedAssertionConditionV1::Dynamic
                );
                Ok(())
            })
            .unwrap();
            assert!(matches!(assertion_project(materialized),
                Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                    block: 0, kind: actual, ..
                }) if actual == kind));
        }
    }

    #[test]
    fn genuine_checked_induction_retains_its_separate_source_progress_proof() {
        let function = checked_induction_assertion();
        let types = assertion_types();
        let (inductions, _, _) = project_test_inductions_with_types(&types, &function).unwrap();
        assert_eq!(inductions.len(), 1);
        assert_eq!(
            inductions[0]
                .source_progress
                .update
                .proved_overflow_assert_block(),
            Some(3)
        );
        let materialized = assertion_materialized(function.clone());
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            assert_eq!(
                facts.condition(3, false, SemanticBlockIdV1::from_index(4))?,
                ProjectedAssertionConditionV1::Dynamic
            );
            assert_eq!(
                projected_cfg_terminator(&function, 3, &[], true, &mut facts, &[], &[])?,
                ProjectedCfgTerminatorV1::Branch(4)
            );
            Ok(())
        })
        .unwrap();
        assertion_project(materialized).unwrap();
    }

    #[test]
    fn genuine_absent_source_assertion_is_not_queried_or_relabelled_elided() {
        let function = assertion_root(
            vec![(A_UNIT, SemanticLocalRoleV1::Return)],
            Vec::new(),
            vec![
                block(201, Vec::new(), SemanticTerminatorKindV1::Return),
                block(
                    202,
                    Vec::new(),
                    assertion_terminator(typed_constant(A_BOOL, 0, 1), true, 0),
                ),
            ],
        );
        let materialized = assertion_materialized(function);
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            assert!(facts.is_materialized_block(0)?);
            assert!(!facts.is_materialized_block(1)?);
            // Absence is not a condition binding, even when explicitly queried.
            assert!(matches!(
                facts.condition(1, true, SemanticBlockIdV1::from_index(0)),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Origin(_)
                ))
            ));
            Ok(())
        })
        .unwrap();
        materialized.semantic_ssa().verify_replay().unwrap();
        // Origin queries handle absence, but the retained V1 source-induction
        // checker still rejects an unreachable source block before projection.
        assert!(matches!(
            assertion_project(materialized),
            Err(ProductionRankedProjectionErrorV1::SemanticU32Induction(
                fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                    "the semantic CFG contains an unreachable block"
                )
            ))
        ));
    }

    #[test]
    fn genuine_present_block_without_an_assert_origin_rejects_a_query() {
        let materialized = assertion_materialized(literal_assertion(true, true, false));
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            assert!(facts.is_materialized_block(1)?);
            assert!(matches!(
                facts.condition(1, true, SemanticBlockIdV1::from_index(1)),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Origin(_)
                ))
            ));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn projected_edges_cannot_reach_an_absent_materialized_block() {
        for (entry, terminators) in [
            (
                0,
                vec![
                    ProjectedCfgTerminatorV1::Branch(1),
                    ProjectedCfgTerminatorV1::AbsentMaterialized,
                ],
            ),
            (0, vec![ProjectedCfgTerminatorV1::AbsentMaterialized]),
        ] {
            assert!(matches!(
                reachable_projected_blocks(entry, &terminators),
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a projected CFG reaches a source block absent from the materialized graph"
                ))
            ));
        }
        assert_eq!(
            reachable_projected_blocks(
                0,
                &[
                    ProjectedCfgTerminatorV1::Return,
                    ProjectedCfgTerminatorV1::AbsentMaterialized
                ]
            )
            .unwrap(),
            vec![true, false]
        );
    }

    #[test]
    fn genuine_binding_rejects_changed_polarity_success_and_source_alias() {
        let materialized = assertion_materialized(literal_assertion(true, true, false));
        with_canonical_assertions_v1(&materialized, |session| {
            {
                let mut facts = session.for_source(ROOT, ROOT);
                for (expected, success) in [(false, 1), (true, 0)] {
                    assert!(matches!(
                        facts.condition(0, expected, SemanticBlockIdV1::from_index(success)),
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::Binding(_)
                        ))
                    ));
                }
            }
            for (owner, function) in [(1, 0), (0, 1)] {
                let mut facts = session.for_source(
                    SemanticFunctionIdV1::from_index(owner),
                    SemanticFunctionIdV1::from_index(function),
                );
                assert!(matches!(
                    facts.is_materialized_block(0),
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Origin(_)
                    ))
                ));
                assert!(matches!(
                    facts.condition(0, true, SemanticBlockIdV1::from_index(1)),
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Origin(_)
                    ))
                ));
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn genuine_one_site_query_exact_and_one_under_work_preserve_its_floor() {
        let materialized = assertion_materialized(literal_assertion(true, true, false));
        with_canonical_assertions_v1(&materialized, |session| {
            // Query-only contract: the explicit floor includes all owners/report
            // still retained by the outer session, plus a nonzero caller prefix.
            let floor = session.retained_floor_for_test_v1() + 23;
            for (limit, succeeds, used) in [(14, true, 14), (13, false, 6)] {
                let mut work = Work::new(7 + limit);
                let mut budget = Budget::new(&mut work, floor);
                budget.charge_work(7).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = {
                    let mut facts =
                        session.for_source_with_query_budget_v1(&mut budget, ROOT, ROOT);
                    facts.condition(0, true, SemanticBlockIdV1::from_index(1))
                };
                if succeeds {
                    assert_eq!(result.unwrap(), ProjectedAssertionConditionV1::Bool(true));
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::Sparse(_)
                        ))
                    ));
                }
                assert_eq!(budget.work(), 7 + used);
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor);
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn graph_session_restores_caller_floor_on_success_early_and_late_errors() {
        let materialized = assertion_materialized(literal_assertion(true, true, false));
        let floor = materialized.retained_analysis_storage_v1() + 29;
        for mode in [0, 1, 2] {
            let mut work = Work::new(if mode == 0 { 18 } else { usize::MAX });
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result =
                with_canonical_assertions_budget_v1(&materialized, &mut budget, |session| {
                    assert_ne!(mode, 0, "an exhausted derive must never invoke the body");
                    let mut facts = session.for_source(ROOT, ROOT);
                    assert_eq!(
                        facts.condition(0, true, SemanticBlockIdV1::from_index(1))?,
                        ProjectedAssertionConditionV1::Bool(true)
                    );
                    if mode == 1 {
                        Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "test-only late failure",
                        ))
                    } else {
                        Ok(())
                    }
                });
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() >= 18);
            match mode {
                0 => assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(_))
                )),
                1 => assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "test-only late failure"
                    ))
                )),
                _ => result.unwrap(),
            }
        }
        materialized.semantic_ssa().verify_replay().unwrap();
    }

    #[test]
    fn private_decision_helper_never_overrides_an_opposing_graph_constant() {
        for expected in [false, true] {
            for source_proof in [false, true] {
                assert!(projected_assertion_is_proved_v1(
                    ProjectedAssertionConditionV1::Bool(expected),
                    expected,
                    source_proof
                ));
                assert!(!projected_assertion_is_proved_v1(
                    ProjectedAssertionConditionV1::Bool(!expected),
                    expected,
                    source_proof
                ));
                for condition in [
                    ProjectedAssertionConditionV1::Dormant,
                    ProjectedAssertionConditionV1::Unknown,
                    ProjectedAssertionConditionV1::Dynamic,
                    ProjectedAssertionConditionV1::ElidedByExistingRule,
                ] {
                    assert_eq!(
                        projected_assertion_is_proved_v1(condition, expected, source_proof),
                        source_proof
                    );
                }
            }
        }
    }
}
