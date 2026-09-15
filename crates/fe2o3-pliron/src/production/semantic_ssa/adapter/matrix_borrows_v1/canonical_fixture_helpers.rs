fn ty(i: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(i)
}
fn loc() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn local_id(i: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(i)
}
fn place(i: u32, t: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local_id(i), vec![], ty(t)).unwrap()
}
fn copy(i: u32, t: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(i, t))
}
fn zero(t: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(t),
        SemanticConstantValueV1::ZeroSized,
    ))
}
fn aggregate(fields: Vec<SemanticOperandV1>) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Aggregate(
        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, fields).unwrap(),
    )
}
fn assign(i: u32, t: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        loc(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(i, t),
            SemanticRvalueV1::new(ty(t), value),
        )),
    )
}
fn borrow(i: u32, t: u32, owner: u32, owned: u32) -> SemanticStatementV1 {
    assign(
        i,
        t,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(owner, owned),
        },
    )
}
fn block(
    i: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([i + 60; 32]),
        loc(),
        statements,
        SemanticTerminatorV1::new(loc(), terminator),
    )
    .unwrap()
}
fn call(
    callee: u32,
    args: Vec<SemanticOperandV1>,
    local: u32,
    t: u32,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            args,
            Some(SemanticCallDestinationV1::new(
                place(local, t),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(next),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn scalar() -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
}
fn unit() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([100; 32]),
        SemanticLayoutIdentityV1::from_sha256([100; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )
}
fn zst(t: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([100 + t; 32]),
        SemanticLayoutIdentityV1::from_sha256([100 + t; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}
fn pointee(size: u64, align: u64) -> SemanticAbiPointeeInfoV1 {
    SemanticAbiPointeeInfoV1::new(
        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        size,
        align,
    )
    .unwrap()
}
fn reference(t: u8, owned: u32) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([100 + t; 32]),
        SemanticLayoutIdentityV1::from_sha256([100 + t; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(scalar()),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(owned),
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
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_rustc_layout_is_noundef(true)
            .with_scalar_pointee_info(
                Some(if owned == 7 {
                    pointee(16, 8)
                } else {
                    pointee(0, 1)
                }),
                None,
            ),
    )
}
fn aggregate_type(
    t: u8,
    fields: &[u32],
    offsets: &[u64],
    size: u64,
    pair: bool,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([100 + t; 32]),
        SemanticLayoutIdentityV1::from_sha256([100 + t; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(size),
            8,
            if pair {
                SemanticBackendReprV1::ScalarPair {
                    first: scalar(),
                    second: scalar(),
                }
            } else {
                SemanticBackendReprV1::scalar(scalar())
            },
            false,
            SemanticAggregateLayoutV1::new(offsets.to_vec(), vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(fields.iter().copied().map(ty).collect()).unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_rustc_layout_is_noundef(true)
            .with_scalar_pointee_info(
                Some(if pair { pointee(0, 1) } else { pointee(16, 8) }),
                pair.then_some(pointee(0, 1)),
            ),
    )
}
fn attributes() -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}
fn input_mode(t: u32) -> SemanticAbiPassModeV1 {
    if matches!(t, 5 | 6 | 8 | 12) {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                if t == 8 { 16 } else { 0 },
                (t == 8).then_some(8),
            )
            .unwrap(),
        )
    } else {
        mode(t)
    }
}
fn mode(t: u32) -> SemanticAbiPassModeV1 {
    match t {
        7 => SemanticAbiPassModeV1::Pair {
            first: attributes(),
            second: attributes(),
        },
        8 | 9 => SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                Some(8),
            )
            .unwrap(),
        ),
        5 | 6 | 12 => SemanticAbiPassModeV1::Direct(attributes()),
        _ => SemanticAbiPassModeV1::Ignore,
    }
}
fn abi(tag: u8, kernel: bool, inputs: &[u32], output: u32) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([90; 32]),
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
        inputs.iter().copied().map(ty).collect(),
        ty(output),
        inputs
            .iter()
            .map(|&t| SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty(t), input_mode(t))))
            .collect(),
        SemanticAbiValueV1::new(ty(output), mode(output)),
    )
    .unwrap()
    .with_source_argument_ownership(
        inputs
            .iter()
            .map(|t| match t {
                5 | 6 | 8 | 12 => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                _ => SemanticSourceArgumentOwnershipV1::ByValue,
            })
            .collect(),
    )
    .unwrap()
}
fn function(
    tag: u8,
    kernel: bool,
    inputs: &[u32],
    output: u32,
    locals: &[(u32, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if kernel {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        loc(),
        abi(tag, kernel, inputs, output),
        locals
            .iter()
            .enumerate()
            .map(|(i, &(t, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 70; 32]),
                    ty(t),
                    role,
                    loc(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn terminal(
    tag: u8,
    inputs: &[u32],
    output: u32,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            loc(),
            abi(tag, false, inputs, output),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}
