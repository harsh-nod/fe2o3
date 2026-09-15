use super::*;

pub(super) fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
pub(super) fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
pub(super) fn projection(
    kind: SemanticProjectionKindV1,
    ty: SemanticTypeIdV1,
) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, ty).unwrap()
}
pub(super) fn assignment(
    destination: SemanticPlaceV1,
    result: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementKindV1 {
    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(result, value),
    ))
}

// Component fixtures use the actual layout/ABI fact constructor, not a forged
// fact or a replacement matcher. They are not admitted source/kernel authority.
pub(super) fn types() -> Vec<SemanticTypeDeclV1> {
    let mut types = Vec::new();
    for base in [0u32, 15] {
        let id = |i| ty(base + i);
        let decl = |tag: u8, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag + base as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + base as u8; 32]),
                layout,
                shape,
            )
        };
        let scalar = |tag, bits: u16| {
            decl(
                tag,
                SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits,
                }),
            )
        };
        let aggregate = |tag, fields, offsets, bytes, align| {
            decl(
                tag,
                SemanticTypeLayoutV1::aggregate(
                    Some(bytes),
                    align,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            )
        };
        let reference = |tag, pointee, metadata, bytes| {
            decl(
                tag,
                SemanticTypeLayoutV1::new(Some(bytes), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        metadata,
                    )
                    .unwrap(),
                ),
            )
        };
        types.extend([
            aggregate(20, vec![], vec![], 0, 1),
            scalar(21, 16),
            scalar(22, 64),
            scalar(23, 32),
            decl(
                24,
                SemanticTypeLayoutV1::new(None, 2).unwrap(),
                SemanticTypeShapeV1::Slice { element: id(1) },
            ),
            reference(25, id(4), SemanticPointerMetadataV1::SliceLength, 16),
            aggregate(26, vec![id(5), id(0)], vec![0, 16], 16, 8),
            reference(27, id(6), SemanticPointerMetadataV1::None, 8),
            aggregate(
                28,
                vec![id(7), id(2), id(2), id(2), id(2), id(0), id(0), id(0)],
                vec![0, 8, 16, 24, 32, 40, 40, 40],
                40,
                8,
            ),
            reference(29, id(8), SemanticPointerMetadataV1::None, 8),
            aggregate(30, vec![id(3), id(0), id(0), id(0)], vec![0, 4, 4, 4], 4, 4),
            reference(31, id(10), SemanticPointerMetadataV1::None, 8),
            aggregate(32, vec![id(14), id(0), id(0)], vec![0, 8, 8], 8, 2),
            aggregate(33, vec![id(1)], vec![0], 2, 2),
            decl(
                34,
                SemanticTypeLayoutV1::new(Some(8), 2).unwrap(),
                SemanticTypeShapeV1::Array {
                    element: id(13),
                    length: 4,
                },
            ),
        ]);
    }
    types
}

pub(super) fn callable(base: u32, tag: u8) -> SemanticCallableDeclV1 {
    let id = |i| ty(base + i);
    let identity = SemanticFunctionIdentityV1::from_sha256([tag; 32]);
    let contract = SemanticGlobalBf16MatrixLoadV1::new(
        SemanticGlobalBf16MatrixTypesV1 {
            matrix: id(8),
            global: id(6),
            lane: id(10),
            fragment: id(12),
            element: id(1),
            index: id(2),
        },
        SemanticMfmaOperandContractV1 {
            role: SemanticMfmaOperandRoleV1::A,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: 64,
        },
        SemanticTypeIdentityV1::from_sha256([60; 32]),
        SemanticTypeIdentityV1::from_sha256([61; 32]),
        SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
        )
        .unwrap(),
        identity,
    )
    .unwrap();
    let value = |ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![value(id(9)), value(id(11)), value(id(2)), value(id(2))],
        value(id(12)),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            identity,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

pub(super) fn facts() -> Vec<GlobalBf16BorrowV1> {
    let types = types();
    [(0, 62), (15, 63), (0, 64)]
        .into_iter()
        .map(|(base, tag)| {
            GlobalBf16BorrowV1::for_callable(&types, &callable(base, tag))
                .expect("real fact constructor")
        })
        .collect()
}

pub(super) fn capture(base: u32, change: u8) -> SemanticStatementKindV1 {
    let id = |i| ty(base + i);
    let mut destination = place(10, id(8));
    let mut result = id(8);
    let mut source = place(1, id(7));
    let mut kind = SemanticAggregateKindV1::Aggregate;
    let mut length = 8;
    match change {
        2 => destination = place(10, id(0)),
        3 => result = id(0),
        4 => kind = SemanticAggregateKindV1::Tuple,
        5 => length = 7,
        6 => length = 9,
        7 => source = place(1, id(9)),
        8 => {
            source = SemanticPlaceV1::new(
                source.local(),
                vec![projection(SemanticProjectionKindV1::Field(0), id(7))],
                id(7),
            )
            .unwrap()
        }
        9 => {
            destination = SemanticPlaceV1::new(
                destination.local(),
                vec![projection(SemanticProjectionKindV1::Field(0), id(8))],
                id(8),
            )
            .unwrap()
        }
        _ => {}
    }
    let mut operands = (0..length)
        .map(|i| SemanticOperandV1::Copy(place(20 + i, id(if i < 5 { 2 } else { 0 }))))
        .collect::<Vec<_>>();
    operands[0] = if change == 1 {
        SemanticOperandV1::Move(source)
    } else {
        SemanticOperandV1::Copy(source)
    };
    assignment(
        destination,
        result,
        SemanticRvalueKindV1::Aggregate(SemanticAggregateRvalueV1::new(kind, operands).unwrap()),
    )
}

pub(super) fn metadata(base: u32, change: u8) -> SemanticStatementKindV1 {
    let id = |i| ty(base + i);
    let mut destination = place(10, id(5));
    let mut path = vec![
        projection(SemanticProjectionKindV1::Dereference, id(6)),
        projection(SemanticProjectionKindV1::Field(0), id(5)),
    ];
    match change {
        2 => path[0] = projection(SemanticProjectionKindV1::Field(0), id(6)),
        3 => path[0] = projection(SemanticProjectionKindV1::Dereference, id(8)),
        4 => path[1] = projection(SemanticProjectionKindV1::Field(1), id(5)),
        5 => {
            path.pop();
        }
        6 => path.push(projection(SemanticProjectionKindV1::Field(0), id(5))),
        7 => {
            destination = SemanticPlaceV1::new(
                destination.local(),
                vec![projection(SemanticProjectionKindV1::Field(0), id(5))],
                id(5),
            )
            .unwrap()
        }
        _ => {}
    }
    let source_type = path.last().unwrap().result_type();
    let source = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), path, source_type).unwrap();
    let operand = if change == 1 {
        SemanticOperandV1::Move(source)
    } else {
        SemanticOperandV1::Copy(source)
    };
    assignment(destination, id(5), SemanticRvalueKindV1::Use(operand))
}
