//! Exact retained typed-store call and source versions. This fixture isolates
//! the private memory-version consumer; it does not issue a physical allocation.
use super::deferred_fixture::rebuild;
use super::fixtures::{self, ROOT, UNIT, U32, Shape, assignment, block, constant, place};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

pub(super) fn owner(overwrite_before_store: bool) -> ProductionSemanticSsaOwnerV1 {
    let seed = fixtures::owner(Shape::Diamond);
    let original = &seed.source_semantic().functions()[0];
    let boolean = original.locals()[1].ty();
    let mut types = seed.source_semantic().types().to_vec();
    let index = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([170; 32]),
        SemanticLayoutIdentityV1::from_sha256([171; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    let slice = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([172; 32]),
        SemanticLayoutIdentityV1::from_sha256([191; 32]),
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
        SemanticTypeShapeV1::Slice { element: U32 },
    ));
    let pointer_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let length_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let pair = SemanticBackendReprV1::scalar_pair(pointer_scalar, length_scalar);
    let physical = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([173; 32]),
        SemanticLayoutIdentityV1::from_sha256([192; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(Some(16), 8, pair, false).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                slice,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    ));
    let view = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([174; 32]),
        SemanticLayoutIdentityV1::from_sha256([193; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            pair,
            types[physical.index() as usize].layout().largest_niche(),
            false,
            None,
            8,
            16,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![physical]).unwrap()),
    ));
    let receiver = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([175; 32]),
        SemanticLayoutIdentityV1::from_sha256([173; 32]),
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
                view,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let context = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([176; 32]),
        SemanticLayoutIdentityV1::from_sha256([194; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(pointer_scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                UNIT,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    for (ty, kind, alignment) in [
        (
            physical,
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
            4,
        ),
        (
            view,
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
            4,
        ),
        (
            receiver,
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
            8,
        ),
        (
            context,
            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            1,
        ),
    ] {
        types[ty.index() as usize] = types[ty.index() as usize]
            .clone()
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(SemanticAbiPointeeInfoV1::new(kind, 0, alignment).unwrap()),
                    None,
                ),
            );
    }
    let attributes = |ty| {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                false,
                None,
                [physical, view, receiver, context].contains(&ty),
                false,
                false,
                true,
            ),
            if ty == boolean {
                SemanticAbiExtensionV1::ZeroExtend
            } else {
                SemanticAbiExtensionV1::None
            },
            0,
            if ty == receiver {
                Some(8)
            } else if ty == physical || ty == view {
                Some(4)
            } else {
                None
            },
        )
        .unwrap()
    };
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            if ty == physical || ty == view {
                SemanticAbiPassModeV1::Pair {
                    first: attributes(ty),
                    second: attributes(index),
                }
            } else {
                SemanticAbiPassModeV1::Direct(attributes(ty))
            },
        )
    };
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([174; 32]),
        SemanticLayoutIdentityV1::from_sha256([175; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        [boolean, context, physical]
            .into_iter()
            .map(|ty| SemanticAbiArgumentV1::source(direct(ty)))
            .collect(),
        original.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ])
    .unwrap();
    let call_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([176; 32]),
        SemanticLayoutIdentityV1::from_sha256([177; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        3,
        [receiver, index, U32]
            .into_iter()
            .map(|ty| SemanticAbiArgumentV1::source(direct(ty)))
            .collect(),
        direct(boolean),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let binding = SemanticNonBodyCallableBindingV1::new(
        SemanticFunctionIdentityV1::from_sha256([178; 32]),
        SemanticItemDefinitionIdentityV1::from_sha256([179; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([180; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([181; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([182; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        call_abi,
    );
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        ROOT,
        original.kernel_entry().unwrap().kernel_binding_identity(),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([183; 32]),
        types[UNIT.index() as usize].identity(),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([184; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([185; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([186; 32]),
    )
    .unwrap();
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveStore {
            view,
            index,
            element: U32,
            result: boolean,
            contract: SemanticCapabilityMemoryContractV1::global_exclusive_read_write(),
            provenance,
            source_identity: binding.identity(),
        },
        binding,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([187; 32]),
    };
    let mut locals = original.locals().to_vec();
    let view_local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([188; 32]),
        view,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let result_local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([189; 32]),
        boolean,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let context_local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([195; 32]),
        context,
        SemanticLocalRoleV1::Argument(1),
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let physical_local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([196; 32]),
        physical,
        SemanticLocalRoleV1::Argument(2),
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let receiver_local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([197; 32]),
        receiver,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let mut blocks = original.blocks().to_vec();
    let mut before_store = if overwrite_before_store {
        vec![assignment(2, U32, SemanticRvalueKindV1::Use(constant(13)))]
    } else {
        vec![]
    };
    before_store.push(assignment(
        receiver_local,
        receiver,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: place(view_local, view),
        },
    ));
    blocks[4] = block(
        24,
        before_store,
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![
                    SemanticOperandV1::Move(place(receiver_local, receiver)),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        index,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
                    )),
                    SemanticOperandV1::Copy(place(2, U32)),
                ],
                Some(SemanticCallDestinationV1::new(
                    place(result_local, boolean),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(5),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    blocks.push(block(
        25,
        vec![
            assignment(2, U32, SemanticRvalueKindV1::Use(constant(29))),
            assignment(
                3,
                U32,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: SemanticOperandV1::Copy(place(2, U32)),
                    right: SemanticOperandV1::Copy(place(2, U32)),
                },
            ),
        ],
        SemanticTerminatorKindV1::Return,
    ));
    let bind_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([198; 32]),
        SemanticLayoutIdentityV1::from_sha256([199; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        [context, physical]
            .into_iter()
            .map(|ty| SemanticAbiArgumentV1::source(direct(ty)))
            .collect(),
        direct(view),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ])
    .unwrap();
    let bind = SemanticNonBodyCallableBindingV1::new(
        SemanticFunctionIdentityV1::from_sha256([200; 32]),
        SemanticItemDefinitionIdentityV1::from_sha256([201; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([202; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([203; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([204; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        bind_abi,
    );
    let binder = SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindExclusiveReadWrite {
            context: UNIT,
            physical,
            view,
            element: U32,
            contract: SemanticCapabilityMemoryContractV1::global_exclusive_read_write(),
            provenance,
            source_identity: bind.identity(),
        },
        binding: bind,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([205; 32]),
    };
    blocks.push(block(
        26,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(2),
                vec![
                    SemanticOperandV1::Copy(place(context_local, context)),
                    SemanticOperandV1::Move(place(physical_local, physical)),
                ],
                Some(SemanticCallDestinationV1::new(
                    place(view_local, view),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(0),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    ));
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        root_abi,
        locals.clone(),
        SemanticBlockIdV1::from_index(6),
        blocks.clone(),
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    rebuild(
        &function,
        types,
        locals,
        blocks,
        vec![SemanticCallableDeclV1::defined(ROOT), callable, binder],
    )
}

pub(super) fn sites(function: &SemanticFunctionDeclV1) -> (usize, usize, usize, &SemanticPlaceV1) {
    let store = function.blocks().iter().position(|block|
        matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call) if call.arguments().len() == 3)).unwrap();
    let (block, statement, place) = function
        .blocks()
        .iter()
        .enumerate()
        .find_map(|(b, block)| {
            block
                .statements()
                .iter()
                .enumerate()
                .find_map(|(s, statement)| {
                    let SemanticStatementKindV1::Assign(a) = statement.kind() else {
                        return None;
                    };
                    let SemanticRvalueKindV1::Binary {
                        left: SemanticOperandV1::Copy(place),
                        ..
                    } = a.value().kind()
                    else {
                        return None;
                    };
                    Some((b, s, place))
                })
        })
        .unwrap();
    (store, block, statement, place)
}
