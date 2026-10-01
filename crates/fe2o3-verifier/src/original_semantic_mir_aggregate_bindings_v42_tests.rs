use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

// This remains an admitted original program, shared by producer and cut tests.
pub(in super::super) fn checked_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    operation: SemanticCheckedBinaryOpV1,
    failure_move: bool,
    backedge: bool,
) {
    let word = SemanticTypeIdV1::from_index(0);
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
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
    let pair = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![word, boolean]).unwrap()),
    ));
    let old = functions.last_mut().unwrap();
    let source = old.source();
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 4);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([242; 32]),
        pair,
        SemanticLocalRoleV1::Temporary,
        source,
    ));
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let field = |ordinal, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let assign = |destination, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let block = |identity, statements, terminator| {
        SemanticBasicBlockV1::new(
            identity,
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let mut statements = old.blocks()[0].statements().to_vec();
    statements.push(assign(
        place(4, pair),
        pair,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            operation,
            SemanticOperandV1::Copy(place(1, word)),
            SemanticOperandV1::Copy(place(2, word)),
        )),
    ));
    let message = if failure_move {
        SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(field(0, word)))
    } else {
        SemanticAssertMessageV1::ResumedAfterPanic
    };
    let mut blocks = vec![
        block(
            old.blocks()[0].identity(),
            statements,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([243; 32]),
            vec![],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Move(field(1, boolean)),
                expected: false,
                message,
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([244; 32]),
            vec![assign(
                place(0, word),
                word,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(0, word))),
            )],
            if backedge {
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(1, word)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                    )
                    .unwrap(),
                }
            } else {
                SemanticTerminatorKindV1::Return
            },
        ),
    ];
    if backedge {
        blocks.push(block(
            SemanticBlockIdentityV1::from_sha256([245; 32]),
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
        ));
        blocks.push(block(
            SemanticBlockIdentityV1::from_sha256([246; 32]),
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
    }
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
}

pub(in super::super) fn partial_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    backedge: bool,
) {
    checked_transform(
        types,
        functions,
        SemanticCheckedBinaryOpV1::Add,
        false,
        backedge,
    );
    let old = functions.last_mut().unwrap();
    let source = old.source();
    let word = SemanticTypeIdV1::from_index(0);
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word).unwrap()],
        word,
    )
    .unwrap();
    let mut statements = old.blocks()[0].statements().to_vec();
    assert!(matches!(statements.pop().unwrap().kind(),
        SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::CheckedBinary(_))));
    statements.push(SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            field,
            SemanticRvalueV1::new(
                word,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], word).unwrap(),
                )),
            ),
        )),
    ));
    let mut blocks = old.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        source,
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        source,
        vec![],
        SemanticTerminatorV1::new(
            source,
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        ),
    )
    .unwrap();
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap();
}

pub(in super::super) fn deinitialized_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    backedge: bool,
) {
    checked_transform(
        types,
        functions,
        SemanticCheckedBinaryOpV1::Add,
        false,
        backedge,
    );
    let old = functions.last_mut().unwrap();
    let source = old.source();
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32 - 2);
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 5);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([247; 32]),
        boolean,
        SemanticLocalRoleV1::Temporary,
        source,
    ));
    let flag = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), boolean).unwrap()],
        boolean,
    )
    .unwrap();
    let mut blocks = old.blocks().to_vec();
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        source,
        vec![
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], boolean)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        boolean,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(flag.clone())),
                    ),
                )),
            ),
            SemanticStatementV1::new(source, SemanticStatementKindV1::Deinitialize(flag)),
        ],
        SemanticTerminatorV1::new(
            source,
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        ),
    )
    .unwrap();
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
}

fn source_boundary_value(
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    block: u32,
    local: u32,
    out: &mut Writer<'_, '_>,
) -> Result<Value> {
    let row = plan.instance(root, instance, out)?;
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let function = &semantic.functions()[row.function.index() as usize];
    let archive = source.source_ssa(out.budget)?;
    let ssa = archive
        .plan_for_function(row.function)
        .ok_or_else(mismatch)?
        .plan();
    let mut successors = vector(function.blocks().len(), out)?;
    for block in function.blocks() {
        let mut edges = vector(block.terminator().kind().edge_count(), out)?;
        block.terminator().kind().try_for_each_edge(|edge| {
            out.budget.charge_work(1)?;
            edges.push(Block::new(edge.target().index()));
            Ok::<_, Error>(())
        })?;
        successors.push(edges);
    }
    let boundaries = Boundaries::derive(
        ssa,
        ControlInput {
            entry: Block::new(function.entry().index()),
            successors: &successors,
        },
        out,
    )?;
    boundaries.value(Block::new(block), Variable::new(local), out)
}

fn run(
    operation: SemanticCheckedBinaryOpV1,
    failure_move: bool,
    backedge: bool,
    work: usize,
    storage: usize,
    examine: impl FnOnce(&SourceSlots<'_, '_>, SemanticFunctionIdV1, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| checked_transform(types, functions, operation, failure_move, backedge),
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let function = SemanticFunctionIdV1::from_index(
                    slots
                        .correspondence(out)?
                        .source(out.budget)?
                        .source_semantic(out.budget)?
                        .functions()
                        .len() as u32
                        - 1,
                );
                examine(slots, function, out)
            })
        },
    )
}

pub(in super::super) fn call_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    aggregate_return: bool,
) {
    checked_transform(
        types,
        functions,
        SemanticCheckedBinaryOpV1::Add,
        false,
        false,
    );
    let pair = SemanticTypeIdV1::from_index(types.len() as u32 - 1);
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32 - 2);
    let word = SemanticTypeIdV1::from_index(0);
    let old_pair = &types[pair.index() as usize];
    types[pair.index() as usize] = SemanticTypeDeclV1::new(
        old_pair.identity(),
        old_pair.layout_identity(),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::scalar_pair(
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                ),
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                ),
            ),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![word, boolean]).unwrap()),
    );
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let field = |local, ordinal, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let helper_index = functions.len() - 1;
    let old = &functions[helper_index];
    let source = old.source();
    let SemanticAbiPassModeV1::Direct(attributes) = old.abi().arguments()[0].mode() else {
        panic!("scalar fixture ABI");
    };
    let pair_mode = SemanticAbiPassModeV1::Pair {
        first: *attributes,
        second: SemanticAbiValueAttributesV1::new(
            attributes.regular(),
            SemanticAbiExtensionV1::ZeroExtend,
            0,
            None,
        )
        .unwrap(),
    };
    let result_type = if aggregate_return { pair } else { word };
    let result_mode = if aggregate_return {
        pair_mode.clone()
    } else {
        old.abi().return_value().mode().clone()
    };
    let abi = SemanticFunctionAbiV1::new(
        old.abi().identity(),
        old.abi().layout_identity(),
        old.abi().canon_abi(),
        false,
        false,
        vec![
            SemanticAbiValueV1::new(pair, pair_mode),
            old.abi().arguments()[1].value().clone(),
        ],
        SemanticAbiValueV1::new(result_type, result_mode),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let mut locals = old.locals()[..4].to_vec();
    locals[0] = SemanticLocalDeclV1::new(
        locals[0].identity(),
        result_type,
        SemanticLocalRoleV1::Return,
        source,
    );
    locals[1] = SemanticLocalDeclV1::new(
        locals[1].identity(),
        pair,
        SemanticLocalRoleV1::Argument(0),
        source,
    );
    let body = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0, result_type),
            SemanticRvalueV1::new(
                result_type,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(if aggregate_return {
                    place(1, pair)
                } else {
                    field(1, 0, word)
                })),
            ),
        )),
    );
    functions[helper_index] = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        abi,
        locals,
        old.entry(),
        vec![
            SemanticBasicBlockV1::new(
                old.blocks()[0].identity(),
                source,
                vec![body],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    for root in 0..helper_index {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([242; 32]),
            pair,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
        let mut blocks = Vec::new();
        for block in 0..4 {
            let statements = if block == 0 {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(4, pair),
                        SemanticRvalueV1::new(
                            pair,
                            SemanticRvalueKindV1::CheckedBinary(
                                SemanticCheckedBinaryRvalueV1::new(
                                    SemanticCheckedBinaryOpV1::Add,
                                    SemanticOperandV1::Copy(place(1, word)),
                                    SemanticOperandV1::Copy(place(2, word)),
                                ),
                            ),
                        ),
                    )),
                )]
            } else {
                vec![]
            };
            let terminator = if block < 2 {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(helper_index as u32),
                        vec![
                            SemanticOperandV1::Copy(place(4, pair)),
                            SemanticOperandV1::Copy(place(2, word)),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            if aggregate_return {
                                place(4, pair)
                            } else {
                                field(4, 0, word)
                            },
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(block + 1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            } else if block == 2 {
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Move(field(4, 1, boolean)),
                    expected: false,
                    message: SemanticAssertMessageV1::ResumedAfterPanic,
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(3),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                }
            } else {
                SemanticTerminatorKindV1::Return
            };
            blocks.push(
                SemanticBasicBlockV1::new(
                    old.blocks()[block as usize].identity(),
                    source,
                    statements,
                    SemanticTerminatorV1::new(source, terminator),
                )
                .unwrap(),
            );
        }
        functions[root] = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source,
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
}

#[test]
fn original_mir_aggregate_calls_and_returns_keep_complete_exclusive_snapshots() {
    super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| call_transform(types, functions, true),
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let children: Vec<_> = paired
                    .instances
                    .iter()
                    .flatten()
                    .filter(|instance| instance.owners.len() == 2)
                    .collect();
                assert_eq!(children.len(), 4);
                for child in children {
                    let SourceValue::Aggregate(argument) = child.arguments[0].source else {
                        panic!("aggregate argument");
                    };
                    assert_eq!(paired.aggregates[argument].components.len(), 2);
                    let SourceValue::Aggregate(returned) = child.returned.unwrap().source else {
                        panic!("aggregate return");
                    };
                    assert_eq!(paired.aggregates[returned].components.len(), 2);
                }
                program.emit(out)?;
                paired.emit(out)?;
                assert!(
                    out.text
                        .contains("arguments: Seq<InvocationSourceValueV42>")
                );
                assert!(
                    out.text
                        .contains("invocation_source_aggregate_complete_v42(snapshot)")
                );
                assert!(
                    out.text
                        .contains("InvocationSourceValueV42::Aggregate(value)")
                );
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

pub(in super::super) fn retained_call_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    sibling: bool,
    backedge: bool,
) {
    let helper = functions.last().unwrap().clone();
    checked_transform(
        types,
        functions,
        SemanticCheckedBinaryOpV1::Add,
        false,
        false,
    );
    *functions.last_mut().unwrap() = helper;
    let helper_index = functions.len() - 1;
    let pair = SemanticTypeIdV1::from_index(types.len() as u32 - 1);
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32 - 2);
    let word = SemanticTypeIdV1::from_index(0);
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let field = |ordinal, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    for root in 0..helper_index {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        assert_eq!(locals.len(), 4);
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([242; 32]),
            pair,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
        let mut blocks = Vec::new();
        for block in 0..4 {
            let statements = if block == 0 && sibling {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        field(1, boolean),
                        SemanticRvalueV1::new(
                            boolean,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    boolean,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(0, 1).unwrap(),
                                    ),
                                ),
                            )),
                        ),
                    )),
                )]
            } else if block == 2 {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(3, word),
                        SemanticRvalueV1::new(
                            word,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(0, word))),
                        ),
                    )),
                )]
            } else if block == 3 {
                old.blocks()[3].statements().to_vec()
            } else {
                vec![]
            };
            let terminator = if block < 2 {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(helper_index as u32),
                        vec![
                            SemanticOperandV1::Copy(place(1, word)),
                            SemanticOperandV1::Copy(place(2, word)),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            field(0, word),
                            edge(SemanticEdgeRoleV1::CallReturn, block + 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            } else if block == 2 && sibling {
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Move(field(1, boolean)),
                    expected: false,
                    message: SemanticAssertMessageV1::ResumedAfterPanic,
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                    unwind: SemanticUnwindActionV1::Unreachable,
                }
            } else if block == 2 {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3))
            } else if backedge {
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(1, word)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 0),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                    )
                    .unwrap(),
                }
            } else {
                SemanticTerminatorKindV1::Return
            };
            blocks.push(
                SemanticBasicBlockV1::new(
                    old.blocks()[block as usize].identity(),
                    source,
                    statements,
                    SemanticTerminatorV1::new(source, terminator),
                )
                .unwrap(),
            );
        }
        if backedge {
            blocks.push(
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([247; 32]),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            );
        }
        functions[root] = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source,
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
}

#[test]
fn original_mir_aggregate_callsite_backedge_keeps_component_contracts_across_frame_reuse() {
    for sibling in [false, true] {
        super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| retained_call_transform(types, functions, sibling, true),
            |plan, out| {
                super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    let paired =
                        PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                    for root in &paired.roots {
                        let entry = paired.instances[root.instances.start].as_ref().unwrap();
                        assert_eq!(entry.outgoing.len(), 2);
                        for instance in root.instances.start + 1..root.instances.end {
                            let returned = paired.instances[instance]
                                .as_ref()
                                .unwrap()
                                .returned
                                .unwrap();
                            assert!(matches!(returned.source, SourceValue::ReturnSnapshot));
                            assert!(returned.definition.is_some());
                        }
                    }
                    program.emit(out)?;
                    paired.emit(out)?;
                    assert!(
                        out.text
                            .contains("byte_enter_frame_v30(source.machine.frames,")
                    );
                    assert!(
                        out.text
                            .contains("invocation_source_logical_clear_v38(source.logical,")
                    );
                    assert!(
                        out.text
                            .contains("memory: Some((InvocationSourceByteAccessV36")
                    );
                    assert!(
                        out.text
                            .contains("base: InvocationSourceByteBaseV36::ObjectLocal(")
                    );
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_aggregate_partial_initialization_keeps_the_mixed_memory_cut_relation() {
    for backedge in [false, true] {
        super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| partial_transform(types, functions, backedge),
            |plan, out| {
                super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    let paired = PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                    for (root, scope) in paired.roots.iter().enumerate() {
                        for instance in 1..=2 {
                            assert!(slots.has_original_object(root, instance, 4, out)?);
                            let local = plan.instance(root, instance, out)?.locals.start + 4;
                            for cut in scope.cuts.iter().flatten() {
                                assert!(!cut.live.iter().any(|binding| matches!(binding.source,
                                    SourceValue::Aggregate(index) if paired.aggregates[index].local == local)));
                            }
                        }
                    }
                    program.emit(out)?;
                    paired.emit(out)?;
                    assert!(out.text.contains("InvocationSourceByteDestinationV36::Memory("));
                    assert!(out.text.contains("InvocationSourceByteBaseV36::ObjectLocal("));
                    for root in 0..paired.roots.len() {
                        assert!(out.text.contains(&format!(
                            "&& invocation_source_byte_storage_related_{root}_v36(source, target)"
                        )));
                    }
                    assert!(!out.text.contains("InvocationSourceByteEventV36::Checked"));
                    Ok(())
                })
            },
        ).0.unwrap();
    }
}

#[test]
fn original_mir_aggregate_cuts_preserve_partial_move_payload_across_join_and_backedge() {
    for backedge in [false, true] {
        for failure_move in [false, true] {
            super::super::super::invocations::tests::run_source_transform(
                LIMIT,
                LIMIT,
                |types, functions| {
                    checked_transform(
                        types,
                        functions,
                        SemanticCheckedBinaryOpV1::Add,
                        failure_move,
                        backedge,
                    )
                },
                |plan, out| {
                    super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                        let mut program = SourceByteProgram::derive(plan, slots, out)?;
                        let paired = PairedInvocations::derive(
                            plan,
                            &program,
                            FormalIndexWidth::Bits64,
                            out,
                        )?;
                        let mut checked = 0;
                        for (root, scope) in paired.roots.iter().enumerate() {
                            for cut in scope.cuts.iter().flatten() {
                                let instance = cut.instance - scope.instances.start;
                                let row = plan.instance(root, instance, out)?;
                                let block = cut.source - row.blocks.start;
                                if instance == 0 || !matches!(block, 2 | 3) {
                                    continue;
                                }
                                let binding = cut
                                    .live
                                    .iter()
                                    .find_map(|binding| match binding.source {
                                        SourceValue::Aggregate(index)
                                            if paired.aggregates[index].local
                                                == row.locals.start + 4 =>
                                        {
                                            Some(&paired.aggregates[index])
                                        }
                                        _ => None,
                                    })
                                    .expect("live original payload has an aggregate cut binding");
                                assert_eq!(binding.components.len(), 1);
                                assert_eq!(binding.components[0].leaf, 0);
                                assert!(binding.components[0].definition.is_some());
                                checked += 1;
                            }
                        }
                        assert_eq!(checked, if backedge { 8 } else { 4 });
                        program.emit(out)?;
                        paired.emit(out)?;
                        assert!(out.text.contains("invocation_source_aggregate_leaf_v42("));
                        assert!(out.text.contains("MemoryOperationEffectV30::Trap"));
                        Ok(())
                    })
                },
            )
            .0
            .unwrap();
        }
    }
}

#[test]
fn original_mir_aggregate_cuts_skip_missing_dead_carriers_but_require_live_and_complete_snapshots()
{
    use fe2o3_lower_mir_kernel::ProductionSourceSsaCarrierShapeV37 as Carrier;
    for backedge in [false, true] {
        super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| deinitialized_transform(types, functions, backedge),
            |plan, out| {
                super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    let program = SourceByteProgram::derive(plan, slots, out)?;
                    let mut paired =
                        PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                    let relation = slots.correspondence(out)?;
                    let inventory = relation.inventory(out.budget)?;
                    for root in 0..paired.roots.len() {
                        let scope = plan.root(root, out)?;
                        assert!(scope.instances.len() >= 3);
                        for instance in 1..=2 {
                            let row = plan.instance(root, instance, out)?;
                            let value = source_boundary_value(plan, root, instance, 2, 4, out)?;
                            let endpoint = relation
                                .ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
                            // Deinit clears source definedness, not physical
                            // storage. Its old canonical flag carrier survives.
                            let flag = endpoint.component(1, out.budget)?;
                            assert_eq!(flag.carrier_shape(out.budget)?, Carrier::Value);
                            let flag_definition = flag.original_definition(out.budget)?;
                            assert!(flag_definition.is_some());
                            let physical =
                                inventory.functions()[scope.physical].definitions.clone();
                            let binding = paired.binding(
                                plan,
                                root,
                                instance,
                                4,
                                value,
                                1,
                                &physical,
                                Some(ComponentCut::at(2)),
                                out,
                            )?;
                            let SourceValue::Aggregate(index) = binding.source else {
                                panic!("aggregate leaf binding");
                            };
                            assert_eq!(paired.aggregates[index].components.len(), 1);
                            assert_eq!(paired.aggregates[index].components[0].leaf, 0);
                            // Locating old bits does not restore source
                            // definedness. Required leaves still receive an
                            // independent source-presence guard at runtime.
                            for demanded in [Some(ComponentCut::at(1)), None] {
                                let complete = paired.binding(
                                    plan, root, instance, 4, value, 1, &physical, demanded, out,
                                )?;
                                let SourceValue::Aggregate(index) = complete.source else {
                                    panic!("complete aggregate carrier locator");
                                };
                                let components = &paired.aggregates[index].components;
                                assert_eq!(components.len(), 2);
                                assert_eq!(components[0].leaf, 0);
                                assert_eq!(components[1].leaf, 1);
                                assert_eq!(components[1].definition, flag_definition);
                            }
                            for (function, local, ty) in [
                                (
                                    SemanticFunctionIdV1::from_index(0),
                                    4,
                                    endpoint.source_type(out.budget)?,
                                ),
                                (row.function, 3, endpoint.source_type(out.budget)?),
                                (row.function, 4, SemanticTypeIdV1::from_index(0)),
                            ] {
                                assert!(matches!(
                                    paired.aggregate_binding(
                                        root,
                                        instance,
                                        local,
                                        function,
                                        ty,
                                        &endpoint,
                                        row.locals.start,
                                        1,
                                        &physical,
                                        Some(ComponentCut::at(2)),
                                        out
                                    ),
                                    Err(Error::Statement(_))
                                ));
                            }
                            let foreign = inventory.functions()
                                [(scope.physical + 1) % paired.roots.len()]
                            .definitions
                            .clone();
                            assert!(matches!(
                                paired.binding(
                                    plan,
                                    root,
                                    instance,
                                    4,
                                    value,
                                    1,
                                    &foreign,
                                    Some(ComponentCut::at(2)),
                                    out
                                ),
                                Err(Error::Statement(_))
                            ));
                        }
                    }
                    paired.emit(out)?;
                    assert!(
                        out.text
                            .contains("match invocation_source_aggregate_leaf_v42(")
                    );
                    assert!(out.text.contains("None => false"));
                    assert!(
                        super::super::source_bytes::SOURCE_BYTES_V36
                            .contains("invocation_source_aggregate_complete_v42(snapshot)")
                    );
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_aggregate_component_demands_keep_success_payload_after_assert_flag_move() {
    for operation in [
        SemanticCheckedBinaryOpV1::Add,
        SemanticCheckedBinaryOpV1::Subtract,
        SemanticCheckedBinaryOpV1::Multiply,
    ] {
        for failure_move in [false, true] {
            run(
                operation,
                failure_move,
                false,
                LIMIT,
                LIMIT,
                |slots, function, out| {
                    let demands = ComponentDemandsV42::derive(slots, function, out)?;
                    for (block, expected) in
                        [(0, [false, false]), (1, [true, true]), (2, [true, false])]
                    {
                        for (leaf, expected) in expected.into_iter().enumerate() {
                            assert_eq!(
                                demands.leaf_required(function, block, 4, leaf, out)?,
                                expected
                            );
                        }
                    }
                    Ok(())
                },
            )
            .0
            .unwrap();
        }
    }
}

#[test]
fn original_mir_aggregate_component_demands_reach_fixed_point_across_backedges() {
    run(
        SemanticCheckedBinaryOpV1::Add,
        false,
        true,
        LIMIT,
        LIMIT,
        |slots, function, out| {
            let demands = ComponentDemandsV42::derive(slots, function, out)?;
            for block in [2, 3] {
                assert!(demands.leaf_required(function, block, 4, 0, out)?);
                assert!(!demands.leaf_required(function, block, 4, 1, out)?);
            }
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_mir_aggregate_component_demands_latch_undercut_after_credit_restoration() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let result = run(
        SemanticCheckedBinaryOpV1::Add,
        false,
        false,
        LIMIT,
        LIMIT,
        |slots, function, out| {
            let demands = ComponentDemandsV42::derive(slots, function, out)?;
            let floor = out.budget.storage();
            out.budget.release_storage(1)?;
            assert!(matches!(
                demands.leaf_required(function, 2, 4, 0, out),
                Err(Error::Source(SourceError::Resource(Resource::Accounting)))
            ));
            out.budget.reserve_storage(1)?;
            let before = (out.budget.work(), out.budget.storage(), out.text.len());
            let result = demands.leaf_required(function, 2, 4, 0, out);
            assert!(matches!(
                result,
                Err(Error::Source(SourceError::Resource(Resource::Accounting)))
            ));
            assert_eq!(out.budget.storage(), floor);
            assert_eq!(
                (out.budget.work(), out.budget.storage(), out.text.len()),
                before
            );
            result.map(|_| ())
        },
    );
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_mir_aggregate_component_demands_reject_funded_foreign_ledger_before_debit() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let result = run(
        SemanticCheckedBinaryOpV1::Add,
        false,
        false,
        LIMIT,
        LIMIT,
        |slots, function, out| {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(out.budget.storage())?;
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            {
                let mut foreign = Writer::new(&mut budget)?;
                assert!(matches!(
                    ComponentDemandsV42::derive(slots, function, &mut foreign),
                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                ));
                assert!(foreign.text.is_empty());
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                before
            );
            let primary = (out.budget.work(), out.budget.storage(), out.text.len());
            let result = ComponentDemandsV42::derive(slots, function, out).map(|_| ());
            assert_eq!(
                (out.budget.work(), out.budget.storage(), out.text.len()),
                primary
            );
            result
        },
    );
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_mir_aggregate_component_demands_have_exact_and_one_short_resources() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let examine = |slots: &SourceSlots<'_, '_>, function, out: &mut Writer<'_, '_>| {
        let demands = ComponentDemandsV42::derive(slots, function, out)?;
        assert!(demands.leaf_required(function, 2, 4, 0, out)?);
        Ok(())
    };
    let measured = run(
        SemanticCheckedBinaryOpV1::Add,
        true,
        false,
        LIMIT,
        LIMIT,
        examine,
    );
    measured.0.unwrap();
    run(
        SemanticCheckedBinaryOpV1::Add,
        true,
        false,
        measured.1,
        measured.3,
        examine,
    )
    .0
    .unwrap();
    for (work, storage, is_work) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let refused = run(
            SemanticCheckedBinaryOpV1::Add,
            true,
            false,
            work,
            storage,
            examine,
        );
        assert!(
            matches!((is_work, &refused.0),
            (true, Err(Error::Source(SourceError::Resource(Resource::Work(error)))))
                if error.limit() == work && error.actual() == measured.1)
                || matches!((is_work, &refused.0),
                (false, Err(Error::Source(SourceError::Resource(Resource::Storage(error)))))
                    if error.limit() == storage && error.actual() == measured.3),
            "{:?}",
            refused.0
        );
        assert!(refused.1 <= work && refused.3 <= storage);
    }
}

#[test]
fn original_mir_aggregate_component_queries_require_exact_function_block_local_and_leaf() {
    run(
        SemanticCheckedBinaryOpV1::Add,
        false,
        false,
        LIMIT,
        LIMIT,
        |slots, function, out| {
            let demands = ComponentDemandsV42::derive(slots, function, out)?;
            for (function, block, local, leaf) in [
                (SemanticFunctionIdV1::from_index(0), 2, 4, 0),
                (function, 99, 4, 0),
                (function, 2, 0, 0),
                (function, 2, 99, 0),
                (function, 2, 4, 2),
            ] {
                assert!(matches!(
                    demands.leaf_required(function, block, local, leaf, out),
                    Err(Error::Statement(_))
                ));
            }
            assert!(demands.leaf_required(function, 2, 4, 0, out)?);
            assert!(!demands.leaf_required(function, 2, 4, 1, out)?);
            assert!(out.text.is_empty());
            Ok(())
        },
    )
    .0
    .unwrap();
}
