use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

#[path = "production_source_shared_intrinsic_witness_v29_tests.rs"]
mod shared_intrinsic_witness_tests;

#[path = "production_source_external_reference_v29_tests.rs"]
mod external_reference_tests;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const INDEX: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CARRIER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const BORROW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const OPTIONAL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn provenance() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}
fn integer(bits: u16) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8)),
        SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
    )
}
fn mutable_reference(
    tag: u8,
    pointee: SemanticTypeIdV1,
    size: u64,
    alignment: u64,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
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
                pointee,
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
                    size,
                    alignment,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}
fn abi(
    tag: u8,
    kernel: bool,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let unique = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    let arguments = inputs
        .iter()
        .map(|&ty| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                ty,
                if ty == CARRIER {
                    SemanticAbiPassModeV1::Pair {
                        first: plain,
                        second: plain,
                    }
                } else {
                    SemanticAbiPassModeV1::Direct(if ty == BORROW { unique } else { plain })
                },
            ))
        })
        .collect();
    let result = if output == UNIT {
        SemanticAbiPassModeV1::Ignore
    } else if output == OPTIONAL {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                Some(4),
            )
            .unwrap(),
        )
    } else {
        SemanticAbiPassModeV1::Direct(plain)
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
        arguments,
        SemanticAbiValueV1::new(output, result),
    )
    .unwrap()
    .with_source_argument_ownership(
        inputs
            .iter()
            .map(|&ty| {
                if ty == CARRIER {
                    SemanticSourceArgumentOwnershipV1::ExclusiveOwner
                } else if ty == BORROW {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::ByValue
                }
            })
            .collect(),
    )
    .unwrap()
}
fn intrinsic(
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
            provenance(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}
fn edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}
fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    local: u32,
    ty: SemanticTypeIdV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(local, ty),
                edge(SemanticEdgeRoleV1::CallReturn, next),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn block(
    id: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([60 + id; 32]),
        provenance(),
        statements,
        SemanticTerminatorV1::new(provenance(), terminator),
    )
    .unwrap()
}

// Original admitted semantic MIR/SSA, not a rustc source receipt. Two original
// same-typed ExclusiveOwner inputs make physical argument identity observable.
fn owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with_access(true)
}

pub(super) fn owner_with_access(used: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with_shape(1, usize::from(used))
}

pub(super) fn owner_with_shape(
    issuer_count: usize,
    access_count: usize,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_shape_and_read(issuer_count, access_count, false)
}

pub(super) fn owner_with_shape_uncaptured(
    issuer_count: usize,
    access_count: usize,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_issuer_locals_uncaptured(
        issuer_count,
        access_count,
        false,
        None,
        true,
        false,
        true,
        true,
    )
}

fn owner_with_shape_and_read(
    issuer_count: usize,
    access_count: usize,
    read: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_shape_and_effects(issuer_count, access_count, read, None)
}

pub(super) fn owner_with_shape_and_effects(
    issuer_count: usize,
    access_count: usize,
    read: bool,
    ordered: Option<SemanticVolatilityV1>,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_shape_effects_and_guard(issuer_count, access_count, read, ordered, true)
}

fn owner_with_shape_effects_and_guard(
    issuer_count: usize,
    access_count: usize,
    read: bool,
    ordered: Option<SemanticVolatilityV1>,
    valid_guard: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_shape_effects_guard_and_atomic(
        issuer_count,
        access_count,
        read,
        ordered,
        valid_guard,
        false,
    )
}

fn owner_with_shape_effects_guard_and_atomic(
    issuer_count: usize,
    access_count: usize,
    read: bool,
    ordered: Option<SemanticVolatilityV1>,
    valid_guard: bool,
    atomic: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_issuer_locals(
        issuer_count,
        access_count,
        read,
        ordered,
        valid_guard,
        atomic,
        true,
        true,
    )
}

pub(super) fn owner_with_reused_option_issuers(
    issuer_count: usize,
    access_count: usize,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_issuer_locals(
        issuer_count,
        access_count,
        false,
        None,
        true,
        false,
        false,
        true,
    )
}

pub(super) fn owner_with_reused_discriminator_issuers(
    issuer_count: usize,
    access_count: usize,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_issuer_locals(
        issuer_count,
        access_count,
        false,
        None,
        true,
        false,
        true,
        false,
    )
}

fn owner_with_issuer_locals(
    issuer_count: usize,
    access_count: usize,
    read: bool,
    ordered: Option<SemanticVolatilityV1>,
    valid_guard: bool,
    atomic: bool,
    distinct_options: bool,
    distinct_discriminators: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let mut owner = owner_with_issuer_locals_uncaptured(
        issuer_count,
        access_count,
        read,
        ordered,
        valid_guard,
        atomic,
        distinct_options,
        distinct_discriminators,
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}

fn owner_with_issuer_locals_uncaptured(
    issuer_count: usize,
    access_count: usize,
    read: bool,
    ordered: Option<SemanticVolatilityV1>,
    valid_guard: bool,
    atomic: bool,
    distinct_options: bool,
    distinct_discriminators: bool,
) -> ProductionSemanticSsaOwnerV1 {
    // Fixture block identities are single-byte ordered tags, not a production
    // workload limit. The scale controls stay inside that identity domain.
    assert!((1..=32).contains(&issuer_count));
    let base = super::source_allocation_receiver_v29_tests::owner();
    let mut types = base.source_semantic().types().to_vec();
    types[BORROW.index() as usize] = mutable_reference(6, CARRIER, 16, 8);
    types.push(mutable_reference(7, U32, 4, 4));
    types.push(declaration(
        8,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(integer(64)),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![INDEX, UNIT]).unwrap()),
    ));
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let nonnull = SemanticScalarValidityRangeV1::new(1, u64::MAX.into());
    let niche = SemanticLayoutNicheV1::new(0, pointer, nonnull).unwrap();
    let nullable =
        SemanticBackendScalarV1::initialized(pointer, SemanticScalarValidityRangeV1::new(1, 0));
    types.push(
        declaration(
            9,
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                8,
                8,
                SemanticBackendReprV1::scalar(nullable),
                false,
                SemanticEnumLayoutV1::new(
                    vec![
                        SemanticEnumVariantLayoutV1::from_rustc(
                            0,
                            8,
                            8,
                            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                            SemanticBackendReprV1::memory(true),
                            None,
                            false,
                            None,
                            8,
                            0,
                            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                        )
                        .unwrap(),
                        SemanticEnumVariantLayoutV1::from_rustc(
                            1,
                            8,
                            8,
                            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
                            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                                pointer, nonnull,
                            )),
                            Some(niche),
                            false,
                            None,
                            8,
                            0,
                            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                        )
                        .unwrap(),
                    ],
                    SemanticEnumEncodingV1::Niche(
                        SemanticNicheEnumEncodingV1::new(
                            0,
                            SemanticNicheSourceV1::new(
                                vec![SemanticNichePathComponentV1::Field(0)],
                                0,
                            )
                            .unwrap(),
                            niche,
                            nullable,
                            1,
                            0,
                            0,
                            0,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Enum {
                discriminant: U32,
                variants: vec![
                    SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![REFERENCE]).unwrap(),
                    ),
                ]
                .into_boxed_slice(),
            },
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
        ),
    );
    let some = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), OPTIONAL).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), REFERENCE).unwrap(),
            ],
            REFERENCE,
        )
        .unwrap()
    };
    let pointer = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(7),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let atomic = atomic.then_some(SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Device,
    ));
    let store = SemanticStatementV1::new(
        provenance(),
        if let Some(volatility) = ordered {
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                pointer,
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U32,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
                )),
                volatility,
                atomic,
            ))
        } else {
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                pointer,
                SemanticRvalueV1::new(
                    U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            U32,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, 4).unwrap(),
                            ),
                        ),
                    )),
                ),
            ))
        },
    );
    let mut issued_access = vec![assign(
        7,
        REFERENCE,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(some(5))),
    )];
    for _ in 0..access_count {
        if read {
            issued_access.push(assign(
                8,
                U32,
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(7),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32)
                                .unwrap(),
                        ],
                        U32,
                    )
                    .unwrap(),
                    ordered.unwrap_or(SemanticVolatilityV1::NonVolatile),
                    atomic,
                )),
            ));
        }
        issued_access.push(store.clone());
    }
    let mut locals: Vec<_> = [
        UNIT, CARRIER, CARRIER, BORROW, WITNESS, OPTIONAL, U32, REFERENCE,
    ]
    .into_iter()
    .enumerate()
    .map(|(local, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([40 + local as u8; 32]),
            ty,
            match local {
                0 => SemanticLocalRoleV1::Return,
                1 => SemanticLocalRoleV1::Argument(0),
                2 => SemanticLocalRoleV1::Argument(1),
                _ => SemanticLocalRoleV1::Temporary,
            },
            provenance(),
        )
    })
    .collect();
    if read {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([48; 32]),
            U32,
            SemanticLocalRoleV1::Temporary,
            provenance(),
        ));
    }
    let mut blocks = Vec::new();
    for issuer in 0..issuer_count {
        let option = if distinct_options && issuer != 0 {
            let local = u32::try_from(locals.len()).unwrap();
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([40 + u8::try_from(local).unwrap(); 32]),
                OPTIONAL,
                SemanticLocalRoleV1::Temporary,
                provenance(),
            ));
            local
        } else {
            5
        };
        // Producer and discriminator uniqueness are independent source contracts.
        // Keep the reused-Option negative's discriminators otherwise admissible.
        let discriminator = if distinct_discriminators && issuer != 0 {
            let local = u32::try_from(locals.len()).unwrap();
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([40 + u8::try_from(local).unwrap(); 32]),
                U32,
                SemanticLocalRoleV1::Temporary,
                provenance(),
            ));
            local
        } else {
            6
        };
        let mut access = issued_access.clone();
        access[0] = assign(
            7,
            REFERENCE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(some(option))),
        );
        let base = u32::try_from(issuer * 4).unwrap();
        blocks.push(block(
            base as u8,
            vec![assign(
                3,
                BORROW,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(1, CARRIER),
                },
            )],
            call(1, vec![], 4, WITNESS, base + 1),
        ));
        blocks.push(block(
            (base + 1) as u8,
            vec![],
            call(
                2,
                vec![
                    SemanticOperandV1::Move(place(3, BORROW)),
                    SemanticOperandV1::Move(place(4, WITNESS)),
                ],
                option,
                OPTIONAL,
                base + 2,
            ),
        ));
        blocks.push(block(
            (base + 2) as u8,
            vec![assign(
                discriminator,
                U32,
                SemanticRvalueKindV1::Discriminant(place(option, OPTIONAL)),
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(discriminator, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(
                            SemanticEdgeRoleV1::SwitchValue,
                            base + if valid_guard { 3 } else { 4 },
                        ),
                    )],
                    edge(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        base + if valid_guard { 4 } else { 3 },
                    ),
                )
                .unwrap(),
            },
        ));
        blocks.push(block(
            (base + 3) as u8,
            access,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, base + 4)),
        ));
    }
    blocks.push(block(
        (issuer_count * 4) as u8,
        vec![assign(
            0,
            UNIT,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                UNIT,
                SemanticConstantValueV1::ZeroSized,
            ))),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([20; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([20; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([20; 32]),
        provenance(),
        abi(20, true, &[CARRIER, CARRIER], UNIT),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"issued_pointer_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([23; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let source = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            intrinsic(
                21,
                abi(21, false, &[], WITNESS),
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: WITNESS,
                    raw_index: INDEX,
                },
            ),
            intrinsic(
                22,
                abi(22, false, &[BORROW, WITNESS], OPTIONAL),
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                    disjoint_slice: CARRIER,
                    index_witness: WITNESS,
                    element: U32,
                    raw_index: INDEX,
                },
            ),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

thread_local! {
    static ISSUED_SOURCE_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ISSUED_SOURCE_VISITED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_original(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if ISSUED_SEMANTIC_REPLAY_OBSERVE.get() {
        observe_issued_semantic_replay_v29(pending, instances, references, budget)?;
    }
    let fault = ISSUED_SOURCE_FAULT.get();
    let body = pending.function.body.as_mut().unwrap();
    assert_eq!(body.parameters.len(), 2);
    if fault == 1 {
        body.parameters.swap(0, 1);
    }
    let other = body.parameters[1];
    let mut length = None;
    for block in &body.blocks {
        for op in &block.operations {
            budget.charge_work(1)?;
            if matches!(op.kind, OperationKind::SliceLength { .. }) {
                assert!(length.replace(op.results[0].id).is_none());
            }
        }
    }
    let length = length.expect("one original receiver length");
    let mut changed = 0;
    for block in &mut body.blocks {
        for op in &mut block.operations {
            budget.charge_work(1)?;
            match (&mut op.kind, fault) {
                (OperationKind::SliceData { slice }, 2) => {
                    *slice = other;
                    changed += 1;
                }
                (OperationKind::GetElementPointer { offset, .. }, 3) => {
                    *offset = length;
                    changed += 1;
                }
                (OperationKind::Compare { predicate, .. }, 4) => {
                    *predicate = ComparePredicate::GreaterThan;
                    changed += 1;
                }
                (OperationKind::SliceLength { slice }, 6) => {
                    *slice = other;
                    changed += 1;
                }
                (OperationKind::Store { access, .. }, 7)
                | (OperationKind::Load { access, .. }, 8) => {
                    access.volatile = !access.volatile;
                    changed += 1;
                }
                _ => {}
            }
        }
        if fault == 5 {
            match block.terminator.as_mut().unwrap() {
                Terminator::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                } => {
                    std::mem::swap(then_target, else_target);
                    std::mem::swap(then_arguments, else_arguments);
                    changed += 1;
                }
                Terminator::Switch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    let case = cases.iter_mut().find(|case| case.value == 1).unwrap();
                    std::mem::swap(&mut case.target, default_target);
                    std::mem::swap(&mut case.arguments, default_arguments);
                    changed += 1;
                }
                Terminator::IntegerSwitch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    let case = cases
                        .iter_mut()
                        .find(|case| source_issued_constant_bool_v29(&case.value) == Some(true))
                        .unwrap();
                    std::mem::swap(&mut case.target, default_target);
                    std::mem::swap(&mut case.arguments, default_arguments);
                    changed += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(changed, usize::from(fault >= 2));
    ISSUED_SOURCE_VISITED.set(true);
    Ok(())
}

struct RestoreObserver(Option<RootExecutionArchiveObserverV29>);
impl Drop for RestoreObserver {
    fn drop(&mut self) {
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
    }
}

pub(super) fn with_issued_source_fault_v29<T>(fault: u8, run: impl FnOnce() -> T) -> (T, bool) {
    struct Scope(Option<RootExecutionArchiveObserverV29>, u8, bool);
    impl Drop for Scope {
        fn drop(&mut self) {
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
            ISSUED_SOURCE_FAULT.set(self.1);
            ISSUED_SOURCE_VISITED.set(self.2);
        }
    }
    let _scope = Scope(
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(observe_original)),
        ISSUED_SOURCE_FAULT.replace(fault),
        ISSUED_SOURCE_VISITED.replace(false),
    );
    let result = run();
    (result, ISSUED_SOURCE_VISITED.get())
}

fn run_original(fault: u8) {
    run_original_with_access(fault, true)
}

fn run_original_with_access(fault: u8, used: bool) {
    run_original_owner(
        fault,
        used,
        false,
        if used {
            owner()
        } else {
            owner_with_access(false)
        },
    );
}

fn run_original_owner(fault: u8, used: bool, read: bool, owner: ProductionSemanticSsaOwnerV1) {
    run_original_owner_counts(fault, (usize::from(used), usize::from(read), 0), owner);
}

fn run_original_owner_counts(
    fault: u8,
    expected: (usize, usize, usize),
    owner: ProductionSemanticSsaOwnerV1,
) {
    run_original_owner_case(fault, expected, owner, None);
}

#[derive(Clone, Copy, Debug)]
enum EarlyIssuedSourceRefusal {
    Unsupported(&'static str),
    Accounting,
}

fn run_original_owner_case(
    fault: u8,
    expected: (usize, usize, usize),
    owner: ProductionSemanticSsaOwnerV1,
    early_source_refusal: Option<EarlyIssuedSourceRefusal>,
) {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    let occurrence_storage = owner
        .occurrence_storage()
        .expect("explicit original occurrence capture")
        .retained_storage();
    let semantic = owner.source_semantic();
    let ordered_read = semantic.functions()[0]
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .any(|statement| {
            matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(load)
                if load.volatility() == SemanticVolatilityV1::Volatile))
        });
    let ordered_write = semantic.functions()[0]
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .any(|statement| {
            matches!(statement.kind(), SemanticStatementKindV1::Store(store)
            if store.volatility() == SemanticVolatilityV1::Volatile)
        });
    let hash = *owner.source_semantic_sha256();
    let binding = *semantic.functions()[0]
        .kernel_entry()
        .unwrap()
        .kernel_binding_identity()
        .as_bytes();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "issued_pointer_source",
            binding,
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let arguments: Vec<_> = ["first", "second"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, name)| ProductionKernelArgumentAbiArgumentV18 {
            semantic_type_identity: semantic.types()[CARRIER.index() as usize].identity(),
            kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
                argument: LogicalArgumentV1::disjoint_slice(
                    ordinal as u16,
                    ValidName::new(name).unwrap(),
                    &source,
                    &layout,
                    fe2o3_kernel_descriptor::AccessMode::ReadWrite,
                    (ordinal * 16) as u32,
                )
                .unwrap(),
            },
        })
        .collect();
    let roots = [ProductionKernelArgumentAbiRootV18 {
        kernel_binding: &binding,
        export: "issued_pointer_source",
        arguments: &arguments,
        explicit_argument_bytes: 32,
        kernarg_alignment_bytes: 8,
    }];
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &hash,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000_000);
    budget.reserve_storage(37 + occurrence_storage).unwrap();
    ISSUED_SOURCE_FAULT.set(fault);
    ISSUED_SOURCE_VISITED.set(false);
    let _restore =
        RestoreObserver(ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(observe_original)));
    let result = ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
        owner,
        launch,
        input,
        ProductionKernelArgumentAbiInputV18 { roots: &roots },
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    if let Some(expected) = early_source_refusal {
        assert!(
            !ISSUED_SOURCE_VISITED.get(),
            "C1 refusal must precede completed source emission"
        );
        let exact = match (expected, result.as_ref()) {
            (
                EarlyIssuedSourceRefusal::Unsupported(expected),
                Err(ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail,
                    },
                )),
            ) => *detail == expected,
            (
                EarlyIssuedSourceRefusal::Accounting,
                Err(ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting,
                    ),
                )),
            ) => true,
            _ => false,
        };
        assert!(
            exact,
            "expected source refusal {expected:?}: {:?}",
            result.as_ref().err()
        );
        assert_eq!(budget.storage(), 37 + occurrence_storage);
        budget.release_storage(occurrence_storage).unwrap();
        assert_eq!(budget.storage(), 37);
        return;
    }
    assert!(
        ISSUED_SOURCE_VISITED.get(),
        "original emitter/archive proof must reach the final-source observer; fault {fault}, refusal: {:?}",
        result.as_ref().err()
    );
    if fault != 0 {
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("changed actual issuer must refuse"),
        };
        assert!(
            if fault >= 7 {
                matches!(
                    error,
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "scoped memory anchors differ from their source instance",
                            ..
                        }
                    )
                )
            } else {
                matches!(
                    error,
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "source issued pointer differs from its original issuer or actual guard",
                            ..
                        }
                    )
                )
            },
            "fault {fault}: {error:?}"
        );
        assert_eq!(budget.storage(), 37 + occurrence_storage);
        budget.release_storage(occurrence_storage).unwrap();
        assert_eq!(budget.storage(), 37);
        return;
    }
    let pending = result
        .unwrap_or_else(|error| panic!("original issued pointer full source admission: {error:?}"));
    let mut stores = 0;
    let mut loads = 0;
    let mut gep = 0;
    let mut generic = 0;
    for function in &pending.pending_module().functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for operation in &block.operations {
                match operation.kind {
                    OperationKind::Store { access, .. } => {
                        assert!(matches!(
                            access.address_space,
                            AddressSpace::Global | AddressSpace::Generic
                        ));
                        generic += usize::from(access.address_space == AddressSpace::Generic);
                        assert_eq!(access.volatile, ordered_write);
                        stores += 1;
                    }
                    OperationKind::Load { access, .. } => {
                        assert!(matches!(
                            access.address_space,
                            AddressSpace::Global | AddressSpace::Generic
                        ));
                        generic += usize::from(access.address_space == AddressSpace::Generic);
                        assert_eq!(access.volatile, ordered_read);
                        loads += 1;
                    }
                    OperationKind::GetElementPointer { .. } => gep += 1,
                    _ => {}
                }
            }
        }
    }
    assert_eq!((stores, loads, generic), expected);
    assert_eq!(gep, 1);
    let retained = pending.adopted_storage();
    assert_eq!(budget.storage(), 37 + occurrence_storage + retained);
    drop(pending);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 37 + occurrence_storage);
    budget.release_storage(occurrence_storage).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn issued_pointer_original_get_mut_reaches_complete_pending_source() {
    run_original(0);
}

#[test]
fn issued_pointer_original_read_and_write_keep_separate_issuer_census_from_private_loans() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_issued_holder_family)));
    ISSUED_HOLDER_FAMILY_COUNTS.with(|counts| counts.set((0, 0)));
    run_original_owner(0, true, true, owner_with_shape_and_read(1, 1, true));
    let (reads, writes) = ISSUED_HOLDER_FAMILY_COUNTS.with(|counts| counts.get());
    assert!(
        reads > 0 && writes > 0,
        "both genuine access families must reach the original archive: {reads}/{writes}"
    );
    for fault in 1..=5 {
        run_original_owner(0, true, true, owner_with_shape_and_read(1, 1, true));
        run_original_owner(fault, true, true, owner_with_shape_and_read(1, 1, true));
    }
}

thread_local! {
    static ISSUED_HOLDER_FAMILY_COUNTS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

fn observe_issued_holder_family(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.unwrap().plan;
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let headers = h::<bool>()
        + h::<&SemanticValueBindingV1>()
        + h::<Option<&SemanticValueBindingV1>>()
        + h::<std::slice::Iter<'_, SemanticProjectionV1>>()
        + h::<Option<&SemanticProjectionV1>>()
        + h::<&SemanticProjectionV1>()
        + h::<&[SemanticProjectionV1]>()
        + h::<&Vec<SemanticValueBindingV1>>()
        + h::<&SemanticSourceReferenceBindingV29>()
        + h::<Option<usize>>();
    for lowered in emitted.iter().flatten() {
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.instance(instance).unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let archive = lowered.execution_observation.as_ref().unwrap();
        let occurrences = instances.occurrences(instance).unwrap();
        for anchor in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Access {
                payload: Some(payload),
                ..
            } = anchor.kind
            else {
                continue;
            };
            let Some(frame) = anchor.source else {
                continue;
            };
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                continue;
            };
            let Some(place) = scoped_source_place_v29(original.declaration(), frame.site, role)
            else {
                continue;
            };
            if place.local().index() != 7 || place.projections().len() != 1 {
                continue;
            }
            let writing = match payload {
                ScopedMemoryPayloadV29::Load { read, .. } if read.prefix == 1 => false,
                ScopedMemoryPayloadV29::Store { .. } => true,
                _ => continue,
            };
            let mut event = None;
            for (index, row) in occurrences.events().iter().enumerate() {
                if row.site() == frame.site
                    && row.operand() == role
                    && row.role() == ExecutionEventV29::BaseUse
                    && row.event().variable().get() == place.local().index()
                {
                    assert!(event.replace(index).is_none());
                }
            }
            with_canonical_call_scratch_v1(budget, |budget| {
                let capture = source_object_base_occurrence_v29(
                    &occurrences,
                    frame.site,
                    role,
                    place,
                    event.unwrap(),
                    budget,
                )?;
                let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = capture else {
                    panic!("issued pointer is promoted");
                };
                let binding =
                    archive.lookup_original_v29(instances, instance, definition, budget)?;
                assert!(
                    matches!(binding, SemanticValueBindingV1::Value { ty: Type::Pointer(pointer), .. }
                    if pointer.address_space == AddressSpace::Global && pointer.access == AccessMode::ReadWrite
                        && *pointer.pointee == Type::Scalar(ScalarType::U32))
                );
                let before = (budget.work(), budget.storage());
                // Candidate-only false: no object or issuer authority is returned.
                // Ten prepay guards, entrance and charge guards, plus four work.
                for _ in 0..16 {
                    with_canonical_call_scratch_v1(budget, |budget| {
                        let floor = budget.storage();
                        assert!(!source_object_private_holder_family_v29(
                            plan, binding, place, budget
                        )?);
                        assert_eq!(budget.storage() - floor, headers);
                        Ok(())
                    })?;
                    assert_eq!(budget.storage(), before.1);
                }
                assert_eq!(budget.work() - before.0, 16 * (12 * 5 + 4 + 2));
                Ok(())
            })?;
            ISSUED_HOLDER_FAMILY_COUNTS.with(|counts| {
                let (reads, writes) = counts.get();
                counts.set(if writing {
                    (reads, writes + 1)
                } else {
                    (reads + 1, writes)
                });
            });
        }
    }
    Ok(())
}

#[test]
fn issued_pointer_original_source_rejects_changed_actual_argument_tail_and_some_edge() {
    // A complete unchanged source positive precedes every independent mutation;
    // observer panics cannot stand in for the exact final-source refusal.
    for fault in 1..=5 {
        run_original(0);
        run_original(fault);
    }
}

#[test]
fn issued_pointer_unused_original_issuer_still_checks_actual_input_index_and_tail() {
    for fault in 1..=4 {
        run_original_with_access(0, false);
        run_original_with_access(fault, false);
    }
}

include!("production_source_issued_semantic_v29_tests.rs");
