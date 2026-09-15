use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn binding() -> SemanticKernelBindingIdentityV1 {
    SemanticKernelBindingIdentityV1::from_sha256([19; 32])
}

fn scalar(
    primitive: SemanticBackendPrimitiveV1,
    start: u128,
    end: u128,
) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(start, end))
}

fn types(unpin: bool) -> Vec<SemanticTypeDeclV1> {
    let make = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            layout,
            shape,
        )
    };
    vec![
        make(
            1,
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
        make(
            2,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(scalar(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    0,
                    u32::MAX.into(),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        ),
        make(
            3,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::array(4, 0),
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
            SemanticTypeShapeV1::Slice { element: ELEMENT },
        ),
        make(
            4,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(
                    scalar(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        1,
                        u64::MAX.into(),
                    ),
                    scalar(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        0,
                        u64::MAX.into(),
                    ),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SLICE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
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
                        SemanticAbiPointeeKindV1::MutableReference { unpin },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    ]
}

fn argument(noalias: bool) -> SemanticAbiArgumentV1 {
    argument_with_alignment(noalias, 4)
}

fn argument_with_alignment(noalias: bool, alignment: u64) -> SemanticAbiArgumentV1 {
    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        REFERENCE,
        SemanticAbiPassModeV1::Pair {
            first: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(noalias, None, true, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                (alignment > 1).then_some(alignment),
            )
            .unwrap(),
            second: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        },
    ))
}

fn abi(tag: u8, root: bool, noalias: bool) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        if root {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if root {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        2,
        vec![argument(noalias), argument(noalias)],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow; 2])
    .unwrap()
}

fn block(tag: u8, kind: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn function(
    root: bool,
    noalias: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticFunctionDeclV1 {
    let tag = if root { 10 } else { 20 };
    let local = |index, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag + index; 32]),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    };
    let place =
        |index, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap();
    let body = if root {
        vec![
            block(
                tag,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![
                            SemanticOperandV1::Move(place(1, REFERENCE)),
                            SemanticOperandV1::Move(place(2, REFERENCE)),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            place(3, UNIT),
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
            block(tag + 1, SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![block(tag, SemanticTerminatorKindV1::Return)]
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if root {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi(tag, root, noalias)
            .with_source_argument_ownership(vec![ownership; 2])
            .unwrap(),
        vec![
            local(0, UNIT, SemanticLocalRoleV1::Return),
            local(1, REFERENCE, SemanticLocalRoleV1::Argument(0)),
            local(2, REFERENCE, SemanticLocalRoleV1::Argument(1)),
            local(3, UNIT, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        body,
    )
    .unwrap();
    if root {
        function.with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"unique_slice_source_fixture".to_vec()).unwrap(),
            binding(),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ))
    } else {
        function
    }
}

fn owner(unpin: bool, noalias: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with_ownership(
        unpin,
        noalias,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    )
}

fn owner_with_ownership(
    unpin: bool,
    noalias: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> ProductionSemanticSsaOwnerV1 {
    let admitted = request_with_ownership(unpin, noalias, ownership)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(admitted, Default::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(owner, Default::default()).unwrap()
}

fn request_with_ownership(
    unpin: bool,
    noalias: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([99; 32])),
        types(unpin),
        vec![],
        vec![],
        vec![],
        vec![
            function(true, noalias, ownership),
            function(false, noalias, ownership),
        ],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![ROOT],
    )
    .unwrap()
}

fn allocation(ordinal: usize) -> AllocationContractV1 {
    AllocationContractV1 {
        allocation_origin: ordinal as u64 + 1,
        noalias_class: 0,
        writable: true,
        singleton_object: false,
    }
}

#[test]
fn unique_slice_source_fixture_retains_exported_gpu_root_and_rust_helper() {
    let owner = owner(false, false);
    let semantic = owner.source_semantic();
    semantic.require_complete_kernel_entries().unwrap();
    assert_eq!(semantic.roots(), &[ROOT]);
    assert_eq!(
        semantic
            .select_kernel_body_for_root_v1(ROOT)
            .unwrap()
            .body(),
        ROOT,
    );
    let source = &semantic.functions()[ROOT.index() as usize];
    let entry = source.kernel_entry().unwrap();
    assert_eq!(entry.kernel_binding_identity(), binding());
    assert_eq!(source.abi().canon_abi(), SemanticCanonAbiV1::GpuKernel);
    assert_eq!(source.abi().extern_abi(), SemanticExternAbiV1::GpuKernel);
    let helper = &semantic.functions()[1];
    assert_eq!(helper.abi().canon_abi(), SemanticCanonAbiV1::Rust);
    assert_eq!(helper.abi().extern_abi(), SemanticExternAbiV1::Rust);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    assert!(view.has_expanded_calls());
    assert_eq!(view.body().kernel_entry(), source.kernel_entry());
    assert_eq!(view.body().abi(), source.abi());
    assert_eq!(
        helper.abi().source_input_types(),
        source.abi().source_input_types()
    );
    assert_eq!(
        helper.abi().source_argument_ownership(),
        source.abi().source_argument_ownership(),
    );
    let proof = UniqueSliceSourceV1::for_root(&owner, ROOT, binding()).unwrap();
    assert!(
        proof
            .allocation(
                semantic.types(),
                view.body(),
                0,
                &view.body().abi().arguments()[0],
                allocation(0),
            )
            .is_some()
    );
    assert!(
        UniqueSliceSourceV1::for_root(&owner, SemanticFunctionIdV1::from_index(1), binding())
            .is_none()
    );
    assert!(
        proof
            .allocation(
                semantic.types(),
                helper,
                0,
                &helper.abi().arguments()[0],
                allocation(0),
            )
            .is_none()
    );
}

#[test]
fn unique_slice_source_accepts_conservative_metadata_without_rewriting_abi() {
    for unpin in [false, true] {
        let owner = owner(unpin, false);
        let semantic = owner.source_semantic();
        let bytes = semantic.canonical_encoding().to_vec();
        let function = owner.execution_view_for_root(ROOT).unwrap().body();
        let before = function.abi().clone();
        let proof = UniqueSliceSourceV1::for_root(&owner, ROOT, binding()).unwrap();
        assert!(proof.view.has_expanded_calls());
        for ordinal in 0..2 {
            let arg = &function.abi().arguments()[ordinal];
            let result = proof
                .allocation(
                    semantic.types(),
                    function,
                    ordinal,
                    arg,
                    allocation(ordinal),
                )
                .unwrap();
            assert_eq!(result.allocation_origin, ordinal as u64 + 1);
            assert_eq!(result.noalias_class, ordinal as u64 + 2);
            assert!(result.writable);
            assert!(!result.singleton_object);
        }
        let origins = local_provenance_v1(semantic.types(), function)
            .unwrap()
            .allocation_origins;
        assert!(local_allocation_contracts(semantic.types(), function, &origins).is_err());
        let projected = local_allocation_contracts_with_source_v1(
            semantic.types(),
            function,
            &origins,
            Some(&proof),
        )
        .unwrap();
        assert_eq!(projected[1].unwrap().noalias_class, 2);
        assert_eq!(projected[2].unwrap().noalias_class, 3);
        assert_eq!(function.abi(), &before);
        assert_eq!(semantic.canonical_encoding(), bytes);
    }
}

#[test]
fn unique_slice_source_existing_abi_noalias_path_is_unchanged() {
    let owner = owner(true, true);
    let types = owner.source_semantic().types();
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let origins = local_provenance_v1(types, function)
        .unwrap()
        .allocation_origins;
    let proof = UniqueSliceSourceV1::for_root(&owner, ROOT, binding()).unwrap();
    assert_eq!(
        local_allocation_contracts(types, function, &origins).unwrap(),
        local_allocation_contracts_with_source_v1(types, function, &origins, Some(&proof)).unwrap(),
    );
}

#[test]
fn unique_slice_source_requires_unique_source_ownership_not_a_pointer_shape_alone() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::Unspecified,
    ] {
        let owner = owner_with_ownership(false, false, ownership);
        let proof = UniqueSliceSourceV1::for_root(&owner, ROOT, binding()).unwrap();
        let function = proof.view.body();
        assert!(
            proof
                .allocation(
                    proof.types,
                    function,
                    0,
                    &function.abi().arguments()[0],
                    allocation(0)
                )
                .is_none(),
            "{ownership:?}"
        );
    }
}

#[test]
fn unique_slice_source_rejects_misclassified_reference_ownership_at_admission() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::RawPointer,
    ] {
        assert!(matches!(
            request_with_ownership(false, false, ownership)
                .admit_current_production(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn unique_slice_source_requires_every_primitive_layout_to_match_exactly() {
    let mut shapes = vec![SemanticScalarTypeV1::Bool];
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            shapes.push(SemanticScalarTypeV1::Integer { signed, bits });
        }
    }
    shapes.extend([
        SemanticScalarTypeV1::Float { bits: 32 },
        SemanticScalarTypeV1::Float { bits: 64 },
    ]);
    for shape in shapes {
        let (primitive, maximum) = match shape {
            SemanticScalarTypeV1::Bool => (SemanticBackendPrimitiveV1::integer(false, 8, 1), 1),
            SemanticScalarTypeV1::Integer { signed, bits } => (
                SemanticBackendPrimitiveV1::integer(signed, bits, u64::from(bits / 8)),
                (1_u128 << bits) - 1,
            ),
            SemanticScalarTypeV1::Float { bits } => (
                SemanticBackendPrimitiveV1::float(bits, u64::from(bits / 8)),
                (1_u128 << bits) - 1,
            ),
            _ => unreachable!(),
        };
        let size = primitive.size_bytes().unwrap();
        let mut types = types(false);
        replace_type(&mut types, ELEMENT, |old| {
            SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(size),
                    size,
                    SemanticBackendReprV1::scalar(scalar(primitive, 0, maximum)),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(shape),
            )
        });
        replace_type(&mut types, SLICE, |old| {
            SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    size,
                    SemanticFieldsShapeV1::array(size, 0),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(false),
                    None,
                    false,
                    None,
                    size,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                old.shape().clone(),
            )
        });
        replace_type(&mut types, REFERENCE, |old| {
            old.clone().with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                            0,
                            size,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            )
        });
        assert!(
            exact_primitive_slice(&types, &argument_with_alignment(false, size)),
            "{shape:?}"
        );
        replace_type(&mut types, ELEMENT, |old| {
            SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                old.layout().clone(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 128,
                }),
            )
        });
        assert!(!exact_primitive_slice(&types, &argument(false)));
    }
}

#[test]
fn unique_slice_source_rejects_foreign_owner_function_types_and_argument() {
    let owner = owner(false, false);
    let proof = UniqueSliceSourceV1::for_root(&owner, ROOT, binding()).unwrap();
    let types = owner.source_semantic().types();
    let function = proof.view.body();
    let argument = &function.abi().arguments()[0];
    let cloned_function = function.clone();
    let cloned_types = types.to_vec();
    let cloned_argument = argument.clone();
    assert!(
        proof
            .allocation(types, &cloned_function, 0, argument, allocation(0))
            .is_none()
    );
    assert!(
        proof
            .allocation(&cloned_types, function, 0, argument, allocation(0))
            .is_none()
    );
    assert!(
        proof
            .allocation(types, function, 0, &cloned_argument, allocation(0))
            .is_none()
    );
    assert!(
        proof
            .allocation(types, function, 1, argument, allocation(1))
            .is_none()
    );
    assert!(
        proof
            .allocation(types, function, 0, argument, allocation(1))
            .is_none()
    );
    assert!(
        proof
            .allocation(types, function, usize::MAX, argument, allocation(0))
            .is_none()
    );
    let foreign = super::tests::owner(false, false);
    let foreign_function = foreign.execution_view_for_root(ROOT).unwrap().body();
    assert!(
        proof
            .allocation(
                foreign.source_semantic().types(),
                foreign_function,
                0,
                &foreign_function.abi().arguments()[0],
                allocation(0)
            )
            .is_none()
    );
}

#[test]
fn unique_slice_source_rejects_wrong_root_binding_and_expanded_parameter_mapping() {
    let owner = owner(false, false);
    assert!(
        UniqueSliceSourceV1::for_root(
            &owner,
            ROOT,
            SemanticKernelBindingIdentityV1::from_sha256([88; 32])
        )
        .is_none()
    );
    assert!(
        UniqueSliceSourceV1::for_root(&owner, SemanticFunctionIdV1::from_index(1), binding())
            .is_none()
    );
    assert!(
        UniqueSliceSourceV1::for_root(&owner, SemanticFunctionIdV1::from_index(99), binding())
            .is_none()
    );
    let proof = UniqueSliceSourceV1::for_root(&owner, ROOT, binding()).unwrap();
    assert!(!exact_argument_mapping(proof.source, proof.view, 0, 2));
    let helper = proof
        .view
        .local_origins()
        .iter()
        .position(|origin| origin.instance().index() != 0)
        .unwrap();
    assert!(!exact_argument_mapping(
        proof.source,
        proof.view,
        0,
        helper + 1
    ));
    let forged = UniqueSliceSourceV1 {
        arguments: vec![Some(2), Some(1)],
        ..proof
    };
    let function = forged.view.body();
    assert!(
        forged
            .allocation(
                forged.types,
                function,
                0,
                &function.abi().arguments()[0],
                allocation(0)
            )
            .is_none()
    );
}

fn replace_type(
    types: &mut [SemanticTypeDeclV1],
    index: SemanticTypeIdV1,
    change: impl FnOnce(&SemanticTypeDeclV1) -> SemanticTypeDeclV1,
) {
    types[index.index() as usize] = change(&types[index.index() as usize]);
}

#[test]
fn unique_slice_source_rejects_raw_shared_thin_and_wrong_address_space() {
    for (kind, mutability, metadata, address_space) in [
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::SliceLength,
            0,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::SliceLength,
            0,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
            0,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::SliceLength,
            1,
        ),
    ] {
        let mut types = types(false);
        replace_type(&mut types, REFERENCE, |old| {
            SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                old.layout().clone(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SLICE,
                        kind,
                        mutability,
                        address_space,
                        64,
                        metadata,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(old.abi_properties())
        });
        assert!(!exact_primitive_slice(&types, &argument(false)));
    }
}

#[test]
fn unique_slice_source_rejects_aggregate_reference_and_unsafe_cell_shaped_elements() {
    for shape in [
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ELEMENT]).unwrap()),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ELEMENT,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        SemanticTypeShapeV1::Opaque,
    ] {
        let mut types = types(false);
        // Even a scalar-layout wrapper cannot substitute for a primitive. No type-name test.
        replace_type(&mut types, ELEMENT, |old| {
            SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                old.layout().clone(),
                shape,
            )
        });
        assert!(!exact_primitive_slice(&types, &argument(false)));
    }
}

#[test]
fn unique_slice_source_rejects_wrong_layout_pointee_and_pass_mode() {
    let mut changed = types(false);
    replace_type(&mut changed, SLICE, |old| {
        SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::array(8, 0),
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
            old.shape().clone(),
        )
    });
    assert!(!exact_primitive_slice(&changed, &argument(false)));
    let mut changed = types(false);
    replace_type(&mut changed, REFERENCE, |old| {
        old.clone().with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        )
    });
    assert!(!exact_primitive_slice(&changed, &argument(false)));
    let direct = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        REFERENCE,
        SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
    ));
    assert!(!exact_primitive_slice(&types(false), &direct));
    let overridden = SemanticAbiArgumentV1::source(
        argument(false).value().clone().with_pointee_override(
            SemanticAbiPointeeInfoV1::new(
                SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                0,
                4,
            )
            .unwrap(),
        ),
    );
    assert!(!exact_primitive_slice(&types(false), &overridden));
}

#[test]
fn unique_slice_source_diagnostic_retains_the_live_argument_tuple() {
    let owner = owner(false, false);
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let argument = &function.abi().arguments()[1];
    let diagnostic = ArgumentDiagnosticV1 {
        function,
        ordinal: 1,
        ty: &owner.source_semantic().types()[REFERENCE.index() as usize],
        argument,
    }
    .to_string();
    for field in [
        "function=",
        "function_role=KernelRoot",
        "canon_abi=GpuKernel",
        "extern_abi=GpuKernel",
        "root_binding=",
        "arg=1",
        "type_id=3",
        "type_identity=",
        "ownership=Some(UniqueBorrow)",
        "MutableReference { unpin: false }",
        "override=None",
        "adjusted_role=Source",
        "passmode=Pair",
    ] {
        assert!(diagnostic.contains(field), "missing {field}: {diagnostic}");
    }
}
