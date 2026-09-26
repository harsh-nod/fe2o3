// Regression controls for the ordinary shared recipe/CFG seam. Component facts
// below are synthetic, not admitted nominal owners or memory certificates.
mod root_recipe_core_v1_controls {
    use super::*;
    use canonical_assertion_facts_v1::ProjectedAssertionConditionV1 as Condition;

    #[derive(Default)]
    struct TraceFacts {
        events: Vec<(u8, usize)>,
        contradiction: bool,
        reject_progress: bool,
    }
    impl ProjectedAssertionFactsV1 for TraceFacts {
        fn charge_private_array_work(
            &mut self,
            amount: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            self.events.push((0, amount));
            Ok(())
        }
        fn private_array_initializer_count(
            &mut self,
            _: usize,
            _: usize,
        ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
            Ok(None)
        }
        fn is_materialized_block(
            &mut self,
            block: usize,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            self.events.push((1, block));
            Ok(true)
        }
        fn masked_assertion_source_proved_v1(
            &mut self,
            _: &SemanticFunctionDeclV1,
            block: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            self.events.push((2, block));
            Ok(false)
        }
        fn condition(
            &mut self,
            block: usize,
            expected: bool,
            _: SemanticBlockIdV1,
        ) -> Result<Condition, ProductionRankedProjectionErrorV1> {
            self.events.push((3, block));
            Ok(if self.contradiction {
                Condition::Bool(!expected)
            } else {
                Condition::Dynamic
            })
        }
        fn require_guarded_source_progress_v1(
            &mut self,
            _: &[SemanticTypeDeclV1],
            _: &SemanticFunctionDeclV1,
            induction: &ProjectedUniformInductionV1,
            _: &[ProductionRankedOperationV1],
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            self.events.push((4, induction.header));
            if self.reject_progress {
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "root recipe test guarded progress denied",
                ))
            } else {
                Ok(())
            }
        }
    }

    fn empty_projected(function: &SemanticFunctionDeclV1) -> Vec<ProjectedSemanticBlockV1> {
        function
            .blocks()
            .iter()
            .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
            .collect()
    }

    fn cfg_with_prepared(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        mask: PreparedRootCfgAssertionsV1<'_>,
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<
        (
            Vec<ProductionRankedBlockV1>,
            Vec<ProjectedAccessSourceV1>,
            Vec<ProductionRankedExecutableEffectSourceV1>,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        build_ranked_cfg_from_prepared_assertions_v1(
            types,
            function,
            &[],
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &[],
            vec![],
            empty_projected(function),
            facts,
            None,
            mask,
        )
    }

    #[test]
    fn prepared_mask_moves_the_original_allocation_and_matches_the_existing_analyzer() {
        let types = assertion_proof_types();
        for hostile in [false, true] {
            let function = checked_arithmetic_chain_function(hostile);
            let expected = SemanticAssertProofsV1::analyze(&types, &function).unwrap();
            let mask = PreparedRootCfgAssertionsV1::analyze(&types, &function).unwrap();
            let pointer = mask.decisions.as_ptr();
            let capacity = mask.decisions.capacity();
            let decisions = mask.into_decisions_for(&types, &function).unwrap();
            assert_eq!(decisions, expected);
            assert_eq!(decisions.as_ptr(), pointer);
            assert_eq!(decisions.capacity(), capacity);
        }
    }

    #[test]
    fn prepared_cfg_has_exact_full_output_error_and_fact_query_order_parity() {
        let types = assertion_proof_types();
        for hostile in [false, true] {
            let function = checked_arithmetic_chain_function(hostile);
            for contradiction in [false, true] {
                let mut ordinary_facts = TraceFacts {
                    contradiction,
                    ..TraceFacts::default()
                };
                let ordinary = super::super::build_ranked_cfg(
                    &types,
                    &function,
                    &[],
                    &vec![None; function.locals().len()],
                    &vec![None; function.blocks().len()],
                    &[],
                    vec![],
                    empty_projected(&function),
                    &mut ordinary_facts,
                    None,
                );
                let mut prepared_facts = TraceFacts {
                    contradiction,
                    ..TraceFacts::default()
                };
                let prepared = cfg_with_prepared(
                    &types,
                    &function,
                    PreparedRootCfgAssertionsV1::analyze(&types, &function).unwrap(),
                    &mut prepared_facts,
                );
                assert_eq!(format!("{ordinary:?}"), format!("{prepared:?}"));
                assert_eq!(ordinary_facts.events, prepared_facts.events);
                if contradiction {
                    assert!(matches!(
                        prepared,
                        Err(ProductionRankedProjectionErrorV1::UnprovenAssert { .. })
                    ));
                } else if !hostile {
                    assert!(prepared.is_ok());
                }
            }
        }
    }

    #[test]
    fn prepared_mask_rejects_equal_content_foreign_function_and_type_storage() {
        let types = assertion_proof_types();
        let function = checked_arithmetic_chain_function(false);
        let foreign_function = function.clone();
        let foreign_types = types.clone();
        for (candidate_types, candidate_function) in [
            (types.as_slice(), &foreign_function),
            (foreign_types.as_slice(), &function),
        ] {
            let mut facts = TraceFacts::default();
            let result = cfg_with_prepared(
                candidate_types,
                candidate_function,
                PreparedRootCfgAssertionsV1::analyze(&types, &function).unwrap(),
                &mut facts,
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "prepared root assertion source binding differs"
                ))
            ));
            assert!(facts.events.is_empty());
        }
    }

    #[test]
    fn prepared_mask_rejects_wrong_length_without_querying_graph_facts() {
        let types = assertion_proof_types();
        let function = checked_arithmetic_chain_function(false);
        for extra in [false, true] {
            let mut mask = PreparedRootCfgAssertionsV1::analyze(&types, &function).unwrap();
            if extra {
                mask.decisions.push(false);
            } else {
                mask.decisions.pop();
            }
            let mut facts = TraceFacts::default();
            assert!(matches!(
                cfg_with_prepared(&types, &function, mask, &mut facts),
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "prepared root assertion source binding differs"
                ))
            ));
            assert!(facts.events.is_empty());
        }
    }

    #[test]
    fn historical_projected_block_refusal_still_precedes_assertion_and_binding_work() {
        let types = assertion_proof_types();
        let function = checked_arithmetic_chain_function(false);
        let mut mask = PreparedRootCfgAssertionsV1::analyze(&types, &function).unwrap();
        mask.decisions.clear(); // Deliberately invalid, synthetic component state.
        let mut facts = TraceFacts::default();
        let result = build_ranked_cfg_from_prepared_assertions_v1(
            &types,
            &function,
            &[],
            &[],
            &[],
            &[],
            vec![],
            vec![],
            &mut facts,
            None,
            mask,
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection lost a basic block"
            ))
        ));
        assert!(facts.events.is_empty());
        let ordinary = super::super::build_ranked_cfg(
            &types,
            &function,
            &[],
            &[],
            &[],
            &[],
            vec![],
            vec![],
            &mut facts,
            None,
        );
        assert!(matches!(
            ordinary,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection lost a basic block"
            ))
        ));
        assert!(facts.events.is_empty());
    }

    #[test]
    fn authenticated_induction_or_is_retained_and_graph_contradiction_still_wins() {
        let types = assertion_proof_types();
        let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
        let (inductions, operations, _) =
            project_test_inductions_with_types(&types, &function).unwrap();
        assert_eq!(inductions.len(), 1);
        let assertion_block = inductions[0]
            .source_progress
            .update
            .proved_overflow_assert_block()
            .expect("fixture must carry the authenticated overflow assertion");
        for (contradiction, reject_progress) in [(false, false), (true, false), (false, true)] {
            let mut mask = PreparedRootCfgAssertionsV1::analyze(&types, &function).unwrap();
            // Synthetic base decision isolates the existing induction OR. No
            // nominal/admitted preparation is modified or manufactured here.
            mask.decisions[assertion_block] = false;
            let mut facts = TraceFacts {
                contradiction,
                reject_progress,
                ..TraceFacts::default()
            };
            let result = build_ranked_cfg_from_prepared_assertions_v1(
                &types,
                &function,
                &[],
                &vec![None; function.locals().len()],
                &vec![None; function.blocks().len()],
                &inductions,
                operations.clone(),
                empty_projected(&function),
                &mut facts,
                None,
                mask,
            );
            assert_eq!(facts.events.first(), Some(&(4, inductions[0].header)));
            if reject_progress {
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "root recipe test guarded progress denied"
                    ))
                ));
                assert_eq!(facts.events.len(), 1);
            } else if contradiction {
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::UnprovenAssert { .. })
                ));
            } else {
                assert!(result.is_ok(), "{result:?}");
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn with_recipe_for_existing_owner<T>(
        owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        ordinal: usize,
        input: &ProductionRankedRootInputV1,
        row: ProductionSourceLaunchRootV1,
        inspect: impl FnOnce(
            &PreparedRankedRootRecipeV1<'_>,
        ) -> Result<T, ProductionRankedProjectionErrorV1>,
    ) -> Result<(T, (usize, usize, usize)), ProductionRankedProjectionErrorV1> {
        let source = ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(owner)?;
        let semantic = owner.semantic_ssa().source_semantic();
        let selection = semantic
            .select_kernel_body_for_root_v1(semantic.roots()[ordinal])
            .unwrap();
        let function = &semantic.functions()[selection.body().index() as usize];
        let references =
            crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
        let mut observation = None;
        ranked_projection_source_v1::with_projection_source_budget_v1(&source, |budget| {
            let program = canonical_assertion_facts_v1::with_canonical_assertions_source_budget_v1(
                &source,
                budget,
                |session| {
                    let effects = session.callable_effect_summaries(&source)?;
                    let mut facts = session.for_source(selection.root(), selection.body());
                    scalar_singleton_projection_v1::with_scalar_private_singletons_v1(
                        semantic.types(),
                        function,
                        &mut facts,
                        |singletons, facts| {
                            scalar_borrow_projection_v1::with_scalar_private_borrows_v1(
                                semantic.types(),
                                function,
                                semantic.target(),
                                facts,
                                |borrows, facts| {
                                    multi_entry_induction_v1::with_scope(facts, |scope, facts| {
                                        with_prepared_ranked_root_recipe_v1(
                                            semantic,
                                            &effects,
                                            selection,
                                            input,
                                            row,
                                            &references,
                                            facts,
                                            singletons,
                                            borrows,
                                            scope,
                                            |recipe, facts| {
                                                assert_eq!(recipe.helper_expression_storage, 0);
                                                observation = Some(inspect(&recipe)?);
                                                verify_prepared_ranked_root_recipe_v1(
                                                    recipe,
                                                    selection,
                                                    input,
                                                    row,
                                                    &references,
                                                    facts,
                                                )
                                            },
                                        )
                                    })
                                },
                            )
                        },
                    )
                },
            )?;
            drop(program);
            let result = observation.ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "root recipe test callback was not reached",
            ))?;
            Ok((
                result,
                (budget.work(), budget.peak_storage(), budget.storage()),
            ))
        })
    }

    #[test]
    fn complete_existing_memory_recipe_and_source_rows_match_ordinary_verification() {
        let (owner, inputs) = source_launch_materialized_two_root_fixture_v1();
        let semantic = owner.semantic_ssa().source_semantic();
        let references =
            crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
        for (ordinal, input) in inputs.iter().enumerate() {
            let row = owner.source_launch().roots()[ordinal];
            let (observed, prepared_counts) =
                with_recipe_for_existing_owner(&owner, ordinal, input, row, |recipe| {
                    assert!(
                        !recipe.access_sources.is_empty(),
                        "fixture must retain real memory rows"
                    );
                    assert!(!recipe.sources.is_empty());
                    assert!(
                        recipe
                            .kernel
                            .blocks()
                            .iter()
                            .flat_map(|b| b.operations())
                            .any(|op| matches!(
                                op,
                                ProductionRankedOperationV1::Access {
                                    kind: AccessKindAttr::Write,
                                    ..
                                }
                            ))
                    );
                    Ok((
                        format_ranked_cfg(
                            function_name(recipe.root_function)?,
                            recipe.kernel.blocks(),
                        )?,
                        format!("{:?}", recipe.access_sources),
                        format!("{:?}", recipe.executable_effect_sources),
                        format!("{:?}", recipe.reference_writes),
                        recipe.kernel_binding,
                        recipe.root_function.identity(),
                    ))
                })
                .unwrap();
            let source =
                ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
            let selection = semantic
                .select_kernel_body_for_root_v1(semantic.roots()[ordinal])
                .unwrap();
            let (ordinary, ordinary_counts) =
                ranked_projection_source_v1::with_projection_source_budget_v1(&source, |budget| {
                    let ordinary =
                        canonical_assertion_facts_v1::with_canonical_assertions_source_budget_v1(
                            &source,
                            budget,
                            |session| {
                                let effects = session.callable_effect_summaries(&source)?;
                                let mut facts =
                                    session.for_source(selection.root(), selection.body());
                                project_and_verify_ranked_root_v1(
                                    semantic,
                                    &effects,
                                    selection,
                                    input,
                                    row,
                                    &references,
                                    &mut facts,
                                )
                            },
                        )?;
                    Ok((
                        ordinary,
                        (budget.work(), budget.peak_storage(), budget.storage()),
                    ))
                })
                .unwrap();
            assert_eq!(prepared_counts, ordinary_counts);
            assert_eq!(observed.0, ordinary.ranked_ir);
            assert_eq!(observed.1, format!("{:?}", ordinary.access_sources));
            assert_eq!(
                observed.2,
                format!("{:?}", ordinary.executable_effect_sources)
            );
            assert_eq!(
                observed.3,
                format!("{:?}", ordinary.observed_reference_writes)
            );
            assert_eq!(observed.4, ordinary.kernel_binding);
            assert_eq!(observed.5, ordinary.semantic_root_identity);
        }
    }

    #[test]
    fn wrong_launch_root_never_reaches_the_complete_recipe_callback() {
        let (owner, inputs) = source_launch_materialized_two_root_fixture_v1();
        let entered = std::cell::Cell::new(false);
        let result = with_recipe_for_existing_owner(
            &owner,
            0,
            &inputs[0],
            owner.source_launch().roots()[1],
            |_recipe| {
                entered.set(true);
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source launch roster root changed before ranked projection"
            ))
        ));
        assert!(!entered.get());
    }
}
