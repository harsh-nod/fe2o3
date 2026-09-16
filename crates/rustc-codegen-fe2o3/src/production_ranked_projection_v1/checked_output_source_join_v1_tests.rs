mod checked_output_source_join_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    const WORK: usize = 16_000_000;
    const STORAGE: usize = 64 * 1024 * 1024;
    const PREFIX: usize = 97;

    #[test]
    fn collected_prefix_retains_real_none_and_old_presence_gate_cost_on_all_roots() {
        for multiple in [false, true] {
            with_source(multiple, true, |materialized, inputs, budget| {
                let references =
                    crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                let floor = budget.storage();
                let before = budget.work();
                let mut entered = false;
                with_source_ranked_prefix_v1(
                    materialized,
                    inputs,
                    &references,
                    budget,
                    |source, original, verification, _effects, partition, recorders, _budget| {
                        entered = true;
                        assert!(std::ptr::eq(source.materialized(), materialized));
                        assert!(std::ptr::eq(original.materialized(), materialized));
                        assert_eq!(original.root_count(), inputs.len());
                        assert_eq!(verification.root_count(), inputs.len());
                        assert_eq!(partition.len(), inputs.len());
                        assert_eq!(recorders.len(), inputs.len());
                        assert!(partition.iter().all(|part| part.as_slice().is_empty()));
                        for root in verification.roots() {
                            assert!(
                                !root
                                    .verification()
                                    .has_authenticated_functional_verification()
                            );
                            assert!(root.verification().aggregate_verus_execution().is_none());
                        }
                        Ok(())
                    },
                )
                .unwrap();
                assert!(entered);
                assert_eq!(budget.storage(), floor);
                let prefix_work = budget.work() - before;
                let before_gate = budget.work();
                let mut gated = false;
                let error = with_source_ranked_custody_v1(
                    materialized,
                    inputs,
                    &references,
                    budget,
                    |_, _| {
                        gated = true;
                        Ok(())
                    },
                )
                .unwrap_err();
                assert!(!gated);
                assert!(matches!(
                    error,
                    SourceJoinPipelineErrorV1::RankedVerification(
                        ProductionRankedVerificationErrorV1::RosterMetadata(
                            "every source-first root requires retained functional and aggregate custody"
                        )
                    )
                ));
                // Existing presence gate: roster2 + first missing root4. No
                // diagnostic entry/report/loop charge enters this old path.
                assert_eq!(budget.work() - before_gate, prefix_work + 6);
                assert_eq!(budget.storage(), floor);
                assert!(references.as_slice().is_empty());
            });
        }
    }

    #[test]
    fn collected_prefix_missing_capture_never_enters_its_callback() {
        with_source(false, false, |source, inputs, budget| {
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let floor = budget.storage();
            let mut entered = false;
            let error = with_source_ranked_prefix_v1(
                source,
                inputs,
                &references,
                budget,
                |_, _, _, _, _, _, _| {
                    entered = true;
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(!entered);
            assert!(matches!(
                error,
                SourceJoinPipelineErrorV1::RankedVerification(
                    ProductionRankedVerificationErrorV1::RosterMetadata(
                        "source-first SSA capture absent"
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
        });
    }

    #[test]
    fn collected_prefix_real_r1_callback_error_and_exact_panic_restore_floor_and_reenter() {
        with_source(false, true, |source, inputs, budget| {
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let floor = budget.storage();
            let before = budget.work();
            let mut entered = false;
            let error = with_source_ranked_prefix_v1(
                source,
                inputs,
                &references,
                budget,
                |_, original, verification, _, _, _, budget| {
                    entered = true;
                    assert!(std::ptr::eq(original.materialized(), source));
                    assert!(
                        !verification.roots()[0]
                            .verification()
                            .has_authenticated_functional_verification()
                    );
                    budget.charge_work(3).unwrap();
                    Err::<(), _>(SourceJoinPipelineErrorV1::RankedProjection(
                        ProductionRankedProjectionErrorV1::Incomplete(
                            "collected prefix callback error",
                        ),
                    ))
                },
            )
            .unwrap_err();
            assert!(entered);
            assert!(matches!(
                error,
                SourceJoinPipelineErrorV1::RankedProjection(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "collected prefix callback error"
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
            let before_panic = budget.work();
            let mut panic_entered = false;
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_source_ranked_prefix_v1::<()>(
                    source,
                    inputs,
                    &references,
                    budget,
                    |_, original, verification, _, _, _, budget| {
                        panic_entered = true;
                        assert!(std::ptr::eq(original.materialized(), source));
                        assert!(
                            verification.roots()[0]
                                .verification()
                                .aggregate_verus_execution()
                                .is_none()
                        );
                        budget.charge_work(3).unwrap();
                        std::panic::panic_any(0x340_u32);
                    },
                )
            }))
            .expect_err("completed real R1 callback must unwind");
            assert!(panic_entered);
            assert_eq!(payload.downcast_ref::<u32>(), Some(&0x340));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before_panic);
            let before_reentry = budget.work();
            let mut reentered = false;
            with_source_ranked_prefix_v1(
                source,
                inputs,
                &references,
                budget,
                |_, _, verification, _, _, _, _| {
                    reentered = true;
                    assert!(
                        !verification.roots()[0]
                            .verification()
                            .has_authenticated_functional_verification()
                    );
                    Ok(())
                },
            )
            .unwrap();
            assert!(reentered);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before_reentry);
        });
    }

    #[test]
    fn collected_address_source_audit_keeps_n_only_prefix_and_original_presence_gate() {
        let text = include_str!("checked_output_source_join_v1.rs");
        let production = text
            .split("pub(crate) fn with_source_ranked_custody_v1<T>(")
            .nth(1)
            .unwrap()
            .split("fn with_source_ranked_prefix_v1<T>(")
            .next()
            .unwrap();
        assert!(
            production.find("with_source_ranked_prefix_v1(").unwrap()
                < production
                    .find("require_source_functional_roster_v1(")
                    .unwrap()
        );
        assert!(
            production
                .find("require_source_functional_roster_v1(")
                .unwrap()
                < production.find("&SourceRankedCustodyV1 {").unwrap()
        );
        let prefix = text
            .split_once("fn with_source_ranked_prefix_v1<T>(")
            .expect("source-only prefix declaration")
            .1
            .split_once("fn with_source_preservation_output_v1<'owners>(")
            .expect("separate source/output consumer declaration")
            .0;
        assert!(prefix.contains("with_canonical_assertions_source_budget_v1("));
        assert!(!prefix.contains("with_checked_output_assertions_view_budget_v1("));
        let diagnostic = text
            .split("pub(crate) fn observe_collected_ranked_addresses_v1")
            .nth(1)
            .unwrap();
        assert!(
            diagnostic
                .find("if !references.as_slice().is_empty()")
                .unwrap()
                < diagnostic.find("with_source_ranked_prefix_v1(").unwrap()
        );
        assert!(
            diagnostic.find("with_source_ranked_prefix_v1(").unwrap()
                < diagnostic
                    .find("with_checked_output_assertions_view_budget_v1(")
                    .unwrap()
        );
        assert!(
            diagnostic.find("partition.len() != inputs.len()").unwrap()
                < diagnostic
                    .find("with_prepared_canonical_memory_session_v1(")
                    .unwrap()
        );
        for forbidden in [
            "SourceRankedCustodyV1 {",
            "with_complete_formal_memory_module_v1(",
            "AuthenticatedReferenceEffectBindingsV1::default",
            "prove_and_compile(",
        ] {
            assert!(!diagnostic.contains(forbidden));
        }
    }

    fn with_source(
        multiple: bool,
        capture: bool,
        next: impl FnOnce(
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &[ProductionRankedRootInputV1],
            &mut Budget<'_>,
        ),
    ) {
        let seed = neutral_ranked_program_v1();
        let semantic = seed.materialized.semantic_ssa().source_semantic();
        assert_eq!(semantic.functions().len(), 1);
        assert_eq!(semantic.callables().len(), 4);
        let root = &semantic.functions()[0];
        let names = if multiple {
            &['z', 'a'][..]
        } else {
            &['z'][..]
        };
        let mut callables = semantic.callables().to_vec();
        if multiple {
            callables.insert(
                1,
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            );
        }
        let mut calls = 0;
        let blocks = root
            .blocks()
            .iter()
            .map(|block| {
                let kind = match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => {
                        calls += 1;
                        let original = call.callee().index() as usize;
                        assert!((1..semantic.callables().len()).contains(&original));
                        assert!(matches!(
                            &semantic.callables()[original],
                            SemanticCallableDeclV1::CompilerIntrinsic { .. }
                        ));
                        let remapped = original + usize::from(multiple);
                        assert_eq!(callables[remapped], semantic.callables()[original]);
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                                SemanticCallableIdV1::from_index(remapped as u32),
                                call.arguments().to_vec(),
                                call.variadic_argument_abis().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        )
                    }
                    SemanticTerminatorKindV1::Return => SemanticTerminatorKindV1::Return,
                    other => panic!("unexpected genuine neutral terminator: {other:?}"),
                };
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    block.statements().to_vec(),
                    SemanticTerminatorV1::new(block.terminator().source(), kind),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, 3);
        let functions = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                SemanticFunctionDeclV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(231 + index as u8)),
                    root.role(),
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(233 + index as u8)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(235 + index as u8)),
                    root.generic_type_arguments_identity(),
                    root.const_generic_arguments_identity(),
                    root.source(),
                    root.abi().clone(),
                    root.locals().to_vec(),
                    root.entry(),
                    blocks.clone(),
                )
                .unwrap()
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(format!("{name}_source_first").into_bytes()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256(bytes(247 - index as u8)),
                    root.kernel_entry().unwrap().source_contract(),
                ))
            })
            .collect();
        let roots = (0..names.len())
            .map(|index| SemanticFunctionIdV1::from_index(index as u32))
            .collect();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            semantic.types().to_vec(),
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            functions,
            callables,
            roots,
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
            owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        drop(seed);
        let inputs = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                ranked_root_input_1d(&format!("{name}_source_first"), 247 - index as u8, 64)
            })
            .collect::<Vec<_>>();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let captured = if capture {
            let receipt = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let rows = ssa.occurrences_v1().unwrap();
            for index in 0..names.len() {
                assert!(std::ptr::eq(
                    rows.function(SemanticFunctionIdV1::from_index(index as u32))
                        .unwrap()
                        .owner(),
                    &ssa
                ));
            }
            receipt.retained_storage()
        } else {
            0
        };
        let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
        let source =
            fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .unwrap();
        let retained = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(retained).unwrap();
        let floor = budget.storage();
        next(&source, &inputs, &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(source);
        budget.release_storage(retained).unwrap();
        budget.release_storage(captured).unwrap();
        assert_eq!(budget.storage(), PREFIX);
    }

    #[test]
    fn source_first_missing_capture_stops_before_any_production_callback() {
        with_source(false, false, |source, inputs, budget| {
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let mut called = false;
            let error =
                with_source_ranked_custody_v1(source, inputs, &references, budget, |_, _| {
                    called = true;
                    Ok(())
                })
                .unwrap_err();
            assert!(!called);
            assert!(matches!(
                error,
                SourceJoinPipelineErrorV1::RankedVerification(
                    ProductionRankedVerificationErrorV1::RosterMetadata(
                        "source-first SSA capture absent"
                    )
                )
            ));
        });
    }

    #[test]
    fn source_first_real_r1_none_refuses_before_endpoint_on_one_and_two_roots() {
        for multiple in [false, true] {
            with_source(multiple, true, |source, inputs, budget| {
                let references =
                    crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                let floor = budget.storage();
                for _ in 0..2 {
                    let before = budget.work();
                    let mut called = false;
                    let error = with_source_ranked_custody_v1(
                        source,
                        inputs,
                        &references,
                        budget,
                        |_, _| {
                            called = true;
                            Ok(())
                        },
                    )
                    .unwrap_err();
                    assert!(!called);
                    assert!(
                        matches!(
                            error,
                            SourceJoinPipelineErrorV1::RankedVerification(
                                ProductionRankedVerificationErrorV1::RosterMetadata(
                                    "every source-first root requires retained functional and aggregate custody"
                                )
                            )
                        ),
                        "{error:?}"
                    );
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                    assert!(source.semantic_ssa().occurrences_v1().is_some());
                }
            });
        }
    }

    #[test]
    fn source_only_recorder_preserves_full_projection_without_o_control() {
        with_source(true, true, |materialized, inputs, budget| {
            let ledger = std::ptr::from_ref(&*budget);
            let floor = budget.storage();
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let source = RankedProjectionSourceV1::from_legacy(materialized).unwrap();
            with_ranked_root_preparation_v1(&source, inputs, &references, |effects, partition| {
                with_canonical_assertions_source_budget_v1(&source, budget, |session| {
                    session.with_recording_budget_v1(|budget| {
                        assert_eq!(std::ptr::from_ref(&*budget), ledger);
                        Ok(())
                    })?;
                    for (ordinal, source_root) in source.source_launch().roots().iter().enumerate()
                    {
                        let selection = source
                            .semantic_ssa()
                            .source_semantic()
                            .select_kernel_body_for_root_v1(source_root.selected_root())
                            .unwrap();
                        let mut facts =
                            session.for_source(source_root.selected_root(), selection.body());
                        assert!(!facts.checked_control_enabled_v1());
                        assert!(facts.checked_block_coverage_v1(0).is_err());
                        let old = project_and_verify_ranked_root_control_inner_v1(
                            source.semantic_ssa(),
                            effects,
                            selection,
                            &inputs[ordinal],
                            *source_root,
                            &partition[ordinal],
                            &mut facts,
                        )?;
                        let mut recorder =
                            canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(
                                &mut facts,
                            )?;
                        let recorded =
                            project_and_verify_ranked_root_control_with_address_claims_v1(
                                source.semantic_ssa(),
                                effects,
                                selection,
                                &inputs[ordinal],
                                *source_root,
                                &partition[ordinal],
                                &mut facts,
                                Some(&mut recorder),
                            )?;
                        assert_eq!(old.ranked_ir, recorded.ranked_ir);
                        assert_eq!(old.access_sources, recorded.access_sources);
                        assert_eq!(
                            old.executable_effect_sources,
                            recorded.executable_effect_sources
                        );
                        assert_eq!(
                            old.all_kernel_checks_are_clean(),
                            recorded.all_kernel_checks_are_clean()
                        );
                        assert!(!facts.checked_control_enabled_v1());
                    }
                    Ok(())
                })
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }

    #[test]
    fn source_recorder_empty_header_prefix_is_six_and_allocation_denial_releases_scope() {
        use canonical_memory_control_v1::CanonicalMemoryControlRecorderV1 as Recorder;
        let header = std::mem::size_of::<Vec<Recorder>>();
        for limit in [13, 12] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, PREFIX + header);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(PREFIX).unwrap();
            let result = source_recorders_v1(0, &mut budget);
            if limit == 13 {
                let rows = result.unwrap();
                assert!(rows.is_empty() && rows.capacity() == 0);
                assert_eq!(budget.storage(), PREFIX + header);
                drop(rows);
                budget.release_storage(header).unwrap();
            } else {
                assert!(result.is_err());
            }
            assert_eq!(budget.storage(), PREFIX);
            assert_eq!(budget.work(), if limit == 13 { 13 } else { 7 });
        }
        with_source(false, true, |materialized, _, budget| {
            let floor = budget.storage();
            let source = RankedProjectionSourceV1::from_legacy(materialized).unwrap();
            let error = with_canonical_assertions_source_budget_v1(&source, budget, |session| {
                session.with_recording_budget_v1(|budget| {
                    let live = budget.storage();
                    let padding = budget.storage_limit() - live;
                    budget.reserve_storage(padding).unwrap();
                    let result = source_recorders_v1(1, budget);
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                                Resource::Storage(_)
                            )
                        ))
                    ));
                    assert_eq!(budget.storage(), live + padding);
                    budget.release_storage(padding).unwrap();
                    Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                        "source recorder denial marker",
                    ))
                })
            })
            .unwrap_err();
            assert!(matches!(
                error,
                ProductionRankedProjectionErrorV1::Incomplete("source recorder denial marker")
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.failed_storage().is_some());
        });
    }

    #[test]
    fn source_recording_scope_error_and_unwind_restore_live_source() {
        with_source(false, true, |materialized, _, budget| {
            let source = RankedProjectionSourceV1::from_legacy(materialized).unwrap();
            let floor = budget.storage();
            for panic in [false, true] {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_canonical_assertions_source_budget_v1(&source, budget, |session| {
                        let root = source.source_launch().roots()[0].selected_root();
                        let mut facts = session.for_source(root, root);
                        let recorder =
                            canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(
                                &mut facts,
                            )?;
                        assert!(recorder.candidate().arguments.is_empty());
                        if panic {
                            std::panic::panic_any(0x334_u32);
                        }
                        Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                            "source recorder callback marker",
                        ))
                    })
                }));
                if panic {
                    assert_eq!(outcome.unwrap_err().downcast_ref::<u32>(), Some(&0x334));
                } else {
                    assert!(matches!(
                        outcome.unwrap(),
                        Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "source recorder callback marker"
                        ))
                    ));
                }
                assert_eq!(budget.storage(), floor);
            }
        });
    }

    #[test]
    fn source_recording_broken_floor_precedes_callback_error_or_panic() {
        with_source(false, true, |materialized, _, budget| {
            let source = RankedProjectionSourceV1::from_legacy(materialized).unwrap();
            let floor = budget.storage();
            for panic in [false, true] {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_canonical_assertions_source_budget_v1(&source, budget, |session| {
                        session.with_recording_budget_v1(|budget| {
                            budget
                                .release_storage(budget.storage() - floor + 1)
                                .unwrap();
                            if panic {
                                std::panic::panic_any(0x335_u32);
                            }
                            Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                                "broken source recording floor marker",
                            ))
                        })
                    })
                }));
                assert!(matches!(
                    outcome.unwrap(),
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                            Resource::Accounting
                        )
                    ))
                ));
                assert_eq!(budget.storage(), floor - 1);
                // Repair only the deliberately stolen source byte for teardown.
                budget.reserve_storage(1).unwrap();
            }
        });
    }

    #[test]
    fn held_route_captures_before_n_and_keeps_source_gate_before_b_o() {
        let pipeline = include_str!("../production_pipeline.rs");
        let materialization = pipeline
            .split("fn with_materialized_capture_mode_v1<T>(")
            .nth(1)
            .unwrap()
            .split("impl MaterializedNeutralProductionCompilation")
            .next()
            .unwrap();
        assert_eq!(
            materialization
                .matches(".try_capture_occurrences_with_budget_v1(")
                .count(),
            1
        );
        assert!(
            materialization
                .find(".try_capture_occurrences_with_budget_v1(")
                .unwrap()
                < materialization
                    .find("::try_materialize_with_budget(")
                    .unwrap()
        );
        assert!(pipeline.contains("self.with_materialized_capture_mode_v1(false, next)"));
        let endpoint = include_str!("../production_checked_output_pipeline_v1.rs");
        let setup = endpoint
            .split("fn with_source_checked_output_transaction_v1<T>(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn lower_checked_output_memory_target_v1(")
            .next()
            .unwrap();
        let stages = [
            ".with_captured_materialized_target_neutral_v1(",
            "::with_source_ranked_custody_v1(",
            "with_checked_output_target_endpoint_v1(",
        ];
        let mut last = None;
        for stage in stages {
            let position = setup.find(stage).unwrap();
            assert!(last.is_none_or(|last| last < position));
            last = Some(position);
        }
        let old = endpoint
            .split("pub(crate) fn lower_checked_output_memory_target_v1(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn with_checked_output_local_relations_v1(")
            .next()
            .unwrap();
        let new = endpoint
            .split("pub(crate) fn with_checked_output_local_relations_v1(")
            .nth(1)
            .unwrap();
        for (route, join, continuation) in [
            (
                old,
                "::with_source_checked_output_module_v1(",
                ".lower_inert_target_v1(",
            ),
            (
                new,
                "::with_source_checked_output_module_catalog_v1(",
                ".validate_inert_target_geometry_v1(",
            ),
        ] {
            assert!(
                route
                    .find("self.with_source_checked_output_transaction_v1(")
                    .unwrap()
                    < route.find(join).unwrap()
            );
            assert!(route.find(join).unwrap() < route.find(continuation).unwrap());
            assert!(!route.contains("AuthenticatedReferenceEffectBindingsV1::default"));
        }
        assert!(
            new.find(".validate_inert_target_geometry_v1(").unwrap()
                < new
                    .find("::with_source_checked_output_local_relations_v1(")
                    .unwrap()
        );
        assert!(!setup.contains("AuthenticatedReferenceEffectBindingsV1::default"));
        let join = include_str!("checked_output_module_join_v1.rs");
        assert!(
            join.find(".check_borrowed_ranked_addresses_v1(").unwrap()
                < join
                    .find("::with_complete_formal_memory_module_v1(")
                    .unwrap()
        );
    }
}
