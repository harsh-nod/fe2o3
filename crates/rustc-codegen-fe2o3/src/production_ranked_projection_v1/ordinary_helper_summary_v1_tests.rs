mod ordinary_helper_effect_only_tests_v1 {
    use super::*;

    #[derive(Clone, Copy)]
    enum ResultShape {
        Array,
        Tuple,
        UnitTuple,
    }

    #[derive(Clone, Copy)]
    enum BodyShape {
        Straight,
        Join,
        Loop,
        Read,
        Write,
        Cycle,
    }

    const RESULT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);

    fn initialized_attributes() -> SemanticAbiValueAttributesV1 {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap()
    }

    fn result_catalog(
        mut types: Vec<SemanticTypeDeclV1>,
        shape: ResultShape,
    ) -> (Vec<SemanticTypeDeclV1>, SemanticAbiPassModeV1) {
        assert_eq!(types.len(), RESULT.index() as usize);
        let SemanticBackendReprV1::Scalar(u32_scalar) =
            types[A_U32.index() as usize].layout().backend_repr()
        else {
            panic!()
        };
        let SemanticBackendReprV1::Scalar(u64_scalar) =
            types[A_U64.index() as usize].layout().backend_repr()
        else {
            panic!()
        };
        let (layout, result, mode) = match shape {
            ResultShape::Array => (
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    8,
                    4,
                    SemanticFieldsShapeV1::array(4, 2),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    4,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Array {
                    element: A_U32,
                    length: 2,
                },
                SemanticAbiPassModeV1::cast(
                    false,
                    SemanticAbiCastV1::new(
                        [None; 8],
                        None,
                        SemanticAbiUniformV1::new(
                            SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8)
                                .unwrap(),
                            8,
                        )
                        .unwrap(),
                        initialized_attributes(),
                    ),
                ),
            ),
            ResultShape::Tuple => (
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    16,
                    8,
                    SemanticFieldsShapeV1::arbitrary(vec![8, 0], vec![1, 0]).unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::scalar_pair(*u64_scalar, *u32_scalar),
                    None,
                    false,
                    None,
                    8,
                    0,
                    SemanticTypeLayoutDetailsV1::Aggregate(
                        SemanticAggregateLayoutV1::new(
                            vec![8, 0],
                            vec![SemanticPaddingV1::new(12, 4).unwrap()],
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![A_U32, A_U64]).unwrap(),
                ),
                SemanticAbiPassModeV1::Pair {
                    first: initialized_attributes(),
                    second: initialized_attributes(),
                },
            ),
            ResultShape::UnitTuple => (
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    4,
                    4,
                    SemanticFieldsShapeV1::arbitrary(vec![0, 0], vec![0, 1]).unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::scalar(*u32_scalar),
                    None,
                    false,
                    None,
                    4,
                    0,
                    SemanticTypeLayoutDetailsV1::Aggregate(
                        SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
                    ),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![A_UNIT, A_U32]).unwrap(),
                ),
                SemanticAbiPassModeV1::Direct(initialized_attributes()),
            ),
        };
        let declaration = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(248)),
            SemanticLayoutIdentityV1::from_sha256(bytes(248)),
            layout,
            result,
        );
        let declaration = if matches!(shape, ResultShape::Array) {
            // The frontend's noundef walker visits the initialized U32 element
            // of this compact array. Its exact Cast ABI retains NoUndef.
            let properties = declaration
                .abi_properties()
                .with_rustc_layout_is_noundef(true);
            declaration.with_rustc_abi_properties(properties)
        } else {
            declaration
        };
        types.push(declaration);
        (types, mode)
    }

    fn whole(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
    }

    fn output(shape: ResultShape, first: u128) -> SemanticStatementV1 {
        let (kind, operands) = match shape {
            ResultShape::Array => (
                SemanticAggregateKindV1::Array,
                vec![
                    typed_constant(A_U32, first, 4),
                    typed_constant(A_U32, 29, 4),
                ],
            ),
            ResultShape::Tuple => (
                SemanticAggregateKindV1::Tuple,
                vec![
                    typed_constant(A_U32, first, 4),
                    typed_constant(A_U64, 29, 8),
                ],
            ),
            ResultShape::UnitTuple => (
                SemanticAggregateKindV1::Tuple,
                vec![
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        A_UNIT,
                        SemanticConstantValueV1::ZeroSized,
                    )),
                    typed_constant(A_U32, first, 4),
                ],
            ),
        };
        typed_assignment(
            0,
            RESULT,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(kind, operands).unwrap(),
            ),
        )
    }

    fn helper_call(callee: u32, destination: u32, target: u32) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    whole(destination, RESULT),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn helper(
        tag: u8,
        mode: &SemanticAbiPassModeV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256(bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(bytes(250)),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                0,
                vec![],
                SemanticAbiValueV1::new(RESULT, mode.clone()),
            )
            .unwrap(),
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn component(index: u64) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: index,
                        minimum_length: 2,
                        from_end: false,
                    },
                    A_U32,
                )
                .unwrap(),
            ],
            A_U32,
        )
        .unwrap()
    }

    fn source_fixture(shape: ResultShape, body: BodyShape) -> ProductionSemanticSsaOwnerV1 {
        let original = genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::Parameter, 1);
        let source = original.semantic_ssa().source_semantic();
        let old = &source.functions()[0];
        let (types, mode) = result_catalog(source.types().to_vec(), shape);
        assert_eq!(&types[..RESULT.index() as usize], source.types());
        let mut locals = old.locals().to_vec();
        let result_local = locals.len() as u32;
        locals.push(local(114, RESULT, SemanticLocalRoleV1::Temporary));
        let mut blocks = old.blocks().to_vec();
        let entry = &blocks[0];
        blocks[0] = SemanticBasicBlockV1::new(
            entry.identity(),
            entry.source(),
            entry.statements().to_vec(),
            SemanticTerminatorV1::new(entry.terminator().source(), helper_call(1, result_local, 2)),
        )
        .unwrap();
        let root = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
        let forwarding = helper(
            248,
            &mode,
            vec![local(120, RESULT, SemanticLocalRoleV1::Return)],
            vec![
                block(130, vec![], helper_call(2, 0, 1)),
                block(131, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        let mut leaf_locals = vec![local(120, RESULT, SemanticLocalRoleV1::Return)];
        let blocks = match body {
            BodyShape::Straight => vec![block(
                130,
                vec![output(shape, 11)],
                SemanticTerminatorKindV1::Return,
            )],
            BodyShape::Cycle => vec![
                block(130, vec![], helper_call(1, 0, 1)),
                block(131, vec![], SemanticTerminatorKindV1::Return),
            ],
            BodyShape::Read => {
                leaf_locals.push(local(121, A_U32, SemanticLocalRoleV1::Temporary));
                vec![block(
                    130,
                    vec![
                        output(shape, 11),
                        typed_assignment(
                            1,
                            A_U32,
                            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                component(0),
                                SemanticVolatilityV1::NonVolatile,
                                None,
                            )),
                        ),
                    ],
                    SemanticTerminatorKindV1::Return,
                )]
            }
            BodyShape::Write => vec![block(
                130,
                vec![
                    output(shape, 11),
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        component(0),
                        SemanticRvalueV1::new(
                            A_U32,
                            SemanticRvalueKindV1::Use(typed_constant(A_U32, 7, 4)),
                        ),
                    ))),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            BodyShape::Join | BodyShape::Loop => {
                leaf_locals.push(local(121, A_U32, SemanticLocalRoleV1::Temporary));
                if matches!(body, BodyShape::Loop) {
                    vec![
                        block(
                            130,
                            vec![
                                output(shape, 11),
                                typed_assignment(
                                    1,
                                    A_U32,
                                    SemanticRvalueKindV1::Use(typed_constant(A_U32, 0, 4)),
                                ),
                            ],
                            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                        ),
                        block(131, vec![], zero_switch(1, A_U32, 2, 3)),
                        block(
                            132,
                            vec![
                                output(shape, 31),
                                typed_assignment(
                                    1,
                                    A_U32,
                                    SemanticRvalueKindV1::Use(typed_constant(A_U32, 1, 4)),
                                ),
                            ],
                            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                        ),
                        block(133, vec![], SemanticTerminatorKindV1::Return),
                    ]
                } else {
                    vec![
                        block(
                            130,
                            vec![typed_assignment(
                                1,
                                A_U32,
                                SemanticRvalueKindV1::Use(typed_constant(A_U32, 0, 4)),
                            )],
                            zero_switch(1, A_U32, 1, 2),
                        ),
                        block(
                            131,
                            vec![output(shape, 11)],
                            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                        ),
                        block(
                            132,
                            vec![output(shape, 31)],
                            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                        ),
                        block(133, vec![], SemanticTerminatorKindV1::Return),
                    ]
                }
            }
        };
        let leaf = helper(249, &mode, leaf_locals, blocks);
        assertion_ssa_functions(types, vec![root, forwarding, leaf])
    }

    #[test]
    fn admitted_ordinary_helper_calls_remain_executable_after_normal_projection() {
        for (shape, body) in [
            (ResultShape::Array, BodyShape::Straight),
            (ResultShape::Tuple, BodyShape::Straight),
            (ResultShape::UnitTuple, BodyShape::Straight),
            (ResultShape::Array, BodyShape::Join),
            (ResultShape::Array, BodyShape::Loop),
        ] {
            let ssa = source_fixture(shape, body);
            let semantic = ssa.source_semantic();
            let summaries = derive_defined_callable_empty_effect_summaries_v1(
                semantic.types(),
                semantic.functions(),
                semantic.callables(),
            )
            .unwrap();
            for function in [1, 2] {
                assert!(summaries.is_exact_empty(SemanticFunctionIdV1::from_index(function)));
                assert!(!summaries.is_exact_empty_deterministic_scalar(
                    SemanticFunctionIdV1::from_index(function)
                ));
            }
            let materialized =
                materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 1)])
                    .unwrap();
            let program = project_and_verify_ranked_materialized_semantic_mir_v1(
                materialized,
                &[ranked_root_input_1d(A_NAME, 247, 1)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            )
            .unwrap();
            assert_eq!(program.root_count(), 1);
            assert!(program.all_kernel_checks_are_clean());
            let module = program.materialized.executable().module();
            let helper_ids = module
                .functions
                .iter()
                .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
                .map(|function| &function.id)
                .collect::<Vec<_>>();
            assert_eq!(helper_ids.len(), 2);
            let calls = module.functions.iter().filter_map(|function| function.body.as_ref()).flat_map(|body| &body.blocks).flat_map(|block| &block.operations)
                .filter(|operation| matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Call { callee, .. } if helper_ids.contains(&callee))).collect::<Vec<_>>();
            assert_eq!(calls.len(), 2);
            let count = if matches!(shape, ResultShape::UnitTuple) {
                1
            } else {
                2
            };
            assert!(
                calls
                    .iter()
                    .all(|operation| operation.results.len() == count)
            );
            let leaf = module
                .functions
                .iter()
                .find(|function| &function.id == helper_ids[1])
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            assert_eq!(
                leaf.blocks.len(),
                if matches!(body, BodyShape::Join | BodyShape::Loop) {
                    4
                } else {
                    1
                }
            );
            // This exercises the normal materialized source projector, not O
            // call census, result-value correlation, LLVM or final attachment.
        }
    }

    #[test]
    fn admitted_memory_and_recursive_helpers_reject_transitive_empty_effects() {
        // These are admitted-source summary negatives. Retained array memory
        // need not pass the independent executable aggregate-storage gates.
        for body in [BodyShape::Read, BodyShape::Write, BodyShape::Cycle] {
            let ssa = source_fixture(ResultShape::Array, body);
            let semantic = ssa.source_semantic();
            let summaries = derive_defined_callable_empty_effect_summaries_v1(
                semantic.types(),
                semantic.functions(),
                semantic.callables(),
            )
            .unwrap();
            assert!(!summaries.is_exact_empty(SemanticFunctionIdV1::from_index(1)));
            assert!(!summaries.is_exact_empty(SemanticFunctionIdV1::from_index(2)));
        }
    }

    #[test]
    fn source_admission_rejects_array_cast_noundef_metadata_mismatch() {
        let ssa = source_fixture(ResultShape::Array, BodyShape::Straight);
        let semantic = ssa.source_semantic();
        let mut types = semantic.types().to_vec();
        let declaration = types[RESULT.index() as usize].clone();
        assert!(declaration.abi_properties().rustc_layout_is_noundef());
        let properties = declaration
            .abi_properties()
            .with_rustc_layout_is_noundef(false);
        types[RESULT.index() as usize] = declaration.with_rustc_abi_properties(properties);
        let request = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types,
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            semantic.functions().to_vec(),
            semantic.callables().to_vec(),
            semantic.roots().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            request.admit_current_production(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }

    #[test]
    fn admitted_recursive_helper_is_refused_during_real_materialization() {
        let ssa = source_fixture(ResultShape::Array, BodyShape::Cycle);
        let launch = source_launch_roster_for_ranked_inputs_v1(
            &ssa,
            &[ranked_root_input_1d(A_NAME, 247, 1)],
        )
        .unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
        );
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        // The public materializer rejects the reachable recursive closure before
        // a ranked projector can run. No synthetic executable owner is supplied.
        let result =
            fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            );
        assert!(matches!(
            result,
            Err(
                fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Lowering(
                    fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "recursive deterministic helper call graph is unsupported",
                    }
                )
            )
        ));
    }

    #[test]
    fn transport_adapter_preserves_projection_denial_identity_and_counter_prefix() {
        use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1 as Error;
        for (initial, amount, expected_work, expected_detail) in [
            (
                MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 - 2,
                3,
                MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 + 1,
                "defined-callable effect-summary work exceeded its production limit",
            ),
            (
                usize::MAX - 1,
                2,
                usize::MAX - 1,
                "defined-callable effect-summary work overflowed",
            ),
        ] {
            let mut work = initial;
            let result = ordinary_helper_transport_query_v1(&mut work, |charge| {
                assert!(charge(amount).is_err());
                // Even a hostile callback which continues after denial cannot
                // reset the module counter or substitute a semantic refusal.
                assert!(charge(17).is_err());
                Err(Error::Unsupported {
                    function: 99,
                    block: None,
                    statement: None,
                    detail: "this must not hide the original budget error",
                })
            });
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(detail))
                    if detail == expected_detail
            ));
            assert_eq!(work, expected_work);
        }
        let mut work = 7;
        assert!(ordinary_helper_transport_query_v1(&mut work, |charge| charge(3)).unwrap());
        assert_eq!(work, 10);
    }

    #[test]
    fn transport_adapter_only_classifies_semantic_subset_refusals_as_ineligible() {
        use fe2o3_lower_mir_kernel::{
            ProductionSemanticKirErrorV1 as Error, ProductionSemanticKirResourceV1 as Resource,
        };
        for error in [
            Error::Unsupported {
                function: 71,
                block: Some(9),
                statement: Some(4),
                detail: "unsupported transport subset",
            },
            Error::ScalarTypeUnavailable {
                semantic_type: 17,
                shape: "a capability".into(),
            },
        ] {
            let mut work = 7;
            assert!(
                !ordinary_helper_transport_query_v1(&mut work, |charge| {
                    charge(3)?;
                    Err(error)
                })
                .unwrap()
            );
            assert_eq!(work, 10);
        }
        for error in [
            Error::CorrespondenceMismatch,
            Error::AllocationFailure {
                resource: Resource::AnalysisStorage,
            },
            Error::ResourceLimit {
                resource: Resource::AnalysisStorage,
                actual: 7,
                limit: 6,
            },
            Error::ResourceLimit {
                resource: Resource::AnalysisWork,
                actual: 41,
                limit: 40,
            },
        ] {
            let expected = format!("{error:?}");
            let mut work = 7;
            let result = ordinary_helper_transport_query_v1(&mut work, |charge| {
                charge(3)?;
                Err(error)
            });
            let Err(ProductionRankedProjectionErrorV1::StructuralValidation(actual)) = result
            else {
                panic!("a fatal query error was converted to eligibility")
            };
            assert_eq!(format!("{actual:?}"), expected);
            assert_eq!(work, 10);
        }
    }

    #[test]
    fn ordinary_summary_uses_one_module_counter_and_preserves_call_edge_boundaries() {
        let ssa = source_fixture(ResultShape::Array, BodyShape::Straight);
        let semantic = ssa.source_semantic();
        let leaf = &semantic.functions()[2];
        // One zero-argument, one-local, one-block array constructor:
        // argument admission 1 + shared result query 34 + local 1 + block
        // pre-scan 1 + block 1 + assignment 8 + Return 1 = 47 logical units.
        const LEAF_WORK: usize = 47;
        for (repeats, missing) in [(1, 0), (1, 1), (2, 0), (2, 1)] {
            let mut work = MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 - repeats * LEAF_WORK + missing;
            let mut edges = 0;
            for ordinal in 0..repeats {
                let result = ordinary_direct_defined_callable_summary_v1(
                    semantic.types(),
                    leaf,
                    semantic.functions().len(),
                    semantic.callables(),
                    &mut edges,
                    &mut work,
                );
                if missing == 1 && ordinal + 1 == repeats {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "defined-callable effect-summary work exceeded its production limit"
                        ))
                    ));
                } else {
                    let summary = result.unwrap().unwrap();
                    assert!(summary.empty_eligible);
                    assert!(!summary.deterministic_scalar_eligible);
                    assert!(summary.callees.is_empty());
                }
            }
            assert_eq!(work, MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 + missing);
            assert_eq!(edges, 0);
        }
        for missing in [0, 1] {
            let mut work = 0;
            let mut edges = MAX_DEFINED_CALLABLE_SUMMARY_EDGES_V1 - 1 + missing;
            let result = ordinary_direct_defined_callable_summary_v1(
                semantic.types(),
                &semantic.functions()[1],
                semantic.functions().len(),
                semantic.callables(),
                &mut edges,
                &mut work,
            );
            if missing == 0 {
                assert_eq!(result.unwrap().unwrap().callees, vec![2]);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "defined-callable effect-summary edge count exceeded its production limit"
                    ))
                ));
            }
            assert_eq!(edges, MAX_DEFINED_CALLABLE_SUMMARY_EDGES_V1 + missing);
        }
    }

    #[test]
    fn ordinary_summary_component_grammar_excludes_storage_addresses_and_unknown_calls() {
        let ssa = source_fixture(ResultShape::Array, BodyShape::Straight);
        let semantic = ssa.source_semantic();
        let leaf = &semantic.functions()[2];
        let grammar = OrdinaryHelperSummaryV1 {
            types: semantic.types(),
            function: leaf,
            function_count: semantic.functions().len(),
            callables: semantic.callables(),
        };
        // Unadmitted mutations isolate this grammar; they are not source proof.
        let mut work = 0;
        assert!(grammar.place(&whole(0, RESULT), true, &mut work).unwrap());
        assert!(!grammar.place(&component(0), true, &mut work).unwrap());
        assert!(!grammar.place(&component(0), false, &mut work).unwrap());
        assert!(!grammar.place(&whole(99, RESULT), false, &mut work).unwrap());
        for kind in [
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: whole(0, RESULT),
            },
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: whole(0, RESULT),
            },
            SemanticRvalueKindV1::Length(whole(0, RESULT)),
            SemanticRvalueKindV1::Discriminant(whole(0, RESULT)),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                component(0),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ] {
            assert!(
                !grammar
                    .rvalue(&SemanticRvalueV1::new(A_U32, kind), &mut work)
                    .unwrap()
            );
        }
        for kind in [
            SemanticStatementKindV1::Deinitialize(whole(0, RESULT)),
            SemanticStatementKindV1::SetDiscriminant {
                place: whole(0, RESULT),
                variant_index: 0,
            },
        ] {
            assert!(!grammar.statement(&statement(kind), &mut work).unwrap());
        }
        for callables in [
            vec![],
            vec![SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(99),
            )],
        ] {
            let invalid = OrdinaryHelperSummaryV1 {
                callables: &callables,
                ..grammar
            };
            let SemanticTerminatorKindV1::Call(call) = helper_call(0, 0, 0) else {
                panic!()
            };
            assert!(
                !invalid
                    .call(&call, &mut Vec::new(), &mut 0, &mut work)
                    .unwrap()
            );
        }
        let SemanticTerminatorKindV1::Call(mutated) = helper_call(1, 0, 0) else {
            panic!()
        };
        let aggregate_argument = SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(1),
            vec![SemanticOperandV1::Copy(whole(0, RESULT))],
            mutated.destination().cloned(),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        assert!(
            !grammar
                .call(&aggregate_argument, &mut Vec::new(), &mut 0, &mut work)
                .unwrap()
        );
    }
}
