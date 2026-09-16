mod identity_getter_physical_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_lower_mir_kernel::{
        ProductionCanonicalMemoryAnalysisCandidateV1 as Candidate,
        ProductionScopedCanonicalStoreAnalysisV1 as Analysis,
    };

    fn with_fixture(
        value: u32,
        collected_modes: bool,
        profile: Profile,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut CanonicalMemoryProjectedRootV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        with_fixture_stores(value, collected_modes, profile, 1, body)
    }

    fn with_fixture_stores(
        value: u32,
        collected_modes: bool,
        profile: Profile,
        stores: usize,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut CanonicalMemoryProjectedRootV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let ssa = identity_getter_shape_tests::source_ssa_options(value, collected_modes, stores);
        with_admitted_fixture(ssa, collected_modes, profile, body)
    }

    fn with_admitted_fixture(
        ssa: ProductionSemanticSsaOwnerV1,
        collected_modes: bool,
        profile: Profile,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut CanonicalMemoryProjectedRootV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        with_admitted_source_fixture(ssa, collected_modes, profile, |source, view, inputs, budget| {
            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            with_ranked_root_preparation_v1(&projection, inputs, &references, |effects, partition| {
                checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                    view,
                    budget,
                    |session| session.with_canonical_memory_scope_v1(|session| {
                        let source_root = projection.source_launch().roots()[0];
                        let selection = projection.semantic_ssa().source_semantic()
                            .select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                        let mut root = {
                            let mut facts = session.for_source(source_root.selected_root(), selection.body());
                            project_canonical_memory_root_v1(
                                projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                &partition[0], &mut facts,
                            )?
                        };
                        session.with_output_occurrences_v1(|view, budget| {
                            body(view, &mut root, budget).map_err(|error|
                                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Output(error),
                                ))
                        })
                    }),
                )
            })
        })
    }

    fn with_admitted_source_fixture(
        ssa: ProductionSemanticSsaOwnerV1,
        collected_modes: bool,
        profile: Profile,
        body: impl FnOnce(
            &ProductionPreRankedKirOwnerV1,
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &[ProductionRankedRootInputV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionRankedProjectionErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        with_admitted_source_inputs_fixture(
            ssa,
            profile,
            || {
                [ranked_root_input_1d(
                    if collected_modes {
                        "other_identity_entry"
                    } else {
                        A_NAME
                    },
                    if collected_modes { 246 } else { 247 },
                    64,
                )]
            },
            body,
        )
    }

    fn with_admitted_source_inputs_fixture<I: AsRef<[ProductionRankedRootInputV1]>>(
        ssa: ProductionSemanticSsaOwnerV1,
        profile: Profile,
        make_inputs: impl FnOnce() -> I,
        body: impl FnOnce(
            &ProductionPreRankedKirOwnerV1,
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &[ProductionRankedRootInputV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionRankedProjectionErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        with_admitted_source_inputs_and_checked_fixture(
            ssa,
            profile,
            make_inputs,
            |_, _, _| {},
            body,
        )
    }

    fn with_admitted_source_inputs_and_checked_fixture<I: AsRef<[ProductionRankedRootInputV1]>>(
        mut ssa: ProductionSemanticSsaOwnerV1,
        profile: Profile,
        make_inputs: impl FnOnce() -> I,
        inspect: impl FnOnce(
            &Owner,
            &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
            &mut Budget<'_>,
        ),
        body: impl FnOnce(
            &ProductionPreRankedKirOwnerV1,
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &[ProductionRankedRootInputV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionRankedProjectionErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.reserve_storage(PREFIX).unwrap();
        let capture = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let inputs = make_inputs();
        let inputs = inputs.as_ref();
        let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, inputs).unwrap();
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let source_bytes = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(source_bytes).unwrap();
        let source_floor = budget.storage();
        let result;
        {
            let bound =
                dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                    .unwrap();
            let (input, storage) =
                Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                    .unwrap();
            let input_bytes = storage.retained_storage();
            budget.reserve_storage(input_bytes).unwrap();
            let observed =
                fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget)
                    .unwrap();
            budget
                .reserve_storage(observed.storage().retained_storage())
                .unwrap();
            let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
            let output_bytes = checked.storage().retained_storage();
            budget.reserve_storage(output_bytes).unwrap();
            let inspection_floor = budget.storage();
            inspect(&input, &checked, &mut budget);
            assert_eq!(budget.storage(), inspection_floor);
            let coordinate_bytes;
            {
                let (coordinates, storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        source.executable(),
                        &input,
                        profile,
                        &mut budget,
                    )
                    .unwrap();
                coordinate_bytes = storage.retained_storage();
                budget.reserve_storage(coordinate_bytes).unwrap();
                let (view, storage) =
                    fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                        &source,
                        &coordinates,
                        &checked,
                        &mut budget,
                    )
                    .unwrap();
                let view_bytes = storage.retained_storage();
                budget.reserve_storage(view_bytes).unwrap();
                assert!(std::ptr::eq(view.output(), checked.owner()));
                let floor = budget.storage();
                result = body(&source, &view, inputs, &mut budget);
                assert_eq!(budget.storage(), floor);
                drop(view);
                budget.release_storage(view_bytes).unwrap();
            }
            budget.release_storage(coordinate_bytes).unwrap();
            drop(checked);
            budget.release_storage(output_bytes).unwrap();
            drop(input);
            budget.release_storage(input_bytes).unwrap();
        }
        assert_eq!(budget.storage(), source_floor);
        drop(source);
        budget.release_storage(source_bytes).unwrap();
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        result
    }

    mod source_preservation_output_tests {
        use super::*;
        use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1 as References;
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        use fe2o3_lower_mir_kernel::{
            ProductionBorrowedRankedCorrespondenceV1 as Original,
            ProductionSourceOutputErrorV1 as OutputError,
        };

        // Rebuild only admitted syntax from the existing getter factory. Every
        // variant receives fresh admission, SSA planning, capture and N.
        fn source_case(
            value: u32,
            mode: bool,
            stores: usize,
            formal: bool,
            multiple: bool,
        ) -> (
            ProductionSemanticSsaOwnerV1,
            Vec<ProductionRankedRootInputV1>,
        ) {
            let seed = identity_getter_shape_tests::source_ssa_options(value, mode, stores);
            let semantic = seed.source_semantic();
            let mut functions = semantic.functions().to_vec();
            if formal {
                let root = &functions[0];
                let mut locals = root.locals().to_vec();
                let argument = SemanticLocalIdV1::from_index(locals.len() as u32);
                locals.push(local(249, A_U32, SemanticLocalRoleV1::Argument(1)));
                let mut blocks = root.blocks().to_vec();
                let mut changed = 0;
                for block in &mut blocks {
                    let statements = block
                        .statements()
                        .iter()
                        .map(|statement| {
                            let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                            else {
                                return statement.clone();
                            };
                            if assignment.destination().projections().is_empty() {
                                return statement.clone();
                            }
                            assert_eq!(assignment.destination().ty(), A_U32);
                            changed += 1;
                            SemanticStatementV1::new(
                                statement.source(),
                                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                    assignment.destination().clone(),
                                    SemanticRvalueV1::new(
                                        A_U32,
                                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                                            SemanticPlaceV1::new(argument, vec![], A_U32).unwrap(),
                                        )),
                                    ),
                                )),
                            )
                        })
                        .collect();
                    *block = SemanticBasicBlockV1::new(
                        block.identity(),
                        block.source(),
                        statements,
                        block.terminator().clone(),
                    )
                    .unwrap();
                }
                assert_eq!(changed, stores);
                let old = root.abi();
                let mut arguments = old.arguments().to_vec();
                arguments.push(SemanticAbiArgumentV1::source(
                    neutral_plain_direct_abi_value_v1(A_U32),
                ));
                let abi = SemanticFunctionAbiV1::from_rustc(
                    old.identity(),
                    old.layout_identity(),
                    old.canon_abi(),
                    old.extern_abi(),
                    old.can_unwind(),
                    old.c_variadic(),
                    2,
                    arguments,
                    old.return_value().clone(),
                )
                .unwrap()
                .with_source_argument_ownership(vec![
                    SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ])
                .unwrap();
                functions[0] = ordinary_rebuild_v1(root, abi, locals, blocks);
            }
            let mut inputs = vec![ranked_root_input_1d(
                if mode { "other_identity_entry" } else { A_NAME },
                if mode { 246 } else { 247 },
                64,
            )];
            let mut callables = semantic.callables().to_vec();
            let mut roots = semantic.roots().to_vec();
            if multiple {
                let first = &functions[0];
                let second = SemanticFunctionDeclV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(251)),
                    first.role(),
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(252)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(253)),
                    first.generic_type_arguments_identity(),
                    first.const_generic_arguments_identity(),
                    first.source(),
                    first.abi().clone(),
                    first.locals().to_vec(),
                    first.entry(),
                    first.blocks().to_vec(),
                )
                .unwrap()
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(b"source_preservation_second".to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256(bytes(248)),
                    first.kernel_entry().unwrap().source_contract(),
                ));
                let second_id = SemanticFunctionIdV1::from_index(functions.len() as u32);
                functions.push(second);
                let inserted = second_id.index() as usize;
                assert_eq!(inserted, 1);
                assert_eq!(semantic.callables().len(), 3);
                callables.insert(inserted, SemanticCallableDeclV1::defined(second_id));
                // Defined functions form the callable prefix. Preserve each
                // original intrinsic declaration when shifting both roots.
                for function in &mut functions {
                    let mut blocks = function.blocks().to_vec();
                    let mut shifted = [false; 2];
                    for block in &mut blocks {
                        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                            continue;
                        };
                        let old = call.callee().index() as usize;
                        assert!((inserted..semantic.callables().len()).contains(&old));
                        assert!(!shifted[old - inserted]);
                        shifted[old - inserted] = true;
                        let callee = SemanticCallableIdV1::from_index((old + 1) as u32);
                        assert_eq!(
                            &callables[callee.index() as usize],
                            &semantic.callables()[old],
                        );
                        let replacement =
                            SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                                callee,
                                call.arguments().to_vec(),
                                call.variadic_argument_abis().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap();
                        *block = SemanticBasicBlockV1::new(
                            block.identity(),
                            block.source(),
                            block.statements().to_vec(),
                            SemanticTerminatorV1::new(
                                block.terminator().source(),
                                SemanticTerminatorKindV1::Call(replacement),
                            ),
                        )
                        .unwrap();
                    }
                    assert_eq!(shifted, [true, true]);
                    *function = ordinary_rebuild_v1(
                        function,
                        function.abi().clone(),
                        function.locals().to_vec(),
                        blocks,
                    );
                }
                for (ordinal, callable) in callables[..functions.len()].iter().enumerate() {
                    assert_eq!(
                        callable,
                        &SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(
                            ordinal as u32
                        )),
                    );
                }
                roots.push(second_id);
                inputs.push(ranked_root_input_1d("source_preservation_second", 248, 64));
            }
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
            let mir = ProductionSemanticMirOwnerV1::try_new(
                admitted,
                fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap();
            let ssa = ProductionSemanticSsaOwnerV1::try_new(
                mir,
                fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap();
            (ssa, inputs)
        }

        fn with_case(
            profile: Profile,
            value: u32,
            mode: bool,
            stores: usize,
            formal: bool,
            multiple: bool,
            body: impl FnOnce(
                &ProductionPreRankedKirOwnerV1,
                &ProductionSourceOutputOccurrencesV1<'_, '_>,
                &[ProductionRankedRootInputV1],
                &mut Budget<'_>,
            ),
        ) {
            let (ssa, inputs) = source_case(value, mode, stores, formal, multiple);
            with_admitted_source_inputs_fixture(
                ssa,
                profile,
                || inputs,
                |source, view, inputs, budget| {
                    body(source, view, inputs, budget);
                    Ok(())
                },
            )
            .expect("genuine getter source/N/B/O fixture");
        }

        fn with_original(
            source: &ProductionPreRankedKirOwnerV1,
            inputs: &[ProductionRankedRootInputV1],
            references: &References,
            budget: &mut Budget<'_>,
            body: impl FnOnce(&Original<'_>, &mut Budget<'_>),
        ) {
            let floor = budget.storage();
            let mut entered = false;
            with_source_ranked_prefix_v1(source, inputs, references, budget,
                |_, original, verification, _, partition, recorders, budget| {
                    entered = true;
                    assert!(std::ptr::eq(original.materialized(), source));
                    assert_eq!(original.root_count(), inputs.len());
                    assert_eq!(verification.root_count(), inputs.len());
                    assert_eq!(partition.len(), inputs.len());
                    assert_eq!(recorders.len(), inputs.len());
                    assert!(references.as_slice().is_empty());
                    for root in verification.roots() {
                        assert!(!root.verification().has_authenticated_functional_verification());
                        assert!(root.verification().aggregate_verus_execution().is_none());
                    }
                    assert!(matches!(require_source_functional_roster_v1(verification, inputs.len(), budget),
                        Err(SourceJoinPipelineErrorV1::RankedVerification(
                            ProductionRankedVerificationErrorV1::RosterMetadata(
                                "every source-first root requires retained functional and aggregate custody"
                            )
                        ))));
                    body(original, budget);
                    Ok(())
                },
            ).expect("genuine full Expression and same-N R1 with honest None");
            assert!(entered);
            assert_eq!(budget.storage(), floor);
        }

        #[test]
        fn source_preservation_consumer_closes_all_roots_values_and_stores_without_some() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for mode in [false, true] {
                    for (value, stores, formal, multiple) in [
                        (0, 1, false, false),
                        (u32::MAX, 2, false, true),
                        (17, 1, true, true),
                        (29, 2, true, false),
                    ] {
                        with_case(
                            profile,
                            value,
                            mode,
                            stores,
                            formal,
                            multiple,
                            |source, view, inputs, budget| {
                                let references = References::default();
                                with_original(source, inputs, &references, budget, |_, _| {});
                                let floor = budget.storage();
                                let before = budget.work();
                                let mut completed = false;
                                with_source_preservation_output_v1::<()>(view, profile, inputs, &references, budget,
                                |preservation, same_references, formals, accesses, budget| {
                                    completed = true;
                                    assert!(std::ptr::eq(same_references, &references));
                                    assert!(same_references.as_slice().is_empty());
                                    assert_eq!(preservation.roots().len(), inputs.len());
                                    assert_eq!(formals.len(), inputs.len());
                                    assert_eq!(accesses, inputs.len() * stores);
                                    for (ordinal, (root, complete)) in preservation.roots().iter().zip(formals).enumerate() {
                                        assert_eq!(root.selected_root(), source.semantic_ssa().source_semantic().roots()[ordinal]);
                                        assert_eq!(complete.selected_root(), root.selected_root());
                                        assert_eq!(complete.selected_function(), root.selected_body());
                                        assert!(std::ptr::eq(complete.output(), view.output()));
                                        let premises = root.preconditions();
                                        assert_eq!(premises.source_argument(), 0);
                                        assert_eq!(premises.neutral_argument(), 0);
                                        assert!(premises.requires_valid_exclusive_global_u32_slice());
                                        assert!(!premises.requires_representable_invocation_address());
                                        let function = &source.executable().module().functions[root.neutral_function().0 as usize];
                                        let body = function.body.as_ref().unwrap();
                                        assert_eq!(premises.neutral_subject().0, body.parameters[0]);
                                        assert_eq!(body.parameters.len(), if formal { 2 } else { 1 });
                                        assert_eq!(root.checked_rule_count(), 9 + stores);
                                        let mut actual_stores = 0;
                                        for rule in 0..root.checked_rule_count() {
                                            let row = root.checked_rule_v1(rule, budget).unwrap().unwrap();
                                            assert!(row.0 < 6);
                                            assert!(row.3.end <= body.blocks[row.2 as usize].operations.len());
                                            actual_stores += usize::from(row.4 == "store");
                                        }
                                        assert_eq!(actual_stores, stores);
                                        assert!(root.checked_rule_v1(root.checked_rule_count(), budget).unwrap().is_none());
                                    }
                                    Ok(())
                                },
                            ).expect("conditional source/N + existing N/B/O + D/P/R2 + actual Complete");
                                assert!(completed);
                                assert!(references.as_slice().is_empty());
                                assert!(budget.work() > before);
                                assert_eq!(budget.storage(), floor);
                            },
                        );
                    }
                }
            }
        }

        #[test]
        fn source_preservation_consumer_rejects_incomplete_rosters_before_callback() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_case(
                    profile,
                    29,
                    false,
                    1,
                    false,
                    true,
                    |_, view, inputs, budget| {
                        let references = References::default();
                        let floor = budget.storage();
                        for length in [0, 1] {
                            let mut entered = false;
                            let result = with_source_preservation_output_v1::<()>(
                                view,
                                profile,
                                &inputs[..length],
                                &references,
                                budget,
                                |_, _, _, _, _| {
                                    entered = true;
                                    Ok(())
                                },
                            );
                            assert!(matches!(
                                result,
                                Err(SourceJoinPipelineErrorV1::RankedProjection(
                                    ProductionRankedProjectionErrorV1::Unsupported(
                                        "an incomplete typed/semantic ranked root roster"
                                    )
                                ))
                            ));
                            assert!(!entered);
                            assert_eq!(budget.storage(), floor);
                        }
                        let mut reentered = false;
                        with_source_preservation_output_v1::<()>(
                            view,
                            profile,
                            inputs,
                            &references,
                            budget,
                            |_, same, formals, accesses, _| {
                                assert!(std::ptr::eq(same, &references));
                                assert_eq!(formals.len(), 2);
                                assert_eq!(accesses, 2);
                                reentered = true;
                                Ok(())
                            },
                        )
                        .unwrap();
                        assert!(reentered);
                        assert_eq!(budget.storage(), floor);
                    },
                );
            }
        }

        #[test]
        fn source_preservation_consumer_callback_error_panic_and_reentry_keep_floor() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_case(
                    profile,
                    29,
                    true,
                    2,
                    false,
                    true,
                    |_, view, inputs, budget| {
                        let references = References::default();
                        let floor = budget.storage();
                        let mut entered = false;
                        let error = with_source_preservation_output_v1::<()>(
                            view,
                            profile,
                            inputs,
                            &references,
                            budget,
                            |preservation, same, formals, accesses, _| {
                                assert_eq!(preservation.roots().len(), 2);
                                assert_eq!(formals.len(), 2);
                                assert_eq!(accesses, 4);
                                assert!(std::ptr::eq(same, &references));
                                entered = true;
                                Err(SourceJoinPipelineErrorV1::RankedProjection(
                                    ProductionRankedProjectionErrorV1::Incomplete(
                                        "source preservation consumer callback marker",
                                    ),
                                ))
                            },
                        );
                        assert!(entered);
                        assert!(matches!(
                            error,
                            Err(SourceJoinPipelineErrorV1::RankedProjection(
                                ProductionRankedProjectionErrorV1::Incomplete(
                                    "source preservation consumer callback marker"
                                )
                            ))
                        ));
                        assert_eq!(budget.storage(), floor);
                        let mut panic_entered = false;
                        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            with_source_preservation_output_v1::<()>(
                                view,
                                profile,
                                inputs,
                                &references,
                                budget,
                                |preservation, same, formals, accesses, _| {
                                    assert_eq!(preservation.roots().len(), formals.len());
                                    assert_eq!(accesses, 4);
                                    assert!(std::ptr::eq(same, &references));
                                    panic_entered = true;
                                    std::panic::panic_any(0x3450_77u64)
                                },
                            )
                        }))
                        .expect_err("callback must unwind after actual complete conjunction");
                        assert!(panic_entered);
                        assert_eq!(panic.downcast_ref::<u64>(), Some(&0x3450_77));
                        assert_eq!(budget.storage(), floor);
                        let mut reentered = false;
                        with_source_preservation_output_v1::<()>(
                            view,
                            profile,
                            inputs,
                            &references,
                            budget,
                            |_, _, formals, accesses, _| {
                                assert_eq!(formals.len(), 2);
                                assert_eq!(accesses, 4);
                                reentered = true;
                                Ok(())
                            },
                        )
                        .unwrap();
                        assert!(reentered);
                        assert_eq!(budget.storage(), floor);
                    },
                );
            }
        }

        #[test]
        fn collected_preservation_observer_counts_and_callback_outcomes_keep_custody() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for mode in [false, true] {
                    with_case(
                        profile,
                        29,
                        mode,
                        2,
                        false,
                        true,
                        |_, view, inputs, budget| {
                            let references = References::default();
                            let floor = budget.storage();
                            let before = budget.work();
                            let mut entered = false;
                            let error = observe_collected_ranked_addresses_v1(
                                view,
                                profile,
                                inputs,
                                &references,
                                budget,
                                |roots, accesses, complete_roots, _| {
                                    assert_eq!((roots, accesses, complete_roots), (2, 4, 2));
                                    entered = true;
                                    Err("collected conditional callback marker".to_owned())
                                },
                            );
                            assert!(entered);
                            assert_eq!(
                                error,
                                Err("collected conditional callback marker".to_owned())
                            );
                            assert_eq!(budget.storage(), floor);
                            let mut panic_entered = false;
                            let panic =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    observe_collected_ranked_addresses_v1(
                                        view,
                                        profile,
                                        inputs,
                                        &references,
                                        budget,
                                        |roots, accesses, complete_roots, _| {
                                            assert_eq!(
                                                (roots, accesses, complete_roots),
                                                (2, 4, 2)
                                            );
                                            panic_entered = true;
                                            std::panic::panic_any(0x3490_17u64)
                                        },
                                    )
                                }))
                                .expect_err("actual collected callback panic remains a panic");
                            assert!(panic_entered);
                            assert_eq!(panic.downcast_ref::<u64>(), Some(&0x3490_17));
                            assert_eq!(budget.storage(), floor);
                            let mut completed = false;
                            observe_collected_ranked_addresses_v1(
                                view,
                                profile,
                                inputs,
                                &references,
                                budget,
                                |roots, accesses, complete_roots, _| {
                                    assert_eq!((roots, accesses, complete_roots), (2, 4, 2));
                                    completed = true;
                                    Ok(())
                                },
                            )
                            .unwrap();
                            assert!(completed);
                            assert!(references.as_slice().is_empty());
                            assert_eq!(budget.storage(), floor);
                            assert!(budget.work() > before);
                        },
                    );
                }
            }
        }

        #[test]
        fn collected_preservation_observer_accounting_precedes_callback_outcomes() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_case(
                    profile,
                    17,
                    false,
                    1,
                    false,
                    false,
                    |_, view, inputs, budget| {
                        let references = References::default();
                        let floor = budget.storage();
                        for outcome in 0..3 {
                            let mut entered = false;
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    observe_collected_ranked_addresses_v1(
                                        view,
                                        profile,
                                        inputs,
                                        &references,
                                        budget,
                                        |roots, accesses, complete_roots, budget| {
                                            assert_eq!(
                                                (roots, accesses, complete_roots),
                                                (1, 1, 1)
                                            );
                                            entered = true;
                                            budget
                                                .release_storage(budget.storage() - floor)
                                                .unwrap();
                                            match outcome {
                                                0 => Ok(()),
                                                1 => Err("must lose to accounting".to_owned()),
                                                _ => std::panic::panic_any(0x3490_18u64),
                                            }
                                        },
                                    )
                                }))
                                .expect(
                                    "outer retained-floor Accounting takes precedence over panic",
                                );
                            assert!(entered);
                            let error = result.expect_err("live proof receipts were released");
                            assert!(error.contains("storage accounting failed"), "{error}");
                            assert!(!error.contains("must lose to accounting"));
                            assert_eq!(budget.storage(), floor);
                        }
                        let mut reentered = false;
                        observe_collected_ranked_addresses_v1(
                            view,
                            profile,
                            inputs,
                            &references,
                            budget,
                            |roots, accesses, complete_roots, _| {
                                assert_eq!((roots, accesses, complete_roots), (1, 1, 1));
                                reentered = true;
                                Ok(())
                            },
                        )
                        .unwrap();
                        assert!(reentered);
                        assert_eq!(budget.storage(), floor);
                    },
                );
            }
        }

        #[test]
        fn source_preservation_genuine_r1_rejects_foreign_original_and_query_ledger() {
            with_case(
                Profile::Gfx942,
                17,
                false,
                1,
                false,
                false,
                |foreign_source, _, foreign_inputs, foreign_budget| {
                    let foreign_references = References::default();
                    with_original(
                        foreign_source,
                        foreign_inputs,
                        &foreign_references,
                        foreign_budget,
                        |foreign, _| {
                            with_case(
                                Profile::Gfx942,
                                17,
                                false,
                                1,
                                false,
                                false,
                                |source, _, inputs, budget| {
                                    let references = References::default();
                                    with_original(
                                        source,
                                        inputs,
                                        &references,
                                        budget,
                                        |original, budget| {
                                            assert!(!std::ptr::eq(
                                                original.materialized(),
                                                foreign.materialized()
                                            ));
                                            assert_eq!(
                                                original
                                                    .materialized()
                                                    .semantic_ssa()
                                                    .source_semantic()
                                                    .semantic_sha256(),
                                                foreign
                                                    .materialized()
                                                    .semantic_ssa()
                                                    .source_semantic()
                                                    .semantic_sha256()
                                            );
                                            let floor = budget.storage();
                                            original.with_source_preservation_v1(budget, |proof, budget| {
                                assert!(matches!(proof.require_original_v1(foreign, budget),
                                    Err(OutputError::Invalid("source preservation original roster differs"))));
                                let held = budget.storage();
                                let before = budget.work();
                                let mut work = Work::new(LIMIT);
                                let mut other = Budget::new(&mut work, held);
                                other.reserve_storage(held).unwrap();
                                assert!(matches!(proof.require_original_v1(original, &mut other),
                                    Err(OutputError::Resource(Resource::Accounting))));
                                assert!(matches!(proof.roots()[0].checked_rule_v1(0, &mut other),
                                    Err(OutputError::Resource(Resource::Accounting))));
                                assert_eq!(other.work(), 4 + 8);
                                assert_eq!(budget.work(), before);
                                assert_eq!(other.storage(), held);
                                assert_eq!(budget.storage(), held);
                                let before = budget.work();
                                proof.require_original_v1(original, budget)?;
                                assert_eq!(budget.work(), before + 4);
                                assert!(proof.roots()[0].checked_rule_v1(0, budget)?.is_some());
                                assert_eq!(budget.work(), before + 4 + 8);
                                Ok(())
                            }).unwrap();
                                            assert_eq!(budget.storage(), floor);
                                        },
                                    );
                                },
                            );
                        },
                    );
                },
            );
        }

        #[test]
        fn source_preservation_genuine_r1_queries_refuse_lost_live_floor_then_reenter() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_case(
                    profile,
                    17,
                    false,
                    1,
                    false,
                    false,
                    |source, _, inputs, budget| {
                        let references = References::default();
                        with_original(source, inputs, &references, budget, |original, budget| {
                            let floor = budget.storage();
                            original
                                .with_source_preservation_v1(budget, |proof, budget| {
                                    let held = budget.storage();
                                    budget.release_storage(1).unwrap();
                                    assert!(matches!(
                                        proof.require_original_v1(original, budget),
                                        Err(OutputError::Resource(Resource::Accounting))
                                    ));
                                    assert!(matches!(
                                        proof.roots()[0].checked_rule_v1(0, budget),
                                        Err(OutputError::Resource(Resource::Accounting))
                                    ));
                                    budget.reserve_storage(1).unwrap();
                                    proof.require_original_v1(original, budget)?;
                                    assert!(proof.roots()[0].checked_rule_v1(0, budget)?.is_some());
                                    assert_eq!(budget.storage(), held);
                                    Ok(())
                                })
                                .unwrap();
                            assert_eq!(budget.storage(), floor);
                        });
                    },
                );
            }
        }

        #[test]
        fn source_preservation_genuine_r1_postflight_accounting_precedes_all_callback_outcomes() {
            for outcome in 0..3 {
                with_case(
                    Profile::Gfx942,
                    29,
                    true,
                    2,
                    false,
                    false,
                    |source, _, inputs, budget| {
                        let references = References::default();
                        with_original(source, inputs, &references, budget, |original, budget| {
                            let floor = budget.storage();
                            let mut entered = false;
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    original.with_source_preservation_v1::<()>(
                                        budget,
                                        |proof, budget| {
                                            proof.require_original_v1(original, budget)?;
                                            entered = true;
                                            budget.release_storage(1).unwrap();
                                            match outcome {
                                                0 => Ok(()),
                                                1 => Err(OutputError::Invalid(
                                                    "lost-floor error marker",
                                                )),
                                                _ => std::panic::panic_any(0x3450_99u64),
                                            }
                                        },
                                    )
                                }))
                                .expect(
                                    "Accounting must replace a callback panic after floor loss",
                                );
                            assert!(entered);
                            assert!(matches!(
                                result,
                                Err(OutputError::Resource(Resource::Accounting))
                            ));
                            assert_eq!(budget.storage(), floor);
                            let mut reentered = false;
                            original
                                .with_source_preservation_v1(budget, |proof, budget| {
                                    proof.require_original_v1(original, budget)?;
                                    reentered = true;
                                    Ok(())
                                })
                                .unwrap();
                            assert!(reentered);
                            assert_eq!(budget.storage(), floor);
                        });
                    },
                );
            }
        }

        #[test]
        fn source_preservation_genuine_r1_header_denial_preserves_incoming_floor() {
            with_case(
                Profile::Gfx942,
                17,
                false,
                1,
                false,
                false,
                |source, _, inputs, budget| {
                    let references = References::default();
                    with_original(source, inputs, &references, budget, |original, budget| {
                        let floor = budget.storage();
                        let header = std::mem::size_of::<
                            Vec<fe2o3_lower_mir_kernel::ProductionSourcePreservationRootV1>,
                        >() + std::mem::size_of::<
                            fe2o3_lower_mir_kernel::ProductionScopedSourcePreservationV1<'_, '_>,
                        >();
                        let padding = STORAGE_LIMIT - floor - (header - 1);
                        budget.reserve_storage(padding).unwrap();
                        let padded = budget.storage();
                        let mut entered = false;
                        let result = original.with_source_preservation_v1(budget, |_, _| {
                            entered = true;
                            Ok(())
                        });
                        assert!(
                            matches!(result, Err(OutputError::Resource(Resource::Storage(error)))
                        if error.actual() == STORAGE_LIMIT + 1 && error.limit() == STORAGE_LIMIT)
                        );
                        assert!(!entered);
                        assert_eq!(budget.storage(), padded);
                        budget.release_storage(padding).unwrap();
                        assert_eq!(budget.storage(), floor);
                        original
                            .with_source_preservation_v1(budget, |proof, budget| {
                                proof.require_original_v1(original, budget)
                            })
                            .unwrap();
                        assert_eq!(budget.storage(), floor);
                    });
                },
            );
        }
    }

    mod full_expression_recorder_tests {
        use super::*;
        use canonical_memory_control_v1::CanonicalMemoryControlRecorderV1 as Recorder;
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        use fe2o3_lower_mir_kernel::ProductionProjectionArgumentComponentV1 as Component;

        fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
            )
        }

        pub(super) fn with_full_roots(
            source: &ProductionPreRankedKirOwnerV1,
            view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
            inputs: &[ProductionRankedRootInputV1],
            budget: &mut Budget<'_>,
            inspect: impl FnOnce(
                ProductionRankedRootProgramV1,
                &Recorder,
                &mut Budget<'_>,
            ) -> Result<(), ProductionRankedProjectionErrorV1>,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            with_full_roots_and_references(
                source,
                view,
                inputs,
                budget,
                |full, recorder, _, budget| inspect(full, recorder, budget),
            )
        }

        pub(super) fn with_full_roots_and_references(
            source: &ProductionPreRankedKirOwnerV1,
            view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
            inputs: &[ProductionRankedRootInputV1],
            budget: &mut Budget<'_>,
            inspect: impl FnOnce(
                ProductionRankedRootProgramV1,
                &Recorder,
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
                &mut Budget<'_>,
            ) -> Result<(), ProductionRankedProjectionErrorV1>,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
            // This admitted factory declares no reference expression. Keep the
            // actual original object and its derived partitions throughout.
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            assert!(references.as_slice().is_empty());
            let result = with_ranked_root_preparation_v1(&projection, inputs, &references, |effects, partition| {
                checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                    session.with_canonical_memory_scope_v1(|session| {
                        let source_root = projection.source_launch().roots()[0];
                        let selection = projection.semantic_ssa().source_semantic()
                            .select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                        let (legacy, recorder, full) = {
                            let mut facts = session.for_source(source_root.selected_root(), selection.body());
                            let legacy = project_and_verify_ranked_root_control_inner_v1(
                                projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                &partition[0], &mut facts,
                            )?;
                            let mut recorder = Recorder::new(&mut facts)?;
                            let full = project_and_verify_ranked_root_control_with_address_claims_v1(
                                projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                &partition[0], &mut facts, Some(&mut recorder),
                            )?;
                            (legacy, recorder, full)
                        };
                        assert_eq!(legacy.lowering.kernel(), full.lowering.kernel());
                        assert_eq!(legacy.ranked_ir, full.ranked_ir);
                        assert_eq!(legacy.access_sources, full.access_sources);
                        assert_eq!(legacy.executable_effect_sources, full.executable_effect_sources);
                        assert_eq!(legacy.logical_name, full.logical_name);
                        assert_eq!(legacy.export_symbol, full.export_symbol);
                        assert_eq!(legacy.semantic_root, full.semantic_root);
                        assert_eq!(legacy.semantic_root_identity, full.semantic_root_identity);
                        assert_eq!(legacy.kernel_binding, full.kernel_binding);
                        assert_eq!(legacy.source_rank, full.source_rank);
                        drop(legacy);
                        session.with_output_occurrences_v1(|same_view, budget| {
                            assert!(std::ptr::eq(same_view.source(), source));
                            inspect(full, &recorder, &references, budget)
                        })
                    })
                })
            });
            assert!(references.as_slice().is_empty());
            result
        }

        fn source_leaves(
            function: &SemanticFunctionDeclV1,
            callables: &[SemanticCallableDeclV1],
        ) -> (SemanticLocalIdV1, SemanticLocalIdV1, usize) {
            let calls = function.blocks().iter().filter_map(|block| {
                let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { return None; };
                matches!(callables.get(call.callee().index() as usize), Some(
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }, ..
                    }
                )).then_some(call)
            }).collect::<Vec<_>>();
            assert_eq!(calls.len(), 1);
            let [receiver, witness] = calls[0].arguments() else { panic!("exact getter operands"); };
            let receiver = raw_operand_place(receiver).unwrap();
            let witness = raw_operand_place(witness).unwrap();
            assert!(receiver.projections().is_empty() && witness.projections().is_empty());
            let slices = function.blocks().iter().flat_map(|block| block.statements()).filter_map(|statement| {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { return None; };
                if assignment.destination().local() != receiver.local() { return None; }
                let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else { panic!("receiver borrow"); };
                assert!(place.projections().is_empty());
                assert!(matches!(function.locals()[place.local().index() as usize].role(), SemanticLocalRoleV1::Argument(_)));
                Some(place.local())
            }).collect::<Vec<_>>();
            assert_eq!(slices.len(), 1);
            (witness.local(), slices[0], calls[0].arguments().len())
        }

        #[test]
        fn full_identity_expression_has_typed_parity_own_claims_and_genuine_same_n_r1() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for (value, collected_modes) in [(17, false), (29, true)] {
                    for stores in [1, 2] {
                        let ssa = identity_getter_shape_tests::source_ssa_options(value, collected_modes, stores);
                        let mut completed = false;
                        with_admitted_source_fixture(ssa, collected_modes, profile, |source, view, inputs, budget| {
                            let semantic = source.semantic_ssa().source_semantic();
                            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                            let selected = semantic.select_kernel_body_for_root_v1(projection.source_launch().roots()[0].selected_root()).unwrap();
                            let function = &semantic.functions()[selected.body().index() as usize];
                            let (witness, slice, _) = source_leaves(function, semantic.callables());
                            with_full_roots(source, view, inputs, budget, |full, recorder, budget| {
                                assert_eq!(full.access_sources.len(), stores);
                                let arguments = &recorder.candidate().arguments;
                                let index_rows = arguments.iter().filter(|row| row.source_local == witness && row.component == Component::Scalar).collect::<Vec<_>>();
                                let extent_rows = arguments.iter().filter(|row| row.source_local == slice && row.component == Component::SliceLength).collect::<Vec<_>>();
                                assert_eq!(index_rows.len(), 1);
                                assert_eq!(extent_rows.len(), 1);
                                let index = index_rows[0].ranked_value;
                                let extent = extent_rows[0].ranked_value;
                                let operations = full.lowering.kernel().blocks().iter().flat_map(|block| block.operations());
                                assert_eq!(operations.clone().filter(|operation| matches!(operation,
                                    ProductionRankedOperationV1::InvocationIndex { result, dimension: 0, launch_extent: 0 }
                                    if index == ProductionRankedValueV1::Local(*result))).count(), 1);
                                let views = operations.clone().filter_map(|operation| match operation {
                                    ProductionRankedOperationV1::ViewInSpace { result, dynamic_extents, writable: true, memory_space: MemorySpaceAttr::Global, .. }
                                        if dynamic_extents.as_slice() == [extent] => Some(*result),
                                    _ => None,
                                }).collect::<Vec<_>>();
                                assert_eq!(views.len(), 1);
                                assert_eq!(operations.filter(|operation| matches!(operation,
                                    ProductionRankedOperationV1::ValueAccess { kind: AccessKindAttr::Write, view, indices, .. }
                                    if *view == ProductionRankedValueV1::Local(views[0]) && indices.as_slice() == [index])).count(), stores);
                                let floor = budget.storage();
                                with_authenticated_borrowed_ranked_source_roster_v1(source, vec![full].into_boxed_slice(), budget, |original, verification, budget| {
                                    completed = true;
                                    assert!(std::ptr::eq(original.materialized(), source));
                                    assert_eq!(original.root_count(), 1);
                                    assert_eq!(verification.roots().len(), 1);
                                    assert!(!verification.roots()[0].verification().has_authenticated_functional_verification());
                                    assert!(verification.roots()[0].verification().aggregate_verus_execution().is_none());
                                    assert!(budget.storage() >= floor);
                                    Ok(())
                                }).expect("actual full Expression must satisfy genuine same-N R1; no D-root substitute");
                                assert_eq!(budget.storage(), floor);
                                Ok(())
                            })
                        }).expect("actual recorded and unrecorded full Expression prerequisite");
                        assert!(completed);
                    }
                }
            }
        }

        // Meter-only adapter: unexpected semantic queries panic. It cannot
        // create checked output/control or supply source-proof authority.
        struct Facts<'a, 'w> {
            budget: &'a mut Budget<'w>,
            work_calls: Vec<usize>,
            reservations: Vec<usize>,
        }
        impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
            fn charge_private_array_work(&mut self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
                self.work_calls.push(amount);
                self.budget.charge_work(amount).map_err(resource)
            }
            fn reserve_checked_control_storage_v1(&mut self, bytes: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
                self.reservations.push(bytes);
                self.budget.reserve_storage(bytes).map_err(resource)
            }
            fn private_array_access(&mut self, _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1, _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1) -> Result<bool, ProductionRankedProjectionErrorV1> { panic!("meter-only recorder component"); }
            fn private_array_constant_index(&mut self, _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1, _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> { panic!("meter-only recorder component"); }
            fn is_materialized_block(&mut self, _: usize) -> Result<bool, ProductionRankedProjectionErrorV1> { panic!("meter-only recorder component"); }
            fn condition(&mut self, _: usize, _: bool, _: SemanticBlockIdV1) -> Result<canonical_assertion_facts_v1::ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> { panic!("meter-only recorder component"); }
        }

        fn with_geometry(body: impl FnOnce(&SemanticFunctionDeclV1, &[SemanticCallableDeclV1], &mut IntrinsicProjectionV1)) {
            let ssa = identity_getter_shape_tests::source_ssa_options(17, false, 1);
            with_admitted_source_fixture(ssa, false, Profile::Gfx942, |source, view, inputs, budget| {
                let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                with_ranked_root_preparation_v1(&projection, inputs, &references, |effects, partition| {
                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| session.with_canonical_memory_scope_v1(|session| {
                        let root = projection.source_launch().roots()[0];
                        let semantic = projection.semantic_ssa().source_semantic();
                        let selection = semantic.select_kernel_body_for_root_v1(root.selected_root()).unwrap();
                        let mut facts = session.for_source(root.selected_root(), selection.body());
                        let mut geometry = prepare_projected_ranked_geometry_v1(projection.semantic_ssa(), effects, selection, &inputs[0], root, &partition[0], &mut facts, ProjectedGlobalWriteValuesV1::Expression, None)?;
                        assert!(geometry.incomplete.is_none());
                        assert_eq!(geometry.intrinsic.guarded_accesses.len(), 1);
                        body(&semantic.functions()[selection.body().index() as usize], semantic.callables(), &mut geometry.intrinsic);
                        Ok(())
                    }))
                })
            }).unwrap();
        }

        fn scan_schedule(function: &SemanticFunctionDeclV1, arguments: usize) -> Vec<usize> {
            let mut schedule = vec![8];
            schedule.extend(std::iter::repeat_n(4, function.blocks().len()));
            schedule.push(arguments + 8);
            for block in function.blocks() {
                schedule.push(5);
                schedule.extend(std::iter::repeat_n(14, block.statements().len()));
            }
            schedule
        }

        #[test]
        fn full_identity_recorder_unsupported_components_add_no_claims() {
            with_geometry(|function, callables, intrinsic| {
                let (witness, _, _) = source_leaves(function, callables);
                let access = intrinsic.guarded_accesses[0].clone();
                let projected = intrinsic.index_values[witness.index() as usize].unwrap();
                for case in 0..9 {
                    intrinsic.guarded_accesses = vec![access.clone()];
                    intrinsic.index_values[witness.index() as usize] = Some(projected);
                    match case {
                        0 => intrinsic.guarded_accesses.clear(),
                        1 => intrinsic.guarded_accesses.push(access.clone()),
                        2 => intrinsic.guarded_accesses[0].comparisons.push(access.comparisons[0]),
                        3 => intrinsic.guarded_accesses[0].checked_success = Some(projected.value),
                        4 => intrinsic.guarded_accesses[0].access = AccessKindAttr::Read,
                        5 => intrinsic.index_values[witness.index() as usize] = None,
                        6 => intrinsic.index_values[witness.index() as usize].as_mut().unwrap().precondition = Some((projected.value, projected.value)),
                        7 => intrinsic.guarded_accesses[0].memory_space = MemorySpaceAttr::Private,
                        8 => intrinsic.guarded_accesses[0].indices.clear(),
                        _ => unreachable!(),
                    }
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                    {
                        let mut facts = Facts { budget: &mut budget, work_calls: Vec::new(), reservations: Vec::new() };
                        let mut recorder = Recorder::new(&mut facts).unwrap();
                        recorder.arguments(&[Some(u32::MAX)], &[], &mut facts).unwrap();
                        let before = recorder.candidate().arguments.clone();
                        let reservations = facts.reservations.clone();
                        recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts).unwrap();
                        assert_eq!(recorder.candidate().arguments, before, "unsupported component {case}");
                        assert_eq!(facts.reservations, reservations);
                    }
                    budget.release_storage(budget.storage()).unwrap();
                }
            });
        }

        #[test]
        fn full_identity_recorder_reuses_exact_claims_and_rejects_conflicts() {
            with_geometry(|function, callables, intrinsic| {
                let (_, _, arguments) = source_leaves(function, callables);
                let scan = scan_schedule(function, arguments).into_iter().sum::<usize>();
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                {
                    let mut facts = Facts { budget: &mut budget, work_calls: Vec::new(), reservations: Vec::new() };
                    let mut recorder = Recorder::new(&mut facts).unwrap();
                    recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts).unwrap();
                    assert_eq!(recorder.candidate().arguments.len(), 2);
                    let rows = recorder.candidate().arguments.clone();
                    let storage = facts.budget.storage();
                    let before = facts.budget.work();
                    recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts).unwrap();
                    assert_eq!(recorder.candidate().arguments, rows);
                    assert_eq!(facts.budget.storage(), storage);
                    assert_eq!(facts.budget.work() - before, scan + 2 * (2 + 3));
                    for index in 0..2 {
                        recorder.candidate_mut().arguments[index].source_local = SemanticLocalIdV1::from_index(u32::MAX);
                        let before = facts.budget.work();
                        assert!(matches!(recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts), Err(ProductionRankedProjectionErrorV1::Incomplete("canonical identity anchor conflicts with another claim"))));
                        assert_eq!(facts.budget.work() - before, scan + (index + 1) * (2 + 3));
                        assert_eq!(facts.budget.storage(), storage);
                        recorder.candidate_mut().arguments[index] = rows[index];
                    }
                    recorder.candidate_mut().arguments[0].component = Component::SliceLength;
                    assert!(matches!(recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts), Err(ProductionRankedProjectionErrorV1::Incomplete("canonical identity anchor conflicts with another claim"))));
                    assert_eq!(facts.budget.storage(), storage);
                }
                budget.release_storage(budget.storage()).unwrap();
            });
        }

        #[test]
        fn full_identity_recorder_prepays_every_scan_prefix_before_claims() {
            with_geometry(|function, callables, intrinsic| {
                let (_, _, arguments) = source_leaves(function, callables);
                let mut schedule = scan_schedule(function, arguments);
                schedule.extend([3, 6]);
                let mut accepted = 1;
                for (ordinal, charge) in schedule.iter().copied().enumerate() {
                    let limit = accepted + charge - 1;
                    let mut work = Work::new(limit);
                    {
                        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                        {
                        let mut facts = Facts { budget: &mut budget, work_calls: Vec::new(), reservations: Vec::new() };
                        let mut recorder = Recorder::new(&mut facts).unwrap();
                        let error = recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts).unwrap_err();
                        assert!(matches!(error, ProductionRankedProjectionErrorV1::CanonicalAssertions(canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error))) if error.actual() == accepted + charge && error.limit() == limit));
                        assert_eq!(facts.budget.work(), accepted);
                        assert_eq!(&facts.work_calls[1..], &schedule[..=ordinal]);
                        assert!(recorder.candidate().arguments.is_empty());
                        assert_eq!(facts.reservations, [std::mem::size_of::<Recorder>()]);
                        }
                        budget.release_storage(budget.storage()).unwrap();
                    }
                    assert_eq!(work.failed_work(), Some(accepted + charge));
                    accepted += charge;
                }
            });
        }

        #[test]
        fn full_identity_recorder_first_growth_and_post_allocation_denials_are_exact() {
            with_geometry(|function, callables, intrinsic| {
                let (_, _, arguments) = source_leaves(function, callables);
                let scan = scan_schedule(function, arguments).into_iter().sum::<usize>();
                let header = std::mem::size_of::<Recorder>();
                let row = std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionProjectionArgumentCandidateV1>();
                for storage_denial in [true, false] {
                    // New recorder 1, scan, first join 3, push 6, empty-growth
                    // relocation 0; second join 4 is denied after the first row.
                    let accepted = 1 + scan + 3 + 6;
                    let work_limit = if storage_denial { LIMIT } else { accepted + 4 - 1 };
                    let storage_limit = if storage_denial { header + row - 1 } else { STORAGE_LIMIT };
                    let mut work = Work::new(work_limit);
                    let mut budget = Budget::new(&mut work, storage_limit);
                    {
                        let mut facts = Facts { budget: &mut budget, work_calls: Vec::new(), reservations: Vec::new() };
                        let mut recorder = Recorder::new(&mut facts).unwrap();
                        let error = recorder.identity_address_arguments_v1(function, callables, intrinsic, &[], &mut facts).unwrap_err();
                        assert_eq!(facts.budget.work(), accepted);
                        if storage_denial {
                            assert!(matches!(error, ProductionRankedProjectionErrorV1::CanonicalAssertions(canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Storage(error))) if error.actual() == header + row && error.limit() == storage_limit));
                            assert!(recorder.candidate().arguments.is_empty());
                            assert_eq!(recorder.candidate().arguments.capacity(), 0);
                            assert_eq!(facts.budget.failed_storage(), Some(header + row));
                        } else {
                            assert!(matches!(error, ProductionRankedProjectionErrorV1::CanonicalAssertions(canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error))) if error.actual() == accepted + 4 && error.limit() == work_limit));
                            assert_eq!(recorder.candidate().arguments.len(), 1);
                            assert!(recorder.candidate().arguments.capacity() >= 1);
                            assert_eq!(facts.budget.storage(), header + recorder.candidate().arguments.capacity() * row);
                        }
                    }
                    budget.release_storage(budget.storage()).unwrap();
                }
            });
        }

        #[test]
        fn full_identity_recorder_balanced_callback_error_panic_and_reentry_keep_floor() {
            let ssa = identity_getter_shape_tests::source_ssa_options(17, false, 1);
            with_admitted_source_fixture(ssa, false, Profile::Gfx942, |source, view, inputs, budget| {
                let floor = budget.storage();
                let before = budget.work();
                let mut error_entered = false;
                let error = with_full_roots(source, view, inputs, budget, |full, recorder, _| {
                    error_entered = true;
                    assert!(!full.access_sources.is_empty());
                    assert!(recorder.candidate().arguments.len() >= 2);
                    Err(ProductionRankedProjectionErrorV1::Incomplete("full recorder callback error"))
                });
                assert!(error_entered);
                assert!(matches!(error, Err(ProductionRankedProjectionErrorV1::Incomplete("full recorder callback error"))));
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > before);
                let after_error = budget.work();
                let mut panic_entered = false;
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_full_roots(source, view, inputs, budget, |full, recorder, _| {
                        panic_entered = true;
                        assert!(!full.access_sources.is_empty());
                        assert!(recorder.candidate().arguments.len() >= 2);
                        panic!("full recorder callback panic");
                    })
                })).expect_err("the completed full recorder callback must unwind");
                assert!(panic_entered);
                let message = panic.downcast_ref::<&'static str>().copied()
                    .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
                assert_eq!(message, Some("full recorder callback panic"));
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > after_error);
                let after_panic = budget.work();
                let mut reentered = false;
                with_full_roots(source, view, inputs, budget, |full, recorder, _| {
                    reentered = true;
                    assert!(!full.access_sources.is_empty());
                    assert!(recorder.candidate().arguments.len() >= 2);
                    Ok(())
                })?;
                assert!(reentered);
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > after_panic);
                Ok(())
            }).expect("actual full Expression callback prerequisite, not D projection");
        }
    }

    mod full_identity_address_tests {
        use super::*;
        use fe2o3_lower_mir_kernel::{
            ProductionBorrowedRankedCorrespondenceV1 as Original,
            ProductionProjectionArgumentComponentV1 as Component,
            ProductionProjectionControlCandidateV1 as Claims,
        };

        fn with_join(
            profile: Profile,
            mode: bool,
            stores: usize,
            body: impl FnOnce(
                &Analysis<'_, '_, '_>,
                &Original<'_>,
                &Claims,
                &mut Budget<'_>,
            ) -> Result<(), ProductionSourceOutputErrorV1>,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            let ssa = identity_getter_shape_tests::source_ssa_options(29, mode, stores);
            with_admitted_source_fixture(ssa, mode, profile, |source, view, inputs, budget| {
                full_expression_recorder_tests::with_full_roots_and_references(
                    source,
                    view,
                    inputs,
                    budget,
                    |full, recorder, references, budget| {
                        assert_eq!(full.access_sources.len(), stores);
                        let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                        let floor = budget.storage();
                        let mut r1_entered = false;
                        let result = with_authenticated_borrowed_ranked_source_roster_v1(
                            source, vec![full].into_boxed_slice(), budget,
                            |original, verification, budget| {
                                r1_entered = true;
                                assert!(std::ptr::eq(original.materialized(), source));
                                assert_eq!(original.root_count(), 1);
                                assert_eq!(verification.roots().len(), 1);
                                assert!(!verification.roots()[0].verification().has_authenticated_functional_verification());
                                assert!(verification.roots()[0].verification().aggregate_verus_execution().is_none());
                                Ok(with_ranked_root_preparation_v1(&projection, inputs, references, |effects, partition| {
                                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                                        with_prepared_canonical_memory_session_v1(&projection, inputs, effects, partition, session, |analyses, budget| {
                                            assert_eq!(analyses.len(), 1);
                                            assert_eq!(analyses[0].access_count(), stores);
                                            assert!(std::ptr::eq(analyses[0].output(), view.output()));
                                            body(&analyses[0], original, recorder.candidate(), budget)
                                        })
                                    })
                                }))
                            },
                        ).expect("genuine full Expression/R1 prerequisite, never a D-root substitute");
                        assert!(r1_entered);
                        assert_eq!(budget.storage(), floor);
                        assert!(references.as_slice().is_empty());
                        result
                    },
                )
            })
        }

        fn claims_copy(claims: &Claims) -> Claims {
            Claims {
                blocks: claims.blocks.clone(),
                arguments: claims.arguments.clone(),
            }
        }

        #[test]
        fn selected_zero_source_r1_d_p_r2_composition_keeps_each_own_some_store() {
            use fe2o3_kernel_ir::{
                CanonicalKirDefinitionCoordinateV1 as Def, CanonicalKirUseCoordinateV1 as Use,
                CastKind, OperationKind as Op, Terminator as Term,
            };
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for mode in [false, true] {
                    for stores in [1, 2] {
                        let mut source_entered = false;
                        let mut r2_completed = false;
                        with_join(profile, mode, stores, |analysis, original, claims, budget| {
                            let floor = budget.storage();
                            original.with_source_preservation_v1(budget, |proof, budget| {
                                source_entered = true;
                                proof.require_original_v1(original, budget)?;
                                assert_eq!(proof.roots().len(), 1);
                                let root = &proof.roots()[0];
                                let premises = root.preconditions();
                                assert!(premises.requires_valid_exclusive_global_u32_slice());
                                assert!(!premises.requires_representable_invocation_address());
                                let n = original.materialized().executable().module().functions
                                    [root.neutral_function().0 as usize].body.as_ref().unwrap();
                                let mut getters = 0;
                                let mut source_stores = 0;
                                for ordinal in 0..root.checked_rule_count() {
                                    let (_, _, block, span, rule) =
                                        root.checked_rule_v1(ordinal, budget)?.unwrap();
                                    if rule == "getter" {
                                        getters += 1;
                                        assert_eq!(span.len(), 6);
                                        let operations = &n.blocks[block as usize].operations[span];
                                        let Op::GetElementPointer { offset, .. } = operations[5].kind else {
                                            panic!("exact source/N getter GEP")
                                        };
                                        assert_eq!(offset, operations[3].results[0].id);
                                        identity_getter_shape_tests::selected_offset_shape(n, offset);
                                    }
                                    source_stores += usize::from(rule == "store");
                                }
                                assert_eq!((getters, source_stores), (1, stores));
                                let held = budget.storage();
                                analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                                    assert_eq!(relation.global_access_count(), stores);
                                    assert_eq!(relation.private_access_count(), 0);
                                    let mut previous_store = None;
                                    let mut shared_offset = None;
                                    let mut shared_condition = None;
                                    for ordinal in 0..stores {
                                        let address = relation.access(ordinal, budget)?.unwrap();
                                        let own = analysis.access(ordinal, budget)?.unwrap();
                                        assert_eq!(address.operation(), own.operation());
                                        assert_eq!(own.source().semantic_block(), 3);
                                        assert_eq!(own.source().semantic_statement(), Some(1 + ordinal as u32));
                                        assert_eq!(own.source().semantic_access_ordinal(), 0);
                                        assert_ne!(previous_store, Some(address.operation()));
                                        previous_store = Some(address.operation());
                                        let body = relation.output().module().functions
                                            [address.gep().block.function.0 as usize].body.as_ref().unwrap();
                                        let gep = &body.blocks[address.gep().block.block as usize]
                                            .operations[address.gep().operation as usize];
                                        let Op::GetElementPointer { base, offset } = gep.kind else {
                                            panic!("own physical GEP")
                                        };
                                        assert_eq!(address.offset_use(), Use::OperationOperand {
                                            operation: address.gep(), operand: 1,
                                        });
                                        let Def::Result { operation: selected_at, result: 0 } =
                                            address.offset_definition() else { panic!("actual selected definition") };
                                        assert_eq!(selected_at.block.function, address.gep().block.function);
                                        let selected = &body.blocks[selected_at.block.block as usize]
                                            .operations[selected_at.operation as usize];
                                        assert_eq!(selected.results[0].id, offset);
                                        let (condition, raw) =
                                            identity_getter_shape_tests::selected_offset_shape(body, offset);
                                        assert_ne!(offset, raw);
                                        if let Some(previous) = shared_offset {
                                            assert_eq!(previous, address.offset_definition());
                                        }
                                        shared_offset = Some(address.offset_definition());
                                        if let Some(previous) = shared_condition {
                                            assert_eq!(previous, condition);
                                        }
                                        shared_condition = Some(condition);
                                        let data = body.blocks.iter().flat_map(|block| &block.operations)
                                            .find(|operation| operation.results.iter().any(|value| value.id == base))
                                            .unwrap();
                                        assert!(matches!(data.kind, Op::SliceData { slice } if slice == body.parameters[0]));
                                        let store = &body.blocks[address.operation().block.block as usize]
                                            .operations[address.operation().operation as usize];
                                        assert!(matches!(store.kind, Op::Store { pointer, .. } if pointer == gep.results[0].id));
                                    }
                                    let condition = shared_condition.unwrap();
                                    let body = relation.output().module().functions[0].body.as_ref().unwrap();
                                    let mut selections = 0;
                                    for block in &body.blocks {
                                        let (some, none) = match block.terminator.as_ref().unwrap() {
                                            Term::ConditionalBranch { condition: actual, then_target, else_target, .. } => {
                                                assert_eq!(*actual, condition);
                                                (*then_target, *else_target)
                                            }
                                            Term::Switch { selector, cases, default_target, .. } => {
                                                assert_eq!(cases.iter().map(|case| case.value).collect::<Vec<_>>(), vec![0, 1]);
                                                let cast = body.blocks.iter().flat_map(|block| &block.operations).find(|operation|
                                                    operation.results.iter().any(|value| value.id == *selector)).unwrap();
                                                assert!(matches!(cast.kind, Op::Cast { kind: CastKind::ZeroExtend, value, .. }
                                                    if value == condition));
                                                assert_eq!(identity_getter_shape_tests::branch_path_shape(body, *default_target), (0, true));
                                                (cases[1].target, cases[0].target)
                                            }
                                            _ => continue,
                                        };
                                        selections += 1;
                                        assert_eq!(identity_getter_shape_tests::branch_path_shape(body, some), (stores, false));
                                        assert_eq!(identity_getter_shape_tests::branch_path_shape(body, none), (0, false));
                                    }
                                    assert_eq!(selections, 1);
                                    let live = budget.storage();
                                    relation.check_borrowed_ranked_addresses_v1(original, 0, claims, budget)?;
                                    assert_eq!(budget.storage(), live);
                                    r2_completed = true;
                                    Ok(())
                                })?;
                                assert_eq!(budget.storage(), held);
                                proof.require_original_v1(original, budget)
                            })?;
                            assert_eq!(budget.storage(), floor);
                            Ok(())
                        }).expect("real source/N, full Expression/R1, D/P/R2 composition without formal discharge");
                        assert!(source_entered && r2_completed);
                    }
                }
            }
        }

        #[test]
        fn selected_zero_hostile_outputs_stop_at_the_checked_transition_not_address_seals() {
            use fe2o3_kernel_analysis::{
                CanonicalKirInventoryV1 as Inventory,
                CanonicalKirTransitionErrorV1 as TransitionError,
                check_canonical_kir_transition_v1 as check,
            };
            use fe2o3_kernel_ir::{
                ComparePredicate, Constant, Operation, OperationKind as Op, Terminator as Term,
                Type, ValueDef, ValueId,
            };
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for mode in [false, true] {
                    let ssa = identity_getter_shape_tests::source_ssa_options(29, mode, 2);
                    let mut inspected = false;
                    let mut unchanged_view_entered = false;
                    with_admitted_source_inputs_and_checked_fixture(
                        ssa,
                        profile,
                        || {
                            [ranked_root_input_1d(
                                if mode { "other_identity_entry" } else { A_NAME },
                                if mode { 246 } else { 247 },
                                64,
                            )]
                        },
                        |input, checked, budget| {
                            let floor = budget.storage();
                            let (input_inventory, input_storage) =
                                Inventory::derive(input, budget).unwrap();
                            budget
                                .reserve_storage(input_storage.retained_storage())
                                .unwrap();
                            let (baseline, baseline_storage) =
                                Inventory::derive(checked.owner(), budget).unwrap();
                            budget
                                .reserve_storage(baseline_storage.retained_storage())
                                .unwrap();
                            let (accepted, receipt) = check(
                                &input_inventory,
                                &baseline,
                                checked.occurrences().candidate(),
                                budget,
                            )
                            .unwrap();
                            budget.reserve_storage(receipt.retained_storage()).unwrap();
                            assert!(!accepted.grants_authority());
                            drop(accepted);
                            budget.release_storage(receipt.retained_storage()).unwrap();
                            drop(baseline);
                            budget
                                .release_storage(baseline_storage.retained_storage())
                                .unwrap();
                            let held = budget.storage();
                            for case in 0..5 {
                                // Bounded fixture-owned proposals are verified as graphs,
                                // never adopted as checked O or fabricated D/P/R2 owners.
                                let mut proposed = checked.owner().module().clone();
                                let body = proposed.functions[0].body.as_mut().unwrap();
                                assert!(
                                    body.blocks.iter().all(|block| block.parameters.is_empty())
                                );
                                let coordinates = body
                                    .blocks
                                    .iter()
                                    .enumerate()
                                    .flat_map(|(b, block)| {
                                        block.operations.iter().enumerate().filter_map(
                                            move |(o, operation)| {
                                                matches!(operation.kind, Op::Select { .. })
                                                    .then_some((b, o))
                                            },
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                assert_eq!(coordinates.len(), 1);
                                let (selected_block, selected_operation) = coordinates[0];
                                let selected =
                                    &body.blocks[selected_block].operations[selected_operation];
                                let Op::Select {
                                    condition,
                                    true_value: raw,
                                    false_value: zero,
                                } = selected.kind
                                else {
                                    unreachable!()
                                };
                                let selected_value = selected.results[0].id;
                                let mut changed = 0;
                                let expected = match case {
                                    0 => {
                                        for block in &mut body.blocks {
                                            match block.terminator.as_mut().unwrap() {
                                                Term::Switch { cases, .. } => {
                                                    assert_eq!(
                                                        cases
                                                            .iter()
                                                            .map(|case| case.value)
                                                            .collect::<Vec<_>>(),
                                                        vec![0, 1]
                                                    );
                                                    let some = cases[1].target;
                                                    cases[1].target = cases[0].target;
                                                    cases[0].target = some;
                                                    changed += 1;
                                                }
                                                Term::ConditionalBranch {
                                                    then_target,
                                                    else_target,
                                                    ..
                                                } => {
                                                    std::mem::swap(then_target, else_target);
                                                    changed += 1;
                                                }
                                                _ => {}
                                            }
                                        }
                                        TransitionError::Rule("outgoing edge origin or destination")
                                    }
                                    1 => {
                                        body.blocks[selected_block].operations
                                            [selected_operation]
                                            .kind = Op::Select {
                                            condition,
                                            true_value: zero,
                                            false_value: raw,
                                        };
                                        changed = 1;
                                        TransitionError::Rule(
                                            "final operand has no exact descendant",
                                        )
                                    }
                                    2 => {
                                        for operation in body
                                            .blocks
                                            .iter_mut()
                                            .flat_map(|block| &mut block.operations)
                                        {
                                            if let Op::GetElementPointer { offset, .. } =
                                                &mut operation.kind
                                            {
                                                assert_eq!(*offset, selected_value);
                                                *offset = raw;
                                                changed += 1;
                                            }
                                        }
                                        TransitionError::Rule(
                                            "final operand has no exact descendant",
                                        )
                                    }
                                    3 => {
                                        let compare = body
                                            .blocks
                                            .iter_mut()
                                            .flat_map(|block| &mut block.operations)
                                            .find(|operation| {
                                                operation
                                                    .results
                                                    .iter()
                                                    .any(|value| value.id == condition)
                                            })
                                            .unwrap();
                                        let Op::Compare { predicate, .. } = &mut compare.kind
                                        else {
                                            panic!("own getter comparison")
                                        };
                                        assert_eq!(*predicate, ComparePredicate::LessThan);
                                        *predicate = ComparePredicate::Equal;
                                        changed = 1;
                                        TransitionError::Rule("retained operation payload")
                                    }
                                    4 => {
                                        let next = body
                                            .blocks
                                            .iter()
                                            .flat_map(|block| &block.operations)
                                            .flat_map(|operation| &operation.results)
                                            .map(|value| value.id.0)
                                            .chain(body.parameters.iter().map(|value| value.0))
                                            .max()
                                            .unwrap()
                                            .checked_add(1)
                                            .unwrap();
                                        let foreign = ValueId(next);
                                        body.blocks[selected_block].operations
                                            [selected_operation]
                                            .kind = Op::Select {
                                            condition: foreign,
                                            true_value: raw,
                                            false_value: zero,
                                        };
                                        body.blocks[selected_block].operations.insert(
                                            selected_operation,
                                            Operation::effect_free(
                                                ValueDef::new(foreign, Type::BOOL),
                                                Op::Constant(Constant::Bool(true)),
                                            ),
                                        );
                                        changed = 1;
                                        // Adding a foreign predicate already violates the
                                        // unchanged actual policy3 occurrence roster.
                                        TransitionError::IncompleteRows
                                    }
                                    _ => unreachable!(),
                                };
                                assert_eq!(changed, 1, "one intended mutation, case {case}");
                                let (proposed, storage) =
                                    Owner::from_module_ref_with_verification_budget_v12(
                                        &proposed, budget,
                                    )
                                    .expect("hostile output remains well-typed verified KIR");
                                budget.reserve_storage(storage.retained_storage()).unwrap();
                                let (inventory, inventory_storage) =
                                    Inventory::derive(&proposed, budget).unwrap();
                                budget
                                    .reserve_storage(inventory_storage.retained_storage())
                                    .unwrap();
                                if case == 4 {
                                    assert_eq!(
                                        inventory.operations().len(),
                                        checked.occurrences().candidate().operations.len() + 1
                                    );
                                }
                                let query_floor = budget.storage();
                                let result = check(
                                    &input_inventory,
                                    &inventory,
                                    checked.occurrences().candidate(),
                                    budget,
                                );
                                assert!(
                                    matches!(result, Err(actual) if actual == expected),
                                    "case {case}"
                                );
                                assert_eq!(budget.storage(), query_floor);
                                drop(inventory);
                                budget
                                    .release_storage(inventory_storage.retained_storage())
                                    .unwrap();
                                drop(proposed);
                                budget.release_storage(storage.retained_storage()).unwrap();
                                assert_eq!(budget.storage(), held);
                            }
                            drop(input_inventory);
                            budget
                                .release_storage(input_storage.retained_storage())
                                .unwrap();
                            assert_eq!(budget.storage(), floor);
                            inspected = true;
                        },
                        |_, _, _, _| {
                            unchanged_view_entered = true;
                            Ok(())
                        },
                    )
                    .expect("only unchanged policy3 O enters the source/output view");
                    assert!(inspected && unchanged_view_entered);
                }
            }
        }

        #[test]
        fn full_identity_r2_joins_actual_own_stores_with_same_n_r1_on_both_profiles() {
            use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Def, OperationKind as Op};
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for mode in [false, true] {
                    for stores in [1, 2] {
                        let mut joined = false;
                        with_join(profile, mode, stores, |analysis, original, claims, budget| {
                            assert_eq!(claims.arguments.len(), 2);
                            assert_eq!(claims.arguments.iter().filter(|row| row.component == Component::Scalar).count(), 1);
                            assert_eq!(claims.arguments.iter().filter(|row| row.component == Component::SliceLength).count(), 1);
                            let floor = budget.storage();
                            let before = budget.work();
                            analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                                assert_eq!(relation.global_access_count(), stores);
                                assert_eq!(relation.private_access_count(), 0);
                                let mut previous = None;
                                for ordinal in 0..stores {
                                    let address = relation.access(ordinal, budget)?.unwrap();
                                    let ordinary = analysis.access(ordinal, budget)?.unwrap();
                                    assert_eq!(address.operation(), ordinary.operation());
                                    assert_ne!(previous, Some(address.operation()));
                                    previous = Some(address.operation());
                                    assert_eq!(address.element_bytes(), 4);
                                    assert_eq!(address.pointer_definition(), Def::Result { operation: address.gep(), result: 0 });
                                    let body = relation.output().module().functions[address.operation().block.function.0 as usize].body.as_ref().unwrap();
                                    let store = &body.blocks[address.operation().block.block as usize].operations[address.operation().operation as usize];
                                    let gep = &body.blocks[address.gep().block.block as usize].operations[address.gep().operation as usize];
                                    assert!(matches!(store.kind, Op::Store { pointer, .. } if pointer == gep.results[0].id));
                                    assert!(matches!(gep.kind, Op::GetElementPointer { .. }));
                                }
                                assert!(relation.access(stores, budget)?.is_none());
                                for _ in 0..2 {
                                    let live = budget.storage();
                                    relation.check_borrowed_ranked_addresses_v1(original, 0, claims, budget)?;
                                    assert_eq!(budget.storage(), live);
                                }
                                joined = true;
                                Ok(())
                            })?;
                            assert_eq!(budget.storage(), floor);
                            assert!(budget.work() > before);
                            Ok(())
                        }).expect("full identity R2 must pass after real full Expression, R1, D and P");
                        assert!(joined);
                    }
                }
            }
        }

        #[test]
        fn full_identity_r2_rejects_independent_witness_extent_and_duplicate_claims() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_join(profile, true, 2, |analysis, original, claims, budget| {
                    analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                        let index = claims.arguments.iter().position(|row| row.component == Component::Scalar).unwrap();
                        let extent = claims.arguments.iter().position(|row| row.component == Component::SliceLength).unwrap();
                        assert_ne!(claims.arguments[index].source_local, claims.arguments[extent].source_local);
                        let semantic = original.materialized().semantic_ssa().source_semantic();
                        let option = semantic.functions().iter().flat_map(|function| function.blocks()).find_map(|block| {
                            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { return None; };
                            matches!(semantic.callables().get(call.callee().index() as usize), Some(
                                SemanticCallableDeclV1::CompilerIntrinsic {
                                    operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }, ..
                                }
                            )).then(|| call.destination().unwrap().place().local())
                        }).unwrap();
                        assert_ne!(option, claims.arguments[index].source_local);
                        let floor = budget.storage();
                        for case in 0..10 {
                            let mut changed = claims_copy(claims);
                            let expected = match case {
                                0 => { changed.arguments[index].source_local = claims.arguments[extent].source_local; "full address leaf source mapping differs" }
                                1 => { changed.arguments[extent].source_local = claims.arguments[index].source_local; "full address leaf source mapping differs" }
                                2 => { changed.arguments[index].component = Component::SliceLength; "full address leaf source mapping differs" }
                                3 => { changed.arguments[extent].component = Component::Scalar; "full address leaf source mapping differs" }
                                4 => { changed.arguments.remove(index); "full address source claim absent" }
                                5 => { changed.arguments.remove(extent); "full address source claim absent" }
                                6 => { changed.arguments.push(changed.arguments[index]); "duplicate global ranked correlation key" }
                                7 => { let mut conflict = changed.arguments[index]; conflict.source_local = claims.arguments[extent].source_local; changed.arguments.push(conflict); "duplicate global ranked correlation key" }
                                8 => { changed.arguments[index].source_local = option; "full address leaf source mapping differs" }
                                9 => { let value = changed.arguments[index].ranked_value; changed.arguments[index].ranked_value = changed.arguments[extent].ranked_value; changed.arguments[extent].ranked_value = value; "full address leaf source mapping differs" }
                                _ => unreachable!(),
                            };
                            let before = budget.work();
                            assert!(matches!(relation.check_borrowed_ranked_addresses_v1(original, 0, &changed, budget),
                                Err(ProductionSourceOutputErrorV1::Invalid(actual)) if actual == expected), "case {case}");
                            assert!(budget.work() > before);
                            assert_eq!(budget.storage(), floor);
                        }
                        relation.check_borrowed_ranked_addresses_v1(original, 0, claims, budget)
                    })
                }).unwrap();
            }
        }

        #[test]
        fn full_identity_r2_rejects_root_and_foreign_ledger_then_reenters_same_ledger() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_join(profile, false, 1, |analysis, original, claims, budget| {
                    analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                        let floor = budget.storage();
                        assert!(matches!(relation.check_borrowed_ranked_addresses_v1(original, 1, claims, budget),
                            Err(ProductionSourceOutputErrorV1::Invalid("full address root absent"))));
                        let before = budget.work();
                        let mut other_work = Work::new(LIMIT);
                        let mut other = Budget::new(&mut other_work, STORAGE_LIMIT);
                        other.reserve_storage(floor).unwrap();
                        assert!(matches!(relation.check_borrowed_ranked_addresses_v1(original, 0, claims, &mut other),
                            Err(ProductionSourceOutputErrorV1::Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting))));
                        assert_eq!(other.storage(), floor);
                        assert_eq!(budget.work(), before);
                        assert_eq!(budget.storage(), floor);
                        relation.check_borrowed_ranked_addresses_v1(original, 0, claims, budget)
                    })
                }).unwrap();
            }
        }

        #[test]
        fn full_identity_r2_rejects_distinct_original_n_with_equal_fixture_numbers() {
            with_join(Profile::Gfx942, false, 1, |_, foreign, _, _| {
                let mut checked = false;
                with_join(
                    Profile::Gfx942,
                    false,
                    1,
                    |analysis, original, claims, budget| {
                        assert!(!std::ptr::eq(
                            foreign.materialized(),
                            original.materialized()
                        ));
                        analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                            let floor = budget.storage();
                            assert!(matches!(
                                relation
                                    .check_borrowed_ranked_addresses_v1(foreign, 0, claims, budget),
                                Err(ProductionSourceOutputErrorV1::Invalid(
                                    "full address original owner differs"
                                ))
                            ));
                            assert_eq!(budget.storage(), floor);
                            relation
                                .check_borrowed_ranked_addresses_v1(original, 0, claims, budget)?;
                            checked = true;
                            Ok(())
                        })
                    },
                )
                .unwrap();
                assert!(checked);
                Ok(())
            })
            .unwrap();
        }

        #[test]
        fn full_identity_r2_callback_error_and_exact_panic_preserve_floor_and_reentry() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_join(profile, true, 2, |analysis, original, claims, budget| {
                    let floor = budget.storage();
                    let before = budget.work();
                    let mut error_entered = false;
                    let error = analysis.with_physical_address_relation_v1::<()>(
                        budget,
                        |relation, budget| {
                            relation
                                .check_borrowed_ranked_addresses_v1(original, 0, claims, budget)?;
                            error_entered = true;
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "identity R2 callback marker",
                            ))
                        },
                    );
                    assert!(error_entered);
                    assert!(matches!(
                        error,
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "identity R2 callback marker"
                        ))
                    ));
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                    let mut panic_entered = false;
                    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        analysis.with_physical_address_relation_v1::<()>(
                            budget,
                            |relation, budget| {
                                relation.check_borrowed_ranked_addresses_v1(
                                    original, 0, claims, budget,
                                )?;
                                panic_entered = true;
                                std::panic::panic_any(193u32)
                            },
                        )
                    }))
                    .expect_err("the exact post-R2 callback panic must resume");
                    assert!(panic_entered);
                    assert_eq!(panic.downcast_ref::<u32>(), Some(&193));
                    assert_eq!(budget.storage(), floor);
                    analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                        relation.check_borrowed_ranked_addresses_v1(original, 0, claims, budget)
                    })?;
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                })
                .unwrap();
            }
        }

        #[test]
        fn hostile_full_access_sites_are_rejected_before_borrowed_r1_callback() {
            for case in 0..3 {
                let ssa = identity_getter_shape_tests::source_ssa_options(17, false, 1);
                with_admitted_source_fixture(
                    ssa,
                    false,
                    Profile::Gfx942,
                    |source, view, inputs, budget| {
                        full_expression_recorder_tests::with_full_roots(
                            source,
                            view,
                            inputs,
                            budget,
                            |mut full, _, budget| {
                                let access = full.access_sources[0];
                                full.access_sources[0] = ProductionRankedAccessSourceV1::new(
                                    if case == 0 {
                                        u32::MAX
                                    } else {
                                        access.semantic_block()
                                    },
                                    if case == 1 {
                                        Some(u32::MAX)
                                    } else {
                                        access.semantic_statement()
                                    },
                                    if case == 2 {
                                        1
                                    } else {
                                        access.semantic_access_ordinal()
                                    },
                                    access.ranked_block(),
                                    access.ranked_operation(),
                                );
                                let floor = budget.storage();
                                let mut entered = false;
                                let result = with_authenticated_borrowed_ranked_source_roster_v1(
                                    source,
                                    vec![full].into_boxed_slice(),
                                    budget,
                                    |_, _, _| {
                                        entered = true;
                                        Ok(())
                                    },
                                );
                                assert!(
                                    !entered,
                                    "hostile source site must not acquire R1, case {case}"
                                );
                                assert!(
                                    matches!(
                                        result,
                                        Err(ProductionRankedVerificationErrorV1::Custody(_))
                                    ),
                                    "case {case}: {result:?}"
                                );
                                assert_eq!(budget.storage(), floor);
                                Ok(())
                            },
                        )
                    },
                )
                .unwrap();
            }
        }

        #[test]
        fn hostile_full_view_width_requires_exact_r1_or_r2_refusal() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let ssa = identity_getter_shape_tests::source_ssa_options(17, false, 1);
                with_admitted_source_fixture(ssa, false, profile, |source, view, inputs, budget| {
                    full_expression_recorder_tests::with_full_roots_and_references(
                        source, view, inputs, budget, |mut full, recorder, references, budget| {
                            let candidate = Candidate {
                                selected_root: full.semantic_root, selected_function: full.semantic_root,
                                lowering: &full.lowering, access_sources: &full.access_sources,
                                executable_effect_sources: &full.executable_effect_sources,
                                control: recorder.candidate(),
                            };
                            full.lowering = physical_address_width_mutation_v1(&candidate);
                            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                            let floor = budget.storage();
                            let mut r1_entered = false;
                            let mut r2_rejected = false;
                            let result = with_authenticated_borrowed_ranked_source_roster_v1(
                                source, vec![full].into_boxed_slice(), budget, |original, verification, budget| {
                                    r1_entered = true;
                                    assert!(!verification.roots()[0].verification().has_authenticated_functional_verification());
                                    assert!(verification.roots()[0].verification().aggregate_verus_execution().is_none());
                                    Ok(with_ranked_root_preparation_v1(&projection, inputs, references, |effects, partition| {
                                        checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                                            with_prepared_canonical_memory_session_v1(&projection, inputs, effects, partition, session, |analyses, budget| {
                                                assert_eq!(analyses.len(), 1);
                                                analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                                                    let live = budget.storage();
                                                    assert!(matches!(relation.check_borrowed_ranked_addresses_v1(original, 0, recorder.candidate(), budget),
                                                        Err(ProductionSourceOutputErrorV1::Invalid("physical address requires one exact scalar slice extent and width"))));
                                                    assert_eq!(budget.storage(), live);
                                                    r2_rejected = true;
                                                    Ok(())
                                                })
                                            })
                                        })
                                    }))
                                },
                            );
                            match result {
                                Err(ProductionRankedVerificationErrorV1::Custody(
                                    fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                                        detail: "ranked projection receipt contains a rejected mandatory kernel check", ..
                                    },
                                )) => {
                                    assert!(!r1_entered && !r2_rejected);
                                    eprintln!("hostile full width: exact R1 mandatory-check refusal; no R2 claim");
                                }
                                Ok(result) => {
                                    result?;
                                    assert!(r1_entered && r2_rejected);
                                    eprintln!("hostile full width: exact R2 width refusal after real R1/D/P");
                                }
                                Err(other) => panic!("unexpected full-width prerequisite refusal: {other}"),
                            }
                            assert_eq!(budget.storage(), floor);
                            Ok(())
                        },
                    )
                }).unwrap();
            }
        }
    }

    mod empty_default_coverage_tests {
        use super::*;
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        use fe2o3_lower_mir_kernel::ProductionSourceOutputBlockCoverageV1 as Coverage;
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticBasicBlockV1, SemanticCallDestinationV1, SemanticControlFlowEdgeV1,
            SemanticDirectCallV1, SemanticStatementV1, SemanticSwitchTargetV1,
            SemanticSwitchTargetsV1, SemanticTerminatorV1, SemanticUnwindActionV1,
        };

        const MISMATCH: &str = "checked all-live source CFG and sealed block disposition disagree";

        fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
            )
        }

        // Copies of genuinely queried records feed only this private numeric
        // component. Its one-unit query is not the production query's tariff,
        // and this adapter cannot construct a public checked owner or proof.
        struct Facts<'a, 'w> {
            budget: &'a mut Budget<'w>,
            coverage: &'a [Coverage],
            events: Vec<(&'static str, usize)>,
            reservations: Vec<usize>,
            scopes: usize,
            floor: usize,
            panic_after_allocation: bool,
        }

        impl<'a, 'w> Facts<'a, 'w> {
            fn new(budget: &'a mut Budget<'w>, coverage: &'a [Coverage]) -> Self {
                let floor = budget.storage();
                Self {
                    budget,
                    coverage,
                    events: Vec::new(),
                    reservations: Vec::new(),
                    scopes: 0,
                    floor,
                    panic_after_allocation: false,
                }
            }
        }

        impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
            fn checked_control_enabled_v1(&self) -> bool {
                true
            }
            fn with_checked_control_scope_v1<T>(
                &mut self,
                body: impl FnOnce(&mut Self) -> Result<T, ProductionRankedProjectionErrorV1>,
            ) -> Result<T, ProductionRankedProjectionErrorV1> {
                self.scopes += 1;
                let floor = self.budget.storage();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
                let retained = self
                    .budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or_else(|| resource(Resource::Accounting))?;
                self.budget.release_storage(retained).map_err(resource)?;
                match result {
                    Ok(result) => result,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
            fn charge_private_array_work(
                &mut self,
                amount: usize,
            ) -> Result<(), ProductionRankedProjectionErrorV1> {
                self.events.push(("work", amount));
                if self.panic_after_allocation && self.budget.storage() > self.floor {
                    panic!("empty default initialized allocation panic");
                }
                self.budget.charge_work(amount).map_err(resource)
            }
            fn reserve_checked_control_storage_v1(
                &mut self,
                bytes: usize,
            ) -> Result<(), ProductionRankedProjectionErrorV1> {
                self.reservations.push(bytes);
                self.budget.reserve_storage(bytes).map_err(resource)
            }
            fn checked_block_coverage_v1(
                &mut self,
                block: usize,
            ) -> Result<Coverage, ProductionRankedProjectionErrorV1> {
                self.events.push(("query", block));
                self.budget.charge_work(1).map_err(resource)?;
                Ok(self.coverage[block])
            }
            fn private_array_access(
                &mut self,
                _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
                _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
            ) -> Result<bool, ProductionRankedProjectionErrorV1> {
                panic!("numeric coverage component");
            }
            fn private_array_constant_index(
                &mut self,
                _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
                _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
            ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
                panic!("numeric coverage component");
            }
            fn is_materialized_block(
                &mut self,
                _: usize,
            ) -> Result<bool, ProductionRankedProjectionErrorV1> {
                panic!("numeric coverage component");
            }
            fn condition(
                &mut self,
                _: usize,
                _: bool,
                _: SemanticBlockIdV1,
            ) -> Result<
                canonical_assertion_facts_v1::ProjectedAssertionConditionV1,
                ProductionRankedProjectionErrorV1,
            > {
                panic!("numeric coverage component");
            }
        }

        fn with_source_rows(body: impl FnOnce(&SemanticFunctionDeclV1, &[Coverage])) {
            let ssa = identity_getter_shape_tests::source_ssa_options(17, false, 1);
            with_admitted_source_fixture(ssa, false, Profile::Gfx942, |source, view, _, budget| {
                let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                let root = projection.source_launch().roots()[0].selected_root();
                let semantic = source.semantic_ssa().source_semantic();
                let selected = semantic.select_kernel_body_for_root_v1(root).unwrap();
                let function = &semantic.functions()[selected.body().index() as usize];
                let mut coverage = Vec::new();
                for index in 0..function.blocks().len() {
                    coverage.push(
                        view.block_coverage(
                            root,
                            selected.body(),
                            SemanticBlockIdV1::from_index(index as u32),
                            budget,
                        )
                        .unwrap(),
                    );
                }
                body(function, &coverage);
                Ok(())
            })
            .unwrap();
        }

        // These are descriptive component inputs, not a constructed full root.
        // Genuine full-Expression/R1 qualification remains in the six unchanged
        // recorder tests and the captured-source positive below.
        fn rows(
            function: &SemanticFunctionDeclV1,
        ) -> (
            Vec<ProjectedSemanticBlockV1>,
            Vec<ProjectedCfgTerminatorV1>,
            Vec<bool>,
            usize,
            usize,
        ) {
            let mut switch = None;
            let mut default = None;
            let terminators = function
                .blocks()
                .iter()
                .enumerate()
                .map(|(index, block)| match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => ProjectedCfgTerminatorV1::Branch(
                        call.destination().unwrap().edge().target().index() as usize,
                    ),
                    SemanticTerminatorKindV1::Goto(edge) => {
                        ProjectedCfgTerminatorV1::Branch(edge.target().index() as usize)
                    }
                    SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
                        assert!(switch.replace(index).is_none());
                        default = Some(targets.otherwise().target().index() as usize);
                        ProjectedCfgTerminatorV1::Predicate {
                            predicate: GuardPredicateV1 {
                                comparisons: vec![(
                                    ProductionRankedValueV1::Argument(0),
                                    ProductionRankedValueV1::Argument(1),
                                )],
                            },
                            true_block: targets
                                .values()
                                .iter()
                                .find(|row| row.value() == 1)
                                .unwrap()
                                .edge()
                                .target()
                                .index() as usize,
                            false_block: targets
                                .values()
                                .iter()
                                .find(|row| row.value() == 0)
                                .unwrap()
                                .edge()
                                .target()
                                .index() as usize,
                        }
                    }
                    SemanticTerminatorKindV1::Return => ProjectedCfgTerminatorV1::Return,
                    SemanticTerminatorKindV1::Unreachable => ProjectedCfgTerminatorV1::Trap,
                    _ => panic!("unexpected actual fixture source terminator"),
                })
                .collect::<Vec<_>>();
            let reachable =
                reachable_projected_blocks(function.entry().index() as usize, &terminators)
                    .unwrap();
            let projected = vec![ProjectedSemanticBlockV1 { items: Vec::new() }; terminators.len()];
            (
                projected,
                terminators,
                reachable,
                switch.unwrap(),
                default.unwrap(),
            )
        }

        fn replace_block(
            function: &SemanticFunctionDeclV1,
            index: usize,
            statements: Vec<SemanticStatementV1>,
            kind: SemanticTerminatorKindV1,
        ) -> SemanticFunctionDeclV1 {
            let mut blocks = function.blocks().to_vec();
            let old = &blocks[index];
            blocks[index] = SemanticBasicBlockV1::new(
                old.identity(),
                old.source(),
                statements,
                SemanticTerminatorV1::new(old.terminator().source(), kind),
            )
            .unwrap();
            with_blocks(function, blocks)
        }

        fn with_blocks(
            function: &SemanticFunctionDeclV1,
            blocks: Vec<SemanticBasicBlockV1>,
        ) -> SemanticFunctionDeclV1 {
            let mut changed = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function.abi().clone(),
                function.locals().to_vec(),
                function.entry(),
                blocks,
            )
            .unwrap();
            if let Some(entry) = function.kernel_entry() {
                changed = changed.with_kernel_entry(entry.clone());
            }
            changed
        }

        fn edge(role: SemanticEdgeRoleV1, target: usize) -> SemanticControlFlowEdgeV1 {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target as u32))
        }

        fn expect_mismatch(result: Result<(), ProductionRankedProjectionErrorV1>) {
            assert!(
                matches!(result, Err(ProductionRankedProjectionErrorV1::Incomplete(detail)) if detail == MISMATCH)
            );
        }

        #[test]
        fn empty_default_genuine_sealed_coverage_has_zero_operations_in_both_profiles() {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for collected_modes in [false, true] {
                    let ssa =
                        identity_getter_shape_tests::source_ssa_options(29, collected_modes, 1);
                    with_admitted_source_fixture(ssa, collected_modes, profile, |source, view, inputs, budget| {
                        let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                        let root = projection.source_launch().roots()[0].selected_root();
                        let semantic = source.semantic_ssa().source_semantic();
                        let selected = semantic.select_kernel_body_for_root_v1(root).unwrap();
                        let function = &semantic.functions()[selected.body().index() as usize];
                        let (_, _, reachable, _, default) = rows(function);
                        assert!(!reachable[default]);
                        let coverage = view.block_coverage(root, selected.body(), SemanticBlockIdV1::from_index(default as u32), budget).unwrap();
                        assert!(matches!(coverage.disposition(), fe2o3_lower_mir_kernel::ProductionSourceOutputBlockV1::Materialized { executable: true, .. }));
                        assert_eq!(coverage.source_statements(), 0);
                        assert_eq!(coverage.original_operations(), Some(0));
                        let floor = budget.storage();
                        let mut entered = false;
                        full_expression_recorder_tests::with_full_roots(source, view, inputs, budget, |full, recorder, _| {
                            entered = true;
                            assert_eq!(full.access_sources.len(), 1);
                            assert_eq!(recorder.candidate().arguments.len(), 2);
                            Ok(())
                        })?;
                        assert!(entered);
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    }).unwrap();
                }
            }
        }

        #[test]
        fn empty_default_actual_coverage_gate_all_matching_keeps_old_schedule() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, _, _, default) = rows(function);
                let reachable = vec![true; coverage.len()];
                assert!(matches!(
                    terminators[default],
                    ProjectedCfgTerminatorV1::Trap
                ));
                let mut work = Work::new(3 * coverage.len());
                let mut budget = Budget::new(&mut work, PREFIX);
                budget.reserve_storage(PREFIX).unwrap();
                {
                    let mut facts = Facts::new(&mut budget, coverage);
                    bounds_cfg_v1::verify_all_live_coverage(
                        function,
                        &projected,
                        &terminators,
                        &reachable,
                        &mut facts,
                    )
                    .unwrap();
                    let expected = (0..coverage.len())
                        .flat_map(|index| [("work", 2), ("query", index)])
                        .collect::<Vec<_>>();
                    assert_eq!(facts.events, expected);
                    assert_eq!(facts.scopes, 0);
                    assert!(facts.reservations.is_empty());
                    assert_eq!(facts.budget.work(), 3 * coverage.len());
                    assert_eq!(facts.budget.storage(), PREFIX);
                }
                budget.release_storage(PREFIX).unwrap();
            });
        }

        #[test]
        fn empty_default_component_exact_cost_one_roster_and_once_only_queries() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, _, default) = rows(function);
                let count = coverage.len();
                let edges = function
                    .blocks()
                    .iter()
                    .map(|block| block.terminator().kind().edge_count())
                    .sum::<usize>();
                let suffix = count - default;
                let expected = 3 * count + 18 + 13 * count + 4 * edges + 6 * suffix;
                let mut work = Work::new(expected);
                let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                budget.reserve_storage(PREFIX).unwrap();
                {
                    let mut facts = Facts::new(&mut budget, coverage);
                    bounds_cfg_v1::verify_all_live_coverage(
                        function,
                        &projected,
                        &terminators,
                        &reachable,
                        &mut facts,
                    )
                    .unwrap();
                    assert_eq!(
                        facts
                            .events
                            .iter()
                            .filter_map(|(kind, value)| (*kind == "query").then_some(*value))
                            .collect::<Vec<_>>(),
                        (0..count).collect::<Vec<_>>()
                    );
                    assert_eq!(facts.scopes, 1);
                    let minimum = std::mem::size_of::<Vec<bounds_cfg_v1::EmptyDefaultIncomingV1>>()
                        + count * std::mem::size_of::<bounds_cfg_v1::EmptyDefaultIncomingV1>();
                    assert_eq!(facts.reservations[0], minimum);
                    assert!(facts.reservations.iter().sum::<usize>() >= minimum);
                    assert_eq!(facts.budget.work(), expected);
                    assert_eq!(facts.budget.storage(), PREFIX);
                }
                budget.release_storage(PREFIX).unwrap();
            });
        }

        #[test]
        fn empty_default_component_shared_and_two_defaults_build_only_once() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, switch, default) = rows(function);
                let SemanticTerminatorKindV1::SwitchInt {
                    discriminant,
                    targets,
                } = function.blocks()[switch].terminator().kind()
                else {
                    unreachable!();
                };
                let second = function
                    .blocks()
                    .iter()
                    .position(|block| {
                        matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return)
                    })
                    .unwrap();
                for shared in [true, false] {
                    let mut blocks = function.blocks().to_vec();
                    let mut projected = projected.clone();
                    let mut terms = terminators.clone();
                    let mut reached = reachable.clone();
                    let mut reports = coverage.to_vec();
                    let second_default = if shared { default } else { blocks.len() };
                    if !shared {
                        // Deliberate numeric component duplication, never source
                        // admission or a claim about a second authenticated block.
                        blocks.push(blocks[default].clone());
                        projected.push(ProjectedSemanticBlockV1 { items: Vec::new() });
                        terms.push(ProjectedCfgTerminatorV1::Trap);
                        reached.push(false);
                        reports.push(coverage[default]);
                    }
                    let old = &blocks[second];
                    blocks[second] = SemanticBasicBlockV1::new(
                        old.identity(),
                        old.source(),
                        old.statements().to_vec(),
                        SemanticTerminatorV1::new(
                            old.terminator().source(),
                            SemanticTerminatorKindV1::SwitchInt {
                                discriminant: discriminant.clone(),
                                targets: SemanticSwitchTargetsV1::new(
                                    targets.values().to_vec(),
                                    edge(SemanticEdgeRoleV1::SwitchOtherwise, second_default),
                                )
                                .unwrap(),
                            },
                        ),
                    )
                    .unwrap();
                    terms[second] = terms[switch].clone();
                    assert!(reached[switch] && reached[second]);
                    let changed = with_blocks(function, blocks);
                    let count = reports.len();
                    let edges = changed
                        .blocks()
                        .iter()
                        .map(|block| block.terminator().kind().edge_count())
                        .sum::<usize>();
                    let expected = 3 * count + 18 + 13 * count + 4 * edges + 6 * (count - default);
                    let mut work = Work::new(expected);
                    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                    budget.reserve_storage(PREFIX).unwrap();
                    {
                        let mut facts = Facts::new(&mut budget, &reports);
                        bounds_cfg_v1::verify_all_live_coverage(
                            &changed, &projected, &terms, &reached, &mut facts,
                        )
                        .unwrap();
                        assert_eq!(facts.scopes, 1);
                        assert_eq!(
                            facts
                                .events
                                .iter()
                                .filter_map(|(kind, value)| (*kind == "query").then_some(*value))
                                .collect::<Vec<_>>(),
                            (0..count).collect::<Vec<_>>()
                        );
                        assert_eq!(facts.budget.work(), expected);
                        assert_eq!(facts.budget.storage(), PREFIX);
                    }
                    budget.release_storage(PREFIX).unwrap();
                }
            });
        }

        #[test]
        fn empty_default_component_non_candidates_keep_exact_refusal_without_allocation() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, _, default) = rows(function);
                for case in 0..6 {
                    let changed = match case {
                        0 => replace_block(
                            function,
                            default,
                            vec![SemanticStatementV1::new(
                                function.source(),
                                SemanticStatementKindV1::Nop,
                            )],
                            SemanticTerminatorKindV1::Unreachable,
                        ),
                        1 => replace_block(
                            function,
                            default,
                            Vec::new(),
                            SemanticTerminatorKindV1::Return,
                        ),
                        _ => function.clone(),
                    };
                    let mut projected = projected.clone();
                    let mut reached = reachable.clone();
                    let mut reports = coverage.to_vec();
                    if case == 2 {
                        reached[function.entry().index() as usize] = false;
                    }
                    if case == 3 {
                        let nonempty = reports
                            .iter()
                            .copied()
                            .find(|row| {
                                row.source_statements() != 0 && row.original_operations() != Some(0)
                            })
                            .unwrap();
                        reports[default] = nonempty;
                    }
                    if case == 4 {
                        projected[default].items.push(ProjectedBlockItemV1::Effect {
                            operation: ProductionRankedOperationV1::InvocationIndex {
                                result: ProductionRankedValueIdV1::new(0),
                                dimension: 0,
                                launch_extent: 0,
                            },
                            source: None,
                        });
                    }
                    if case == 5 {
                        reports[default] = reports
                            .iter()
                            .copied()
                            .find(|row| {
                                row.source_statements() == 0
                                    && row.original_operations().is_some_and(|count| count > 0)
                            })
                            .expect("actual source producer has operations without statements");
                    }
                    let first = if case == 2 {
                        function.entry().index() as usize
                    } else {
                        default
                    };
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, PREFIX);
                    budget.reserve_storage(PREFIX).unwrap();
                    {
                        let mut facts = Facts::new(&mut budget, &reports);
                        expect_mismatch(bounds_cfg_v1::verify_all_live_coverage(
                            &changed,
                            &projected,
                            &terminators,
                            &reached,
                            &mut facts,
                        ));
                        assert_eq!(facts.budget.work(), 3 * (first + 1) + 6);
                        assert_eq!(facts.scopes, 0);
                        assert!(facts.reservations.is_empty());
                    }
                    budget.release_storage(PREFIX).unwrap();
                }
            });
        }

        #[test]
        fn empty_default_component_requires_exact_reachable_predicate_and_edge_roles() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, switch, default) = rows(function);
                let SemanticTerminatorKindV1::SwitchInt {
                    discriminant,
                    targets,
                } = function.blocks()[switch].terminator().kind()
                else {
                    unreachable!();
                };
                for case in 0..8 {
                    let mut terms = terminators.clone();
                    let mut reached = reachable.clone();
                    let mut changed = function.clone();
                    match case {
                        0 => {
                            terms[switch] = ProjectedCfgTerminatorV1::AnalysisSplit {
                                first_block: 3,
                                second_block: 4,
                            }
                        }
                        1 => {
                            terms[switch] = ProjectedCfgTerminatorV1::AnalysisMultiSplit {
                                blocks: vec![3, 4, default],
                            }
                        }
                        2 => {
                            terms[switch] = ProjectedCfgTerminatorV1::ExactSwitch(
                                ProjectedDeterministicSwitchV1 {
                                    source_discriminant: discriminant.clone(),
                                    discriminant: ProductionRankedValueV1::Argument(0),
                                    targets: Vec::new(),
                                    otherwise: default,
                                    lane_uniform: false,
                                },
                            )
                        }
                        3 => {
                            if let ProjectedCfgTerminatorV1::Predicate {
                                true_block,
                                false_block,
                                ..
                            } = &mut terms[switch]
                            {
                                std::mem::swap(true_block, false_block);
                            }
                        }
                        4 => reached[switch] = false,
                        5..=7 => {
                            let values = targets
                                .values()
                                .iter()
                                .map(|target| {
                                    SemanticSwitchTargetV1::new(
                                        if case == 5 && target.value() == 1 {
                                            2
                                        } else {
                                            target.value()
                                        },
                                        if case == 6 && target.value() == 0 {
                                            edge(SemanticEdgeRoleV1::SwitchValue, default)
                                        } else {
                                            target.edge()
                                        },
                                    )
                                })
                                .collect();
                            let otherwise = if case == 7 {
                                edge(SemanticEdgeRoleV1::Goto, default)
                            } else {
                                targets.otherwise()
                            };
                            changed = replace_block(
                                function,
                                switch,
                                function.blocks()[switch].statements().to_vec(),
                                SemanticTerminatorKindV1::SwitchInt {
                                    discriminant: discriminant.clone(),
                                    targets: SemanticSwitchTargetsV1::new(values, otherwise)
                                        .unwrap(),
                                },
                            );
                            // Keep explicit-arm coordinates coherent in the alias
                            // case: role, not target inequality, must veto it.
                            if case == 6 {
                                if let ProjectedCfgTerminatorV1::Predicate { false_block, .. } =
                                    &mut terms[switch]
                                {
                                    *false_block = default;
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                    budget.reserve_storage(PREFIX).unwrap();
                    {
                        let mut facts = Facts::new(&mut budget, coverage);
                        let incoming = bounds_cfg_v1::empty_default_incoming(
                            &changed, &terms, &reached, &mut facts,
                        )
                        .unwrap();
                        assert!(!incoming[default].eligible_only(), "component {case}");
                        let actual =
                            std::mem::size_of::<Vec<bounds_cfg_v1::EmptyDefaultIncomingV1>>()
                                + incoming.capacity()
                                    * std::mem::size_of::<bounds_cfg_v1::EmptyDefaultIncomingV1>();
                        assert_eq!(facts.budget.storage(), PREFIX + actual);
                        assert_eq!(facts.reservations.iter().sum::<usize>(), actual);
                        drop(incoming);
                    }
                    budget.release_storage(budget.storage() - PREFIX).unwrap();
                    if case != 4 {
                        let mut facts = Facts::new(&mut budget, coverage);
                        expect_mismatch(bounds_cfg_v1::verify_all_live_coverage(
                            &changed, &projected, &terms, &reached, &mut facts,
                        ));
                        assert_eq!(facts.budget.storage(), PREFIX);
                    }
                    budget.release_storage(PREFIX).unwrap();
                }
            });
        }

        #[test]
        fn empty_default_component_other_incoming_including_imaginary_unwind_vetoes() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, switch, default) = rows(function);
                let source = switch - 1;
                let SemanticTerminatorKindV1::Call(call) =
                    function.blocks()[source].terminator().kind()
                else {
                    unreachable!();
                };
                for case in 0..4 {
                    let kind = match case {
                        0 => {
                            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, default))
                        }
                        1 => SemanticTerminatorKindV1::FalseEdge {
                            real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal, switch),
                            imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary, default),
                        },
                        2 | 3 => SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                                call.callee(),
                                call.arguments().to_vec(),
                                call.variadic_argument_abis().to_vec(),
                                if case == 2 {
                                    Some(SemanticCallDestinationV1::new(
                                        call.destination().unwrap().place().clone(),
                                        edge(SemanticEdgeRoleV1::CallReturn, default),
                                    ))
                                } else {
                                    call.destination().cloned()
                                },
                                if case == 3 {
                                    SemanticUnwindActionV1::Cleanup(edge(
                                        SemanticEdgeRoleV1::CallUnwind,
                                        default,
                                    ))
                                } else {
                                    call.unwind()
                                },
                            )
                            .unwrap(),
                        ),
                        _ => unreachable!(),
                    };
                    let changed = replace_block(
                        function,
                        source,
                        function.blocks()[source].statements().to_vec(),
                        kind,
                    );
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                    budget.reserve_storage(PREFIX).unwrap();
                    {
                        let mut facts = Facts::new(&mut budget, coverage);
                        expect_mismatch(bounds_cfg_v1::verify_all_live_coverage(
                            &changed,
                            &projected,
                            &terminators,
                            &reachable,
                            &mut facts,
                        ));
                        assert_eq!(facts.scopes, 1);
                        assert!(!facts.reservations.is_empty());
                        assert_eq!(facts.budget.storage(), PREFIX);
                    }
                    {
                        let mut unreachable_source = reachable.clone();
                        unreachable_source[source] = false;
                        let mut facts = Facts::new(&mut budget, coverage);
                        let incoming = bounds_cfg_v1::empty_default_incoming(
                            &changed,
                            &terminators,
                            &unreachable_source,
                            &mut facts,
                        )
                        .unwrap();
                        assert!(
                            !incoming[default].eligible_only(),
                            "unreachable incoming component {case}"
                        );
                        drop(incoming);
                    }
                    budget.release_storage(budget.storage() - PREFIX).unwrap();
                    budget.release_storage(PREFIX).unwrap();
                }
            });
        }

        #[test]
        fn empty_comparison_false_branch_is_not_an_elided_otherwise_default() {
            with_source_rows(|function, coverage| {
                let (projected, mut terminators, _, switch, default) = rows(function);
                let ProjectedCfgTerminatorV1::Predicate {
                    predicate,
                    true_block,
                    false_block,
                } = &mut terminators[switch]
                else {
                    unreachable!();
                };
                predicate.comparisons.clear();
                let (true_block, false_block) = (*true_block, *false_block);
                assert_ne!(false_block, default);
                // Inert component mutation: only the predicate's true path
                // returns; its explicit false arm is a separate empty trap.
                let changed = replace_block(
                    function,
                    true_block,
                    function.blocks()[true_block].statements().to_vec(),
                    SemanticTerminatorKindV1::Return,
                );
                let changed = replace_block(
                    &changed,
                    false_block,
                    Vec::new(),
                    SemanticTerminatorKindV1::Unreachable,
                );
                terminators[true_block] = ProjectedCfgTerminatorV1::Return;
                terminators[false_block] = ProjectedCfgTerminatorV1::Trap;
                let reachable =
                    reachable_projected_blocks(function.entry().index() as usize, &terminators)
                        .unwrap();
                assert!(reachable[switch] && reachable[true_block]);
                assert!(!reachable[false_block] && !reachable[default]);
                assert_eq!(coverage[false_block].source_statements(), 0);
                assert_eq!(coverage[false_block].original_operations(), Some(0));
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                budget.reserve_storage(PREFIX).unwrap();
                {
                    let mut facts = Facts::new(&mut budget, coverage);
                    facts
                        .with_checked_control_scope_v1(|facts| {
                            let incoming = bounds_cfg_v1::empty_default_incoming(
                                &changed,
                                &terminators,
                                &reachable,
                                facts,
                            )?;
                            assert!(incoming[default].eligible_only());
                            assert!(!incoming[false_block].eligible_only());
                            drop(incoming);
                            Ok(())
                        })
                        .unwrap();
                    expect_mismatch(bounds_cfg_v1::verify_all_live_coverage(
                        &changed,
                        &projected,
                        &terminators,
                        &reachable,
                        &mut facts,
                    ));
                    assert_eq!(facts.scopes, 2);
                    assert_eq!(
                        facts
                            .events
                            .iter()
                            .filter_map(|(kind, value)| (*kind == "query").then_some(*value))
                            .collect::<Vec<_>>(),
                        (0..=false_block).collect::<Vec<_>>()
                    );
                    assert_eq!(facts.budget.storage(), PREFIX);
                }
                budget.release_storage(PREFIX).unwrap();
            });
        }

        #[test]
        fn empty_default_component_denial_prefixes_restore_original_floor() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, _, default) = rows(function);
                let count = coverage.len();
                let prefix = 3 * (default + 1);
                let requested = std::mem::size_of::<Vec<bounds_cfg_v1::EmptyDefaultIncomingV1>>()
                    + count * std::mem::size_of::<bounds_cfg_v1::EmptyDefaultIncomingV1>();
                for (limit, accepted, attempted, scope, reserved) in [
                    (1, 0, 2, 0, false),
                    (prefix + 5, prefix, prefix + 6, 0, false),
                    (prefix + 6 + 11, prefix + 6, prefix + 6 + 12, 1, false),
                    (
                        prefix + 6 + 12 + count - 1,
                        prefix + 6 + 12,
                        prefix + 6 + 12 + count,
                        1,
                        true,
                    ),
                ] {
                    let mut work = Work::new(limit);
                    {
                        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                        budget.reserve_storage(PREFIX).unwrap();
                        {
                            let mut facts = Facts::new(&mut budget, coverage);
                            let result = bounds_cfg_v1::verify_all_live_coverage(
                                function,
                                &projected,
                                &terminators,
                                &reachable,
                                &mut facts,
                            );
                            assert!(
                                matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error)))) if error.actual() == attempted && error.limit() == limit)
                            );
                            assert_eq!(facts.budget.work(), accepted);
                            assert_eq!(facts.scopes, scope);
                            assert_eq!(!facts.reservations.is_empty(), reserved);
                            if reserved {
                                assert_eq!(facts.reservations[0], requested);
                            }
                            assert_eq!(facts.budget.storage(), PREFIX);
                        }
                        budget.release_storage(PREFIX).unwrap();
                    }
                    assert_eq!(work.failed_work(), Some(attempted));
                }
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, PREFIX + requested - 1);
                budget.reserve_storage(PREFIX).unwrap();
                {
                    let mut facts = Facts::new(&mut budget, coverage);
                    let result = bounds_cfg_v1::verify_all_live_coverage(
                        function,
                        &projected,
                        &terminators,
                        &reachable,
                        &mut facts,
                    );
                    assert!(
                        matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Storage(error)))) if error.actual() == PREFIX + requested && error.limit() == PREFIX + requested - 1)
                    );
                    assert_eq!(facts.budget.work(), prefix + 18);
                    assert_eq!(facts.reservations, [requested]);
                    assert_eq!(facts.budget.storage(), PREFIX);
                    assert_eq!(facts.budget.failed_storage(), Some(PREFIX + requested));
                }
                budget.release_storage(PREFIX).unwrap();
            });
        }

        #[test]
        fn empty_default_component_post_allocation_panic_then_reentry_keeps_floor() {
            with_source_rows(|function, coverage| {
                let (projected, terminators, reachable, _, _) = rows(function);
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
                budget.reserve_storage(PREFIX).unwrap();
                {
                    let mut facts = Facts::new(&mut budget, coverage);
                    facts.panic_after_allocation = true;
                    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        bounds_cfg_v1::verify_all_live_coverage(
                            function,
                            &projected,
                            &terminators,
                            &reachable,
                            &mut facts,
                        )
                    }))
                    .expect_err("actual post-allocation component charge must panic");
                    assert_eq!(
                        panic.downcast_ref::<&'static str>().copied(),
                        Some("empty default initialized allocation panic")
                    );
                    assert_eq!(facts.scopes, 1);
                    assert!(!facts.reservations.is_empty());
                    assert_eq!(facts.budget.storage(), PREFIX);
                    let before = facts.budget.work();
                    facts.panic_after_allocation = false;
                    bounds_cfg_v1::verify_all_live_coverage(
                        function,
                        &projected,
                        &terminators,
                        &reachable,
                        &mut facts,
                    )
                    .unwrap();
                    assert_eq!(facts.scopes, 2);
                    assert_eq!(facts.budget.storage(), PREFIX);
                    assert!(facts.budget.work() > before);
                }
                budget.release_storage(PREFIX).unwrap();
            });
        }
    }

    fn candidate(root: &CanonicalMemoryProjectedRootV1) -> Candidate<'_> {
        Candidate {
            selected_root: root.selected_root,
            selected_function: root.selected_function,
            lowering: &root.lowering,
            access_sources: &root.access_sources,
            executable_effect_sources: &root.executable_effect_sources,
            control: root.control.candidate(),
        }
    }

    fn complete(
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        root: &CanonicalMemoryProjectedRootV1,
        budget: &mut Budget<'_>,
        body: impl FnOnce(
            &Analysis<'_, '_, '_>,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        let candidates = [candidate(root)];
        view.with_conditional_memory_control_coverage_v1(&candidates, budget, |control, budget| {
            view.with_canonical_store_analysis_v1(
                &candidates,
                control,
                budget,
                |analyses, budget| {
                    assert_eq!(analyses.len(), 1);
                    body(&analyses[0], budget)
                },
            )
        })
    }

    #[test]
    fn identity_physical_joins_each_own_store_on_both_profiles_and_source_modes() {
        use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Def, OperationKind as Op};
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for mode in [false, true] {
                for (value, stores) in [(17, 1), (29, 2)] {
                    let mut entered = false;
                    with_fixture_stores(value, mode, profile, stores, |view, root, budget| {
                        let candidates = [candidate(root)];
                        let ranked = root.control.candidate().arguments.iter()
                            .find(|row| row.source_local.index() == 2).unwrap().ranked_value;
                        let floor = budget.storage();
                        view.with_conditional_memory_control_coverage_v1(
                            &candidates, budget, |control, budget| {
                                assert!(matches!(control.index_leaf(view, &candidates[0], ranked, budget),
                                    Err(ProductionSourceOutputErrorV1::Invalid("identity getter address join is not implemented"))));
                                assert!(matches!(control.index_value(view, &candidates[0], ranked, budget),
                                    Err(ProductionSourceOutputErrorV1::Invalid("identity getter address join is not implemented"))));
                                view.with_canonical_store_analysis_v1(&candidates, control, budget, |analyses, budget| {
                                    let analysis = &analyses[0];
                                    assert_eq!(analysis.access_count(), stores);
                                    let live = budget.storage();
                                    for _ in 0..2 {
                                        analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                                            entered = true;
                                            assert!(std::ptr::eq(relation.output(), view.output()));
                                            assert_eq!(relation.global_access_count(), stores);
                                            assert_eq!(relation.private_access_count(), 0);
                                            let mut previous = None;
                                            for ordinal in 0..stores {
                                                let ordinary = analysis.access(ordinal, budget)?.unwrap();
                                                let address = relation.access(ordinal, budget)?.unwrap();
                                                assert_eq!(address.operation(), ordinary.operation());
                                                assert_eq!(address.element_bytes(), 4);
                                                assert_eq!(address.alignment(), 4);
                                                assert_ne!(previous, Some(address.operation()));
                                                previous = Some(address.operation());
                                                let op = |coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1| {
                                                    &relation.output().module().functions[coordinate.block.function.0 as usize]
                                                        .body.as_ref().unwrap().blocks[coordinate.block.block as usize]
                                                        .operations[coordinate.operation as usize]
                                                };
                                                let Op::Store { pointer, .. } = op(address.operation()).kind else { panic!("own Store") };
                                                let gep = op(address.gep());
                                                assert_eq!(gep.results[0].id, pointer);
                                                assert_eq!(address.pointer_definition(), Def::Result { operation: address.gep(), result: 0 });
                                                let Op::GetElementPointer { base, offset } = gep.kind else { panic!("own GEP") };
                                                let Def::FunctionArgument { function, argument } = address.allocation() else { panic!("own Slice formal") };
                                                assert_eq!(function, address.operation().block.function);
                                                let body = relation.output().module().functions[function.0 as usize].body.as_ref().unwrap();
                                                let mut definitions = body.blocks.iter().flat_map(|block| &block.operations)
                                                    .filter(|row| row.results.iter().any(|result| result.id == base));
                                                let data = definitions.next().unwrap();
                                                assert!(definitions.next().is_none());
                                                let Op::SliceData { slice } = data.kind else { panic!("own SliceData") };
                                                assert_eq!(slice, body.parameters[argument as usize]);
                                                let Def::Result { operation: selected, result: 0 } = address.offset_definition() else { panic!("own selected coordinate") };
                                                assert_eq!(op(selected).results[0].id, offset);
                                                let (_, raw) = identity_getter_shape_tests::selected_offset_shape(body, offset);
                                                assert_ne!(raw, offset);
                                            }
                                            assert!(relation.access(stores, budget)?.is_none());
                                            Ok(())
                                        })?;
                                        assert_eq!(budget.storage(), live);
                                    }
                                    Ok(())
                                })
                            })?;
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    }).unwrap();
                    assert!(entered);
                }
            }
        }
    }

    #[test]
    fn identity_physical_balanced_error_panic_and_storage_denial_restore_live_floor() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(29, true, profile, |view, root, budget| {
                complete(view, root, budget, |analysis, budget| {
                    let floor = budget.storage();
                    let before = budget.work();
                    let mut entered = false;
                    let result = analysis.with_physical_address_relation_v1::<()>(
                        budget,
                        |relation, budget| {
                            entered = true;
                            assert!(relation.access(0, budget)?.is_some());
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "identity P callback marker",
                            ))
                        },
                    );
                    assert!(entered);
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "identity P callback marker"
                        ))
                    ));
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                    entered = false;
                    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        analysis.with_physical_address_relation_v1::<()>(
                            budget,
                            |relation, budget| {
                                assert!(relation.access(0, budget)?.is_some());
                                entered = true;
                                std::panic::panic_any("identity P panic marker")
                            },
                        )
                    }))
                    .expect_err("original callback panic must resume");
                    assert!(entered);
                    assert_eq!(
                        panic.downcast_ref::<&'static str>(),
                        Some(&"identity P panic marker")
                    );
                    assert_eq!(budget.storage(), floor);
                    let held = STORAGE_LIMIT - floor - 1;
                    budget.reserve_storage(held).unwrap();
                    let denied_floor = budget.storage();
                    entered = false;
                    let denied = analysis.with_physical_address_relation_v1(budget, |_, _| {
                        entered = true;
                        Ok(())
                    });
                    assert!(!entered);
                    assert!(matches!(
                        denied,
                        Err(ProductionSourceOutputErrorV1::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(
                                _
                            )
                        ))
                    ));
                    assert_eq!(budget.storage(), denied_floor);
                    let failure = budget.failed_storage();
                    assert!(failure.is_some());
                    budget.release_storage(held).unwrap();
                    analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                        assert!(relation.access(0, budget)?.is_some());
                        Ok(())
                    })?;
                    assert_eq!(budget.failed_storage(), failure);
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                })
            })
            .unwrap();
        }
    }

    #[test]
    fn identity_physical_rejects_distinct_ledger_and_missing_live_relation_storage() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(17, false, profile, |view, root, budget| {
                complete(view, root, budget, |analysis, budget| {
                    analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                        let live = budget.storage();
                        let mut foreign_work = Work::new(LIMIT);
                        let mut foreign = Budget::new(&mut foreign_work, STORAGE_LIMIT);
                        foreign.reserve_storage(live).unwrap();
                        assert!(matches!(relation.access(0, &mut foreign),
                            Err(ProductionSourceOutputErrorV1::Resource(
                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting))));
                        assert_eq!(foreign.storage(), live);
                        // Deliberately violate and restore the caller reservation contract;
                        // this does not replace the original ledger or any owner.
                        budget.release_storage(1).unwrap();
                        assert!(matches!(relation.access(0, budget),
                            Err(ProductionSourceOutputErrorV1::Resource(
                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting))));
                        budget.reserve_storage(1).unwrap();
                        assert!(relation.access(0, budget)?.is_some());
                        assert_eq!(budget.storage(), live);
                        Ok(())
                    })
                })
            }).unwrap();
        }
    }

    #[test]
    fn identity_physical_refuses_ranked_width_mismatch_after_completed_d() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(17, true, profile, |view, root, budget| {
                let original = candidate(root);
                let changed = physical_address_width_mutation_v1(&original);
                let candidates = [Candidate {
                    lowering: &changed,
                    ..original
                }];
                let floor = budget.storage();
                let mut entered_d = false;
                let mut entered_p = false;
                let result = view.with_conditional_memory_control_coverage_v1(
                    &candidates,
                    budget,
                    |control, budget| {
                        view.with_canonical_store_analysis_v1(
                            &candidates,
                            control,
                            budget,
                            |analyses, budget| {
                                entered_d = true;
                                analyses[0].with_physical_address_relation_v1(budget, |_, _| {
                                    entered_p = true;
                                    Ok(())
                                })
                            },
                        )
                    },
                );
                assert!(entered_d);
                assert!(!entered_p);
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "physical address requires one exact scalar slice extent and width"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn identity_physical_never_admits_a_duplicate_source_store_roster() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture_stores(29, true, profile, 2, |view, root, budget| {
                assert_eq!(root.access_sources.len(), 2);
                let saved = root.access_sources[1];
                root.access_sources[1] = root.access_sources[0];
                let floor = budget.storage();
                let mut entered = false;
                let refused = complete(view, root, budget, |analysis, budget| {
                    analysis.with_physical_address_relation_v1(budget, |_, _| {
                        entered = true;
                        Ok(())
                    })
                });
                assert!(!entered);
                // D or the complete Store partition may reject before P.
                assert!(refused.is_err());
                assert_eq!(budget.storage(), floor);
                root.access_sources[1] = saved;
                complete(view, root, budget, |analysis, budget| {
                    analysis.with_physical_address_relation_v1(budget, |relation, _| {
                        entered = true;
                        assert_eq!(relation.global_access_count(), 2);
                        Ok(())
                    })
                })?;
                assert!(entered);
                assert_eq!(budget.storage(), floor);
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn identity_physical_access_work_boundary_propagates_without_replacing_ledger() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for allowance in [22, 23] {
                let mut entered = false;
                let result = with_fixture(17, false, profile, |view, root, budget| {
                    let floor = budget.storage();
                    let result = complete(view, root, budget, |analysis, budget| {
                        analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                            entered = true;
                            let live = budget.storage();
                            // Existing access contract: live-analysis 19 + relation 2 +
                            // one binary-search step and one ordinal comparison.
                            budget
                                .charge_work(LIMIT - budget.work() - allowance)
                                .unwrap();
                            let before = budget.work();
                            let query = relation.access(0, budget);
                            if allowance == 23 {
                                assert!(query.as_ref().is_ok_and(|row| row.is_some()));
                                assert_eq!(budget.work() - before, 23);
                            } else {
                                assert!(matches!(query,
                                    Err(ProductionSourceOutputErrorV1::SourceOrigin(
                                        fe2o3_lower_mir_kernel::SemanticKirAssertOriginErrorV1::Resource(
                                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error))))
                                        if error.actual() == LIMIT + 1 && error.limit() == LIMIT));
                            }
                            assert_eq!(budget.storage(), live);
                            query.map(|_| ())
                        })
                    });
                    assert_eq!(budget.storage(), floor);
                    result
                });
                assert!(entered);
                // Completion still has its own paid liveness checks. No reset or
                // continuation success is inferred from the inner query result.
                if allowance == 22 {
                    assert!(result.is_err());
                }
            }
        }
    }
}

// Genuine source/N/B/O tests. Component-only arithmetic/accounting cases live
// in the lowerer child; no completed analysis is constructed by these tests.
fn with_physical_address_source_v1(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    policy3: bool,
    body: impl for<'scope, 'source, 'output> FnOnce(
        &fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output>,
        &mut Budget<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1>,
) {
    let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    if policy3 {
        with_actual_policy3_canonical_view_v1(source, profile, |checked, view, budget| {
            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
            with_ranked_root_preparation_v1(
                &projection,
                &inputs,
                &references,
                |effects, references| {
                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                        view,
                        budget,
                        |session| {
                            with_prepared_canonical_memory_session_v1(
                                &projection,
                                &inputs,
                                effects,
                                references,
                                session,
                                |roots, budget| {
                                    assert_eq!(roots.len(), 1);
                                    assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                                    body(&roots[0], budget)
                                },
                            )
                        },
                    )
                },
            )
            .unwrap();
        });
    } else {
        with_actual(source, profile, |bound, checked, budget| {
            with_projected_canonical_memory_analysis_v1(
                source,
                bound,
                checked,
                profile,
                &inputs,
                &references,
                budget,
                |roots, budget| {
                    assert_eq!(roots.len(), 1);
                    assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                    body(&roots[0], budget)
                },
            )
            .unwrap();
        });
    }
}

fn physical_address_two_root_source_v1() -> ProductionPreRankedKirOwnerV1 {
    let seed = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(253)),
        root.role(),
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(254)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(255)),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        root.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"address_second".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(248)),
        root.kernel_entry().unwrap().source_contract().clone(),
    ));
    let mut functions = source.functions().to_vec();
    let second_id = SemanticFunctionIdV1::from_index(functions.len() as u32);
    functions.push(second);
    assert_eq!(source.callables().len() + 1, functions.len());
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0), second_id],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    materialize_ranked_fixture_v1(
        ssa,
        &[
            ranked_root_input_1d(A_NAME, 247, 1),
            ranked_root_input_1d("address_second", 248, 1),
        ],
    )
    .unwrap()
}

#[test]
fn physical_address_two_roots_reuse_one_ledger_and_keep_failure_history() {
    let source = physical_address_two_root_source_v1();
    let inputs = [
        ranked_root_input_1d(A_NAME, 247, 1),
        ranked_root_input_1d("address_second", 248, 1),
    ];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            with_projected_canonical_memory_analysis_v1(
                &source,
                bound,
                checked,
                profile,
                &inputs,
                &references,
                budget,
                |roots, budget| {
                    assert_eq!(roots.len(), 2);
                    let floor = budget.storage();
                    let prefix = budget.work();
                    let mut first_operation = None;
                    roots[0].with_physical_address_relation_v1(budget, |relation, budget| {
                        first_operation = Some(relation.access(0, budget)?.unwrap().operation());
                        Ok(())
                    })?;
                    assert_eq!(budget.storage(), floor);
                    let after_first = budget.work();
                    assert!(after_first > prefix);
                    let refused =
                        roots[1].with_physical_address_relation_v1(budget, |_, budget| {
                            // A storage denial leaves work available for continuation.
                            budget
                                .reserve_storage(STORAGE_LIMIT)
                                .map_err(ProductionSourceOutputErrorV1::Resource)
                        });
                    assert!(refused.is_err());
                    let failed_storage = budget.failed_storage();
                    assert!(failed_storage.is_some());
                    assert_eq!(budget.storage(), floor);
                    let after_failure = budget.work();
                    assert!(after_failure > after_first);
                    roots[1].with_physical_address_relation_v1(budget, |relation, budget| {
                        let second = relation.access(0, budget)?.unwrap().operation();
                        assert_ne!(
                            first_operation.unwrap().block.function,
                            second.block.function
                        );
                        assert!(std::ptr::eq(relation.output(), checked.owner()));
                        Ok(())
                    })?;
                    assert!(budget.work() > after_failure);
                    assert_eq!(budget.failed_storage(), failed_storage);
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
            .unwrap();
        });
    }
}

#[test]
fn physical_address_actual_formal_index_and_tuple_slots_match_own_gep_on_both_targets() {
    use fe2o3_kernel_ir::{CanonicalKirUseCoordinateV1 as Use, OperationKind};
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            for field in 0..2 {
                for shape in [BodyShape::Straight, BodyShape::Join, BodyShape::Loop] {
                    let source = canonical_same_typed_tuple_store_source_v1(field, shape);
                    with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                        let floor = budget.storage();
                        let mut completed = false;
                        root.with_physical_address_relation_v1(budget, |relation, budget| {
                            completed = true;
                            assert!(std::ptr::eq(root.output(), relation.output()));
                            assert_eq!(
                                (
                                    relation.global_access_count(),
                                    relation.private_access_count()
                                ),
                                (1, 0)
                            );
                            let access = relation.access(0, budget)?.unwrap();
                            assert_eq!(access.access_ordinal(), 0);
                            assert_eq!(
                                access.operation(),
                                root.access(0, budget)?.unwrap().operation()
                            );
                            assert_eq!(access.element_bytes(), 4);
                            assert_eq!(access.alignment(), 4);
                            let coordinate = access.gep();
                            let function = &relation.output().module().functions
                                [coordinate.block.function.0 as usize];
                            let gep = &function.body.as_ref().unwrap().blocks
                                [coordinate.block.block as usize]
                                .operations[coordinate.operation as usize];
                            assert!(matches!(gep.kind, OperationKind::GetElementPointer { .. }));
                            assert_eq!(
                                access.offset_use(),
                                Use::OperationOperand {
                                    operation: coordinate,
                                    operand: 1
                                }
                            );
                            assert!(relation.access(1, budget)?.is_none());
                            // This is correspondence only. Dynamic formal extraction
                            // remains UnsupportedIndexExpression in the F tests.
                            Ok(())
                        })?;
                        assert!(completed);
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    });
                }
            }
        }
    }
}

#[test]
fn physical_address_actual_nonzero_literal_and_distinct_slice_extents_are_checked() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            for bits in [3, 17] {
                let (source, _) = conditional_literal_source_v1(bits, None, false);
                with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                    root.with_physical_address_relation_v1(budget, |relation, budget| {
                        assert_eq!(relation.global_access_count(), 1);
                        assert!(relation.access(0, budget)?.is_some());
                        Ok(())
                    })
                });
            }
            let source = conditional_literal_two_guards_v1();
            with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                root.with_physical_address_relation_v1(budget, |relation, budget| {
                    assert_eq!(relation.global_access_count(), 2);
                    let first = relation.access(0, budget)?.unwrap();
                    let second = relation.access(1, budget)?.unwrap();
                    assert_ne!(first.allocation(), second.allocation());
                    assert_ne!(first.offset_use(), second.offset_use());
                    assert_ne!(first.ranked_extent(), second.ranked_extent());
                    Ok(())
                })
            });
        }
    }
}

#[test]
fn physical_address_scope_error_panic_and_distinct_ledger_preserve_live_parent() {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                let floor = budget.storage();
                for outcome in 0..3 {
                    let before = budget.work();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        root.with_physical_address_relation_v1(budget, |relation, budget| {
                            let held = budget.storage();
                            let before = budget.work();
                            assert!(relation.access(0, budget)?.is_some());
                            // Parent liveness19 + local floor2 + binary iteration(1+1).
                            assert_eq!(budget.work(), before + 23);
                            budget.release_storage(1).unwrap();
                            let denied = relation.access(0, budget);
                            budget.reserve_storage(1).unwrap();
                            assert!(matches!(denied, Err(Error::Resource(Resource::Accounting))));
                            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
                            let mut other = Budget::new(&mut work, held);
                            other.reserve_storage(held).unwrap();
                            assert!(matches!(
                                relation.access(0, &mut other),
                                Err(Error::Resource(Resource::Accounting))
                            ));
                            assert_eq!(other.storage(), held);
                            assert!(relation.access(0, budget)?.is_some());
                            match outcome {
                                0 => Ok(()),
                                1 => Err(Error::Invalid("physical callback refusal")),
                                _ => panic!("physical callback unwind"),
                            }
                        })
                    }));
                    match outcome {
                        0 => result.unwrap().unwrap(),
                        1 => assert!(result.unwrap().is_err()),
                        _ => assert!(result.is_err()),
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                }
                Ok(())
            });
        }
    }
}

#[test]
fn physical_address_scoped_access_query_has_an_independently_declared_final_work_boundary() {
    // Admission/construction is excluded. Both fixture pipelines explicitly use
    // LIMIT; spend unused capacity, leaving the derived getter cost. This is
    // not calibration of a successful whole-query run.
    const ACCESS: usize = 23;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            for exact in [true, false] {
                with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                    root.with_physical_address_relation_v1(budget, |relation, budget| {
                        let remaining = ACCESS - usize::from(!exact);
                        budget
                            .charge_work(LIMIT - budget.work() - remaining)
                            .unwrap();
                        let floor = budget.storage();
                        let result = relation.access(0, budget);
                        assert_eq!(result.is_ok(), exact);
                        // Under: the binary-search visit consumes the final
                        // unit; its following comparison is the denied unit.
                        assert_eq!(budget.work(), LIMIT);
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    })
                });
            }
        }
    }
}

fn physical_address_width_mutation_v1(
    root: &fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
) -> ProductionRankedKernelLoweringInputV1 {
    let mut changed = 0;
    let blocks = root
        .lowering
        .kernel()
        .blocks()
        .iter()
        .map(|block| {
            let mut operations = block.operations().to_vec();
            for operation in &mut operations {
                if let ProductionRankedOperationV1::ViewInSpace {
                    element_width,
                    memory_space: dialect_kernel::MemorySpaceAttr::Global,
                    ..
                } = operation
                {
                    assert_eq!(*element_width, 32);
                    *element_width = 16;
                    changed += 1;
                }
            }
            fe2o3_pliron::ProductionRankedBlockV1::new(operations, block.terminator().clone())
        })
        .collect();
    assert!(changed > 0);
    let kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
        root.lowering.kernel().function_name(),
        root.lowering.kernel().argument_count(),
        blocks,
    )
    .unwrap();
    fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
        fe2o3_pliron::ProductionConstructionV1::ranked_kernel("physical_width_mutation", kernel)
            .unwrap(),
        fe2o3_pliron::ProductionSessionLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn physical_address_rejects_wrong_element_units_after_real_completed_store_scope() {
    use fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1 as Candidate;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, roots, budget| {
                let root = &roots[0];
                let changed = physical_address_width_mutation_v1(root);
                let candidates = [Candidate {
                    lowering: &changed,
                    selected_root: root.selected_root,
                    selected_function: root.selected_function,
                    access_sources: root.access_sources,
                    executable_effect_sources: root.executable_effect_sources,
                    control: root.control,
                }];
                let floor = budget.storage();
                let mut completed_store = false;
                let result: Result<(), ProductionSourceOutputErrorV1> = view
                    .with_conditional_memory_control_coverage_v1(
                        &candidates,
                        budget,
                        |control, budget| {
                            view.with_canonical_store_analysis_v1(
                                &candidates,
                                control,
                                budget,
                                |roots, budget| {
                                    completed_store = true;
                                    roots[0].with_physical_address_relation_v1(budget, |_, _| {
                                        panic!("wrong element width must not complete")
                                    })
                                },
                            )
                        },
                    );
                assert!(completed_store, "mutation must reach the address check");
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "physical address requires one exact scalar slice extent and width"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn physical_address_changed_actual_offset_is_refused_by_checked_transition_before_scope() {
    use fe2o3_kernel_ir::OperationKind;
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            let mut changed = checked.owner().module().clone();
            let body = changed.functions[0].body.as_mut().unwrap();
            let length = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::SliceLength { .. }))
                .unwrap()
                .results[0]
                .id;
            let mut count = 0;
            for operation in body
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
            {
                if let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind {
                    assert_ne!(*offset, length);
                    *offset = length;
                    count += 1;
                }
            }
            assert_eq!(count, 1);
            let (changed, storage) =
                Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (input, input_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(bound, budget).unwrap();
            budget
                .reserve_storage(input_storage.retained_storage())
                .unwrap();
            let (output, output_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&changed, budget).unwrap();
            budget
                .reserve_storage(output_storage.retained_storage())
                .unwrap();
            assert!(
                fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                    &input,
                    &output,
                    checked.occurrences().candidate(),
                    budget
                )
                .is_err()
            );
            drop(output);
            budget
                .release_storage(output_storage.retained_storage())
                .unwrap();
            drop(input);
            budget
                .release_storage(input_storage.retained_storage())
                .unwrap();
            drop(changed);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn physical_address_actual_query_storage_denial_drops_scratch_before_reentry() {
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                let floor = budget.storage();
                let prefix = budget.work();
                // The nonzero context/workspace header cannot fit in one byte.
                // This is a constructor refusal test, not an inferred exact cap.
                let held = STORAGE_LIMIT - floor - 1;
                budget.reserve_storage(held).unwrap();
                let denied_floor = budget.storage();
                let mut called = false;
                let result = root.with_physical_address_relation_v1(budget, |_, _| {
                    called = true;
                    Ok(())
                });
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(_))
                ));
                assert!(!called);
                assert_eq!(budget.storage(), denied_floor);
                assert!(budget.work() > prefix);
                let failed = budget.failed_storage();
                assert!(failed.is_some_and(|requested| requested > STORAGE_LIMIT));
                budget.release_storage(held).unwrap();
                assert_eq!(budget.storage(), floor);
                root.with_physical_address_relation_v1(budget, |relation, budget| {
                    assert!(relation.access(0, budget)?.is_some());
                    Ok(())
                })?;
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), failed);
                Ok(())
            });
        }
    }
}

#[test]
fn physical_address_genuine_private_c_keeps_separate_partition_on_both_endpoints() {
    let source = canonical_private_constant_store_source_v1();
    let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            let mut completed_store = false;
            let mut completed_address = false;
            let mut inspect =
                |roots: &[fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<
                    '_,
                    '_,
                    '_,
                >],
                 budget: &mut Budget<'_>| {
                    let [root] = roots else {
                        panic!("one actual Private C root")
                    };
                    completed_store = true;
                    let floor = budget.storage();
                    let access = root.access(0, budget)?.unwrap();
                    assert_eq!(root.access_count(), 1);
                    assert!(access.store_value().is_some());
                    let coordinate = access.operation();
                    let operation = &root.output().module().functions
                        [coordinate.block.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[coordinate.block.block as usize]
                        .operations[coordinate.operation as usize];
                    assert!(
                        matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Store { access, .. }
                    if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)
                    );
                    root.with_physical_address_relation_v1(budget, |relation, budget| {
                        completed_address = true;
                        assert!(std::ptr::eq(relation.output(), root.output()));
                        assert_eq!(
                            (
                                relation.global_access_count(),
                                relation.private_access_count()
                            ),
                            (0, 1)
                        );
                        assert!(relation.access(0, budget)?.is_none());
                        Ok(())
                    })?;
                    assert_eq!(budget.storage(), floor);
                    Ok::<(), ProductionSourceOutputErrorV1>(())
                };
            if policy3 {
                with_actual_policy3_canonical_view_v1(&source, profile, |checked, view, budget| {
                    let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
                    with_ranked_root_preparation_v1(
                        &projection, &inputs, &references, |effects, references| {
                            checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                                view, budget, |session| {
                                    with_prepared_canonical_memory_session_v1(
                                        &projection, &inputs, effects, references, session,
                                        |roots, budget| {
                                            assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                                            inspect(roots, budget)
                                        },
                                    )
                                },
                            )
                        },
                    ).unwrap();
                });
            } else {
                with_actual(&source, profile, |bound, checked, budget| {
                    with_projected_canonical_memory_analysis_v1(
                        &source,
                        bound,
                        checked,
                        profile,
                        &inputs,
                        &references,
                        budget,
                        |roots, budget| {
                            assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                            inspect(roots, budget)
                        },
                    )
                    .unwrap();
                });
            }
            assert!(completed_store && completed_address);
        }
    }
}
mod borrowed_full_address_tests {
    use super::*;
    use fe2o3_lower_mir_kernel::{
        ProductionBorrowedRankedCorrespondenceV1, ProductionProjectionArgumentComponentV1,
        ProductionScopedCanonicalStoreAnalysisV1,
    };

    #[test]
    fn full_expression_recording_preserves_actual_call_refusal() {
        let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
        with_actual_policy3_canonical_view_v1(&source, Profile::Gfx942, |_, view, budget| {
            let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
            let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            with_ranked_root_preparation_v1(&projection, &inputs, &references, |effects, partition| {
                checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                    session.with_canonical_memory_scope_v1(|session| {
                        let source_root = projection.source_launch().roots()[0];
                        let selection = projection.semantic_ssa().source_semantic().select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                        let mut facts = session.for_source(source_root.selected_root(), selection.body());
                        let old = project_and_verify_ranked_root_control_inner_v1(
                            projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                            &partition[0], &mut facts,
                        ).err().expect("the historical tuple-result expression path refuses");
                        let mut recorder = canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(&mut facts)?;
                        let recorded = project_and_verify_ranked_root_control_with_address_claims_v1(
                            projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                            &partition[0], &mut facts, Some(&mut recorder),
                        ).err().expect("descriptive recording must not enable recorded-Call control");
                        assert_eq!(format!("{old:?}"), format!("{recorded:?}"));
                        Ok(())
                    })
                })
            }).unwrap();
        });
    }

    fn with_fixture(
        profile: Profile,
        capture: bool,
        multiple: bool,
        mutate: impl FnOnce(&mut [canonical_memory_control_v1::CanonicalMemoryControlRecorderV1]),
        body: impl FnOnce(
            &[ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>],
            &ProductionBorrowedRankedCorrespondenceV1<'_>,
            &[canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) {
        with_index_fixture(
            profile,
            capture,
            multiple,
            false,
            |recorders, _| mutate(recorders),
            body,
        );
    }

    fn with_index_fixture(
        profile: Profile,
        capture: bool,
        multiple: bool,
        formal_index: bool,
        mutate: impl FnOnce(
            &mut [canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
            Option<SemanticLocalIdV1>,
        ),
        body: impl FnOnce(
            &[ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>],
            &ProductionBorrowedRankedCorrespondenceV1<'_>,
            &[canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) {
        // The seed provides admitted syntax only. This fresh owner is captured
        // BEFORE its own materialization; no capture is grafted onto borrowed N.
        let seed = genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::Parameter, 1);
        let semantic = seed.semantic_ssa().source_semantic();
        let mut functions = semantic.functions().to_vec();
        let extra_index =
            formal_index.then(|| {
                let root = &functions[0];
                let mut blocks = root.blocks().to_vec();
                let mut removed = 0;
                for block in &mut blocks {
                    let statements = block.statements().iter().filter(|statement| {
                    if matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local().index() == 8
                            && assignment.destination().projections().is_empty())
                    {
                        removed += 1;
                        false
                    } else {
                        true
                    }
                }).cloned().collect();
                    *block = SemanticBasicBlockV1::new(
                        block.identity(),
                        block.source(),
                        statements,
                        block.terminator().clone(),
                    )
                    .unwrap();
                }
                assert_eq!(removed, 1);
                let mut locals = root.locals().to_vec();
                assert_eq!(locals[8].ty(), A_U64);
                locals[8] = SemanticLocalDeclV1::new(
                    locals[8].identity(),
                    A_U64,
                    SemanticLocalRoleV1::Argument(3),
                    locals[8].source(),
                );
                let extra = SemanticLocalIdV1::from_index(locals.len() as u32);
                locals.push(local(250, A_U64, SemanticLocalRoleV1::Argument(4)));
                let old = root.abi();
                assert_eq!(old.fixed_count(), 3);
                let mut arguments = old.arguments().to_vec();
                let mut ownership = old.source_argument_ownership().to_vec();
                for _ in 0..2 {
                    arguments.push(SemanticAbiArgumentV1::source(
                        neutral_plain_direct_abi_value_v1(A_U64),
                    ));
                    ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
                }
                let abi = SemanticFunctionAbiV1::from_rustc(
                    old.identity(),
                    old.layout_identity(),
                    old.canon_abi(),
                    old.extern_abi(),
                    old.can_unwind(),
                    old.c_variadic(),
                    5,
                    arguments,
                    old.return_value().clone(),
                )
                .unwrap()
                .with_source_argument_ownership(ownership)
                .unwrap();
                functions[0] = ordinary_rebuild_v1(root, abi, locals, blocks);
                extra
            });
        let mut inputs = vec![ranked_root_input_1d(A_NAME, 247, 1)];
        if multiple {
            let first = &functions[0];
            let second = SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(251)),
                first.role(),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(252)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(253)),
                first.generic_type_arguments_identity(),
                first.const_generic_arguments_identity(),
                first.source(),
                first.abi().clone(),
                first.locals().to_vec(),
                first.entry(),
                first.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"full_address_second".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(248)),
                first.kernel_entry().unwrap().source_contract(),
            ));
            functions.push(second);
            inputs.push(ranked_root_input_1d("full_address_second", 248, 1));
        }
        let roots = (0..functions.len())
            .map(|i| SemanticFunctionIdV1::from_index(i as u32))
            .collect::<Vec<_>>();
        let callables = roots
            .iter()
            .map(|root| SemanticCallableDeclV1::defined(*root))
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
        let semantic_owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic_owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        drop(seed);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let capture_bytes = if capture {
            let receipt = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(ssa.occurrence_storage(), Some(receipt));
            let view = ssa.occurrences_v1().unwrap();
            for index in 0..inputs.len() {
                let rows = view
                    .function(SemanticFunctionIdV1::from_index(index as u32))
                    .unwrap();
                assert!(std::ptr::eq(rows.owner(), &ssa));
                let source_use = rows.events().iter().find(|row| {
                    row.site() == fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(1), statement: 0,
                    } && row.operand() == fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination
                        && row.role() == fe2o3_pliron::ProductionSemanticSsaEventRoleV1::ProjectionIndexUse(1)
                }).expect("the exact Store index event must be captured");
                assert!(source_use.is_promoted() && source_use.is_reachable());
                assert!(
                    matches!(source_use.resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Use { variable, .. }) if variable.get() == 8)
                );
                if let Some(extra) = extra_index {
                    use fe2o3_pliron::ProductionSemanticSsaEntryOriginV1 as Origin;
                    let entry = rows
                        .entry_definitions()
                        .iter()
                        .find(|entry| entry.variable().get() == 8)
                        .unwrap();
                    let other = rows
                        .entry_definitions()
                        .iter()
                        .find(|entry| entry.variable().get() == extra.index())
                        .unwrap();
                    assert_eq!(entry.origin(), Origin::Argument(3));
                    assert_eq!(other.origin(), Origin::Argument(4));
                    assert!(entry.value().is_some() && other.value().is_some());
                    assert_ne!(entry.value(), other.value());
                    let Some(fe2o3_mir_model::SsaResolvedEventV1::Use { value, .. }) =
                        source_use.resolved()
                    else {
                        unreachable!()
                    };
                    assert_eq!(entry.value(), Some(value));
                    assert_ne!(other.value(), Some(value));
                }
            }
            receipt.retained_storage()
        } else {
            assert!(ssa.occurrences_v1().is_none());
            0
        };
        let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let source_bytes = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(source_bytes).unwrap();
        let bound =
            dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                .unwrap();
        let (input, input_storage) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let observed =
            fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget)
                .unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
        let output_bytes = checked.storage().retained_storage();
        budget.reserve_storage(output_bytes).unwrap();
        let coordinate_bytes;
        {
            let (coordinates, coordinates_storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    &input,
                    profile,
                    &mut budget,
                )
                .unwrap();
            budget
                .reserve_storage(coordinates_storage.retained_storage())
                .unwrap();
            coordinate_bytes = coordinates_storage.retained_storage();
            let (view, view_storage) =
                fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                    &source,
                    &coordinates,
                    &checked,
                    &mut budget,
                )
                .unwrap();
            budget
                .reserve_storage(view_storage.retained_storage())
                .unwrap();
            let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let floor = budget.storage();
            with_ranked_root_preparation_v1(
                &projection,
                &inputs,
                &references,
                |effects, partition| {
                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                        &view,
                        &mut budget,
                        |session| {
                            session.with_canonical_memory_scope_v1(|session| {
                                let mut full = Vec::new();
                                let mut recorders = Vec::new();
                                for (ordinal, source_root) in
                                    projection.source_launch().roots().iter().enumerate()
                                {
                                    let selection = projection
                                        .semantic_ssa()
                                        .source_semantic()
                                        .select_kernel_body_for_root_v1(source_root.selected_root())
                                        .unwrap();
                                    let mut facts = session
                                        .for_source(source_root.selected_root(), selection.body());
                                    let legacy = project_and_verify_ranked_root_control_inner_v1(
                                        projection.semantic_ssa(),
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
                                    let captured =
                                project_and_verify_ranked_root_control_with_address_claims_v1(
                                    projection.semantic_ssa(),
                                    effects,
                                    selection,
                                    &inputs[ordinal],
                                    *source_root,
                                    &partition[ordinal],
                                    &mut facts,
                                    Some(&mut recorder),
                                )?;
                                    assert_eq!(captured.ranked_ir, legacy.ranked_ir);
                                    assert_eq!(captured.access_sources, legacy.access_sources);
                                    assert_eq!(
                                        captured.all_kernel_checks_are_clean(),
                                        legacy.all_kernel_checks_are_clean()
                                    );
                                    assert!(captured.all_kernel_checks_are_clean());
                                    full.push(captured);
                                    recorders.push(recorder);
                                }
                                mutate(&mut recorders, extra_index);
                                with_prepared_canonical_memory_session_v1(
                                    &projection,
                                    &inputs,
                                    effects,
                                    partition,
                                    session,
                                    |analyses, budget| {
                                        assert_eq!(analyses.len(), inputs.len());
                                        for analysis in analyses {
                                            assert!(std::ptr::eq(
                                                analysis.output(),
                                                checked.owner()
                                            ));
                                        }
                                        with_authenticated_borrowed_ranked_source_roster_v1(
                                            &source,
                                            full.into_boxed_slice(),
                                            budget,
                                            |original, verification, budget| {
                                                assert!(std::ptr::eq(
                                                    original.materialized(),
                                                    &source
                                                ));
                                                for root in verification.roots() {
                                                    assert!(
                                                !root
                                                    .verification()
                                                    .has_authenticated_functional_verification()
                                            );
                                                    assert!(
                                                        root.verification()
                                                            .aggregate_verus_execution()
                                                            .is_none()
                                                    );
                                                }
                                                Ok(body(analyses, original, &recorders, budget))
                                            },
                                        )
                                        .expect("actual full-ranked R1 correspondence must succeed")
                                    },
                                )
                            })
                        },
                    )
                },
            )
            .unwrap();
            assert!(references.as_slice().is_empty());
            assert_eq!(budget.storage(), floor);
            drop(view);
            budget
                .release_storage(view_storage.retained_storage())
                .unwrap();
        }
        budget.release_storage(coordinate_bytes).unwrap();
        drop(checked);
        budget.release_storage(output_bytes).unwrap();
        drop(input);
        budget
            .release_storage(input_storage.retained_storage())
            .unwrap();
        drop(source);
        budget.release_storage(source_bytes).unwrap();
        budget.release_storage(capture_bytes).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        assert!(budget.work() > 7);
    }

    #[test]
    fn captured_scalar_formal_index_matches_exact_own_o_use_on_both_targets() {
        use fe2o3_kernel_ir::{
            CanonicalKirDefinitionCoordinateV1 as Def, CastKind, OperationKind, ScalarType, Type,
        };
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_index_fixture(
                profile,
                true,
                true,
                true,
                |_, extra| {
                    assert!(extra.is_some());
                },
                |analyses, original, recorders, budget| {
                    assert_eq!(analyses.len(), 2);
                    let floor = budget.storage();
                    let mut prior_work = budget.work();
                    for (ordinal, analysis) in analyses.iter().enumerate() {
                        analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                            let access = relation.access(0, budget)?.unwrap();
                            let function = &relation.output().module().functions[access.gep().block.function.0 as usize];
                            assert!(matches!(function.signature.parameters.as_slice(), [
                                Type::Slice(_), Type::Slice(_), Type::Scalar(ScalarType::U32),
                                Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U64),
                            ]));
                            let body = function.body.as_ref().unwrap();
                            let Def::Result { operation, result: 0 } = access.offset_definition() else {
                                panic!("this source U64 index must reach its own INDEX cast result");
                            };
                            assert_eq!(operation.block.function, access.gep().block.function);
                            let cast = &body.blocks[operation.block.block as usize].operations[operation.operation as usize];
                            assert!(matches!(cast.kind, OperationKind::Cast {
                                kind: CastKind::Bitcast, value, to: Type::INDEX,
                            } if value == body.parameters[3]));
                            assert_ne!(body.parameters[3], body.parameters[4]);
                            let gep = &body.blocks[access.gep().block.block as usize].operations[access.gep().operation as usize];
                            assert!(matches!(gep.kind, OperationKind::GetElementPointer { offset, .. }
                                if offset == cast.results[0].id));
                            for _ in 0..2 {
                                let live = budget.storage();
                                relation.check_borrowed_ranked_addresses_v1(
                                    original, ordinal, recorders[ordinal].candidate(), budget,
                                )?;
                                assert_eq!(budget.storage(), live);
                                assert!(budget.work() > prior_work);
                                prior_work = budget.work();
                            }
                            Ok(())
                        })?;
                        assert_eq!(budget.storage(), floor);
                    }
                    Ok(())
                },
            );
        }
    }

    #[test]
    fn captured_scalar_formal_index_rejects_same_typed_claimed_source_use_swap() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_index_fixture(
                profile,
                true,
                false,
                true,
                |recorders, extra| {
                    let extra = extra.unwrap();
                    let claims = &mut recorders[0].candidate_mut().arguments;
                    let index = claims
                        .iter_mut()
                        .find(|claim| {
                            claim.source_local.index() == 8
                                && claim.component
                                    == ProductionProjectionArgumentComponentV1::Scalar
                        })
                        .unwrap();
                    assert_ne!(index.source_local, extra);
                    // Only an inert full claim is hostile. The exact captured
                    // Store use still resolves to formal 3, never formal 4.
                    index.source_local = extra;
                },
                |analyses, original, recorders, budget| {
                    analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                        let floor = budget.storage();
                        let before = budget.work();
                        assert!(matches!(
                            relation.check_borrowed_ranked_addresses_v1(
                                original,
                                0,
                                recorders[0].candidate(),
                                budget,
                            ),
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "full address leaf source mapping differs"
                            ))
                        ));
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work() > before);
                        Ok(())
                    })
                },
            );
        }
    }

    #[test]
    fn captured_full_addresses_match_same_actual_o_without_functional_authority() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for multiple in [false, true] {
                with_fixture(
                    profile,
                    true,
                    multiple,
                    |_| {},
                    |analyses, original, recorders, budget| {
                        let floor = budget.storage();
                        let mut history = budget.work();
                        for (ordinal, analysis) in analyses.iter().enumerate() {
                            analysis.with_physical_address_relation_v1(
                                budget,
                                |relation, budget| {
                                    assert_eq!(relation.global_access_count(), 1);
                                    for _ in 0..2 {
                                        let live = budget.storage();
                                        relation.check_borrowed_ranked_addresses_v1(
                                            original,
                                            ordinal,
                                            recorders[ordinal].candidate(),
                                            budget,
                                        )?;
                                        assert_eq!(budget.storage(), live);
                                        assert!(budget.work() > history);
                                        history = budget.work();
                                    }
                                    Ok(())
                                },
                            )?;
                            assert_eq!(budget.storage(), floor);
                        }
                        Ok(())
                    },
                );
            }
        }
    }

    #[test]
    fn full_address_join_refuses_uncaptured_source_after_real_r1_and_d() {
        with_fixture(
            Profile::Gfx942,
            false,
            false,
            |_| {},
            |analyses, original, recorders, budget| {
                analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                    let floor = budget.storage();
                    assert!(matches!(
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorders[0].candidate(),
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address source SSA capture absent"
                        ))
                    ));
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                })
            },
        );
    }

    #[test]
    fn full_address_claim_mutations_reach_exact_join_after_r1_and_d() {
        for mutation in 0..5 {
            with_fixture(
                Profile::Gfx942,
                true,
                false,
                |recorders| {
                    let claims = &mut recorders[0].candidate_mut().arguments;
                    let index = claims
                        .iter()
                        .position(|row| {
                            row.source_local.index() == 8
                                && row.component == ProductionProjectionArgumentComponentV1::Scalar
                        })
                        .unwrap();
                    let extent = claims
                        .iter()
                        .position(|row| {
                            row.component == ProductionProjectionArgumentComponentV1::SliceLength
                                && row.source_local.index() == 1
                        })
                        .unwrap();
                    match mutation {
                        0 => claims[index].source_local = SemanticLocalIdV1::from_index(7),
                        1 => claims[extent].source_local = SemanticLocalIdV1::from_index(2),
                        2 => {
                            claims.remove(index);
                        }
                        3 => claims[extent] = claims[index],
                        4 => {
                            let value = claims[index].ranked_value;
                            claims[index].ranked_value = claims[extent].ranked_value;
                            claims[extent].ranked_value = value;
                        }
                        _ => unreachable!(),
                    }
                },
                |analyses, original, recorders, budget| {
                    analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                        let floor = budget.storage();
                        let error = relation
                            .check_borrowed_ranked_addresses_v1(
                                original,
                                0,
                                recorders[0].candidate(),
                                budget,
                            )
                            .unwrap_err();
                        match mutation {
                            0 | 1 | 4 => assert!(matches!(
                                error,
                                ProductionSourceOutputErrorV1::Invalid(
                                    "full address leaf source mapping differs"
                                )
                            )),
                            2 => assert!(matches!(
                                error,
                                ProductionSourceOutputErrorV1::Invalid(
                                    "full address source claim absent"
                                )
                            )),
                            3 => {
                                assert!(matches!(error, ProductionSourceOutputErrorV1::Invalid(_)))
                            }
                            _ => unreachable!(),
                        }
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    })
                },
            );
        }
    }

    #[test]
    fn full_address_scopes_keep_receipts_on_error_unwind_storage_denial_and_reentry() {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        with_fixture(
            Profile::Gfx950,
            true,
            true,
            |_| {},
            |analyses, original, recorders, budget| {
                let floor = budget.storage();
                let result: Result<(), ProductionSourceOutputErrorV1> = analyses[0]
                    .with_physical_address_relation_v1(budget, |relation, budget| {
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorders[0].candidate(),
                            budget,
                        )?;
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full-address callback marker",
                        ))
                    });
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "full-address callback marker"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: Result<(), ProductionSourceOutputErrorV1> = analyses[0]
                        .with_physical_address_relation_v1(budget, |relation, budget| {
                            relation.check_borrowed_ranked_addresses_v1(
                                original,
                                0,
                                recorders[0].candidate(),
                                budget,
                            )?;
                            std::panic::panic_any(0x333_u32);
                        });
                }))
                .unwrap_err();
                assert_eq!(panic.downcast_ref::<u32>(), Some(&0x333));
                assert_eq!(budget.storage(), floor);
                let before_second = budget.work();
                analyses[1].with_physical_address_relation_v1(budget, |relation, budget| {
                    let live = budget.storage();
                    assert!(matches!(
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorders[1].candidate(),
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address selected root differs"
                        ))
                    ));
                    let padding = budget.storage_limit() - budget.storage();
                    budget.reserve_storage(padding).unwrap();
                    let denied = relation.check_borrowed_ranked_addresses_v1(
                        original,
                        1,
                        recorders[1].candidate(),
                        budget,
                    );
                    assert!(matches!(
                        denied,
                        Err(ProductionSourceOutputErrorV1::Resource(Resource::Storage(
                            _
                        )))
                    ));
                    assert_eq!(budget.storage(), live + padding);
                    budget.release_storage(padding).unwrap();
                    relation.check_borrowed_ranked_addresses_v1(
                        original,
                        1,
                        recorders[1].candidate(),
                        budget,
                    )?;
                    assert_eq!(budget.storage(), live);
                    Ok(())
                })?;
                assert!(budget.work() > before_second);
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        );
    }
}

mod invocation_coordinate_address_tests {
    use super::*;
    use fe2o3_lower_mir_kernel::{
        ProductionBorrowedRankedCorrespondenceV1, ProductionScopedCanonicalStoreAnalysisV1,
    };

    fn with_fixture(
        profile: Profile,
        control_case: Option<u8>,
        body: impl FnOnce(
            &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
            &ProductionBorrowedRankedCorrespondenceV1<'_>,
            &canonical_memory_control_v1::CanonicalMemoryControlRecorderV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) {
        with_fixture_mode(profile, control_case, false, body)
    }

    fn with_fixture_mode(
        profile: Profile,
        control_case: Option<u8>,
        moved_receiver: bool,
        body: impl FnOnce(
            &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
            &ProductionBorrowedRankedCorrespondenceV1<'_>,
            &canonical_memory_control_v1::CanonicalMemoryControlRecorderV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) {
        with_invocation_coordinate_source_mode_v1(moved_receiver, |source, budget| {
            if moved_receiver {
                assert_eq!(control_case, Some(4));
                let semantic = source.semantic_ssa().source_semantic();
                let SemanticTerminatorKindV1::Call(get) =
                    semantic.functions()[0].blocks()[3].terminator().kind()
                else {
                    panic!("fresh admitted Get call is absent");
                };
                assert!(matches!(get.arguments(), [SemanticOperandV1::Move(place)]
                    if place.local().index() == 15 && place.ty() == INVOCATION_REFERENCE_V1));
                let captured = source
                    .semantic_ssa()
                    .occurrences_v1()
                    .unwrap()
                    .function(ROOT)
                    .unwrap();
                assert!(captured.events().iter().any(|event| event.site()
                    == fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(3),
                    }
                    && event.operand()
                        == fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::CallArgument(0)
                    && event.role() == fe2o3_pliron::ProductionSemanticSsaEventRoleV1::BaseUse
                    && event.is_reachable()
                    && event.is_promoted()));
            }
            let floor = budget.storage();
            let bound =
                dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                    .unwrap();
            let (input, input_storage) =
                Owner::from_module_ref_with_verification_budget_v12(bound.module(), budget)
                    .unwrap();
            budget
                .reserve_storage(input_storage.retained_storage())
                .unwrap();
            let observed =
                fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, budget).unwrap();
            budget
                .reserve_storage(observed.storage().retained_storage())
                .unwrap();
            let checked = observed.try_check_and_finish_v1(budget).unwrap();
            let output_bytes = checked.storage().retained_storage();
            budget.reserve_storage(output_bytes).unwrap();
            let coordinate_bytes;
            {
                let (coordinates, storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        source.executable(),
                        &input,
                        profile,
                        budget,
                    )
                    .unwrap();
                coordinate_bytes = storage.retained_storage();
                budget.reserve_storage(coordinate_bytes).unwrap();
                let (view, storage) =
                    fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                        source,
                        &coordinates,
                        &checked,
                        budget,
                    )
                    .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
                let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
                let references =
                    crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                let live = budget.storage();
                with_ranked_root_preparation_v1(&projection, &inputs, &references, |effects, partition| {
                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(&view, budget, |session| {
                        session.with_canonical_memory_scope_v1(|session| {
                            let source_root = projection.source_launch().roots()[0];
                            let selection = projection.semantic_ssa().source_semantic()
                                .select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                            let mut facts = session.for_source(source_root.selected_root(), selection.body());
                            let legacy = project_and_verify_ranked_root_control_inner_v1(
                                projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                &partition[0], &mut facts)?;
                            let mut recorder = canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(&mut facts)?;
                            let full = project_and_verify_ranked_root_control_with_address_claims_v1(
                                projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                &partition[0], &mut facts, Some(&mut recorder))?;
                            assert_eq!(full.ranked_ir, legacy.ranked_ir);
                            assert_eq!(full.access_sources, legacy.access_sources);
                            if let Some(case) = control_case {
                                let mut root = project_canonical_memory_root_v1(
                                    projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                    &partition[0], &mut facts)?;
                                let ordinal = root.control.candidate().arguments.iter().position(|row|
                                    row.source_local.index() == 8
                                        && row.component == fe2o3_lower_mir_kernel::ProductionProjectionArgumentComponentV1::Scalar)
                                    .expect("the actual recorder must capture the Invocation leaf");
                                let ranked = root.control.candidate().arguments[ordinal].ranked_value;
                                assert!(root.lowering.kernel().blocks().iter().flat_map(|block| block.operations()).any(|operation|
                                    matches!(operation, fe2o3_pliron::ProductionRankedOperationV1::InvocationIndex {
                                        result, dimension: 0, launch_extent: 0,
                                    } if ranked == fe2o3_pliron::ProductionRankedValueV1::Local(*result))));
                                let expected = match case {
                                    0 => {
                                        root.control.candidate_mut().arguments[ordinal].source_local = SemanticLocalIdV1::from_index(9);
                                        Some("invocation source guard local or type differs")
                                    }
                                    1 => {
                                        root.control.candidate_mut().arguments.remove(ordinal);
                                        Some("control projected continuation lost its actual Call")
                                    }
                                    2 => {
                                        root.control.candidate_mut().arguments[ordinal].component =
                                            fe2o3_lower_mir_kernel::ProductionProjectionArgumentComponentV1::SliceLength;
                                        Some("invocation source component is not scalar")
                                    }
                                    3 => None,
                                    4 => {
                                        assert!(moved_receiver);
                                        Some("invocation Get requires one copied typed reference")
                                    }
                                    _ => unreachable!(),
                                };
                                drop(facts);
                                return session.with_output_occurrences_v1(|view, budget| {
                                    let candidates = [fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1 {
                                        selected_root: root.selected_root, selected_function: root.selected_function,
                                        lowering: &root.lowering, access_sources: &root.access_sources,
                                        executable_effect_sources: &root.executable_effect_sources,
                                        control: root.control.candidate(),
                                    }];
                                    let floor = budget.storage();
                                    let mut completed = false;
                                    let result = view.with_conditional_memory_control_coverage_v1(&candidates, budget, |control, budget| {
                                        completed = true;
                                        assert!(control.index_value(view, &candidates[0], ranked, budget)?.is_none());
                                        assert!(control.index_leaf(view, &candidates[0], ranked, budget)?.is_none());
                                        Ok(())
                                    });
                                    if let Some(expected) = expected {
                                        assert!(matches!(result, Err(ProductionSourceOutputErrorV1::Invalid(actual)) if actual == expected));
                                        assert!(!completed);
                                    } else {
                                        result.unwrap();
                                        assert!(completed);
                                    }
                                    assert_eq!(budget.storage(), floor);
                                    Ok(())
                                });
                            }
                            drop(facts);
                            with_prepared_canonical_memory_session_v1(&projection, &inputs, effects, partition, session,
                                |analyses, budget| {
                                    assert_eq!(analyses.len(), 1);
                                    assert!(std::ptr::eq(analyses[0].output(), checked.owner()));
                                    with_authenticated_borrowed_ranked_source_roster_v1(
                                        source, vec![full].into_boxed_slice(), budget,
                                        |original, verification, budget| {
                                            assert!(std::ptr::eq(original.materialized(), source));
                                            assert_eq!(verification.roots().len(), 1);
                                            assert!(!verification.roots()[0].verification().has_authenticated_functional_verification());
                                            assert!(verification.roots()[0].verification().aggregate_verus_execution().is_none());
                                            Ok(body(&analyses[0], original, &recorder, budget))
                                        }).expect("fresh source/N correspondence, not functional Some")
                                })
                        })
                    })
                }).unwrap();
                assert!(references.as_slice().is_empty());
                assert_eq!(budget.storage(), live);
                drop(view);
                budget.release_storage(storage.retained_storage()).unwrap();
            }
            budget.release_storage(coordinate_bytes).unwrap();
            drop(checked);
            budget.release_storage(output_bytes).unwrap();
            drop(input);
            budget
                .release_storage(input_storage.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }

    #[test]
    fn invocation_global_x_joins_actual_policy3_output_and_own_full_address_on_both_targets() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(profile, None, |analysis, original, recorder, budget| {
                assert_eq!(analysis.access_count(), 1);
                let output = analysis.output().module();
                let body = output.functions[0].body.as_ref().unwrap();
                let mut intrinsics = body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| {
                        matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Intrinsic(value)
                        if *value == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d())
                    });
                assert!(intrinsics.next().is_some());
                assert!(intrinsics.next().is_none());
                assert!(body.blocks.iter().flat_map(|block| &block.operations).any(
                    |operation| matches!(
                        operation.kind,
                        fe2o3_kernel_ir::OperationKind::Store { .. }
                    )
                ));
                let floor = budget.storage();
                analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                    assert_eq!(relation.global_access_count(), 1);
                    relation.check_borrowed_ranked_addresses_v1(
                        original,
                        0,
                        recorder.candidate(),
                        budget,
                    )
                })?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
        }
    }

    #[test]
    fn invocation_control_rejects_forged_or_missing_source_claims_and_keeps_public_leaf_closed() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for case in 0..4 {
                with_fixture(profile, Some(case), |_, _, _, _| {
                    panic!("control-only case must not enter the completed address callback")
                });
            }
        }
    }

    #[test]
    fn invocation_fresh_moved_receiver_reaches_the_closed_source_ancestry_refusal() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture_mode(profile, Some(4), true, |_, _, _, _| {
                panic!("moved source receiver must not complete the address callback")
            });
        }
    }

    #[test]
    fn invocation_actual_output_has_fresh_complete_formal_memory_without_discharge() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(profile, None, |analysis, original, recorder, budget| {
                let floor = budget.storage();
                analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                    relation.check_borrowed_ranked_addresses_v1(
                        original,
                        0,
                        recorder.candidate(),
                        budget,
                    )
                })?;
                let mut completed = false;
                analysis.with_complete_formal_memory_v1(budget, |formal, budget| {
                    completed = true;
                    assert!(std::ptr::eq(formal.output(), analysis.output()));
                    formal.require_borrowed_source_v1(original, budget)
                        .map_err(fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1::SourceOutput)?;
                    assert_eq!(formal.selected_root(), analysis.selected_root());
                    let obligations = formal.obligations();
                    assert_eq!(obligations.kernel(), &formal.kernel().id);
                    assert_eq!(obligations.entry(), &formal.kernel().entry);
                    assert_eq!(obligations.accesses().len(), 1);
                    let access = &obligations.accesses()[0];
                    assert_eq!(access.kind(), fe2o3_kernel_ir::FormalMemoryAccessKind::Write);
                    let function = formal.output().module().function(&formal.kernel().entry).unwrap();
                    let location = access.location();
                    let block = function.body.as_ref().unwrap().blocks.iter()
                        .find(|block| block.id == location.block).unwrap();
                    assert!(matches!(block.operations[location.operation_index].kind, fe2o3_kernel_ir::OperationKind::Store { .. }));
                    assert!(obligations.inter_invocation_conflicts().is_empty());
                    Ok(())
                }).expect("actual O must freshly extract Complete; preserve any real Incomplete diagnostic");
                assert!(completed);
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
        }
    }

    #[test]
    fn invocation_full_own_use_rejects_foreign_source_claim_and_root() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(profile, None, |analysis, original, recorder, budget| {
                let floor = budget.storage();
                analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                    let live = budget.storage();
                    let mut claims =
                        fe2o3_lower_mir_kernel::ProductionProjectionControlCandidateV1 {
                            blocks: recorder.candidate().blocks.clone(),
                            arguments: recorder.candidate().arguments.clone(),
                        };
                    let row = claims
                        .arguments
                        .iter_mut()
                        .find(|row| row.source_local.index() == 8)
                        .unwrap();
                    row.source_local = SemanticLocalIdV1::from_index(9);
                    assert!(matches!(
                        relation.check_borrowed_ranked_addresses_v1(original, 0, &claims, budget),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address leaf source mapping differs"
                        ))
                    ));
                    assert!(matches!(
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            1,
                            recorder.candidate(),
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address root absent"
                        ))
                    ));
                    assert_eq!(budget.storage(), live);
                    relation.check_borrowed_ranked_addresses_v1(
                        original,
                        0,
                        recorder.candidate(),
                        budget,
                    )
                })?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
        }
    }

    #[test]
    fn invocation_physical_callback_error_and_unwind_restore_the_original_live_floor() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(profile, None, |analysis, original, recorder, budget| {
                let floor = budget.storage();
                let before = budget.work();
                let failed =
                    analysis.with_physical_address_relation_v1::<()>(budget, |relation, budget| {
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorder.candidate(),
                            budget,
                        )?;
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "invocation callback marker",
                        ))
                    });
                assert!(matches!(
                    failed,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "invocation callback marker"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > before);
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    analysis.with_physical_address_relation_v1::<()>(budget, |relation, budget| {
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorder.candidate(),
                            budget,
                        )?;
                        std::panic::panic_any(73u32)
                    })
                }))
                .expect_err("the original callback panic must resume");
                assert_eq!(panic.downcast_ref::<u32>(), Some(&73));
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
        }
    }
}
