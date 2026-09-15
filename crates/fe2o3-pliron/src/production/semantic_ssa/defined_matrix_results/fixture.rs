use fe2o3_mir_model::semantic_mir_v1::*;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn location() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, value: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(value)).unwrap()
}
fn local(tag: u8, value: u32, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([tag; 32]),
        ty(value),
        role,
        location(),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        location(),
        statements,
        SemanticTerminatorV1::new(location(), terminator),
    )
    .unwrap()
}
fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    local: u32,
    value: u32,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(local, value),
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
fn abi(tag: u8, inputs: &[u32], output: u32, kernel: bool) -> SemanticFunctionAbiV1 {
    let attributes = SemanticAbiValueAttributesV1::new(
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
        None,
    )
    .unwrap();
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([142; 32]),
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
            .map(|&input| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(input),
                    SemanticAbiPassModeV1::Direct(attributes),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(ty(output), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow;
        inputs.len()
    ])
    .unwrap()
}
fn function(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
    kernel: bool,
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
        location(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn terminal(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            location(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

/// Inert component fixture, not authenticated provider or production root input.
pub(super) fn source(repeated: bool, annotated: bool) -> AdmittedInertSemanticMirV1 {
    let zst = |tag: u8| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        )
    };
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([131; 32]),
            SemanticLayoutIdentityV1::from_sha256([132; 32]),
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
        ),
        zst(210),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([211; 32]),
            SemanticLayoutIdentityV1::from_sha256([211; 32]),
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
                    ty(1),
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
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            0,
                            1,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([212; 32]),
            SemanticLayoutIdentityV1::from_sha256([212; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0, 0, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(4), ty(5), ty(6)]).unwrap(),
            ),
        ),
        zst(213),
        zst(214),
        zst(215),
    ];
    let mut root_blocks = vec![
        block(180, vec![], call(3, vec![], 1, 1, 1)),
        block(
            181,
            vec![SemanticStatementV1::new(
                location(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(2, 2),
                    SemanticRvalueV1::new(
                        ty(2),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(1, 1),
                        },
                    ),
                )),
            )],
            call(1, vec![SemanticOperandV1::Copy(place(2, 2))], 3, 3, 2),
        ),
    ];
    if repeated {
        root_blocks.push(block(
            182,
            vec![],
            call(1, vec![SemanticOperandV1::Copy(place(2, 2))], 3, 3, 3),
        ));
    }
    root_blocks.push(block(183, vec![], SemanticTerminatorKindV1::Return));
    let root = function(
        135,
        abi(133, &[], 0, true),
        vec![
            local(180, 0, SemanticLocalRoleV1::Return),
            local(181, 1, SemanticLocalRoleV1::Temporary),
            local(182, 2, SemanticLocalRoleV1::Temporary),
            local(183, 3, SemanticLocalRoleV1::Temporary),
        ],
        root_blocks,
        true,
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"defined_matrix_results_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([143; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let getter = function(
        160,
        abi(160, &[2], 3, false),
        vec![
            local(180, 3, SemanticLocalRoleV1::Return),
            local(181, 2, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![
            block(180, vec![], call(2, vec![], 0, 3, 1)),
            block(181, vec![], SemanticTerminatorKindV1::Return),
        ],
        false,
    );
    let bridge = function(
        161,
        abi(161, &[], 3, false),
        vec![
            local(180, 3, SemanticLocalRoleV1::Return),
            local(181, 4, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(180, vec![], call(4, vec![], 1, 4, 1)),
            block(181, vec![], SemanticTerminatorKindV1::Return),
        ],
        false,
    );
    let mut functions = vec![root, getter, bridge];
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(2)),
        terminal(
            180,
            abi(180, &[], 1, false),
            SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: ty(1) },
        ),
        terminal(
            181,
            abi(181, &[], 4, false),
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: ty(4) },
        ),
    ];
    if annotated {
        let provenance = SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            SemanticKernelBindingIdentityV1::from_sha256([143; 32]),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
        )
        .unwrap();
        let record = SemanticKernelMatrixDeriveV1::for_defined_function(
            SemanticFunctionIdV1::from_index(1),
            &functions,
            &callables,
            &types,
            SemanticKernelMatrixDeriveTypesV1::new([2, 1, 3, 4, 5, 6].map(ty)),
            provenance,
            SemanticTypeIdentityV1::from_sha256([7; 32]),
        )
        .unwrap();
        functions[1] = functions[1]
            .clone()
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record),
            )
            .unwrap();
    }
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([142; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v23(SemanticMirLimitsV1::default())
    .unwrap()
}
