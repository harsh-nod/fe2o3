use fe2o3_mir_model::semantic_mir_v1::SemanticCheckedBinaryRvalueV1;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Diamond,
    Mixed,
    DifferentSources,
    MovedSibling,
    Storage,
    Independent,
    ScalarBinary,
    ScalarUnary,
    ScalarCast,
    ScalarCheckedBinary,
    ScalarAssume,
    ScalarMovedSibling,
    CopyOwnedSibling,
    ScalarSwitch,
    AssertMessage,
    AssertMessagePair,
    AssertCondition,
    AssertConditionFolded,
    AssertConstant,
    Cycle,
}

fn cfg_owner(shape: Shape) -> ProductionSemanticSsaOwnerV1 {
    let original = execution_owner(Flow::Linear).unwrap();
    let mut types = original.source_semantic().types().to_vec();
    let mixed = matches!(
        shape,
        Shape::Mixed
            | Shape::MovedSibling
            | Shape::Storage
            | Shape::ScalarBinary
            | Shape::ScalarUnary
            | Shape::ScalarCast
            | Shape::ScalarCheckedBinary
            | Shape::ScalarAssume
            | Shape::ScalarMovedSibling
            | Shape::CopyOwnedSibling
            | Shape::ScalarSwitch
            | Shape::AssertMessage
            | Shape::AssertMessagePair
            | Shape::AssertCondition
            | Shape::AssertConditionFolded
            | Shape::AssertConstant
    );
    let boolean_sibling = matches!(
        shape,
        Shape::ScalarAssume | Shape::AssertCondition | Shape::AssertConditionFolded
    );
    let bool_ty = SemanticTypeIdV1::from_index(types.len() as u32 + u32::from(mixed));
    let sibling_type = if boolean_sibling { bool_ty } else { U32 };
    if mixed {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([201; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(if boolean_sibling { 1 } else { 4 }),
                if boolean_sibling { 1 } else { 4 },
                SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![CONTEXT, sibling_type]).unwrap(),
            ),
        ));
    }
    if matches!(
        shape,
        Shape::AssertMessage
            | Shape::AssertMessagePair
            | Shape::AssertCondition
            | Shape::AssertConditionFolded
            | Shape::AssertConstant
            | Shape::ScalarAssume
            | Shape::ScalarCheckedBinary
    ) {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([240; 32]),
            SemanticLayoutIdentityV1::from_sha256([240; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ));
    }
    let result = if mixed { PAIR } else { CONTEXT };
    let output_type = if matches!(shape, Shape::ScalarCast | Shape::ScalarCheckedBinary) {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        let (layout, shape) = if matches!(shape, Shape::ScalarCast) {
            (
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                }),
            )
        } else {
            (
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    4,
                    SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![U32, bool_ty]).unwrap(),
                ),
            )
        };
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([246; 32]),
            SemanticLayoutIdentityV1::from_sha256([246; 32]),
            layout,
            shape,
        ));
        ty
    } else {
        result
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let pair = |value| {
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![
                    SemanticOperandV1::Move(place(1, CONTEXT)),
                    if boolean_sibling {
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            bool_ty,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 1).unwrap(),
                            ),
                        ))
                    } else {
                        scalar(value)
                    },
                ],
            )
            .unwrap(),
        )
    };
    let arm = |number, source_local| {
        let value = if mixed {
            pair(number)
        } else {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(source_local, CONTEXT)))
        };
        assign(place(4, result), value)
    };
    let field = |index, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let blocks = if matches!(
        shape,
        Shape::ScalarBinary
            | Shape::ScalarUnary
            | Shape::ScalarCast
            | Shape::ScalarCheckedBinary
            | Shape::ScalarAssume
            | Shape::ScalarMovedSibling
            | Shape::CopyOwnedSibling
            | Shape::ScalarSwitch
            | Shape::AssertMessage
            | Shape::AssertMessagePair
            | Shape::AssertCondition
            | Shape::AssertConditionFolded
            | Shape::AssertConstant
    ) {
        let operand = SemanticOperandV1::Copy(field(1, sibling_type));
        let mut statements = vec![assign(place(4, PAIR), pair(42))];
        if matches!(shape, Shape::ScalarMovedSibling) {
            statements.push(assign(
                place(6, CONTEXT),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, CONTEXT))),
            ));
        }
        let terminator = match shape {
            Shape::ScalarBinary | Shape::ScalarMovedSibling => {
                statements.push(assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: operand,
                        right: SemanticOperandV1::Copy(field(1, U32)),
                    },
                ));
                SemanticTerminatorKindV1::Return
            }
            Shape::ScalarUnary => {
                statements.push(assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::Not,
                        operand,
                    },
                ));
                SemanticTerminatorKindV1::Return
            }
            Shape::ScalarCast => {
                statements.push(assign(
                    place(5, output_type),
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand,
                    },
                ));
                SemanticTerminatorKindV1::Return
            }
            Shape::ScalarCheckedBinary => {
                statements.push(assign(
                    place(5, output_type),
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Add,
                        operand,
                        SemanticOperandV1::Copy(field(1, U32)),
                    )),
                ));
                SemanticTerminatorKindV1::Return
            }
            Shape::ScalarAssume => {
                statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Assume(operand),
                ));
                SemanticTerminatorKindV1::Return
            }
            Shape::CopyOwnedSibling => {
                statements.push(assign(
                    place(6, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(0, CONTEXT))),
                ));
                SemanticTerminatorKindV1::Return
            }
            Shape::ScalarSwitch => SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand,
                targets: SemanticSwitchTargetsV1::new(
                    vec![],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
            Shape::AssertMessage
            | Shape::AssertMessagePair
            | Shape::AssertCondition
            | Shape::AssertConditionFolded
            | Shape::AssertConstant => SemanticTerminatorKindV1::Assert {
                condition: if boolean_sibling {
                    operand.clone()
                } else {
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        bool_ty,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                    ))
                },
                expected: true,
                message: if matches!(shape, Shape::AssertMessagePair) {
                    SemanticAssertMessageV1::BoundsCheck {
                        length: operand.clone(),
                        index: operand,
                    }
                } else {
                    SemanticAssertMessageV1::DivisionByZero(
                        if matches!(
                            shape,
                            Shape::AssertConstant
                                | Shape::AssertCondition
                                | Shape::AssertConditionFolded
                        ) {
                            scalar(42)
                        } else {
                            operand
                        },
                    )
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            _ => unreachable!(),
        };
        if matches!(terminator, SemanticTerminatorKindV1::Return) {
            vec![block(211, statements, terminator)]
        } else {
            vec![
                block(211, statements, terminator),
                block(212, vec![], SemanticTerminatorKindV1::Return),
            ]
        }
    } else if matches!(shape, Shape::Storage) {
        let storage = |live, index| {
            SemanticStatementV1::new(
                source(),
                if live {
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(index))
                } else {
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(index))
                },
            )
        };
        vec![block(
            211,
            vec![
                storage(true, 4),
                assign(place(4, PAIR), pair(42)),
                assign(
                    place(6, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, CONTEXT))),
                ),
                assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, U32))),
                ),
                storage(false, 4),
                storage(false, 1),
            ],
            SemanticTerminatorKindV1::Return,
        )]
    } else if matches!(shape, Shape::Independent) {
        vec![block(
            211,
            vec![
                arm(0, 1),
                assign(
                    place(6, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, CONTEXT))),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )]
    } else if matches!(shape, Shape::MovedSibling) {
        vec![
            block(
                211,
                vec![
                    assign(place(4, PAIR), pair(42)),
                    assign(
                        place(6, CONTEXT),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, CONTEXT))),
                    ),
                ],
                goto(1),
            ),
            block(212, vec![], goto(2)),
            block(
                213,
                vec![assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, U32))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    } else if matches!(shape, Shape::Cycle) {
        vec![
            block(211, vec![], goto(1)),
            block(
                212,
                vec![
                    arm(11, 1),
                    assign(
                        place(1, CONTEXT),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, CONTEXT))),
                    ),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(213, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(
                211,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(212, vec![arm(11, 1)], goto(3)),
            block(
                213,
                vec![arm(
                    22,
                    if matches!(shape, Shape::DifferentSources) {
                        2
                    } else {
                        1
                    },
                )],
                goto(3),
            ),
            block(
                214,
                vec![assign(
                    place(5, result),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, result))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([202; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        vec![
            SemanticAbiArgumentV1::source(ignored(CONTEXT)),
            SemanticAbiArgumentV1::source(ignored(CONTEXT)),
            SemanticAbiArgumentV1::source(direct(U32)),
        ],
        ignored(UNIT),
    )
    .unwrap();
    let root = function(
        203,
        SemanticFunctionRoleV1::KernelRoot,
        abi,
        vec![
            local(220, UNIT, SemanticLocalRoleV1::Return),
            local(221, CONTEXT, SemanticLocalRoleV1::Argument(0)),
            local(222, CONTEXT, SemanticLocalRoleV1::Argument(1)),
            local(223, U32, SemanticLocalRoleV1::Argument(2)),
            local(224, result, SemanticLocalRoleV1::Temporary),
            local(225, output_type, SemanticLocalRoleV1::Temporary),
            local(226, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        blocks,
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"cfg_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([204; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn lower_cfg_fixture(
    shape: Shape,
    change_seed: impl FnOnce(&mut SemanticExecutionBindingV29),
    inspect: impl FnOnce(
        &ProductionSemanticSsaOwnerV1,
        &SemanticExecutionBindingV29,
        Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    ),
) {
    lower_cfg_fixture_with_cursor(shape, change_seed, |_| {}, inspect);
}

fn lower_cfg_fixture_with_cursor(
    shape: Shape,
    change_seed: impl FnOnce(&mut SemanticExecutionBindingV29),
    change_cursor: impl FnOnce(&mut ExecutionAvailabilityV29<'_>),
    inspect: impl FnOnce(
        &ProductionSemanticSsaOwnerV1,
        &SemanticExecutionBindingV29,
        Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    ),
) {
    lower_cfg_fixture_with_limits(
        shape,
        (10_000_000, 10_000_000),
        change_seed,
        change_cursor,
        inspect,
    )
    .unwrap();
}

fn lower_cfg_fixture_with_limits(
    shape: Shape,
    (work_limit, storage_limit): (usize, usize),
    change_seed: impl FnOnce(&mut SemanticExecutionBindingV29),
    change_cursor: impl FnOnce(&mut ExecutionAvailabilityV29<'_>),
    inspect: impl FnOnce(
        &ProductionSemanticSsaOwnerV1,
        &SemanticExecutionBindingV29,
        Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    ),
) -> Result<(usize, usize), String> {
    let mut owner = cfg_owner(shape);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .map_err(|error| format!("{error:?}"))?;
    budget
        .reserve_storage(captured.retained_storage())
        .map_err(|error| format!("{error:?}"))?;
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let seed = SemanticExecutionBindingV29::context(
            owner.source_semantic().types(),
            CONTEXT,
            ProductionCallOccurrenceV1 {
                caller: instances.root(),
                block: SemanticBlockIdV1::from_index(0),
            },
            ValueId(90),
        )
        .unwrap();
        let mut other = seed.clone();
        change_seed(&mut other);
        let result = with_execution_availability_v29(
            instances,
            instances.root(),
            budget,
            |mut cursor, budget| {
                cursor.entry_seeds = vec![
                    (1, SemanticValueBindingV1::Execution(seed.clone())),
                    (2, SemanticValueBindingV1::Execution(other)),
                ];
                change_cursor(&mut cursor);
                let plan = LoweredFunctionPlanV1 {
                    correspondence_owner: ROOT,
                    semantic_function: ROOT,
                    kernel_ir_function: FunctionId::new("cfg_fixture"),
                    role: SemanticKirFunctionRoleV1::KernelEntry,
                    parameter_declarations: vec![(2, 3, U32)],
                    parameter_types: vec![Type::Scalar(ScalarType::U32)],
                    parameter_values: vec![ValueId(100)],
                    call_arguments: vec![],
                    parameter_local_bindings: vec![PlannedParameterLocalBindingV1::Direct {
                        local: 3,
                        value: ValueId(100),
                        ty: Type::Scalar(ScalarType::U32),
                    }],
                    parameter_component_bindings: vec![],
                    borrowed_parameter_bindings: vec![],
                    ignored_parameter_bindings: vec![],
                    result_types: vec![],
                };
                let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
                lower_one_semantic_function_v1(
                    owner.source_semantic(),
                    &plan,
                    owner.plan_for_function(ROOT).unwrap(),
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    Some([64, 1, 1]),
                    if matches!(shape, Shape::AssertConditionFolded) {
                        // Test the existing success-folded emitter path only.
                        BTreeSet::from([0])
                    } else {
                        BTreeSet::new()
                    },
                    1,
                    false,
                    1024,
                    None,
                    &mut private,
                    None,
                    budget,
                    SemanticEmissionPlacementV1 {
                        first_block: 17,
                        first_value: 100,
                    },
                    Some(cursor),
                )
            },
        );
        inspect(&owner, &seed, result);
        Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .map_err(|error| format!("{error:?}"))?;
    Ok((budget.work(), budget.peak_storage()))
}
