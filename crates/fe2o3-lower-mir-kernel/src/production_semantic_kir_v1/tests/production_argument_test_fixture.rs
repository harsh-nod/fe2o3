const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const ZERO: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

fn argument_owner(expanded: bool, empty: bool, shared: bool) -> ProductionSemanticSsaOwnerV1 {
    argument_owner_shape(
        expanded,
        if empty {
            ArgumentTupleShape::EmptyUnit
        } else {
            ArgumentTupleShape::Mixed
        },
        shared,
        false,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum ArgumentTupleShape {
    Mixed,
    AllZero,
    EmptyUnit,
    EmptyTuple,
}

fn argument_owner_shape(
    expanded: bool,
    shape: ArgumentTupleShape,
    shared: bool,
    deep_zero: bool,
) -> ProductionSemanticSsaOwnerV1 {
    argument_owner_shape_with_fixed(expanded, shape, shared, deep_zero, true)
}

fn argument_owner_shape_with_fixed(
    expanded: bool,
    shape: ArgumentTupleShape,
    shared: bool,
    deep_zero: bool,
    fixed: bool,
) -> ProductionSemanticSsaOwnerV1 {
    argument_call_owner(
        expanded,
        shape,
        shared,
        deep_zero,
        fixed,
        ArgumentCallResult::Zero,
        false,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArgumentCallResult {
    Zero,
    Scalar,
    Retained,
    Projected,
}

#[allow(clippy::too_many_arguments)]
fn argument_call_owner(
    expanded: bool,
    shape: ArgumentTupleShape,
    shared: bool,
    deep_zero: bool,
    fixed: bool,
    result_mode: ArgumentCallResult,
    two_calls: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let scalar_result = result_mode != ArgumentCallResult::Zero;
    let private_result = matches!(
        result_mode,
        ArgumentCallResult::Retained | ArgumentCallResult::Projected
    );
    assert!(!scalar_result || shape == ArgumentTupleShape::Mixed);
    let result_ty = if scalar_result { U32 } else { UNIT };
    let empty = matches!(
        shape,
        ArgumentTupleShape::EmptyUnit | ArgumentTupleShape::EmptyTuple
    );
    let middle = if deep_zero { ZERO } else { UNIT };
    let tuple_fields = match shape {
        ArgumentTupleShape::Mixed => vec![PAIR, middle, U32],
        ArgumentTupleShape::AllZero => vec![ZERO, UNIT],
        _ => vec![],
    };
    let source = SemanticSourceProvenanceV1::unavailable();
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    let tuple_type = |tag, fields, offsets, size, alignment, backend| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(size),
                alignment,
                backend,
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
        )
    };
    let mut types = vec![
        unit_type(),
        plain_bit_scalar_type(
            5,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        tuple_type(
            10,
            vec![U32, UNIT, U32],
            vec![0, 4, 4],
            8,
            4,
            SemanticBackendReprV1::scalar_pair(scalar, scalar),
        ),
        tuple_type(
            11,
            if deep_zero {
                vec![
                    SemanticTypeIdV1::from_index(5),
                    SemanticTypeIdV1::from_index(6),
                    SemanticTypeIdV1::from_index(7),
                ]
            } else {
                vec![UNIT, UNIT]
            },
            if deep_zero { vec![0, 0, 0] } else { vec![0, 0] },
            0,
            if deep_zero { 4 } else { 1 },
            SemanticBackendReprV1::memory(true),
        ),
    ];
    types.push(if shape == ArgumentTupleShape::EmptyUnit {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([12; 32]),
            SemanticLayoutIdentityV1::from_sha256([12; 32]),
            unit_type().layout().clone(),
            SemanticTypeShapeV1::Unit,
        )
    } else {
        tuple_type(
            12,
            tuple_fields.clone(),
            if shape == ArgumentTupleShape::Mixed {
                vec![0, 8, 8]
            } else {
                vec![0; tuple_fields.len()]
            },
            if shape == ArgumentTupleShape::Mixed {
                12
            } else {
                0
            },
            if shape == ArgumentTupleShape::Mixed
                || (shape == ArgumentTupleShape::AllZero && deep_zero)
            {
                4
            } else {
                1
            },
            SemanticBackendReprV1::memory(true),
        )
    });
    if deep_zero {
        types.push(tuple_type(
            13,
            vec![UNIT, UNIT],
            vec![0, 0],
            0,
            1,
            SemanticBackendReprV1::memory(true),
        ));
        for (tag, element, length, stride, alignment) in [(14, UNIT, 2, 0, 1), (15, U32, 0, 4, 4)] {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    alignment,
                    SemanticFieldsShapeV1::array(stride, length),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    alignment,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Array { element, length },
            ));
        }
    }
    let reference_ty = SemanticTypeIdV1::from_index(types.len() as u32);
    if private_result {
        types.push(call_result_reference_type());
    }
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let value = |ty| {
        SemanticAbiValueV1::new(
            ty,
            match ty {
                U32 => SemanticAbiPassModeV1::Direct(attrs),
                PAIR => SemanticAbiPassModeV1::Pair {
                    first: attrs,
                    second: attrs,
                },
                _ => SemanticAbiPassModeV1::Ignore,
            },
        )
    };
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            source,
        )
    };
    let place =
        |id, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(id), vec![], ty).unwrap();
    let block = |tag, statements, kind| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, kind),
        )
        .unwrap()
    };
    let function = |tag, role, abi, locals, blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let mut adjusted = if fixed {
        vec![SemanticAbiArgumentV1::source(value(ZERO))]
    } else {
        vec![]
    };
    if !empty {
        adjusted.extend(tuple_fields.iter().copied().enumerate().map(|(field, ty)| {
            SemanticAbiArgumentV1::rust_call_tuple_field(field as u32, value(ty))
        }));
    }
    let helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        u32::from(fixed),
        if fixed {
            vec![ZERO, TUPLE]
        } else {
            vec![TUPLE]
        },
        result_ty,
        adjusted,
        value(result_ty),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue;
        usize::from(fixed) + 1
    ])
    .unwrap();
    let mut helper_locals = vec![local(21, result_ty, SemanticLocalRoleV1::Return)];
    if fixed {
        helper_locals.push(local(22, ZERO, SemanticLocalRoleV1::Argument(0)));
    }
    if !expanded {
        helper_locals.push(local(
            23,
            TUPLE,
            SemanticLocalRoleV1::Argument(u32::from(fixed)),
        ));
    } else if empty {
        helper_locals.push(local(23, UNIT, SemanticLocalRoleV1::Temporary));
    } else {
        for (index, (field, ty)) in tuple_fields.iter().copied().enumerate().rev().enumerate() {
            helper_locals.push(local(
                23 + index as u8,
                ty,
                SemanticLocalRoleV1::RustCallTupleField {
                    argument: u32::from(fixed),
                    field: field as u32,
                },
            ));
        }
    }
    let assign_use = |destination, operand| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(U32, SemanticRvalueKindV1::Use(operand)),
            )),
        )
    };
    let helper_statements = if scalar_result {
        let from = if expanded {
            place(1 + u32::from(fixed), U32)
        } else {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1 + u32::from(fixed)),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(2), U32).unwrap()],
                U32,
            )
            .unwrap()
        };
        vec![assign_use(place(0, U32), SemanticOperandV1::Copy(from))]
    } else {
        vec![]
    };
    let helper = function(
        20,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        helper_locals,
        if scalar_result {
            use fe2o3_mir_model::semantic_mir_v1::{
                SemanticSwitchTargetV1, SemanticSwitchTargetsV1,
            };
            vec![
                block(
                    26,
                    helper_statements,
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(0, U32)),
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
                block(27, vec![], SemanticTerminatorKindV1::Return),
                block(28, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![block(
                26,
                helper_statements,
                SemanticTerminatorKindV1::Return,
            )]
        },
    );
    let mut functions = vec![helper];
    for ordinal in 1..=if shared { 2 } else { 1 } {
        let tag = 40 + ordinal * 20;
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            4,
            [U32, PAIR, PAIR, ZERO]
                .into_iter()
                .map(|ty| SemanticAbiArgumentV1::source(value(ty)))
                .collect(),
            value(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 4])
        .unwrap();
        let mut locals = vec![local(tag + 1, UNIT, SemanticLocalRoleV1::Return)];
        locals.extend(
            [U32, PAIR, PAIR, ZERO]
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    local(
                        tag + 2 + index as u8,
                        ty,
                        SemanticLocalRoleV1::Argument(index as u32),
                    )
                }),
        );
        locals.push(local(tag + 6, TUPLE, SemanticLocalRoleV1::Temporary));
        if scalar_result {
            locals.push(local(tag + 10, U32, SemanticLocalRoleV1::Temporary));
        }
        if private_result {
            locals.push(local(
                tag + 11,
                reference_ty,
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let observed = locals.len() as u32;
        if scalar_result {
            locals.push(local(tag + 12, U32, SemanticLocalRoleV1::Temporary));
        }
        let tuple_value = if shape != ArgumentTupleShape::Mixed {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                TUPLE,
                SemanticConstantValueV1::ZeroSized,
            )))
        } else {
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(2, PAIR)),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            middle,
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Copy(if scalar_result {
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(2),
                                vec![
                                    SemanticProjectionV1::new(
                                        SemanticProjectionKindV1::Field(0),
                                        U32,
                                    )
                                    .unwrap(),
                                ],
                                U32,
                            )
                            .unwrap()
                        } else {
                            place(1, U32)
                        }),
                    ],
                )
                .unwrap(),
            )
        };
        let assign = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(5, TUPLE),
                SemanticRvalueV1::new(TUPLE, tuple_value),
            )),
        );
        let mut setup = vec![assign];
        if private_result {
            setup.push(assign_use(
                place(6, U32),
                SemanticOperandV1::Copy(place(1, U32)),
            ));
            setup.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(7, reference_ty),
                    SemanticRvalueV1::new(
                        reference_ty,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: place(6, U32),
                        },
                    ),
                )),
            ));
        }
        let destination = match result_mode {
            ArgumentCallResult::Zero => place(0, UNIT),
            ArgumentCallResult::Scalar | ArgumentCallResult::Retained => place(6, U32),
            ArgumentCallResult::Projected => SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(7),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
                ],
                U32,
            )
            .unwrap(),
        };
        let call = |target| {
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                if fixed {
                    vec![
                        SemanticOperandV1::Copy(place(4, ZERO)),
                        SemanticOperandV1::Copy(place(5, TUPLE)),
                    ]
                } else {
                    vec![SemanticOperandV1::Copy(place(5, TUPLE))]
                },
                Some(SemanticCallDestinationV1::new(
                    destination.clone(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap()
        };
        let mut blocks = vec![block(
            tag + 7,
            setup,
            SemanticTerminatorKindV1::Call(call(1)),
        )];
        if two_calls {
            blocks.push(block(
                tag + 8,
                vec![],
                SemanticTerminatorKindV1::Call(call(2)),
            ));
        }
        blocks.push(block(
            if two_calls { tag + 13 } else { tag + 8 },
            if scalar_result {
                vec![assign_use(
                    place(observed, U32),
                    SemanticOperandV1::Copy(place(6, U32)),
                )]
            } else {
                vec![]
            },
            SemanticTerminatorKindV1::Return,
        ));
        functions.push(
            function(tag, SemanticFunctionRoleV1::KernelRoot, abi, locals, blocks)
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(format!("argument_root_{ordinal}").into_bytes())
                        .unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256([tag + 9; 32]),
                    SemanticKernelSourceContractV1::new(
                        Some(
                            SemanticKernelLaunchBoundsV1::new(
                                Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                                Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                                None,
                            )
                            .unwrap(),
                        ),
                        None,
                        None,
                    )
                    .unwrap(),
                )),
        );
    }
    let roots = (1..functions.len())
        .map(|index| SemanticFunctionIdV1::from_index(index as u32))
        .collect();
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        roots,
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn call_result_reference_type() -> SemanticTypeDeclV1 {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiPointeeInfoV1, SemanticAbiPointeeKindV1, SemanticPointerTypeV1,
        SemanticTypeAbiPropertiesV1,
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([16; 32]),
        SemanticLayoutIdentityV1::from_sha256([16; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    4,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}
