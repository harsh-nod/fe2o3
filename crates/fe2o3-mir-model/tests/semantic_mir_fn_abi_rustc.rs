use std::collections::BTreeSet;

use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn scalar_type(tag: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn caller_location_type(tag: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
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
                    4,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn opaque_memory_type(tag: u8, rustc_size: u64, alignment: u64, sized: bool) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            rustc_size,
            alignment,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(sized),
            None,
            false,
            None,
            alignment,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn scalar_pair_type(tag: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    let scalar = SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::scalar_pair(scalar, scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn virtual_receiver_type(tag: u8) -> SemanticTypeDeclV1 {
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let metadata = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let raw = || SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap();
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, metadata),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::VTable,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_scalar_pointee_info(Some(raw()), Some(raw())),
    )
}

fn exact_virtual_receiver_adjustment(layout_tag: u8) -> SemanticAbiAdjustedTypeV1 {
    SemanticAbiAdjustedTypeV1::new(
        SemanticTypeIdV1::from_index(0),
        SemanticLayoutIdentityV1::from_sha256(bytes(layout_tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
    )
}

fn indirect_attrs(size: u64, alignment: u64) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            true,
            false,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        size,
        Some(alignment),
    )
    .unwrap()
}

fn direct_value() -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticAbiPassModeV1::Direct(noundef_attrs()),
    )
}

fn noundef_attrs() -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}

fn source(mode: SemanticAbiPassModeV1) -> SemanticAbiArgumentV1 {
    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        SemanticTypeIdV1::from_index(0),
        mode,
    ))
}

fn hidden(role: SemanticAbiHiddenArgumentRoleV1) -> SemanticAbiArgumentV1 {
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
        4,
        Some(4),
    )
    .unwrap();
    SemanticAbiArgumentV1::hidden(
        role,
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(2),
            SemanticAbiPassModeV1::Direct(attributes),
        ),
    )
}

fn abi(
    canon_abi: SemanticCanonAbiV1,
    c_variadic: bool,
    fixed_count: u32,
    arguments: Vec<SemanticAbiArgumentV1>,
) -> Result<SemanticFunctionAbiV1, SemanticMirErrorV1> {
    let extern_abi = match canon_abi {
        SemanticCanonAbiV1::C => SemanticExternAbiV1::C { unwind: false },
        SemanticCanonAbiV1::Rust => SemanticExternAbiV1::Rust,
        SemanticCanonAbiV1::RustCold => SemanticExternAbiV1::RustCold,
        SemanticCanonAbiV1::RustPreserveNone => SemanticExternAbiV1::RustPreserveNone,
        SemanticCanonAbiV1::Custom => SemanticExternAbiV1::Custom,
        SemanticCanonAbiV1::GpuKernel => SemanticExternAbiV1::GpuKernel,
        SemanticCanonAbiV1::Arm(_)
        | SemanticCanonAbiV1::Interrupt(_)
        | SemanticCanonAbiV1::X86(_) => SemanticExternAbiV1::C { unwind: false },
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(1)),
        canon_abi,
        extern_abi,
        false,
        c_variadic,
        fixed_count,
        arguments,
        direct_value(),
    )
}

fn request(abi: SemanticFunctionAbiV1) -> InertSemanticMirRequestV1 {
    request_with_argument_type(abi, scalar_type(1))
}

fn request_with_argument_type(
    abi: SemanticFunctionAbiV1,
    argument_type: SemanticTypeDeclV1,
) -> InertSemanticMirRequestV1 {
    let has_caller_location = !abi.hidden_arguments().is_empty();
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(1)),
        SemanticTypeIdV1::from_index(1),
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )];
    for index in 0..abi.fixed_count() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(u8::try_from(index + 2).unwrap())),
            SemanticTypeIdV1::from_index(0),
            SemanticLocalRoleV1::Argument(index),
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(250)),
        SemanticTypeIdV1::from_index(0),
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(1)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(1)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(1)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    let mut types = vec![argument_type, scalar_type(2)];
    if has_caller_location {
        types.push(caller_location_type(3));
    }
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(1))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admitted_identity(canon_abi: SemanticCanonAbiV1) -> InertSemanticMirSha256V1 {
    request(abi(canon_abi, false, 0, vec![]).unwrap())
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
        .semantic_sha256()
}

#[test]
fn pinned_canon_abi_variants_are_exact_and_collision_free() {
    let admitted_variants = [
        SemanticCanonAbiV1::C,
        SemanticCanonAbiV1::Rust,
        SemanticCanonAbiV1::RustCold,
        SemanticCanonAbiV1::RustPreserveNone,
    ];
    assert_eq!(
        admitted_variants
            .into_iter()
            .map(admitted_identity)
            .collect::<BTreeSet<_>>()
            .len(),
        admitted_variants.len()
    );

    let incompatible_with_amdgpu = [
        SemanticCanonAbiV1::Arm(SemanticArmCallV1::Aapcs),
        SemanticCanonAbiV1::Arm(SemanticArmCallV1::CCmseNonSecureCall),
        SemanticCanonAbiV1::Arm(SemanticArmCallV1::CCmseNonSecureEntry),
        SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::Avr),
        SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::AvrNonBlocking),
        SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::Msp430),
        SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::RiscvMachine),
        SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::RiscvSupervisor),
        SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::X86),
        SemanticCanonAbiV1::X86(SemanticX86CallV1::Fastcall),
        SemanticCanonAbiV1::X86(SemanticX86CallV1::Stdcall),
        SemanticCanonAbiV1::X86(SemanticX86CallV1::SysV64),
        SemanticCanonAbiV1::X86(SemanticX86CallV1::Thiscall),
        SemanticCanonAbiV1::X86(SemanticX86CallV1::Vectorcall),
        SemanticCanonAbiV1::X86(SemanticX86CallV1::Win64),
    ];
    for canon_abi in incompatible_with_amdgpu {
        assert!(matches!(
            abi(canon_abi, false, 0, vec![]),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn fixed_variadic_and_hidden_argument_contract_is_closed_and_ordered() {
    let source = || source(SemanticAbiPassModeV1::Direct(noundef_attrs()));
    let arguments = vec![
        source(),
        source(),
        hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation),
    ];
    let exact = abi(SemanticCanonAbiV1::Rust, false, 2, arguments).unwrap();
    assert!(!exact.c_variadic());
    assert_eq!(exact.fixed_count(), 2);
    assert_eq!(exact.fixed_arguments().len(), 2);
    assert_eq!(exact.adjusted_arguments().len(), 2);
    assert_eq!(exact.hidden_arguments().len(), 1);
    assert_eq!(
        exact.hidden_arguments()[0].role(),
        SemanticAbiArgumentRoleV1::Hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation)
    );
    request(exact.clone())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();

    assert!(matches!(
        abi(
            SemanticCanonAbiV1::GpuKernel,
            true,
            2,
            vec![source(), source()]
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    let c_variadic = abi(SemanticCanonAbiV1::C, true, 1, vec![source()]).unwrap();
    request(c_variadic)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();

    assert!(matches!(
        abi(SemanticCanonAbiV1::Rust, false, 0, vec![source()]),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        abi(
            SemanticCanonAbiV1::Rust,
            false,
            1,
            vec![
                source(),
                hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation),
                source(),
            ],
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        abi(
            SemanticCanonAbiV1::GpuKernel,
            false,
            1,
            vec![
                source(),
                hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation),
            ],
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        abi(
            SemanticCanonAbiV1::Rust,
            false,
            1,
            vec![
                source(),
                hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation),
                hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation),
            ],
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn pass_modes_preserve_rustc_facts_without_rust_layout_inference() {
    let register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 4).unwrap();
    let cases = [
        (
            SemanticCanonAbiV1::C,
            SemanticAbiPassModeV1::Direct(noundef_attrs()),
            scalar_type(1),
        ),
        (
            SemanticCanonAbiV1::C,
            SemanticAbiPassModeV1::Pair {
                first: noundef_attrs(),
                second: noundef_attrs(),
            },
            scalar_pair_type(1),
        ),
        (
            SemanticCanonAbiV1::Rust,
            SemanticAbiPassModeV1::cast(
                false,
                SemanticAbiCastV1::new(
                    [None; 8],
                    None,
                    SemanticAbiUniformV1::new(register, 4).unwrap(),
                    SemanticAbiValueAttributesV1::plain(),
                ),
            ),
            opaque_memory_type(1, 4, 4, true),
        ),
        (
            SemanticCanonAbiV1::C,
            SemanticAbiPassModeV1::Indirect {
                attributes: indirect_attrs(4, 4),
                metadata_attributes: Some(SemanticAbiValueAttributesV1::plain()),
                on_stack: false,
            },
            opaque_memory_type(1, 4, 4, false),
        ),
    ];
    for (canon_abi, mode, ty) in cases {
        request_with_argument_type(abi(canon_abi, false, 1, vec![source(mode)]).unwrap(), ty)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
    }

    let c_cast = SemanticAbiPassModeV1::cast(
        false,
        SemanticAbiCastV1::new(
            [None; 8],
            None,
            SemanticAbiUniformV1::new(register, 4).unwrap(),
            SemanticAbiValueAttributesV1::plain(),
        ),
    );
    for rejected in [
        c_cast,
        SemanticAbiPassModeV1::Indirect {
            attributes: indirect_attrs(4, 8),
            metadata_attributes: None,
            on_stack: true,
        },
    ] {
        assert!(matches!(
            request_with_argument_type(
                abi(SemanticCanonAbiV1::C, false, 1, vec![source(rejected)]).unwrap(),
                opaque_memory_type(1, 4, 4, true),
            )
            .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi | SemanticMirErrorV1::InvalidTypeLayout)
        ));
    }

    let dynamic_mode = |pointee_size| SemanticAbiPassModeV1::Indirect {
        attributes: indirect_attrs(pointee_size, 4),
        metadata_attributes: Some(SemanticAbiValueAttributesV1::plain()),
        on_stack: false,
    };
    let with_lower_bound = request_with_argument_type(
        abi(
            SemanticCanonAbiV1::C,
            false,
            1,
            vec![source(dynamic_mode(4))],
        )
        .unwrap(),
        opaque_memory_type(1, 4, 4, false),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let zero_lower_bound = request_with_argument_type(
        abi(
            SemanticCanonAbiV1::C,
            false,
            1,
            vec![source(dynamic_mode(0))],
        )
        .unwrap(),
        opaque_memory_type(1, 0, 4, false),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(with_lower_bound.types()[0].layout().rustc_size_bytes(), 4);
    assert_eq!(with_lower_bound.types()[0].layout().size_bytes(), None);
    assert_ne!(
        with_lower_bound.semantic_sha256(),
        zero_lower_bound.semantic_sha256()
    );

    let dynamic = opaque_memory_type(1, 4, 4, false);
    assert!(matches!(
        request_with_argument_type(
            abi(
                SemanticCanonAbiV1::C,
                false,
                1,
                vec![source(SemanticAbiPassModeV1::Indirect {
                    attributes: indirect_attrs(4, 4),
                    metadata_attributes: Some(SemanticAbiValueAttributesV1::plain()),
                    on_stack: true,
                })],
            )
            .unwrap(),
            dynamic,
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        request_with_argument_type(
            abi(
                SemanticCanonAbiV1::Rust,
                false,
                1,
                vec![source(SemanticAbiPassModeV1::Indirect {
                    attributes: indirect_attrs(4, 8),
                    metadata_attributes: None,
                    on_stack: true,
                })],
            )
            .unwrap(),
            opaque_memory_type(1, 4, 4, true),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn rustc_type_abi_properties_are_canonical_and_fail_closed() {
    let indirect_mode = || SemanticAbiPassModeV1::Indirect {
        attributes: indirect_attrs(4, 4),
        metadata_attributes: None,
        on_stack: false,
    };
    let plain_type = opaque_memory_type(1, 4, 4, true);
    let indirect_type = plain_type
        .clone()
        .with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(true, false));
    assert!(matches!(
        request_with_argument_type(
            abi(
                SemanticCanonAbiV1::C,
                false,
                1,
                vec![source(SemanticAbiPassModeV1::Direct(noundef_attrs(),))],
            )
            .unwrap(),
            indirect_type.clone(),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let plain = request_with_argument_type(
        abi(
            SemanticCanonAbiV1::C,
            false,
            1,
            vec![source(indirect_mode())],
        )
        .unwrap(),
        plain_type,
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let required = request_with_argument_type(
        abi(
            SemanticCanonAbiV1::C,
            false,
            1,
            vec![source(indirect_mode())],
        )
        .unwrap(),
        indirect_type,
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(plain.semantic_sha256(), required.semantic_sha256());
    assert!(
        required.types()[0]
            .abi_properties()
            .pass_indirectly_in_non_rustic_abis()
    );

    let foreign_tail = opaque_memory_type(1, 4, 4, false)
        .with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(false, true));
    assert!(matches!(
        request_with_argument_type(
            abi(
                SemanticCanonAbiV1::C,
                false,
                1,
                vec![source(SemanticAbiPassModeV1::Indirect {
                    attributes: indirect_attrs(4, 4),
                    metadata_attributes: Some(SemanticAbiValueAttributesV1::plain()),
                    on_stack: false,
                })],
            )
            .unwrap(),
            foreign_tail,
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn unadjusted_and_unwind_abi_restrictions_fail_closed() {
    let unadjusted = |mode| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(2)),
            SemanticLayoutIdentityV1::from_sha256(bytes(1)),
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::Unadjusted,
            false,
            false,
            1,
            vec![source(mode)],
            direct_value(),
        )
        .unwrap()
    };
    let register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 4).unwrap();
    let cast = SemanticAbiPassModeV1::cast(
        false,
        SemanticAbiCastV1::new(
            [None; 8],
            None,
            SemanticAbiUniformV1::new(register, 4).unwrap(),
            SemanticAbiValueAttributesV1::plain(),
        ),
    );
    assert!(matches!(
        request_with_argument_type(unadjusted(cast), opaque_memory_type(1, 4, 4, true))
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        request_with_argument_type(
            unadjusted(SemanticAbiPassModeV1::Indirect {
                attributes: indirect_attrs(4, 4),
                metadata_attributes: None,
                on_stack: false,
            }),
            scalar_type(1),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let gpu_unwind = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(1)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        true,
        false,
        0,
        vec![],
        direct_value(),
    )
    .unwrap();
    assert!(matches!(
        request(gpu_unwind).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let unadjusted_unwind = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(1)),
        SemanticCanonAbiV1::C,
        SemanticExternAbiV1::Unadjusted,
        true,
        false,
        0,
        vec![],
        direct_value(),
    )
    .unwrap();
    assert!(matches!(
        request(unadjusted_unwind).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn unadjusted_spec_abi_is_retained_for_direct_aggregate_validation() {
    let direct = || {
        source(SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::plain(),
        ))
    };
    let exact = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(1)),
        SemanticCanonAbiV1::C,
        SemanticExternAbiV1::Unadjusted,
        false,
        false,
        1,
        vec![direct()],
        direct_value(),
    )
    .unwrap();
    assert!(exact.spec_abi_unadjusted());
    request_with_argument_type(exact, opaque_memory_type(1, 4, 4, true))
        .admit(SemanticMirLimitsV1::default())
        .unwrap();

    assert!(matches!(
        request_with_argument_type(
            abi(SemanticCanonAbiV1::C, false, 1, vec![direct()]).unwrap(),
            opaque_memory_type(1, 4, 4, true),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(2)),
            SemanticLayoutIdentityV1::from_sha256(bytes(1)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Unadjusted,
            false,
            false,
            1,
            vec![direct()],
            direct_value(),
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn virtual_receiver_adjustment_is_source_typed_position_bound_and_exact() {
    let receiver = SemanticTypeIdV1::from_index(0);
    let no_undef = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let adjusted_argument = |adjustment| {
        SemanticAbiValueV1::new_with_adjusted_type(
            receiver,
            adjustment,
            SemanticAbiPassModeV1::Direct(no_undef),
        )
    };
    let rust_abi = |argument| {
        abi(
            SemanticCanonAbiV1::Rust,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(argument)],
        )
        .unwrap()
    };

    let exact = rust_abi(adjusted_argument(exact_virtual_receiver_adjustment(9)));
    assert!(matches!(
        request_with_argument_type(exact, virtual_receiver_type(1))
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let wrong_address_space = SemanticAbiAdjustedTypeV1::new(
        receiver,
        SemanticLayoutIdentityV1::from_sha256(bytes(9)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::union(
                SemanticBackendPrimitiveV1::pointer(1, 8, 8),
            )),
            false,
        )
        .unwrap(),
    );
    for adjustment in [exact_virtual_receiver_adjustment(1), wrong_address_space] {
        assert!(matches!(
            request_with_argument_type(
                rust_abi(adjusted_argument(adjustment)),
                virtual_receiver_type(1),
            )
            .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi | SemanticMirErrorV1::InvalidTypeLayout)
        ));
    }

    let foreign = abi(
        SemanticCanonAbiV1::C,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(adjusted_argument(
            exact_virtual_receiver_adjustment(9),
        ))],
    )
    .unwrap();
    assert!(matches!(
        request_with_argument_type(foreign, virtual_receiver_type(1))
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn hidden_and_adjusted_arguments_are_canonical_and_charged() {
    let source = || source(SemanticAbiPassModeV1::Direct(noundef_attrs()));
    let classified = abi(
        SemanticCanonAbiV1::Rust,
        false,
        2,
        vec![
            source(),
            source(),
            hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation),
        ],
    )
    .unwrap();
    let all_source = abi(
        SemanticCanonAbiV1::Rust,
        false,
        3,
        vec![source(), source(), source()],
    )
    .unwrap();
    let admitted = request(classified.clone())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let all_source = request(all_source)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_ne!(admitted.semantic_sha256(), all_source.semantic_sha256());
    assert!(
        admitted
            .canonical_encoding()
            .starts_with(b"fe2o3.inert-semantic-mir\x01\x00")
    );

    assert!(matches!(
        request(classified).admit(
            SemanticMirLimitsV1::default()
                .with_limit(SemanticMirResourceV1::CallArguments, 4)
                .unwrap()
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CallArguments,
            actual: 5,
            max: 4,
        })
    ));

    assert_eq!(
        admitted.semantic_sha256().as_bytes(),
        &[
            52, 217, 222, 4, 167, 210, 104, 129, 33, 31, 203, 243, 18, 239, 147, 220, 106, 64, 137,
            118, 15, 158, 248, 169, 53, 43, 132, 117, 211, 37, 236, 78,
        ],
        "replace with the intentional V1 ABI golden"
    );
}
