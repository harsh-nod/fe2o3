//! Inert model fixture, not a live provider, source seal, or linear loan proof.
use super::*;
pub(super) const BIND: SemanticFunctionIdV1 = SemanticFunctionIdV1(2);
pub(super) const PHASE_REF: SemanticTypeIdV1 = SemanticTypeIdV1(9);
pub(super) const STORAGE_REF: SemanticTypeIdV1 = SemanticTypeIdV1(10);
pub(super) const LEASE: SemanticTypeIdV1 = SemanticTypeIdV1(8);
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
pub(super) fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], ty).unwrap()
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let shared = ty == PHASE_REF;
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    shared,
                    shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    shared,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                if shared { 16 } else { 0 },
                shared.then_some(8),
            )
            .unwrap(),
        ),
    )
}
fn abi(tag: u8, kernel: bool, output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1([tag; 32]),
        SemanticLayoutIdentityV1([tag; 32]),
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
        2,
        vec![PHASE_REF, STORAGE_REF],
        output,
        vec![
            SemanticAbiArgumentV1::source(direct(PHASE_REF)),
            SemanticAbiArgumentV1::source(direct(STORAGE_REF)),
        ],
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ])
    .unwrap()
}
fn block(tag: u8, kind: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1([tag; 32]),
        source(),
        vec![],
        SemanticTerminatorV1::new(source(), kind),
    )
    .unwrap()
}
fn function(
    tag: u8,
    kernel: bool,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1([tag; 32]),
        if kernel {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1([tag; 32]),
        SemanticMonomorphizationIdentityV1([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1([tag; 32]),
        source(),
        abi(
            tag,
            kernel,
            locals
                .iter()
                .find(|(_, role)| *role == SemanticLocalRoleV1::Return)
                .unwrap()
                .0,
        ),
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1([i as u8 + 1; 32]),
                    *ty,
                    *role,
                    source(),
                )
            })
            .collect(),
        SemanticBlockIdV1(0),
        blocks,
    )
    .unwrap()
}
fn declaration(
    index: u32,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1([index as u8 + 1; 32]),
        SemanticLayoutIdentityV1([index as u8 + 1; 32]),
        layout,
        shape,
    )
}
fn aggregate(index: u32, ids: &[u32], physical: bool) -> SemanticTypeDeclV1 {
    let word = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    declaration(
        index,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(if physical { 16 } else { 0 }),
            if physical { 8 } else { 1 },
            if physical {
                SemanticBackendReprV1::ScalarPair {
                    first: word.clone(),
                    second: word,
                }
            } else {
                SemanticBackendReprV1::memory(true)
            },
            false,
            SemanticAggregateLayoutV1::new(
                if physical {
                    vec![0, 8, 16, 16]
                } else {
                    vec![0; ids.len()]
                },
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(ids.iter().copied().map(SemanticTypeIdV1).collect())
                .unwrap(),
        ),
    )
}
fn reference(index: u32, pointee: u32, mutable: bool) -> SemanticTypeDeclV1 {
    declaration(
        index,
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
                SemanticTypeIdV1(pointee),
                SemanticPointerKindV1::Reference,
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
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
                    if mutable {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    if mutable { 0 } else { 16 },
                    if mutable { 1 } else { 8 },
                )
                .unwrap(),
            ),
            None,
        ),
    )
}
pub(super) fn provenance() -> SemanticKernelCapabilityProvenanceV1 {
    SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1(0),
        SemanticKernelBindingIdentityV1([101; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1([102; 32]),
        SemanticTypeIdentityV1([103; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1([104; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1([105; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1([106; 32]),
    )
    .unwrap()
}
pub(super) fn recipe() -> SemanticDefinedReusablePhaseRecipeV1 {
    SemanticDefinedReusablePhaseRecipeV1::Bind {
        phase_reference: SemanticPhaseReferenceV1 {
            reference: PHASE_REF,
            pointee: SemanticTypeIdV1(6),
            kind: SemanticPhaseReferenceKindV1::Shared,
        },
        storage_reference: SemanticPhaseReferenceV1 {
            reference: STORAGE_REF,
            pointee: SemanticTypeIdV1(7),
            kind: SemanticPhaseReferenceKindV1::Unique,
        },
        phase_workgroup: SemanticTypeIdV1(6),
        reusable_storage: SemanticTypeIdV1(7),
        phase_lds: LEASE,
        element: SemanticTypeIdV1(11),
        uninitialized_marker: SemanticTypeIdV1(3),
        storage_marker: SemanticTypeIdV1(1),
        thread_marker: SemanticTypeIdV1(2),
        brands: SemanticPhaseBrandsV1 {
            root_brand: SemanticTypeIdV1(0),
            outer_workgroup_brand: SemanticTypeIdV1(4),
            phase_brand: SemanticTypeIdV1(4),
            dynamic_epoch: SemanticTypeIdV1(0),
        },
        elements: 64,
    }
}
pub(super) fn request() -> InertSemanticMirRequestV1 {
    use SemanticLocalRoleV1::{Argument, Return, Temporary};
    let unit = SemanticTypeIdV1(0);
    let types = vec![
        declaration(
            0,
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
        aggregate(1, &[], false),
        aggregate(2, &[], false),
        aggregate(3, &[], false),
        aggregate(4, &[], false),
        declaration(
            5,
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
        ),
        aggregate(6, &[5, 5, 4, 2], true),
        aggregate(7, &[1, 4, 2], false),
        aggregate(8, &[1, 3, 4, 0, 2], false),
        reference(9, 6, false),
        reference(10, 7, true),
        declaration(
            11,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        ),
    ];
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(2),
        vec![
            SemanticOperandV1::Copy(place(1, PHASE_REF)),
            SemanticOperandV1::Move(place(2, STORAGE_REF)),
        ],
        Some(SemanticCallDestinationV1::new(
            place(3, LEASE),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let forward = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        vec![
            SemanticOperandV1::Copy(place(1, PHASE_REF)),
            SemanticOperandV1::Move(place(2, STORAGE_REF)),
        ],
        Some(SemanticCallDestinationV1::new(
            place(0, unit),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root = function(
        20,
        true,
        &[
            (unit, Return),
            (PHASE_REF, Argument(0)),
            (STORAGE_REF, Argument(1)),
        ],
        vec![
            block(1, SemanticTerminatorKindV1::Call(forward)),
            block(2, SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"phase_v26_fixture".to_vec()).unwrap(),
        provenance().kernel_binding(),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let body = function(
        21,
        false,
        &[
            (unit, Return),
            (PHASE_REF, Argument(0)),
            (STORAGE_REF, Argument(1)),
            (LEASE, Temporary),
        ],
        vec![
            block(1, SemanticTerminatorKindV1::Call(call)),
            block(2, SemanticTerminatorKindV1::Return),
        ],
    );
    let bind = function(
        22,
        false,
        &[
            (LEASE, Return),
            (PHASE_REF, Argument(0)),
            (STORAGE_REF, Argument(1)),
        ],
        vec![block(1, SemanticTerminatorKindV1::Return)],
    );
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1([200; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, body, bind],
        vec![SemanticFunctionIdV1(0)],
    )
    .unwrap()
}
pub(super) fn observe(request: &InertSemanticMirRequestV1) -> SemanticDefinedReusablePhaseV1 {
    SemanticDefinedReusablePhaseV1::for_defined_function(
        BIND,
        &request.functions,
        &request.callables,
        &request.types,
        provenance(),
        [121; 32],
        recipe(),
        &mut HARD_MAX_VALIDATION_WORK_V1.clone(),
    )
    .unwrap()
}
pub(super) fn attach(request: &mut InertSemanticMirRequestV1) -> SemanticDefinedReusablePhaseV1 {
    let record = observe(request);
    attach_reusable_phase_contracts_v26(
        &mut request.functions,
        &request.callables,
        &request.types,
        &[record],
        &mut HARD_MAX_VALIDATION_WORK_V1.clone(),
    )
    .unwrap();
    record
}
