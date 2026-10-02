// Admitted inert source/SSA fixture, not a rustc producer-authentication claim.
// Nominal values originate only in the source issuer/derive/load/fragment calls.
fn tile_fixture_array(
    types: &mut Vec<SemanticTypeDeclV1>,
    element: SemanticTypeIdV1,
) -> SemanticTypeIdV1 {
    let layout = types[element.index() as usize].layout();
    let stride = layout.size_bytes().unwrap();
    let alignment = layout.alignment_bytes();
    let bytes = stride.checked_mul(2).unwrap();
    declaration(
        types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            bytes,
            alignment,
            SemanticFieldsShapeV1::array(stride, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            bytes,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length: 2 },
        None,
    )
}

fn tile_fixture_slice(types: &mut Vec<SemanticTypeDeclV1>) -> SemanticTypeIdV1 {
    let slice = declaration(
        types,
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
        SemanticTypeShapeV1::Slice { element: U32 },
        None,
    );
    let reference = declaration(
        types,
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
                slice,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
        None,
    );
    types[reference.index() as usize] = types[reference.index() as usize]
        .clone()
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
        );
    reference
}

fn tile_fixture_abi(
    types: &[SemanticTypeDeclV1],
    slice: SemanticTypeIdV1,
    tag: u8,
    kernel: bool,
    inputs: &[(SemanticTypeIdV1, SemanticSourceArgumentOwnershipV1)],
    output: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let value = |ty| {
        if ty != slice {
            return value_abi(types, ty);
        }
        let first = SemanticAbiValueAttributesV1::new(
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
        .unwrap();
        let second = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Pair { first, second })
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
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
        inputs.len() as u32,
        inputs
            .iter()
            .map(|(ty, _)| SemanticAbiArgumentV1::source(value(*ty)))
            .collect(),
        value(output),
    )
    .unwrap()
    .with_source_argument_ownership(inputs.iter().map(|(_, ownership)| *ownership).collect())
    .unwrap()
}

fn tile_fixture_call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
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
}

fn tile_fixture_element(
    local: u32,
    field: u32,
    array: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), array).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: 2,
                    from_end: false,
                },
                element,
            )
            .unwrap(),
        ],
        element,
    )
    .unwrap()
}

pub(in super::super) fn tile_parts_repeated_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let workgroup = semantic.functions()[CALLBACK.index() as usize]
        .abi()
        .source_input_types()[0];
    let SemanticTypeShapeV1::Aggregate(fields) = types[workgroup.index() as usize].shape() else {
        panic!("the existing admitted Workgroup is an aggregate");
    };
    let index_type = fields.fields()[0];
    let marker = fields.fields()[3];
    let slice = tile_fixture_slice(&mut types);
    let workgroup_reference = reference(
        &mut types,
        workgroup,
        SemanticMutabilityV1::Immutable,
        false,
    );
    let boolean = declaration(
        &mut types,
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
        None,
    );
    let values = tile_fixture_array(&mut types, U32);
    let active = tile_fixture_array(&mut types, boolean);
    let tile = aggregate(
        &mut types,
        vec![values, active, marker, marker],
        vec![0, 8, 10, 10],
        12,
        4,
        SemanticBackendReprV1::memory(true),
        Some(SemanticExecutionRoleV29::MaskedTileU32 {
            lanes: 64,
            elements: 2,
        }),
    );
    let fragment = aggregate(
        &mut types,
        vec![values, active, marker, marker],
        vec![0, 8, 10, 10],
        12,
        4,
        SemanticBackendReprV1::memory(true),
        Some(SemanticExecutionRoleV29::LaneFragmentU32 {
            lanes: 64,
            elements: 2,
        }),
    );
    let parts = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(12),
            4,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(
                vec![0, 8],
                vec![SemanticPaddingV1::new(10, 2).unwrap()],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![values, active]).unwrap()),
        None,
    );
    let owned = SemanticSourceArgumentOwnershipV1::ByValue;
    let shared = SemanticSourceArgumentOwnershipV1::SharedBorrow;
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    assert_eq!(functions.len(), 4);
    assert_eq!(callables.len(), 6);

    // Add a real source argument; preserve the existing issuer/provider sites.
    for (function_index, tag, abi_tag, block_tag, expected_local) in [
        (0_usize, 80_u8, 81_u8, 86_u8, 3_usize),
        (1_usize, 100_u8, 101_u8, 91_u8, 6_usize),
    ] {
        let prior = &functions[function_index];
        let mut locals = prior.locals().to_vec();
        assert_eq!(locals.len(), expected_local);
        let argument = prior.abi().source_input_types().len() as u32;
        locals.push(local(
            149 + function_index as u8,
            slice,
            SemanticLocalRoleV1::Argument(argument),
        ));
        let mut inputs: Vec<_> = prior
            .abi()
            .source_input_types()
            .iter()
            .copied()
            .zip(prior.abi().source_argument_ownership().iter().copied())
            .collect();
        inputs.push((slice, shared));
        let mut blocks = prior.blocks().to_vec();
        let SemanticTerminatorKindV1::Call(call) = blocks[1].terminator().kind() else {
            panic!("the existing fixture forwards through its second block");
        };
        let mut arguments = call.arguments().to_vec();
        arguments.push(SemanticOperandV1::Copy(place(expected_local as u32, slice)));
        let call = SemanticDirectCallV1::new_callable(
            call.callee(),
            arguments,
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        blocks[1] = block(
            block_tag,
            blocks[1].statements().to_vec(),
            SemanticTerminatorKindV1::Call(call),
        );
        let mut replacement = function(
            tag,
            prior.role(),
            tile_fixture_abi(
                &types,
                slice,
                abi_tag,
                function_index == 0,
                &inputs,
                prior.abi().source_output_type(),
            ),
            locals,
            blocks,
        );
        if let Some(entry) = prior.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions[function_index] = replacement;
    }

    let callback = &functions[CALLBACK.index() as usize];
    let mut locals = callback.locals().to_vec();
    assert_eq!(locals.len(), 4);
    locals.extend([
        local(154, slice, SemanticLocalRoleV1::Argument(2)),
        local(155, workgroup_reference, SemanticLocalRoleV1::Temporary),
        local(156, workgroup_reference, SemanticLocalRoleV1::Temporary),
    ]);
    let borrow = |destination| {
        assign(
            place(destination, workgroup_reference),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, workgroup),
            },
        )
    };
    functions[CALLBACK.index() as usize] = function(
        110,
        SemanticFunctionRoleV1::InternalHelper,
        tile_fixture_abi(
            &types,
            slice,
            111,
            false,
            &[(workgroup, owned), (U32, owned), (slice, shared)],
            U32,
        ),
        locals,
        vec![
            block(
                115,
                vec![borrow(5)],
                tile_fixture_call(
                    3,
                    vec![
                        SemanticOperandV1::Copy(place(2, U32)),
                        SemanticOperandV1::Move(place(5, workgroup_reference)),
                        SemanticOperandV1::Copy(place(4, slice)),
                    ],
                    place(3, U32),
                    1,
                ),
            ),
            block(
                117,
                vec![borrow(6)],
                tile_fixture_call(
                    3,
                    vec![
                        SemanticOperandV1::Move(place(3, U32)),
                        SemanticOperandV1::Move(place(6, workgroup_reference)),
                        SemanticOperandV1::Copy(place(4, slice)),
                    ],
                    place(0, U32),
                    2,
                ),
            ),
            block(118, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    functions[3] = function(
        130,
        SemanticFunctionRoleV1::InternalHelper,
        tile_fixture_abi(
            &types,
            slice,
            131,
            false,
            &[(U32, owned), (workgroup_reference, shared), (slice, shared)],
            U32,
        ),
        vec![
            local(132, U32, SemanticLocalRoleV1::Return),
            local(133, U32, SemanticLocalRoleV1::Argument(0)),
            local(156, workgroup_reference, SemanticLocalRoleV1::Argument(1)),
            local(157, slice, SemanticLocalRoleV1::Argument(2)),
            local(158, tile, SemanticLocalRoleV1::Temporary),
            local(159, fragment, SemanticLocalRoleV1::Temporary),
            local(160, parts, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                150,
                vec![],
                tile_fixture_call(
                    6,
                    vec![
                        SemanticOperandV1::Move(place(2, workgroup_reference)),
                        SemanticOperandV1::Copy(place(3, slice)),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            index_type,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 8).unwrap(),
                            ),
                        )),
                    ],
                    place(4, tile),
                    1,
                ),
            ),
            block(
                151,
                vec![],
                tile_fixture_call(
                    7,
                    vec![SemanticOperandV1::Move(place(4, tile))],
                    place(5, fragment),
                    2,
                ),
            ),
            block(
                152,
                vec![],
                tile_fixture_call(
                    8,
                    vec![SemanticOperandV1::Move(place(5, fragment))],
                    place(6, parts),
                    3,
                ),
            ),
            block(
                153,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(tile_fixture_element(
                        6, 1, active, boolean,
                    )),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(5),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(4),
                        ),
                    )
                    .unwrap(),
                },
            ),
            block(
                154,
                vec![assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(place(1, U32)),
                        right: SemanticOperandV1::Copy(tile_fixture_element(6, 0, values, U32)),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(
                155,
                vec![assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let intrinsic = |tag, abi, operation| SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Execution(operation),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    };
    callables.extend([
        intrinsic(
            160,
            tile_fixture_abi(
                &types,
                slice,
                160,
                false,
                &[
                    (workgroup_reference, shared),
                    (slice, shared),
                    (index_type, owned),
                ],
                tile,
            ),
            SemanticExecutionOperationV29::MaskedTileLoadU32 { workgroup, tile },
        ),
        intrinsic(
            161,
            tile_fixture_abi(&types, slice, 161, false, &[(tile, owned)], fragment),
            SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 { tile, fragment },
        ),
        intrinsic(
            162,
            tile_fixture_abi(&types, slice, 162, false, &[(fragment, owned)], parts),
            SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 { fragment, parts },
        ),
    ]);
    build(types, functions, callables)
}

pub(in super::super) fn tile_parts_repeated_slot_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = tile_parts_repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let pointer = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 7);
    let SemanticTypeShapeV1::Tuple(fields) = types[locals[6].ty().index() as usize].shape() else {
        panic!("the retained Parts source result is a tuple");
    };
    let values = fields.fields()[0];
    locals.extend([
        local(170, U32, SemanticLocalRoleV1::Temporary),
        local(171, pointer, SemanticLocalRoleV1::Temporary),
        local(172, U32, SemanticLocalRoleV1::Temporary),
    ]);
    let dereference = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(8),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let mut entry = vec![
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(7, U32),
                SemanticOperandV1::Copy(place(1, U32)),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
        assign(
            place(8, pointer),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(7, U32),
            },
        ),
        assign(
            place(9, U32),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                dereference,
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
    ];
    let mut blocks = helper.blocks().to_vec();
    assert_eq!(blocks.len(), 6);
    entry.extend_from_slice(blocks[0].statements());
    blocks[0] = block(150, entry, blocks[0].terminator().kind().clone());
    blocks[4] = block(
        154,
        vec![assign(
            place(0, U32),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(place(9, U32)),
                right: SemanticOperandV1::Copy(tile_fixture_element(6, 0, values, U32)),
            },
        )],
        blocks[4].terminator().kind().clone(),
    );
    blocks[5] = block(
        155,
        vec![assign(
            place(0, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(9, U32))),
        )],
        blocks[5].terminator().kind().clone(),
    );
    functions[3] = function(130, helper.role(), helper.abi().clone(), locals, blocks);
    build(types, functions, semantic.callables().to_vec())
}

pub(in super::super) fn tile_parts_entry_slot_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = tile_parts_repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let callback = functions[CALLBACK.index() as usize].clone();
    let helper = functions[3].clone();
    let workgroup = callback.abi().source_input_types()[0];
    let slice = callback.abi().source_input_types()[2];
    let workgroup_reference = helper.locals()[2].ty();
    let tile = helper.locals()[4].ty();
    let fragment = helper.locals()[5].ty();
    let parts = helper.locals()[6].ty();
    let SemanticTypeShapeV1::Aggregate(fields) = types[workgroup.index() as usize].shape() else {
        panic!("the admitted Workgroup is an aggregate");
    };
    let index_type = fields.fields()[0];
    let SemanticTypeShapeV1::Tuple(fields) = types[parts.index() as usize].shape() else {
        panic!("Parts is a tuple");
    };
    let values = fields.fields()[0];
    let active = fields.fields()[1];
    let SemanticTypeShapeV1::Array {
        element: boolean, ..
    } = types[active.index() as usize].shape()
    else {
        panic!("Parts active lanes are an array");
    };
    let boolean = *boolean;
    let pointer = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let mut locals = callback.locals().to_vec();
    assert_eq!(locals.len(), 7);
    locals.extend([
        local(197, tile, SemanticLocalRoleV1::Temporary),
        local(198, fragment, SemanticLocalRoleV1::Temporary),
    ]);
    let mut blocks = Vec::new();
    for pass in 0u32..2 {
        let first = 3 * pass;
        let borrowed = 5 + pass;
        blocks.push(block(
            180 + first as u8,
            vec![assign(
                place(borrowed, workgroup_reference),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, workgroup),
                },
            )],
            tile_fixture_call(
                6,
                vec![
                    SemanticOperandV1::Move(place(borrowed, workgroup_reference)),
                    SemanticOperandV1::Copy(place(4, slice)),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        index_type,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
                    )),
                ],
                place(7, tile),
                first + 1,
            ),
        ));
        blocks.push(block(
            181 + first as u8,
            vec![],
            tile_fixture_call(
                7,
                vec![SemanticOperandV1::Move(place(7, tile))],
                place(8, fragment),
                first + 2,
            ),
        ));
        blocks.push(block(
            182 + first as u8,
            vec![],
            tile_fixture_call(
                3,
                vec![
                    if pass == 0 {
                        SemanticOperandV1::Copy(place(2, U32))
                    } else {
                        SemanticOperandV1::Move(place(3, U32))
                    },
                    SemanticOperandV1::Move(place(8, fragment)),
                ],
                place(if pass == 0 { 3 } else { 0 }, U32),
                first + 3,
            ),
        ));
    }
    blocks.push(block(186, vec![], SemanticTerminatorKindV1::Return));
    functions[CALLBACK.index() as usize] =
        function(110, callback.role(), callback.abi().clone(), locals, blocks);
    let dereference = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(5),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let owned = SemanticSourceArgumentOwnershipV1::ByValue;
    functions[3] = function(
        130,
        helper.role(),
        tile_fixture_abi(
            &types,
            slice,
            131,
            false,
            &[(U32, owned), (fragment, owned)],
            U32,
        ),
        vec![
            local(190, U32, SemanticLocalRoleV1::Return),
            local(191, U32, SemanticLocalRoleV1::Argument(0)),
            local(192, fragment, SemanticLocalRoleV1::Argument(1)),
            local(193, parts, SemanticLocalRoleV1::Temporary),
            local(194, U32, SemanticLocalRoleV1::Temporary),
            local(195, pointer, SemanticLocalRoleV1::Temporary),
            local(196, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                150,
                vec![
                    SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            place(4, U32),
                            SemanticOperandV1::Copy(place(1, U32)),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    assign(
                        place(5, pointer),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(4, U32),
                        },
                    ),
                    assign(
                        place(6, U32),
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            dereference,
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                ],
                tile_fixture_call(
                    8,
                    vec![SemanticOperandV1::Move(place(2, fragment))],
                    place(3, parts),
                    1,
                ),
            ),
            block(
                151,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(tile_fixture_element(
                        3, 1, active, boolean,
                    )),
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
                            SemanticBlockIdV1::from_index(2),
                        ),
                    )
                    .unwrap(),
                },
            ),
            block(
                152,
                vec![assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(place(6, U32)),
                        right: SemanticOperandV1::Copy(tile_fixture_element(3, 0, values, U32)),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(
                153,
                vec![assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(6, U32))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    build(types, functions, semantic.callables().to_vec())
}
