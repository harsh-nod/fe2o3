// Exact admitted-model layouts copied from the existing lowering fixtures.
// These retain model claims, not rustc source authentication or runtime authority.
fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
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

fn plain_bit_scalar_type(
    tag: u8,
    primitive: SemanticBackendPrimitiveV1,
    shape: SemanticScalarTypeV1,
) -> SemanticTypeDeclV1 {
    let size = primitive.size_bytes().unwrap();
    let maximum = if size == 16 {
        u128::MAX
    } else {
        (1_u128 << (size * 8)) - 1
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag.wrapping_add(1); 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            primitive.alignment_bytes(),
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(shape),
    )
}

fn base_types(float: bool) -> Vec<SemanticTypeDeclV1> {
    let raw = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
    );
    let reference = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
    );
    let decl = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            layout,
            shape,
        )
    };
    let pointer = |tag, pointee, kind, mutability, scalar| {
        decl(
            tag,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::Scalar(scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(pointee),
                    kind,
                    mutability,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    };
    let aggregate = |tag, pointer| {
        decl(
            tag,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(
                    if pointer == 3 {
                        vec![0, 8, 16]
                    } else {
                        vec![0, 8]
                    },
                    if pointer == 3 {
                        vec![0, 1, 2]
                    } else {
                        vec![0, 1]
                    },
                )
                .unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::ScalarPair {
                    first: raw,
                    second: length,
                },
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(
                        if pointer == 3 {
                            vec![0, 8, 16]
                        } else {
                            vec![0, 8]
                        },
                        vec![],
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(if pointer == 3 {
                    vec![ty(pointer), ty(2), ty(0)]
                } else {
                    vec![ty(pointer), ty(2)]
                })
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        )
    };
    vec![
        unit_type(),
        plain_bit_scalar_type(
            141,
            if float {
                SemanticBackendPrimitiveV1::float(32, 4)
            } else {
                SemanticBackendPrimitiveV1::integer(false, 16, 2)
            },
            if float {
                SemanticScalarTypeV1::Float { bits: 32 }
            } else {
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 16,
                }
            },
        ),
        plain_bit_scalar_type(
            143,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        pointer(
            145,
            1,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            raw,
        ),
        aggregate(147, 3),
        pointer(
            149,
            1,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            raw,
        ),
        aggregate(151, 5),
        pointer(
            153,
            6,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            reference,
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        16,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    ]
}

fn publication_types() -> Vec<SemanticTypeDeclV1> {
    let mut types = base_types(true);
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
                17 | 18 => SemanticAbiPassModeV1::Direct(
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
                        if index == 17 { 8 } else { 16 },
                        Some(8),
                    )
                    .unwrap(),
                ),
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
                13 | 17 | 18 => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                _ => SemanticSourceArgumentOwnershipV1::ByValue,
            })
            .collect(),
    )
    .unwrap()
}
