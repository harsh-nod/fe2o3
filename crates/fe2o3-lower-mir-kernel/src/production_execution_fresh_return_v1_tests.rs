#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum FreshReturnCaseV1 {
    Straight,
    TwoExits,
    Loop,
    Nested,
    Mixed,
    NoNormalReturn,
    SharedTarget,
    Borrowed,
}

pub(in super::super) fn fresh_tile_return_owner_v1(
    case: FreshReturnCaseV1,
) -> ProductionSemanticSsaOwnerV1 {
    let original = tile_parts_repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    let worker = &functions[3];
    let inputs: Vec<_> = worker
        .abi()
        .source_input_types()
        .iter()
        .copied()
        .zip(worker.abi().source_argument_ownership().iter().copied())
        .collect();
    let slice = inputs[2].0;
    let tile = worker.locals()[4].ty();
    let fragment = worker.locals()[5].ty();
    assert_eq!(callables.len(), 9);
    assert_eq!(functions.len(), 4);
    let output = if case == FreshReturnCaseV1::Mixed {
        declaration(
            &mut types,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                4,
                SemanticBackendReprV1::memory(true),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 12], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![fragment, U32]).unwrap()),
            None,
        )
    } else if case == FreshReturnCaseV1::Borrowed {
        inputs[1].0
    } else {
        fragment
    };
    let producer = SemanticCallableIdV1::from_index(9);
    callables.push(SemanticCallableDeclV1::Defined {
        function: SemanticFunctionIdV1::from_index(4),
    });
    let arguments = || {
        vec![
            SemanticOperandV1::Copy(place(1, U32)),
            SemanticOperandV1::Move(place(2, inputs[1].0)),
            SemanticOperandV1::Copy(place(3, slice)),
        ]
    };
    let child_call = |callee, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                callee,
                arguments(),
                Some(SemanticCallDestinationV1::new(
                    destination,
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
    let mut worker_locals = worker.locals().to_vec();
    let mut worker_blocks = worker.blocks().to_vec();
    let destination = if case == FreshReturnCaseV1::Mixed {
        let index = worker_locals.len() as u32;
        worker_locals.push(local(201, output, SemanticLocalRoleV1::Temporary));
        worker_blocks[1] = block(
            151,
            vec![
                assign(
                    place(5, fragment),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(index),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Field(0),
                                    fragment,
                                )
                                .unwrap(),
                            ],
                            fragment,
                        )
                        .unwrap(),
                    )),
                ),
                assign(
                    place(1, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(index),
                            vec![
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32)
                                    .unwrap(),
                            ],
                            U32,
                        )
                        .unwrap(),
                    )),
                ),
            ],
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        );
        place(index, output)
    } else {
        worker_blocks[1] = block(
            151,
            vec![],
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        );
        place(5, fragment)
    };
    worker_blocks[0] = block(150, vec![], child_call(producer, destination, 1));
    if case == FreshReturnCaseV1::Borrowed {
        let SemanticTerminatorKindV1::Call(load) = worker.blocks()[0].terminator().kind() else {
            panic!("source load")
        };
        let SemanticTerminatorKindV1::Call(parts) = worker.blocks()[2].terminator().kind() else {
            panic!("source parts")
        };
        let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } =
            worker.blocks()[3].terminator().kind()
        else {
            panic!("source parts switch")
        };
        worker_blocks = vec![
            block(149, vec![], child_call(producer, place(2, inputs[1].0), 1)),
            block(
                150,
                vec![],
                tile_fixture_call(6, load.arguments().to_vec(), place(4, tile), 2),
            ),
            block(
                151,
                vec![],
                tile_fixture_call(
                    7,
                    vec![SemanticOperandV1::Move(place(4, tile))],
                    place(5, fragment),
                    3,
                ),
            ),
            block(
                152,
                vec![],
                tile_fixture_call(
                    8,
                    parts.arguments().to_vec(),
                    place(6, worker.locals()[6].ty()),
                    4,
                ),
            ),
            block(
                153,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: discriminant.clone(),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(6),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(5),
                        ),
                    )
                    .unwrap(),
                },
            ),
            worker.blocks()[4].clone(),
            worker.blocks()[5].clone(),
        ];
    }
    if case == FreshReturnCaseV1::SharedTarget {
        let SemanticTerminatorKindV1::Call(load) = worker.blocks()[0].terminator().kind() else {
            panic!("source load")
        };
        let SemanticTerminatorKindV1::Call(parts) = worker.blocks()[2].terminator().kind() else {
            panic!("source parts")
        };
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = worker.blocks()[3].terminator().kind()
        else {
            panic!("source parts switch")
        };
        assert_eq!(targets.values().len(), 1);
        worker_blocks = vec![
            block(
                148,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(1, U32)),
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
            ),
            block(149, vec![], child_call(producer, place(5, fragment), 4)),
            block(
                150,
                vec![],
                tile_fixture_call(6, load.arguments().to_vec(), place(4, tile), 3),
            ),
            block(
                151,
                vec![],
                tile_fixture_call(
                    7,
                    vec![SemanticOperandV1::Move(place(4, tile))],
                    place(5, fragment),
                    4,
                ),
            ),
            block(
                152,
                vec![],
                tile_fixture_call(
                    8,
                    parts.arguments().to_vec(),
                    place(6, worker.locals()[6].ty()),
                    5,
                ),
            ),
            block(
                153,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: discriminant.clone(),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(7),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(6),
                        ),
                    )
                    .unwrap(),
                },
            ),
            worker.blocks()[4].clone(),
            worker.blocks()[5].clone(),
        ];
    }
    // Added entry blocks precede the original 150..155 identities; no sorting
    // or edge renumbering is allowed to change the intended source CFG.
    let original_offset = match case {
        FreshReturnCaseV1::Borrowed => 1,
        FreshReturnCaseV1::SharedTarget => 2,
        _ => 0,
    };
    assert_eq!(worker_blocks.len(), worker.blocks().len() + original_offset);
    for (old, new) in worker
        .blocks()
        .iter()
        .zip(&worker_blocks[original_offset..])
    {
        assert_eq!(new.identity(), old.identity());
        assert_eq!(new.source(), old.source());
    }
    assert!(
        worker_blocks
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    functions[3] = function(
        130,
        SemanticFunctionRoleV1::InternalHelper,
        worker.abi().clone(),
        worker_locals,
        worker_blocks,
    );

    let mut child_locals = vec![
        local(203, output, SemanticLocalRoleV1::Return),
        local(204, U32, SemanticLocalRoleV1::Argument(0)),
        local(205, inputs[1].0, SemanticLocalRoleV1::Argument(1)),
        local(206, slice, SemanticLocalRoleV1::Argument(2)),
        local(207, tile, SemanticLocalRoleV1::Temporary),
    ];
    let fragment_local = if output == fragment {
        0
    } else {
        child_locals.push(local(208, fragment, SemanticLocalRoleV1::Temporary));
        5
    };
    // Reuse the actual source load operands and intrinsic identities, changing
    // only the local return destination and original control-flow owner.
    let SemanticTerminatorKindV1::Call(load) =
        semantic.functions()[3].blocks()[0].terminator().kind()
    else {
        panic!("tile load")
    };
    let mut child_blocks = vec![
        block(209, vec![], SemanticTerminatorKindV1::Call(load.clone())),
        block(
            210,
            vec![],
            tile_fixture_call(
                7,
                vec![SemanticOperandV1::Move(place(4, tile))],
                place(fragment_local, fragment),
                2,
            ),
        ),
    ];
    match case {
        FreshReturnCaseV1::TwoExits | FreshReturnCaseV1::Loop => {
            child_blocks.push(block(
                211,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(1, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(3),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(4),
                        ),
                    )
                    .unwrap(),
                },
            ));
            child_blocks.push(block(
                212,
                vec![],
                if case == FreshReturnCaseV1::Loop {
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(2),
                    ))
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ));
            child_blocks.push(block(213, vec![], SemanticTerminatorKindV1::Return));
        }
        FreshReturnCaseV1::Mixed => {
            child_blocks.push(block(
                211,
                vec![assign(
                    place(0, output),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![
                                SemanticOperandV1::Move(place(5, fragment)),
                                SemanticOperandV1::Copy(place(1, U32)),
                            ],
                        )
                        .unwrap(),
                    ),
                )],
                SemanticTerminatorKindV1::Return,
            ));
        }
        FreshReturnCaseV1::NoNormalReturn | FreshReturnCaseV1::SharedTarget => {
            child_blocks.push(block(
                211,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(2),
                )),
            ))
        }
        _ => child_blocks.push(block(211, vec![], SemanticTerminatorKindV1::Return)),
    }
    let mut child_abi = tile_fixture_abi(&types, slice, 214, false, &inputs, output);
    if case == FreshReturnCaseV1::Borrowed {
        let pointee = types[output.index() as usize]
            .abi_properties()
            .first_pointee()
            .unwrap();
        let returned_reference = SemanticAbiValueV1::new(
            output,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    (pointee.reliable_alignment_bytes() > 1)
                        .then_some(pointee.reliable_alignment_bytes()),
                )
                .unwrap(),
            ),
        );
        // Shared-reference argument attributes do not describe a returned
        // reference. Preserve the exact source signature/argument ownership.
        let replacement = SemanticFunctionAbiV1::from_rustc(
            child_abi.identity(),
            child_abi.layout_identity(),
            child_abi.canon_abi(),
            child_abi.extern_abi(),
            child_abi.can_unwind(),
            child_abi.c_variadic(),
            child_abi.fixed_count(),
            child_abi.arguments().to_vec(),
            returned_reference,
        )
        .unwrap()
        .with_source_argument_ownership(child_abi.source_argument_ownership().to_vec())
        .unwrap();
        assert_eq!(replacement.identity(), child_abi.identity());
        assert_eq!(replacement.source_signature(), child_abi.source_signature());
        assert_eq!(replacement.arguments(), child_abi.arguments());
        assert_eq!(
            replacement.source_argument_ownership(),
            child_abi.source_argument_ownership()
        );
        child_abi = replacement;
        let SemanticTypeShapeV1::Pointer(reference) = types[output.index() as usize].shape() else {
            panic!("source workgroup reference")
        };
        child_locals.truncate(4);
        child_blocks = vec![block(
            235,
            vec![assign(
                place(0, output),
                SemanticRvalueKindV1::Borrow {
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(2),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Dereference,
                                reference.pointee(),
                            )
                            .unwrap(),
                        ],
                        reference.pointee(),
                    )
                    .unwrap(),
                    kind: SemanticBorrowKindV1::Shared,
                },
            )],
            SemanticTerminatorKindV1::Return,
        )];
    }
    if case == FreshReturnCaseV1::Nested {
        callables.push(SemanticCallableDeclV1::Defined {
            function: SemanticFunctionIdV1::from_index(5),
        });
        functions.push(function(
            215,
            SemanticFunctionRoleV1::InternalHelper,
            child_abi.clone(),
            child_locals[..4].to_vec(),
            vec![
                block(
                    216,
                    vec![],
                    child_call(SemanticCallableIdV1::from_index(10), place(0, output), 1),
                ),
                block(217, vec![], SemanticTerminatorKindV1::Return),
            ],
        ));
        functions.push(function(
            218,
            SemanticFunctionRoleV1::InternalHelper,
            tile_fixture_abi(&types, slice, 219, false, &inputs, output),
            child_locals,
            child_blocks,
        ));
    } else {
        functions.push(function(
            215,
            SemanticFunctionRoleV1::InternalHelper,
            child_abi,
            child_locals,
            child_blocks,
        ));
    }
    // Defined declarations must remain the canonical callable prefix. The
    // construction above uses the old intrinsic coordinates until all bodies
    // exist; remap them together, preserving every referenced declaration.
    let old_function_count = semantic.functions().len();
    let old_callable_count = semantic.callables().len();
    let added = functions.len() - old_function_count;
    let prior_callables = callables;
    let callables: Vec<_> = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .chain(
            prior_callables[old_function_count..old_callable_count]
                .iter()
                .cloned(),
        )
        .collect();
    for prior in &mut functions {
        let blocks = prior
            .blocks()
            .iter()
            .map(|source_block| {
                let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
                    return source_block.clone();
                };
                let old = call.callee().index() as usize;
                let target = if old < old_function_count {
                    old
                } else if old < old_callable_count {
                    old + added
                } else {
                    old_function_count + old - old_callable_count
                };
                assert_eq!(callables[target], prior_callables[old]);
                assert!(call.variadic_argument_abis().is_empty());
                assert!(call.inline_assembly_source_v30().is_none());
                assert!(call.ordered_region_source_v31().is_none());
                assert!(call.ordered_program_source_v32().is_none());
                SemanticBasicBlockV1::new(
                    source_block.identity(),
                    source_block.source(),
                    source_block.statements().to_vec(),
                    SemanticTerminatorV1::new(
                        source_block.terminator().source(),
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(target as u32),
                                call.arguments().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        ),
                    ),
                )
                .unwrap()
            })
            .collect();
        let mut replacement = SemanticFunctionDeclV1::new(
            prior.identity(),
            prior.role(),
            prior.item_definition_identity(),
            prior.monomorphization_identity(),
            prior.generic_type_arguments_identity(),
            prior.const_generic_arguments_identity(),
            prior.source(),
            prior.abi().clone(),
            prior.locals().to_vec(),
            prior.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = prior.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        assert_eq!(replacement.identity(), prior.identity());
        assert_eq!(replacement.abi(), prior.abi());
        assert_eq!(replacement.locals(), prior.locals());
        assert_eq!(replacement.entry(), prior.entry());
        assert_eq!(replacement.kernel_entry(), prior.kernel_entry());
        for (after, before) in replacement.blocks().iter().zip(prior.blocks()) {
            assert_eq!(after.identity(), before.identity());
            assert_eq!(after.source(), before.source());
            assert_eq!(after.statements(), before.statements());
            match (after.terminator().kind(), before.terminator().kind()) {
                (SemanticTerminatorKindV1::Call(after), SemanticTerminatorKindV1::Call(before)) => {
                    assert_eq!(after.arguments(), before.arguments());
                    assert_eq!(after.destination(), before.destination());
                    assert_eq!(after.unwind(), before.unwind());
                    assert_eq!(
                        callables[after.callee().index() as usize],
                        prior_callables[before.callee().index() as usize]
                    );
                }
                (after, before) => assert_eq!(after, before),
            }
        }
        *prior = replacement;
    }
    assert_eq!(
        &callables[..old_function_count],
        &semantic.callables()[..old_function_count]
    );
    assert_eq!(
        &callables[functions.len()..],
        &semantic.callables()[old_function_count..]
    );
    build(types, functions, callables)
}

#[test]
fn fresh_return_fixtures_preserve_original_provider_and_intrinsic_identities() {
    for case in [
        FreshReturnCaseV1::Straight,
        FreshReturnCaseV1::TwoExits,
        FreshReturnCaseV1::Loop,
        FreshReturnCaseV1::Nested,
        FreshReturnCaseV1::Mixed,
        FreshReturnCaseV1::NoNormalReturn,
        FreshReturnCaseV1::SharedTarget,
        FreshReturnCaseV1::Borrowed,
    ] {
        let owner = fresh_tile_return_owner_v1(case);
        assert_eq!(
            owner.source_semantic().functions().len(),
            if case == FreshReturnCaseV1::Nested {
                6
            } else {
                5
            }
        );
        let function = &owner.source_semantic().functions()[4];
        assert_eq!(function.abi().source_input_types().len(), 3);
        assert_ne!(function.abi().source_output_type(), U32);
        assert_eq!(
            function.locals()[0].ty(),
            function.abi().source_output_type()
        );
    }
}
