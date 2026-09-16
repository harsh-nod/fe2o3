mod admitted_source_transport_v1 {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiExtensionV1, SemanticAbiRegularAttributesV1,
    };

    fn initialized_scalar_attributes() -> SemanticAbiValueAttributesV1 {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap()
    }

    fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
        let ty = destination.ty();
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    }

    fn scalar(ty: SemanticTypeIdV1, value: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(value, if ty == U64 { 8 } else { 4 }).unwrap(),
            ),
        ))
    }

    fn aggregate(
        destination: SemanticPlaceV1,
        kind: SemanticAggregateKindV1,
        operands: Vec<SemanticOperandV1>,
    ) -> SemanticStatementV1 {
        assign(
            destination,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(kind, operands).unwrap(),
            ),
        )
    }

    fn field(local: u32, index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap()
    }

    fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
    }

    fn call_to(callee: u32, destination: SemanticPlaceV1) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    destination,
                    edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn rebuild(
        old: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        let result = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source(),
            old.abi().clone(),
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        match old.kernel_entry() {
            Some(entry) => result.with_kernel_entry(entry.clone()),
            None => result,
        }
    }

    fn leaf(
        template: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([221; 32]),
            template.role(),
            SemanticItemDefinitionIdentityV1::from_sha256([222; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([224; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([225; 32]),
            source(),
            template.abi().clone(),
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn admit(
        catalog: Vec<SemanticTypeDeclV1>,
        functions: Vec<SemanticFunctionDeclV1>,
    ) -> ProductionSemanticMirOwnerV1 {
        let admitted = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            catalog,
            vec![],
            vec![],
            vec![],
            functions,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap()
    }

    fn lower(
        catalog: Vec<SemanticTypeDeclV1>,
        functions: Vec<SemanticFunctionDeclV1>,
    ) -> ProductionSemanticKirOwnerV1 {
        let owner = ProductionSemanticKirOwnerV1::try_lower(
            admit(catalog, functions),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        owner.verify_equivalence().unwrap();
        verify_module(owner.module()).unwrap();
        owner
    }

    fn forwarding_chain(
        ty: SemanticTypeIdV1,
        mode: SemanticAbiPassModeV1,
        root_locals: Vec<SemanticLocalDeclV1>,
        root_continuation: Vec<SemanticStatementV1>,
        leaf_locals: Vec<SemanticLocalDeclV1>,
        leaf_blocks: Vec<SemanticBasicBlockV1>,
    ) -> Vec<SemanticFunctionDeclV1> {
        let base = helper_closure_semantic_owner();
        let root = rebuild(
            &base.semantic().functions()[0],
            root_locals,
            vec![
                block(208, vec![], call_to(1, place(1, ty))),
                block(209, root_continuation, SemanticTerminatorKindV1::Return),
            ],
        );
        let helper = function(ty, mode);
        let forwarding = rebuild(
            &helper,
            helper.locals().to_vec(),
            vec![
                block(218, vec![], call_to(2, place(0, ty))),
                block(219, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        vec![root, forwarding, leaf(&helper, leaf_locals, leaf_blocks)]
    }

    fn calls(function: &fe2o3_kernel_ir::Function) -> Vec<&Operation> {
        function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .collect()
    }

    fn returned_constants(function: &fe2o3_kernel_ir::Function) -> Vec<Constant> {
        let body = function.body.as_ref().unwrap();
        let values = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Some(Terminator::Return { values }) => Some(values),
                _ => None,
            })
            .unwrap();
        values
            .iter()
            .map(|value| {
                body.blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find_map(|operation| match &operation.kind {
                        OperationKind::Constant(constant)
                            if operation
                                .results
                                .first()
                                .is_some_and(|result| result.id == *value) =>
                        {
                            Some(constant.clone())
                        }
                        _ => None,
                    })
                    .unwrap()
            })
            .collect()
    }

    fn single_predecessor_origin(
        function: &fe2o3_kernel_ir::Function,
        mut value: ValueId,
    ) -> ValueId {
        let body = function.body.as_ref().unwrap();
        for _ in 0..body.blocks.len() {
            let Some((block, index)) = body.blocks.iter().find_map(|block| {
                block
                    .parameters
                    .iter()
                    .position(|parameter| parameter.id == value)
                    .map(|index| (block, index))
            }) else {
                return value;
            };
            let incoming = body
                .blocks
                .iter()
                .filter_map(|predecessor| match &predecessor.terminator {
                    Some(Terminator::Branch { target, arguments }) if *target == block.id => {
                        Some(arguments[index])
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                incoming.len(),
                1,
                "fixture requires one actual predecessor for this component"
            );
            value = incoming[0];
        }
        panic!("fixture component forwarding is cyclic")
    }

    fn assert_forwarding(owner: &ProductionSemanticKirOwnerV1, expected: &[Type]) {
        assert_eq!(owner.correspondence().lowered_functions().len(), 3);
        let module = owner.module();
        assert_eq!(module.functions.len(), 3);
        for helper in &module.functions[1..] {
            assert_eq!(helper.signature.results, expected);
        }
        for caller in &module.functions[..2] {
            let calls = calls(caller);
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0]
                    .results
                    .iter()
                    .map(|value| value.ty.clone())
                    .collect::<Vec<_>>(),
                expected,
            );
        }
        let forwarding = &module.functions[1];
        let call = calls(forwarding)[0];
        let expected_values = call
            .results
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>();
        let returns = forwarding
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                Some(Terminator::Return { values }) => Some(values),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(returns.len(), 1);
        assert_eq!(
            returns[0]
                .iter()
                .map(|value| single_predecessor_origin(forwarding, *value))
                .collect::<Vec<_>>(),
            expected_values
        );
    }

    #[test]
    fn admitted_reordered_tuple_call_return_and_field_uses_keep_logical_order() {
        let catalog = types();
        assert!(matches!(
            catalog[TUPLE.index() as usize].layout().details(),
            SemanticTypeLayoutDetailsV1::Aggregate(layout) if layout.field_offsets() == [8, 0]
        ));
        let chain = forwarding_chain(
            TUPLE,
            SemanticAbiPassModeV1::Pair {
                first: initialized_scalar_attributes(),
                second: initialized_scalar_attributes(),
            },
            vec![
                local(207, UNIT, SemanticLocalRoleV1::Return),
                local(208, TUPLE, SemanticLocalRoleV1::Temporary),
                local(209, U32, SemanticLocalRoleV1::Temporary),
                local(210, U64, SemanticLocalRoleV1::Temporary),
                local(211, U64, SemanticLocalRoleV1::Temporary),
                // Keep the shared catalog's ARRAY reachable to exact type admission.
                local(212, ARRAY, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, 0, U32))),
                ),
                assign(
                    place(3, U64),
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: SemanticOperandV1::Copy(place(2, U32)),
                    },
                ),
                assign(
                    place(4, U64),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Subtract,
                        left: SemanticOperandV1::Copy(place(3, U64)),
                        right: SemanticOperandV1::Copy(field(1, 1, U64)),
                    },
                ),
            ],
            vec![local(217, TUPLE, SemanticLocalRoleV1::Return)],
            vec![block(
                226,
                vec![aggregate(
                    place(0, TUPLE),
                    SemanticAggregateKindV1::Tuple,
                    vec![scalar(U32, 11), scalar(U64, 29)],
                )],
                SemanticTerminatorKindV1::Return,
            )],
        );
        let owner = lower(catalog, chain);
        assert_forwarding(
            &owner,
            &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64)],
        );
        assert_eq!(
            returned_constants(&owner.module().functions[2]),
            vec![Constant::U32(11), Constant::U64(29)]
        );
        let root = &owner.module().functions[0];
        let operations = root
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::Cast {
                        to: Type::Scalar(ScalarType::U64),
                        ..
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Subtract,
                        ..
                    }
                ))
                .count(),
            1
        );
        let call = calls(root)[0];
        let cast = operations
            .iter()
            .find(|operation| matches!(operation.kind, OperationKind::Cast { .. }))
            .unwrap();
        let OperationKind::Cast { value, .. } = cast.kind else {
            panic!()
        };
        assert_eq!(single_predecessor_origin(root, value), call.results[0].id);
        let binary = operations
            .iter()
            .find(|operation| matches!(operation.kind, OperationKind::Binary { .. }))
            .unwrap();
        let OperationKind::Binary { lhs, rhs, .. } = binary.kind else {
            panic!()
        };
        assert_eq!(lhs, cast.results[0].id);
        assert_eq!(single_predecessor_origin(root, rhs), call.results[1].id);
    }

    #[test]
    fn admitted_unit_tuple_omits_only_the_zero_bit_component_across_calls() {
        let mut catalog = types();
        let scalar_repr = *catalog[U32.index() as usize].layout().backend_repr();
        let old = &catalog[TUPLE.index() as usize];
        catalog[TUPLE.index() as usize] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                4,
                4,
                SemanticFieldsShapeV1::arbitrary(vec![0, 0], vec![0, 1]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                scalar_repr,
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
        let chain = forwarding_chain(
            TUPLE,
            SemanticAbiPassModeV1::Direct(initialized_scalar_attributes()),
            vec![
                local(207, UNIT, SemanticLocalRoleV1::Return),
                local(208, TUPLE, SemanticLocalRoleV1::Temporary),
                local(209, UNIT, SemanticLocalRoleV1::Temporary),
                local(210, U32, SemanticLocalRoleV1::Temporary),
                local(211, U64, SemanticLocalRoleV1::Temporary),
                local(212, ARRAY, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                assign(
                    place(2, UNIT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, 0, UNIT))),
                ),
                assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Subtract,
                        left: SemanticOperandV1::Copy(field(1, 1, U32)),
                        right: scalar(U32, 5),
                    },
                ),
            ],
            vec![local(217, TUPLE, SemanticLocalRoleV1::Return)],
            vec![block(
                226,
                vec![aggregate(
                    place(0, TUPLE),
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            UNIT,
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        scalar(U32, 17),
                    ],
                )],
                SemanticTerminatorKindV1::Return,
            )],
        );
        let owner = lower(catalog, chain);
        assert_forwarding(&owner, &[Type::Scalar(ScalarType::U32)]);
        assert_eq!(
            returned_constants(&owner.module().functions[2]),
            vec![Constant::U32(17)]
        );
        let operations = owner.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Subtract,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert!(operations.iter().all(|operation| {
            operation
                .results
                .iter()
                .all(|result| result.ty != Type::Unit)
        }));
        let root = &owner.module().functions[0];
        let binary = operations
            .iter()
            .find(|operation| matches!(operation.kind, OperationKind::Binary { .. }))
            .unwrap();
        let OperationKind::Binary { lhs, .. } = binary.kind else {
            panic!()
        };
        assert_eq!(
            single_predecessor_origin(root, lhs),
            calls(root)[0].results[0].id
        );
    }

    fn array_value(first: u128, second: u128) -> SemanticStatementV1 {
        aggregate(
            place(0, ARRAY),
            SemanticAggregateKindV1::Array,
            vec![scalar(U32, first), scalar(U32, second)],
        )
    }

    fn switch_counter(equal: u32, otherwise: u32) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(1, U32)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, equal),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
            )
            .unwrap(),
        }
    }

    #[test]
    fn admitted_array_results_cross_join_and_loop_edges_as_scalar_components() {
        for loop_case in [false, true] {
            let leaf_blocks = if loop_case {
                vec![
                    block(
                        226,
                        vec![
                            array_value(11, 29),
                            assign(place(1, U32), SemanticRvalueKindV1::Use(scalar(U32, 0))),
                        ],
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                    ),
                    block(227, vec![], switch_counter(2, 3)),
                    block(
                        228,
                        vec![
                            array_value(31, 47),
                            assign(place(1, U32), SemanticRvalueKindV1::Use(scalar(U32, 1))),
                        ],
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                    ),
                    block(229, vec![], SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![
                    block(
                        226,
                        vec![assign(
                            place(1, U32),
                            SemanticRvalueKindV1::Use(scalar(U32, 0)),
                        )],
                        switch_counter(1, 2),
                    ),
                    block(
                        227,
                        vec![array_value(11, 29)],
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    block(
                        228,
                        vec![array_value(31, 47)],
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    block(229, vec![], SemanticTerminatorKindV1::Return),
                ]
            };
            let chain = forwarding_chain(
                ARRAY,
                integer_cast(8, false),
                vec![
                    local(207, UNIT, SemanticLocalRoleV1::Return),
                    local(208, ARRAY, SemanticLocalRoleV1::Temporary),
                    local(209, TUPLE, SemanticLocalRoleV1::Temporary),
                ],
                vec![],
                vec![
                    local(217, ARRAY, SemanticLocalRoleV1::Return),
                    local(218, U32, SemanticLocalRoleV1::Temporary),
                ],
                leaf_blocks,
            );
            let owner = lower(types(), chain);
            assert_forwarding(
                &owner,
                &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            );
            let body = owner.module().functions[2].body.as_ref().unwrap();
            let joined = &body.blocks[if loop_case { 1 } else { 3 }];
            assert_eq!(joined.parameters.len(), if loop_case { 3 } else { 2 });
            assert!(
                joined
                    .parameters
                    .iter()
                    .all(|value| value.ty == Type::Scalar(ScalarType::U32))
            );
            let incoming = body
                .blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    Some(Terminator::Branch { target, arguments }) if *target == joined.id => {
                        Some(arguments)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(incoming.len(), 2);
            assert!(
                incoming
                    .iter()
                    .all(|arguments| arguments.len() == joined.parameters.len())
            );
            assert_ne!(incoming[0], incoming[1]);
            assert!(body.blocks.iter().flat_map(|block| &block.operations).all(
                |operation| !matches!(
                    operation.kind,
                    OperationKind::Alloca { .. }
                        | OperationKind::Load { .. }
                        | OperationKind::Store { .. }
                )
            ));
            assert!(
                body.blocks
                    .iter()
                    .filter_map(|block| match &block.terminator {
                        Some(Terminator::Return { values }) => Some(values),
                        _ => None,
                    })
                    .all(|values| values.len() == 2)
            );
        }
    }

    #[test]
    fn admitted_retained_array_call_destination_reaches_the_explicit_transport_refusal() {
        let indexed = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 2,
                        from_end: false,
                    },
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap();
        let chain = forwarding_chain(
            ARRAY,
            integer_cast(8, false),
            vec![
                local(207, UNIT, SemanticLocalRoleV1::Return),
                local(208, ARRAY, SemanticLocalRoleV1::Temporary),
                local(209, TUPLE, SemanticLocalRoleV1::Temporary),
            ],
            vec![assign(indexed, SemanticRvalueKindV1::Use(scalar(U32, 7)))],
            vec![local(217, ARRAY, SemanticLocalRoleV1::Return)],
            vec![block(
                226,
                vec![array_value(11, 29)],
                SemanticTerminatorKindV1::Return,
            )],
        );
        let source = admit(types(), chain);
        let error = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap_err();
        assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
            function: 0, block: None, statement: None, detail,
        } if detail == "ordinary helper result needs an unprojected SSA destination; retained or projected destination effects are unsupported"));
    }

    #[test]
    fn admitted_projected_aggregate_call_destination_keeps_the_earlier_storage_refusal() {
        const OUTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
        let mut catalog = types();
        catalog.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([51; 32]),
            SemanticLayoutIdentityV1::from_sha256([52; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                4,
                SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                ),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ARRAY]).unwrap()),
        ));
        let mut chain = forwarding_chain(
            ARRAY,
            integer_cast(8, false),
            vec![
                local(207, UNIT, SemanticLocalRoleV1::Return),
                local(208, OUTER, SemanticLocalRoleV1::Temporary),
                local(209, TUPLE, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![local(217, ARRAY, SemanticLocalRoleV1::Return)],
            vec![block(
                226,
                vec![array_value(11, 29)],
                SemanticTerminatorKindV1::Return,
            )],
        );
        chain[0] = rebuild(
            &chain[0],
            chain[0].locals().to_vec(),
            vec![
                block(208, vec![], call_to(1, field(1, 0, ARRAY))),
                block(209, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        let source = admit(catalog, chain);
        let error = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::RetainedLocalStorage {
            function: 0, retained_locals, retained_count: 1,
        } if retained_locals.len() == 1 && retained_locals[0].0 == 1 && retained_locals[0].1 == OUTER.index())
        );
        // This source cannot reach call emission: retained aggregate storage is
        // an earlier unsupported prerequisite, not permission to flatten memory.
    }

    include!("production_source_store_value_uses_v1_tests.rs");
}
