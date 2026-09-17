fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let mut types = read_only_allocation_v1_tests::types(true);
    let declaration = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            layout,
            shape,
        )
    };
    types.push(plain_bit_scalar_type(
        161,
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        },
    ));
    let SemanticBackendReprV1::Scalar(integer) = *types[8].layout().backend_repr() else {
        unreachable!();
    };
    let SemanticBackendReprV1::Scalar(float) = *types[1].layout().backend_repr() else {
        unreachable!();
    };
    let SemanticBackendReprV1::Scalar(length) = *types[2].layout().backend_repr() else {
        unreachable!();
    };
    for index in 9..=11 {
        let declaration = declaration(
            162 + index as u8,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::Scalar(integer),
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(index - 1)]).unwrap(),
            ),
        );
        types.push(if index == 11 {
            declaration.with_rust_type_kind(SemanticRustTypeKindV1::CoreAtomicU32)
        } else {
            declaration
        });
    }
    types.push(declaration(
        181,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::Array {
                stride_bytes: 4,
                count: 0,
            },
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
        SemanticTypeShapeV1::Slice { element: ty(11) },
    ));
    let reference = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
    );
    types.push(
        declaration(
            183,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: reference,
                    second: length,
                },
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(12),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    types.push(declaration(
        185,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::ScalarPair {
                first: integer,
                second: float,
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(8), ty(1)]).unwrap()),
    ));
    types
}

fn abi(inputs: &[u32], output: u32, kernel: bool) -> SemanticFunctionAbiV1 {
    let initialized = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let reference = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let value = |index| {
        SemanticAbiValueV1::new(
            ty(index),
            match index {
                0 => SemanticAbiPassModeV1::Ignore,
                4 | 14 => SemanticAbiPassModeV1::Pair {
                    first: initialized,
                    second: initialized,
                },
                13 => SemanticAbiPassModeV1::Pair {
                    first: reference,
                    second: initialized,
                },
                _ => SemanticAbiPassModeV1::Direct(initialized),
            },
        )
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([92; 32]),
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
            .copied()
            .map(|index| SemanticAbiArgumentV1::source(value(index)))
            .collect(),
        value(output),
    )
    .unwrap()
    .with_source_argument_ownership(
        inputs
            .iter()
            .map(|index| match index {
                4 => SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                13 => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                _ => SemanticSourceArgumentOwnershipV1::ByValue,
            })
            .collect(),
    )
    .unwrap()
}

fn request(publish: bool, copy: bool, exclusive: bool) -> InertSemanticMirRequestV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut inputs = vec![4, 13, 2];
    let mut arguments = vec![
        if copy {
            SemanticOperandV1::Copy(place(1, 4))
        } else {
            SemanticOperandV1::Move(place(1, 4))
        },
        SemanticOperandV1::Copy(place(2, 13)),
        SemanticOperandV1::Copy(place(3, 2)),
    ];
    if publish {
        inputs.push(1);
        arguments.push(SemanticOperandV1::Copy(place(4, 1)));
    }
    let mut kernel_abi = abi(&[4, 13, 2, 1], 0, true);
    if !exclusive {
        kernel_abi = kernel_abi
            .with_source_argument_ownership(vec![
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ])
            .unwrap();
    }
    let operation = if publish {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
            payload: ty(4),
            flags: ty(13),
            result: ty(14),
        }
    } else {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
            payload: ty(4),
            flags: ty(13),
            result: ty(14),
        }
    };
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([101; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([102; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([103; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([104; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([105; 32]),
            source,
            abi(&inputs, 14, false),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([106; 32]),
    };
    let blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([111; 32]),
            source,
            vec![],
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        arguments,
                        Some(SemanticCallDestinationV1::new(
                            place(5, 14),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([112; 32]),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([121; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([122; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([123; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([124; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([125; 32]),
        source,
        kernel_abi,
        [0, 4, 13, 2, 1, 14, 7]
            .into_iter()
            .enumerate()
            .map(|(local, kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([130 + local as u8; 32]),
                    ty(kind),
                    match local {
                        0 => SemanticLocalRoleV1::Return,
                        1..=4 => SemanticLocalRoleV1::Argument(local as u32 - 1),
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
        SemanticLinkSymbolV1::new(b"static_publication_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([151; 32]),
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
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([161; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            callable,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn owner(publish: bool, copy: bool) -> ProductionSemanticKirOwnerV1 {
    let admitted = request(publish, copy, true)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    // Constructed admitted fixtures test lowering/replay, not source custody,
    // complete-geometry race admission, or executable/runtime authority.
    ProductionSemanticKirOwnerV1::try_lower(source, ProductionSemanticKirLimitsV1::default())
        .unwrap()
}
