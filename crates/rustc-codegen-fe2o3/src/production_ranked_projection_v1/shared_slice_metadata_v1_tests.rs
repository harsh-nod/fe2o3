mod shared_slice_metadata_tests {
    use super::*;

    const M_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
    const M_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

    fn types() -> Vec<SemanticTypeDeclV1> {
        let mut types = assertion_types();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(237)),
            SemanticLayoutIdentityV1::from_sha256(bytes(237)),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::array(4, 0),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(false),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Slice { element: A_U32 },
        ));
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(238)),
                SemanticLayoutIdentityV1::from_sha256(bytes(238)),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(16),
                    8,
                    SemanticBackendReprV1::scalar_pair(
                        SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                        ),
                        SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::integer(false, 64, 8),
                            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                        ),
                    ),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        M_SLICE,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::SliceLength,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            0,
                            4,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
        types
    }

    fn slice_value() -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            M_REF,
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        true,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    0,
                    Some(4),
                )
                .unwrap(),
                second: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            },
        )
    }

    fn abi(
        kernel: bool,
        ownership: SemanticSourceArgumentOwnershipV1,
        argument: SemanticAbiValueV1,
    ) -> SemanticFunctionAbiV1 {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(if kernel { 236 } else { 243 })),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(argument)],
            if kernel {
                SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore)
            } else {
                neutral_plain_direct_abi_value_v1(A_U64)
            },
        )
        .unwrap()
        .with_source_argument_ownership(vec![ownership])
        .unwrap()
    }

    fn rebuild(
        function: &SemanticFunctionDeclV1,
        abi: SemanticFunctionAbiV1,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        let function2 = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            function.locals().to_vec(),
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            function2.with_kernel_entry(entry.clone())
        } else {
            function2
        }
    }

    fn helper(tag: u8, blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi(
                false,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                slice_value(),
            ),
            vec![
                local(180, A_U64, SemanticLocalRoleV1::Return),
                local(181, M_REF, SemanticLocalRoleV1::Argument(0)),
                local(182, A_U32, SemanticLocalRoleV1::Temporary),
                local(183, A_U64, SemanticLocalRoleV1::Temporary),
                local(184, M_REF, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn length_statement(reference: u32) -> SemanticStatementV1 {
        typed_assignment(
            0,
            A_U64,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: typed_operand(reference, M_REF),
            },
        )
    }

    fn call(callee: u32, argument: u32, destination: u32, target: u32) -> SemanticTerminatorKindV1 {
        neutral_test_call_v1(
            callee,
            vec![typed_operand(argument, M_REF)],
            destination,
            A_U64,
            target,
        )
    }

    fn functions(forwarding: bool) -> Vec<SemanticFunctionDeclV1> {
        let root = assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (M_REF, SemanticLocalRoleV1::Argument(0)),
                (A_U64, SemanticLocalRoleV1::Temporary),
                (A_U64, SemanticLocalRoleV1::Temporary),
                (A_U64, SemanticLocalRoleV1::Temporary),
                (A_BOOL, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![M_REF],
            vec![
                block(
                    201,
                    vec![
                        typed_assignment(
                            3,
                            A_U64,
                            SemanticRvalueKindV1::Use(typed_constant(A_U64, 0, 8)),
                        ),
                        typed_assignment(
                            4,
                            A_U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: typed_operand(1, M_REF),
                            },
                        ),
                        typed_assignment(
                            5,
                            A_BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: typed_operand(3, A_U64),
                                right: typed_operand(4, A_U64),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_operand(5, A_BOOL),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: typed_operand(4, A_U64),
                            index: typed_operand(3, A_U64),
                        },
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    202,
                    vec![typed_assignment(
                        6,
                        A_U32,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(element())),
                    )],
                    call(1, 1, 2, 2),
                ),
                block(203, vec![], SemanticTerminatorKindV1::Return),
            ],
            false,
        );
        let root = rebuild(
            &root,
            abi(
                true,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                slice_value(),
            ),
            root.blocks().to_vec(),
        );
        if forwarding {
            vec![
                root,
                helper(
                    244,
                    vec![
                        block(
                            203,
                            vec![typed_assignment(
                                4,
                                M_REF,
                                SemanticRvalueKindV1::Use(typed_operand(1, M_REF)),
                            )],
                            call(2, 4, 0, 1),
                        ),
                        block(204, vec![], SemanticTerminatorKindV1::Return),
                    ],
                ),
                helper(
                    245,
                    vec![block(
                        205,
                        vec![length_statement(1)],
                        SemanticTerminatorKindV1::Return,
                    )],
                ),
            ]
        } else {
            vec![
                root,
                helper(
                    244,
                    vec![block(
                        203,
                        vec![length_statement(1)],
                        SemanticTerminatorKindV1::Return,
                    )],
                ),
            ]
        }
    }

    fn summaries(
        types: &[SemanticTypeDeclV1],
        functions: &[SemanticFunctionDeclV1],
    ) -> DefinedCallableEmptyEffectSummariesV1 {
        let callables = (0..functions.len())
            .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
            .collect::<Vec<_>>();
        derive_defined_callable_empty_effect_summaries_v1(types, functions, &callables).unwrap()
    }

    #[test]
    fn actual_memory_root_crosses_the_normal_projector_with_shared_slice_length_and_forwarding() {
        for forwarding in [false, true] {
            let functions = functions(forwarding);
            let source_helper_count = functions
                .iter()
                .filter(|function| function.role() == SemanticFunctionRoleV1::InternalHelper)
                .count();
            assert_eq!(source_helper_count, if forwarding { 2 } else { 1 });
            let summary = summaries(&types(), &functions);
            for index in 1..functions.len() {
                let id = SemanticFunctionIdV1::from_index(index as u32);
                assert!(summary.is_exact_empty(id));
                assert!(!summary.is_exact_empty_deterministic_scalar(id));
            }
            let source = assertion_materialized_functions(types(), functions);
            let module = source.executable().module();
            let helpers = module
                .functions
                .iter()
                .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
                .collect::<Vec<_>>();
            assert_eq!(helpers.len(), source_helper_count);
            assert!(helpers.iter().all(|helper| helper.body.is_some()));
            let root = &module.functions[0];
            assert!(
                root.body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(
                        operation.kind,
                        fe2o3_kernel_ir::OperationKind::Load { .. }
                    ))
            );
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let bound = dialect_amdgcn::bind_production_target_v1(module, profile).unwrap();
                let llvm = match profile {
                    Profile::Gfx942 => {
                        dialect_amdgcn::lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(
                            bound.module(),
                        )
                        .unwrap()
                    }
                    Profile::Gfx950 => {
                        dialect_amdgcn::lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(
                            bound.module(),
                        )
                        .unwrap()
                    }
                };
                assert!(llvm.contains(" = load i32, ptr addrspace(1) "));
                assert!(llvm.contains("br i1 "));
                assert!(llvm.contains("call void @llvm.trap()"));
                for helper in &helpers {
                    assert!(llvm.contains(&format!(
                        "define internal i64 @{}(ptr addrspace(1) %arg0.data, i64 %arg0.len)",
                        helper.id
                    )));
                    assert!(
                        llvm.contains(&format!(
                            "call i64 @{}(ptr addrspace(1) %arg0.data, i64 %arg0.len)",
                            helper.id
                        )) || (forwarding
                            && llvm
                                .contains(&format!("call i64 @{}(ptr addrspace(1) ", helper.id)))
                    );
                }
            }
            let program = assertion_project(source).unwrap();
            let [root] = program.roots() else {
                panic!("one genuine memory root")
            };
            assert!(!root.access_sources.is_empty());
            assert!(root.bounds_are_clean());
            assert!(root.all_kernel_checks_are_clean());
        }
    }

    fn element() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, M_SLICE).unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                    A_U32,
                )
                .unwrap(),
            ],
            A_U32,
        )
        .unwrap()
    }

    #[test]
    fn metadata_summary_rejects_element_reads_writes_borrows_assertions_and_recursive_forwarding() {
        let load = typed_assignment(
            2,
            A_U32,
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                element(),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        let projected = typed_assignment(
            2,
            A_U32,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(element())),
        );
        let store = statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            element(),
            typed_constant(A_U32, 9, 4),
            SemanticVolatilityV1::NonVolatile,
            None,
        )));
        let borrow = typed_assignment(
            4,
            M_REF,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, M_SLICE)
                            .unwrap(),
                    ],
                    M_SLICE,
                )
                .unwrap(),
            },
        );
        for rejected in [load, projected, store, borrow] {
            let mut functions = functions(true);
            let callee = functions[2].clone();
            functions[2] = rebuild(
                &callee,
                callee.abi().clone(),
                vec![block(
                    205,
                    vec![
                        typed_assignment(
                            3,
                            A_U64,
                            SemanticRvalueKindV1::Use(typed_constant(A_U64, 0, 8)),
                        ),
                        length_statement(1),
                        rejected,
                    ],
                    SemanticTerminatorKindV1::Return,
                )],
            );
            let summary = summaries(&types(), &functions);
            assert!(!summary.is_exact_empty(SemanticFunctionIdV1::from_index(1)));
            assert!(!summary.is_exact_empty(SemanticFunctionIdV1::from_index(2)));
            let callables = vec![SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(1),
            )];
            assert!(
                require_bounds_neutral_callable(
                    &callables,
                    &summary,
                    SemanticCallableIdV1::from_index(0),
                    0,
                    SemanticSourceProvenanceV1::unavailable(),
                    false
                )
                .is_err()
            );
        }
        let mut recursive = functions(true);
        let leaf = recursive[2].clone();
        recursive[2] = rebuild(
            &leaf,
            leaf.abi().clone(),
            vec![
                block(205, vec![], call(1, 1, 0, 1)),
                block(206, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        let summary = summaries(&types(), &recursive);
        assert!(!summary.is_exact_empty(SemanticFunctionIdV1::from_index(1)));
        assert!(!summary.is_exact_empty(SemanticFunctionIdV1::from_index(2)));
        let mut asserted = functions(false);
        let leaf = asserted[1].clone();
        asserted[1] = rebuild(
            &leaf,
            leaf.abi().clone(),
            vec![
                block(
                    205,
                    vec![length_statement(1)],
                    assertion_terminator(typed_constant(A_BOOL, 0, 1), true, 1),
                ),
                block(206, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        assert!(
            !summaries(&types(), &asserted).is_exact_empty(SemanticFunctionIdV1::from_index(1))
        );
    }

    #[test]
    fn shared_slice_summary_keeps_exact_pair_ownership_and_immutable_reference_checks() {
        let types = types();
        let function = functions(false).remove(1);
        for (ownership, value) in [
            (SemanticSourceArgumentOwnershipV1::ByValue, slice_value()),
            (
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                slice_value(),
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                neutral_plain_direct_abi_value_v1(M_REF),
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                slice_value().with_pointee_override(
                    SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                ),
            ),
        ] {
            let hostile = rebuild(
                &function,
                abi(false, ownership, value),
                function.blocks().to_vec(),
            );
            assert!(!immutable_metadata_slice_abi_v1(&types, &hostile, &mut 0).unwrap());
        }
        for (kind, mutability, address_space) in [
            (
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                1,
            ),
        ] {
            let mut hostile = types.clone();
            let original = &types[M_REF.index() as usize];
            hostile[M_REF.index() as usize] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        M_SLICE,
                        kind,
                        mutability,
                        address_space,
                        64,
                        SemanticPointerMetadataV1::SliceLength,
                    )
                    .unwrap(),
                ),
            );
            assert!(!immutable_metadata_slice_abi_v1(&hostile, &function, &mut 0).unwrap());
        }
    }

    #[test]
    fn actual_source_helper_read_still_fails_the_existing_materialization_purity_gate() {
        let mut functions = functions(true);
        let leaf = functions[2].clone();
        functions[2] = rebuild(
            &leaf,
            leaf.abi().clone(),
            vec![block(
                205,
                vec![
                    typed_assignment(
                        3,
                        A_U64,
                        SemanticRvalueKindV1::Use(typed_constant(A_U64, 0, 8)),
                    ),
                    length_statement(1),
                    typed_assignment(
                        2,
                        A_U32,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(element())),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        );
        let ssa = assertion_ssa_functions(types(), functions);
        let launch = source_launch_roster_for_ranked_inputs_v1(
            &ssa,
            &[ranked_root_input_1d(A_NAME, 247, 64)],
        )
        .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(19).unwrap();
        let result = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        );
        assert!(
            matches!(
                &result,
                Err(
                    fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Lowering(
                        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                            detail: "reachable deterministic scalar helper is not interprocedurally complete and pure",
                            ..
                        }
                    )
                )
            ),
            "expected helper purity refusal, got {result:?}"
        );
        assert_eq!(budget.storage(), 19);
        assert!(budget.work() >= 7);
    }

    #[test]
    fn metadata_component_prepays_its_fixed_descriptor_scan_without_resetting_work() {
        let types = types();
        let function = functions(false).remove(1);
        // One pair argument and five locals: 1 + 4 * 1 + 3 * 5 = 20 units.
        let mut exact = MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 - 20;
        assert!(immutable_metadata_slice_abi_v1(&types, &function, &mut exact).unwrap());
        assert_eq!(exact, MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1);
        let mut under = MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 - 19;
        assert!(immutable_metadata_slice_abi_v1(&types, &function, &mut under).is_err());
        assert_eq!(under, MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1 + 1);
    }
}
