fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
}
fn operand(local: u32, kind: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, kind))
}
fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(2),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
    ))
}
fn assign(local: u32, kind: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, kind),
            SemanticRvalueV1::new(ty(kind), value),
        )),
    )
}
fn binary(
    local: u32,
    kind: u32,
    operation: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    assign(
        local,
        kind,
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        },
    )
}

fn complete_types() -> Vec<SemanticTypeDeclV1> {
    let mut types = publication_types();
    let decl = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            layout,
            shape,
        )
    };
    types.push(decl(
        191,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let integer = *types[2].layout().backend_repr();
    types.push(decl(
        193,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            integer,
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![ty(2), ty(0), ty(0)]).unwrap(),
        ),
    ));
    for (tag, pointee, size) in [(195, 16, 8), (197, 4, 16)] {
        types.push(
            decl(
                tag,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        ty(pointee),
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
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
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            size,
                            8,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
    }
    types
}

fn callable(
    tag: u8,
    operation: SemanticCompilerIntrinsicOperationV1,
    inputs: &[u32],
    output: u32,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(inputs, output, false),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

// This is an admitted semantic-model fixture. No rustc extraction, source DefId
// authentication, Verus execution, target memory eligibility or launch is claimed.
fn request(padding: bool) -> InertSemanticMirRequestV1 {
    request_with_flag_bound(padding, 128, false)
}

fn request_with_flag_bound(
    padding: bool,
    flag_length: u64,
    literal_cell_bound: bool,
) -> InertSemanticMirRequestV1 {
    use SemanticBinaryOpV1::{Equal, LessThan, Multiply, Remainder};
    use SemanticCompilerIntrinsicOperationV1 as Intrinsic;
    let source = SemanticSourceProvenanceV1::unavailable();
    let offset = u32::from(padding);
    let edge = |role, block| {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block + offset))
    };
    let call = |callee, arguments, destination, kind, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, kind),
                    edge(SemanticEdgeRoleV1::CallReturn, next),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let branch = |local, yes, no| SemanticTerminatorKindV1::SwitchInt {
        discriminant: operand(local, 15),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, no),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, yes),
        )
        .unwrap(),
    };
    let borrow = |local, kind, source_local, source_kind| {
        assign(
            local,
            kind,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(source_local, source_kind),
            },
        )
    };
    let body = vec![
        (vec![], call(1, vec![], 4, 16, 1)),
        (
            vec![borrow(5, 17, 4, 16)],
            call(2, vec![SemanticOperandV1::Move(place(5, 17))], 6, 2, 2),
        ),
        (
            vec![borrow(8, 18, 1, 4)],
            call(3, vec![SemanticOperandV1::Move(place(8, 18))], 9, 2, 3),
        ),
        (
            vec![
                assign(
                    10,
                    2,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: operand(2, 13),
                    },
                ),
                binary(11, 15, Equal, operand(9, 2), constant(128)),
            ],
            branch(11, 4, 14),
        ),
        (
            vec![binary(
                12,
                15,
                Equal,
                operand(10, 2),
                constant(u128::from(flag_length)),
            )],
            branch(12, 5, 14),
        ),
        (vec![], call(4, vec![], 14, 8, 6)),
        (vec![], call(5, vec![], 15, 8, 7)),
        (
            vec![
                assign(
                    16,
                    2,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: operand(14, 8),
                    },
                ),
                assign(
                    17,
                    2,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: operand(15, 8),
                    },
                ),
                binary(18, 2, Multiply, operand(16, 2), operand(17, 2)),
                binary(19, 15, Equal, operand(18, 2), constant(256)),
            ],
            branch(19, 8, 14),
        ),
        (
            vec![binary(13, 15, LessThan, operand(6, 2), constant(256))],
            branch(13, 9, 14),
        ),
        (
            vec![
                binary(7, 2, Remainder, operand(6, 2), constant(128)),
                binary(21, 15, LessThan, operand(7, 2), operand(9, 2)),
            ],
            branch(21, 10, 14),
        ),
        (
            vec![binary(
                23,
                15,
                LessThan,
                operand(7, 2),
                if literal_cell_bound {
                    constant(128)
                } else {
                    operand(10, 2)
                },
            )],
            branch(23, 11, 14),
        ),
        (
            vec![binary(20, 15, LessThan, operand(6, 2), constant(128))],
            branch(20, 12, 13),
        ),
        (
            vec![],
            call(
                6,
                vec![
                    SemanticOperandV1::Move(place(1, 4)),
                    operand(2, 13),
                    operand(7, 2),
                    operand(3, 1),
                ],
                22,
                14,
                14,
            ),
        ),
        (
            vec![],
            call(
                7,
                vec![
                    SemanticOperandV1::Move(place(1, 4)),
                    operand(2, 13),
                    operand(7, 2),
                ],
                22,
                14,
                14,
            ),
        ),
        (vec![], SemanticTerminatorKindV1::Return),
    ];
    let mut blocks = Vec::new();
    if padding {
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([199; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(
                    source,
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0)),
                ),
            )
            .unwrap(),
        );
    }
    blocks.extend(
        body.into_iter()
            .enumerate()
            .map(|(index, (statements, terminator))| {
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([u8::try_from(200 + index).unwrap(); 32]),
                    source,
                    statements,
                    SemanticTerminatorV1::new(source, terminator),
                )
                .unwrap()
            }),
    );
    let locals = [
        0, 4, 13, 1, 16, 17, 2, 2, 18, 2, 2, 15, 15, 15, 8, 8, 2, 2, 2, 15, 15, 15, 14, 15, 7,
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([221; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([222; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([224; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([225; 32]),
        source,
        abi(&[4, 13, 1], 0, true),
        locals
            .into_iter()
            .enumerate()
            .map(|(local, kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([u8::try_from(100 + local).unwrap(); 32]),
                    ty(kind),
                    match local {
                        0 => SemanticLocalRoleV1::Return,
                        1..=3 => SemanticLocalRoleV1::Argument(u32::try_from(local - 1).unwrap()),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"publication_model_pair".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([226; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([128, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([128, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([227; 32])),
        complete_types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            callable(
                231,
                Intrinsic::ThreadIndex1d {
                    index_witness: ty(16),
                    raw_index: ty(2),
                },
                &[],
                16,
            ),
            callable(
                232,
                Intrinsic::ThreadIndexGet {
                    index_witness: ty(16),
                    raw_index: ty(2),
                },
                &[17],
                2,
            ),
            callable(
                233,
                Intrinsic::DisjointSliceLen {
                    disjoint_slice: ty(4),
                    element: ty(1),
                    raw_index: ty(2),
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                },
                &[18],
                2,
            ),
            callable(
                234,
                Intrinsic::WorkgroupDimension(SemanticAxisV1::X),
                &[],
                8,
            ),
            callable(235, Intrinsic::GridDimension(SemanticAxisV1::X), &[], 8),
            callable(
                236,
                Intrinsic::StaticPublication128PublishF32 {
                    payload: ty(4),
                    flags: ty(13),
                    result: ty(14),
                },
                &[4, 13, 2, 1],
                14,
            ),
            callable(
                237,
                Intrinsic::StaticPublication128TryReadF32 {
                    payload: ty(4),
                    flags: ty(13),
                    result: ty(14),
                },
                &[4, 13, 2],
                14,
            ),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}
