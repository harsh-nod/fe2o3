mod ordinary_helper_result_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiCastV1, SemanticAbiRegisterV1, SemanticAbiUniformV1,
        SemanticAbiValueAttributesV1, SemanticAggregateRvalueV1, SemanticPaddingV1,
        SemanticPointerTypeV1,
    };

    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

    fn source() -> SemanticSourceProvenanceV1 {
        SemanticSourceProvenanceV1::unavailable()
    }

    fn types() -> Vec<SemanticTypeDeclV1> {
        let u32_type = plain_bit_scalar_type(
            11,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        );
        let u64_type = plain_bit_scalar_type(
            21,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        );
        let SemanticBackendReprV1::Scalar(u32_scalar) = u32_type.layout().backend_repr() else {
            panic!()
        };
        let SemanticBackendReprV1::Scalar(u64_scalar) = u64_type.layout().backend_repr() else {
            panic!()
        };
        let array = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([31; 32]),
            SemanticLayoutIdentityV1::from_sha256([32; 32]),
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
                element: U32,
                length: 2,
            },
        );
        let tuple = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([41; 32]),
            SemanticLayoutIdentityV1::from_sha256([42; 32]),
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
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U64]).unwrap()),
        );
        vec![unit_type(), u32_type, u64_type, array, tuple]
    }

    fn integer_cast(size: u64, pad: bool) -> SemanticAbiPassModeV1 {
        SemanticAbiPassModeV1::cast(
            pad,
            SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, size).unwrap(),
                    size,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        )
    }

    fn function(ty: SemanticTypeIdV1, mode: SemanticAbiPassModeV1) -> SemanticFunctionDeclV1 {
        let owner = helper_closure_semantic_owner();
        let helper = &owner.semantic().functions()[1];
        SemanticFunctionDeclV1::new(
            helper.identity(),
            helper.role(),
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            source(),
            SemanticFunctionAbiV1::from_rustc(
                helper.abi().identity(),
                helper.abi().layout_identity(),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                0,
                vec![],
                SemanticAbiValueV1::new(ty, mode),
            )
            .unwrap(),
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([217; 32]),
                ty,
                SemanticLocalRoleV1::Return,
                source(),
            )],
            SemanticBlockIdV1::from_index(0),
            helper.blocks().to_vec(),
        )
        .unwrap()
    }

    fn plan(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> HelperResultTransportV1 {
        plan_ordinary_helper_result_v1(
            types,
            function,
            &mut fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000),
            10_000,
        )
        .unwrap()
    }

    #[test]
    fn arrays_and_physically_reordered_tuples_preserve_logical_result_order() {
        let types = types();
        let array = function(ARRAY, integer_cast(8, false));
        let array_plan = plan(&types, &array);
        assert_eq!(array_plan.types(), vec![Type::Scalar(ScalarType::U32); 2]);
        let source = array_plan.ordinary().unwrap();
        assert_eq!(source.ty, ARRAY);
        assert_eq!(source.layout, types[3].layout_identity());
        assert_eq!(source.abi, array.abi().identity());
        let shared = array_plan.clone();
        match (&array_plan, &shared) {
            (
                HelperResultTransportV1::Ordinary(first),
                HelperResultTransportV1::Ordinary(second),
            ) => {
                assert!(std::rc::Rc::ptr_eq(first, second));
                assert_eq!(first.types.as_ptr(), second.types.as_ptr());
            }
            _ => panic!("ordinary plan clone changed representation"),
        }

        let tuple = function(
            TUPLE,
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::plain(),
                second: SemanticAbiValueAttributesV1::plain(),
            },
        );
        assert_eq!(
            plan(&types, &tuple).types(),
            vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64)]
        );
    }

    #[test]
    fn ordinary_result_grammar_does_not_flatten_nominal_pointer_enum_or_empty_array_carriers() {
        let types = types();
        for shape in [
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    UNIT,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
            SemanticTypeShapeV1::Enum {
                discriminant: U32,
                variants: Vec::new().into_boxed_slice(),
            },
        ] {
            let mut hostile = types.clone();
            let old = &types[1];
            hostile[1] = SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                old.layout().clone(),
                shape,
            );
            assert!(
                plan_ordinary_helper_result_v1(
                    &hostile,
                    &function(ARRAY, integer_cast(8, false)),
                    &mut fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000),
                    10_000
                )
                .is_err()
            );
            let old = &types[3];
            hostile[3] = SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    4,
                    SemanticFieldsShapeV1::array(4, 0),
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
                    element: U32,
                    length: 0,
                },
            );
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
            assert!(
                walk_ordinary_helper_result_v1(
                    &hostile,
                    ARRAY,
                    0,
                    &mut OrdinaryHelperResultWalkV1 {
                        work: &mut |amount| {
                            charge_ordinary_helper_result_work_v1(&mut work, amount)
                        },
                        nodes: 0
                    },
                    &mut |_| Ok(())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn ordinary_tuple_units_do_not_create_abi_or_ssa_components() {
        let mut types = types();
        let SemanticBackendReprV1::Scalar(scalar) = types[1].layout().backend_repr() else {
            panic!()
        };
        let tuple = &types[4];
        types[4] = SemanticTypeDeclV1::new(
            tuple.identity(),
            tuple.layout_identity(),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                4,
                4,
                SemanticFieldsShapeV1::arbitrary(vec![0, 0], vec![0, 1]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::scalar(*scalar),
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
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![UNIT, U32]).unwrap()),
        );
        let plan = plan(
            &types,
            &function(
                TUPLE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            ),
        );
        assert_eq!(plan.types(), &[Type::Scalar(ScalarType::U32)]);
        let binding = binding_from_value_defs(
            &types,
            TUPLE,
            &[ValueDef::new(ValueId(17), Type::Scalar(ScalarType::U32))],
        )
        .unwrap();
        ordinary_helper_result_binding_v1(&types, TUPLE, &binding, &mut 0).unwrap();
        let SemanticValueBindingV1::Aggregate(fields) = binding else {
            panic!()
        };
        assert!(matches!(fields[0], SemanticValueBindingV1::Unit));
        assert_eq!(fields[1].clone().value().unwrap().0, ValueId(17));
    }

    #[test]
    fn ordinary_result_abi_malformed_modes_and_layouts_remain_closed() {
        let types = types();
        for mode in [
            integer_cast(4, false),
            integer_cast(8, true),
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::plain(),
                second: SemanticAbiValueAttributesV1::plain(),
            },
            SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::plain(),
                metadata_attributes: None,
                on_stack: false,
            },
        ] {
            assert!(
                plan_ordinary_helper_result_v1(
                    &types,
                    &function(ARRAY, mode),
                    &mut fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000),
                    10_000
                )
                .is_err()
            );
        }
        let mut hostile = types.clone();
        let old = &types[3];
        hostile[3] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            old.layout().clone(),
            SemanticTypeShapeV1::Array {
                element: U32,
                length: 3,
            },
        );
        assert!(
            plan_ordinary_helper_result_v1(
                &hostile,
                &function(ARRAY, integer_cast(8, false)),
                &mut fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000),
                10_000
            )
            .is_err()
        );
    }

    #[test]
    fn planner_has_independent_work_storage_and_first_failure_boundaries() {
        let types = types();
        let function = function(ARRAY, integer_cast(8, false));
        // One Return local, three visited nodes per pass, two scalar leaves:
        // 16 ABI + 1 local + 3 nodes + 3 allocations + 3 nodes + 2 * (2 + 2)
        // + 1 immutable plan header = 35 work; three two-slot Vecs + 1 header = 7 rows.
        let mut exact = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7 + 35);
        exact.charge_work(7).unwrap();
        plan_ordinary_helper_result_v1(&types, &function, &mut exact, 7).unwrap();
        assert_eq!(exact.work(), 42);
        assert_eq!(exact.failed_work(), None);
        let mut under = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7 + 34);
        under.charge_work(7).unwrap();
        assert!(matches!(
            plan_ordinary_helper_result_v1(&types, &function, &mut under, 7),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 42,
                limit: 41
            })
        ));
        assert_eq!(under.work(), 41);
        assert_eq!(under.failed_work(), Some(42));
        assert!(charge_ordinary_helper_result_work_v1(&mut under, 0).is_err());
        assert_eq!(under.work(), 41);
        let mut storage = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        storage.charge_work(7).unwrap();
        assert!(matches!(
            plan_ordinary_helper_result_v1(&types, &function, &mut storage, 6),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: 7,
                limit: 6
            })
        ));
        assert_eq!(storage.work(), 27);
        assert!(ordinary_helper_result_storage_v1([usize::MAX, 1, 0], usize::MAX).is_err());
        // These are planner-only boundaries, not graph/origin canonical receipts.
    }

    #[test]
    fn ordinary_result_structural_limit_is_checked_before_component_allocation() {
        let mut types = types();
        let old = &types[3];
        types[3] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                1024,
                4,
                SemanticFieldsShapeV1::array(4, 256),
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
                element: U32,
                length: 256,
            },
        );
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        assert!(matches!(
            plan_ordinary_helper_result_v1(
                &types,
                &function(ARRAY, integer_cast(1024, false)),
                &mut work,
                1000
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "ordinary helper result exceeds the structural limit",
                ..
            })
        ));
        assert_eq!(work.work(), 16 + 1 + 257);
    }

    #[test]
    fn successive_plans_share_one_work_budget_without_resetting_the_prefix() {
        let types = types();
        let function = function(ARRAY, integer_cast(8, false));
        // Each fixed two-leaf fixture independently costs 35, including its header.
        let mut exact = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(70);
        let first = plan_ordinary_helper_result_v1(&types, &function, &mut exact, 7).unwrap();
        let second = plan_ordinary_helper_result_v1(&types, &function, &mut exact, 7).unwrap();
        assert_eq!(first.types(), second.types());
        assert_eq!(exact.work(), 70);
        let mut under = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(69);
        plan_ordinary_helper_result_v1(&types, &function, &mut under, 7).unwrap();
        assert!(matches!(
            plan_ordinary_helper_result_v1(&types, &function, &mut under, 7),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 70,
                limit: 69,
            })
        ));
        assert_eq!(under.work(), 69);
        assert_eq!(under.failed_work(), Some(70));
    }

    #[test]
    fn existing_ignored_and_scalar_helper_plans_do_not_pay_or_change_transport() {
        let types = types();
        for (ty, mode, expected) in [
            (UNIT, SemanticAbiPassModeV1::Ignore, vec![]),
            (
                U32,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                vec![Type::Scalar(ScalarType::U32)],
            ),
        ] {
            let function = function(ty, mode);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7);
            work.charge_work(7).unwrap();
            let plan = direct_scalar_helper_plan_v1(
                &types,
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(1),
                &function,
                FunctionId::new("ordinary_test_helper"),
                &mut work,
                0,
            )
            .unwrap();
            assert_eq!(plan.result_transport.types(), expected);
            assert!(plan.result_transport.ordinary().is_none());
            assert_eq!(work.work(), 7);
            assert_eq!(work.failed_work(), None);
        }
    }

    #[test]
    fn inverse_components_reject_truncation_trailing_type_and_capability_shape_changes() {
        let types = types();
        let values = vec![
            ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32)),
            ValueDef::new(ValueId(29), Type::Scalar(ScalarType::U32)),
        ];
        let binding = binding_from_value_defs(&types, ARRAY, &values).unwrap();
        ordinary_helper_result_binding_v1(&types, ARRAY, &binding, &mut 0).unwrap();
        assert_eq!(
            binding.values().unwrap(),
            vec![
                (ValueId(11), Type::Scalar(ScalarType::U32)),
                (ValueId(29), Type::Scalar(ScalarType::U32))
            ]
        );
        assert!(binding_from_value_defs(&types, ARRAY, &values[..1]).is_err());
        let mut changed = values.clone();
        changed.push(values[0].clone());
        assert!(binding_from_value_defs(&types, ARRAY, &changed).is_err());
        changed = values;
        changed[1].ty = Type::Scalar(ScalarType::U64);
        assert!(binding_from_value_defs(&types, ARRAY, &changed).is_err());
        assert!(
            ordinary_helper_result_binding_v1(
                &types,
                ARRAY,
                &SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Unit; 2]),
                &mut 0
            )
            .is_err()
        );
    }

    fn local(index: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([index; 32]),
            ty,
            role,
            source(),
        )
    }
    fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
    }
    fn block(
        tag: u8,
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source(),
            statements,
            SemanticTerminatorV1::new(source(), terminator),
        )
        .unwrap()
    }
    fn call(callee: u32, destination: u32) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(destination, ARRAY),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    #[test]
    fn admitted_source_forwarding_uses_one_shared_array_plan_for_call_and_return() {
        let old = helper_closure_semantic_owner();
        let root = &old.semantic().functions()[0];
        let root = SemanticFunctionDeclV1::new(
            root.identity(),
            root.role(),
            root.item_definition_identity(),
            root.monomorphization_identity(),
            root.generic_type_arguments_identity(),
            root.const_generic_arguments_identity(),
            source(),
            root.abi().clone(),
            vec![
                local(207, UNIT, SemanticLocalRoleV1::Return),
                local(208, ARRAY, SemanticLocalRoleV1::Temporary),
                local(209, U64, SemanticLocalRoleV1::Temporary),
                local(210, TUPLE, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![
                block(208, vec![], call(1, 1)),
                block(209, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap()
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
        let helper = function(ARRAY, integer_cast(8, false));
        let forwarding = SemanticFunctionDeclV1::new(
            helper.identity(),
            helper.role(),
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            source(),
            helper.abi().clone(),
            helper.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            vec![
                block(218, vec![], call(2, 0)),
                block(219, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        let scalar = |value| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                U32,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
            ))
        };
        let assignment = SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0, ARRAY),
                SemanticRvalueV1::new(
                    ARRAY,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Array,
                            vec![scalar(11), scalar(29)],
                        )
                        .unwrap(),
                    ),
                ),
            )),
        );
        let leaf = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([221; 32]),
            helper.role(),
            SemanticItemDefinitionIdentityV1::from_sha256([222; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([224; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([225; 32]),
            source(),
            helper.abi().clone(),
            helper.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            vec![block(
                226,
                vec![assignment],
                SemanticTerminatorKindV1::Return,
            )],
        )
        .unwrap();
        let admitted = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            types(),
            vec![],
            vec![],
            vec![],
            vec![root, forwarding, leaf],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let (module, correspondence) =
            lower_module(&ssa, ProductionSemanticKirLimitsV1::default(), None).unwrap();
        verify_module(&module).unwrap();
        assert_eq!(correspondence.lowered_functions().len(), 3);
        let helpers = module
            .functions
            .iter()
            .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .collect::<Vec<_>>();
        assert_eq!(helpers.len(), 2);
        for helper in helpers {
            assert_eq!(
                helper.signature.results,
                vec![Type::Scalar(ScalarType::U32); 2]
            );
            for block in &helper.body.as_ref().unwrap().blocks {
                if let Some(Terminator::Return { values }) = &block.terminator {
                    assert_eq!(values.len(), 2);
                }
            }
        }
        let calls = module
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|operation| operation.results.len() == 2));
        let canonical_kernel_ir =
            ProductionCanonicalKernelIrV1::from_module(module.clone()).unwrap();
        let mut owner = ProductionSemanticKirOwnerV1 {
            semantic_ssa: ssa,
            module: RetainedProductionKirModuleV1::Legacy(module),
            canonical_kernel_ir,
            correspondence,
            limits: ProductionSemanticKirLimitsV1::default(),
            launch_roots: None,
            generic_checks: Vec::new().into_boxed_slice(),
        };
        owner.verify_equivalence().unwrap();
        let RetainedProductionKirModuleV1::Legacy(changed) = &mut owner.module else {
            panic!()
        };
        let values = changed
            .functions
            .iter_mut()
            .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .filter_map(|function| function.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .find_map(|block| match block.terminator.as_mut() {
                Some(Terminator::Return { values }) if values.len() == 2 => Some(values),
                _ => None,
            })
            .unwrap();
        assert_ne!(values[0], values[1]);
        values.swap(0, 1);
        verify_module(changed).unwrap();
        owner.canonical_kernel_ir =
            ProductionCanonicalKernelIrV1::from_module(changed.clone()).unwrap();
        assert!(matches!(
            owner.verify_equivalence(),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        // This is source/KIR transport coverage, not LLVM or a memory-effect/normal-projector claim.
    }

    include!("production_ordinary_helper_result_source_v1_tests.rs");
    include!("production_ordinary_helper_result_query_v1_tests.rs");
}
