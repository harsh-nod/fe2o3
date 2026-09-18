use super::*;

fn replace_blocks(
    owner: ProductionSemanticMirOwnerV1,
    function: usize,
    blocks: Vec<SemanticBasicBlockV1>,
) -> ProductionSemanticMirOwnerV1 {
    let semantic = owner.semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[function];
    let mut replacement = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = original.kernel_entry() {
        replacement = replacement.with_kernel_entry(entry.clone());
    }
    functions[function] = replacement;
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn assign(place: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place.clone(),
            SemanticRvalueV1::new(place.ty(), value),
        )),
    )
}

fn tuple(first: u32, last: u32) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Aggregate(
        SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::Tuple,
            vec![
                scalar_constant(U32, first, 4),
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                )),
                scalar_constant(U32, last, 4),
            ],
        )
        .unwrap(),
    )
}

fn distinct_calls(expanded: bool, moved: bool) -> ProductionSemanticMirOwnerV1 {
    let call = |receiver, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![
                    scalar_constant(U32, receiver, 4),
                    if moved {
                        SemanticOperandV1::Move(local_place(1, TUPLE))
                    } else {
                        SemanticOperandV1::Copy(local_place(1, TUPLE))
                    },
                ],
                Some(SemanticCallDestinationV1::new(
                    local_place(2, U32),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    replace_blocks(
        owner(
            expanded,
            moved,
            &[U32, UNIT, U32],
            BodyCase::RepeatedCalls(2),
        ),
        0,
        vec![
            block(
                40,
                vec![assign(local_place(1, TUPLE), tuple(29, 11))],
                call(7, 1),
            ),
            block(
                41,
                vec![
                    assign(
                        local_place(3, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(2, U32))),
                    ),
                    assign(local_place(1, TUPLE), tuple(3, 41)),
                ],
                call(101, 2),
            ),
            block(
                42,
                vec![assign(
                    local_place(3, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(local_place(3, U32)),
                        right: SemanticOperandV1::Copy(local_place(2, U32)),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
}

#[test]
fn repeated_rust_calls_keep_distinct_same_typed_arguments_and_results() {
    for expanded in [false, true] {
        for moved in [false, true] {
            let lowered = ProductionSemanticKirOwnerV1::try_lower(
                distinct_calls(expanded, moved),
                ProductionSemanticKirLimitsV1::default(),
            )
            .unwrap_or_else(|error| panic!("expanded={expanded}, moved={moved}: {error:?}"));
            lowered.verify_equivalence().unwrap();
            let module = lowered.module();
            verify_module(module).unwrap();
            assert_eq!(module.functions.len(), 2);
            assert_eq!(
                module.functions[1].signature.parameters,
                vec![Type::Scalar(ScalarType::U32); 3]
            );
            let operations = module.functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .collect::<Vec<_>>();
            let constants = operations
                .iter()
                .filter_map(|operation| match operation.kind {
                    OperationKind::Constant(Constant::U32(value)) => {
                        Some((operation.results[0].id, value))
                    }
                    _ => None,
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            let calls = operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, arguments } => {
                        assert_eq!(*callee, module.functions[1].id);
                        assert_eq!(operation.results.len(), 1);
                        Some((arguments, operation.results[0].id))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 2);
            for ((arguments, _), expected) in calls.iter().zip([[7, 29, 11], [101, 3, 41]]) {
                assert_eq!(
                    arguments
                        .iter()
                        .map(|value| constants[value])
                        .collect::<Vec<_>>(),
                    expected
                );
            }
            assert_ne!(calls[0].1, calls[1].1);
            assert!(operations.iter().any(|operation| matches!(operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add), lhs, rhs,
                } if lhs == calls[0].1 && rhs == calls[1].1)));
        }
    }
}

#[test]
fn rust_call_tuple_move_cannot_be_reused_without_reinitialization() {
    for expanded in [false, true] {
        for fields in [vec![UNIT], vec![U32, UNIT, U32]] {
            let result = ProductionSemanticKirOwnerV1::try_lower(
                owner(expanded, true, &fields, BodyCase::RepeatedCalls(2)),
                ProductionSemanticKirLimitsV1::default(),
            );
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::SemanticSsa(
                    fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
                        function,
                        error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse {
                            block, variable, ..
                        },
                    }
                )) if function.index() == 0 && block.get() == 1 && variable.get() == 1),
                "expanded={expanded}, fields={fields:?}: {result:?}"
            );
        }
    }
}

#[test]
fn rust_call_branch_returns_preserve_each_tuple_component_order() {
    for expanded in [false, true] {
        let field = |ordinal| {
            SemanticOperandV1::Copy(if expanded {
                local_place(2 + ordinal, U32)
            } else {
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(2),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), U32)
                            .unwrap(),
                    ],
                    U32,
                )
                .unwrap()
            })
        };
        let mut blocks = vec![block(
            70,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(local_place(1, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(1),
                        ),
                    )],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(2),
                    ),
                )
                .unwrap(),
            },
        )];
        for (tag, left, right) in [(71, 0, 2), (72, 2, 0)] {
            blocks.push(block(
                tag,
                vec![assign(
                    local_place(0, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Subtract,
                        left: field(left),
                        right: field(right),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ));
        }
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            replace_blocks(
                owner(expanded, true, &[U32, UNIT, U32], BodyCase::Direct),
                1,
                blocks,
            ),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("expanded={expanded}: {error:?}"));
        lowered.verify_equivalence().unwrap();
        verify_module(lowered.module()).unwrap();
        let helper = &lowered.module().functions[1];
        assert_eq!(helper.signature.results, [Type::Scalar(ScalarType::U32)]);
        let body = helper.body.as_ref().unwrap();
        assert_eq!(body.blocks.len(), 3);
        for (id, left, right) in [(BlockId(1), 1, 2), (BlockId(2), 2, 1)] {
            let block = body.blocks.iter().find(|block| block.id == id).unwrap();
            let subtraction = block
                .operations
                .iter()
                .find(|operation| {
                    matches!(operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Subtract), lhs, rhs,
                } if lhs == body.parameters[left] && rhs == body.parameters[right])
                })
                .expect("each branch retains its independently ordered tuple subtraction");
            assert_eq!(subtraction.results.len(), 2);
            assert!(matches!(&block.terminator,
                Some(Terminator::Return { values }) if values == &[subtraction.results[0].id]));
        }
    }
}
