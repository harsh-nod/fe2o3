use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn layout_identity() -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(240))
}

fn full_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1)
}

fn integer_type(tag: u8, bits: u16, initialized: bool) -> SemanticTypeDeclV1 {
    let size_bytes = u64::from(bits / 8);
    let primitive = SemanticBackendPrimitiveV1::integer(false, bits, size_bytes);
    let scalar = if initialized {
        SemanticBackendScalarV1::initialized(primitive, full_range(bits))
    } else {
        SemanticBackendScalarV1::union(primitive)
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            size_bytes,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        if initialized {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            })
        } else {
            SemanticTypeShapeV1::Opaque
        },
    )
}

fn unit_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn memory_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn scalar_pair_type(tag: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    let scalar = SemanticBackendScalarV1::initialized(primitive, full_range(32));
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

fn vector_type(tag: u8) -> SemanticTypeDeclV1 {
    let element = SemanticBackendScalarV1::union(SemanticBackendPrimitiveV1::integer(false, 32, 4));
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            16,
            SemanticBackendReprV1::simd_vector(element, 4),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn reference_pointer_type(tag: u8, pointee: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
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
                pointee,
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

#[allow(clippy::too_many_arguments)]
fn attributes(
    no_alias: bool,
    capture: Option<SemanticAbiPointerCaptureV1>,
    non_null: bool,
    read_only: bool,
    no_undef: bool,
    extension: SemanticAbiExtensionV1,
    pointee_size: u64,
    pointee_alignment: Option<u64>,
) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            no_alias, capture, non_null, read_only, false, no_undef,
        ),
        extension,
        pointee_size,
        pointee_alignment,
    )
    .unwrap()
}

fn initialized_integer_attributes(
    extension: SemanticAbiExtensionV1,
) -> SemanticAbiValueAttributesV1 {
    attributes(false, None, false, false, true, extension, 0, None)
}

fn union_integer_attributes(extension: SemanticAbiExtensionV1) -> SemanticAbiValueAttributesV1 {
    attributes(false, None, false, false, false, extension, 0, None)
}

fn pointer_return_attributes(
    no_alias: bool,
    capture: Option<SemanticAbiPointerCaptureV1>,
    read_only: bool,
    pointee_size: u64,
) -> SemanticAbiValueAttributesV1 {
    attributes(
        no_alias,
        capture,
        true,
        read_only,
        true,
        SemanticAbiExtensionV1::None,
        pointee_size,
        Some(4),
    )
}

fn indirect_attributes(on_stack: bool) -> SemanticAbiPassModeV1 {
    indirect_attributes_for(4, 4, on_stack)
}

fn indirect_attributes_for(size: u64, alignment: u64, on_stack: bool) -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Indirect {
        attributes: attributes(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            true,
            false,
            true,
            SemanticAbiExtensionV1::None,
            size,
            Some(alignment),
        ),
        metadata_attributes: None,
        on_stack,
    }
}

fn function_abi(
    canon_abi: SemanticCanonAbiV1,
    extern_abi: SemanticExternAbiV1,
    argument: Option<SemanticAbiValueV1>,
    return_value: SemanticAbiValueV1,
) -> SemanticFunctionAbiV1 {
    let arguments: Vec<_> = argument
        .into_iter()
        .map(SemanticAbiArgumentV1::source)
        .collect();
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(200)),
        layout_identity(),
        canon_abi,
        extern_abi,
        false,
        false,
        u32::try_from(arguments.len()).unwrap(),
        arguments,
        return_value,
    )
    .unwrap()
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    abi: SemanticFunctionAbiV1,
) -> InertSemanticMirRequestV1 {
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(1)),
        abi.return_value().ty(),
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )];
    for (index, ty) in abi.source_input_types().iter().copied().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(u8::try_from(index).unwrap() + 2)),
            ty,
            SemanticLocalRoleV1::Argument(u32::try_from(index).unwrap()),
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
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
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity()),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn assert_valid(types: Vec<SemanticTypeDeclV1>, abi: SemanticFunctionAbiV1) {
    request(types, abi)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
}

fn assert_invalid(types: Vec<SemanticTypeDeclV1>, abi: SemanticFunctionAbiV1) {
    assert!(matches!(
        request(types, abi).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

fn c_abi(
    argument: Option<SemanticAbiValueV1>,
    return_value: SemanticAbiValueV1,
) -> SemanticFunctionAbiV1 {
    function_abi(
        SemanticCanonAbiV1::C,
        SemanticExternAbiV1::C { unwind: false },
        argument,
        return_value,
    )
}

#[test]
fn pass_indirectly_in_non_rustic_abis_applies_to_arguments_and_returns() {
    let value = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    let direct =
        SemanticAbiPassModeV1::Direct(initialized_integer_attributes(SemanticAbiExtensionV1::None));
    let plain = integer_type(1, 32, true);
    let indirect = plain
        .clone()
        .with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(true, false));

    assert_valid(
        vec![plain, unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(value, direct.clone())),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );
    assert_invalid(
        vec![indirect.clone(), unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(value, direct.clone())),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );
    assert_valid(
        vec![indirect.clone(), unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(value, indirect_attributes(false))),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );
    assert_invalid(
        vec![indirect.clone()],
        c_abi(None, SemanticAbiValueV1::new(value, direct)),
    );
    assert_valid(
        vec![indirect],
        c_abi(
            None,
            SemanticAbiValueV1::new(value, indirect_attributes(false)),
        ),
    );

    let pair = scalar_pair_type(1)
        .with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(true, false));
    let direct_pair = SemanticAbiPassModeV1::Pair {
        first: initialized_integer_attributes(SemanticAbiExtensionV1::None),
        second: initialized_integer_attributes(SemanticAbiExtensionV1::None),
    };
    assert_invalid(
        vec![pair.clone(), unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(value, direct_pair)),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );
    assert_valid(
        vec![pair, unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(
                value,
                indirect_attributes_for(8, 4, false),
            )),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );

    let vector =
        vector_type(1).with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(true, false));
    assert_invalid(
        vec![vector.clone(), unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(
                value,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );
    assert_valid(
        vec![vector, unit_type(2)],
        c_abi(
            Some(SemanticAbiValueV1::new(
                value,
                indirect_attributes_for(16, 16, false),
            )),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );
}

#[test]
fn foreign_direct_union_integer_requires_small_integer_extension() {
    let value = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    let abi = |extension| {
        c_abi(
            Some(SemanticAbiValueV1::new(
                value,
                SemanticAbiPassModeV1::Direct(union_integer_attributes(extension)),
            )),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
    };

    assert_valid(
        vec![integer_type(1, 16, false), unit_type(2)],
        abi(SemanticAbiExtensionV1::ZeroExtend),
    );
    assert_invalid(
        vec![integer_type(1, 16, false), unit_type(2)],
        abi(SemanticAbiExtensionV1::None),
    );
}

#[test]
fn unadjusted_small_integer_skips_amdgpu_extension() {
    let value = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    let abi = |extension| {
        function_abi(
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::Unadjusted,
            Some(SemanticAbiValueV1::new(
                value,
                SemanticAbiPassModeV1::Direct(initialized_integer_attributes(extension)),
            )),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
    };

    assert_valid(
        vec![integer_type(1, 16, true), unit_type(2)],
        abi(SemanticAbiExtensionV1::None),
    );
    assert_invalid(
        vec![integer_type(1, 16, true), unit_type(2)],
        abi(SemanticAbiExtensionV1::ZeroExtend),
    );
}

#[test]
fn direct_pointer_return_rejects_argument_only_attributes() {
    let pointer = SemanticTypeIdV1::from_index(0);
    let pointee = SemanticTypeIdV1::from_index(1);
    let pointer_type = || reference_pointer_type(1, pointee);
    let pointee_type = || integer_type(2, 32, true);
    let return_abi = |attributes| {
        c_abi(
            None,
            SemanticAbiValueV1::new(pointer, SemanticAbiPassModeV1::Direct(attributes)),
        )
    };

    assert_valid(
        vec![pointer_type(), pointee_type()],
        return_abi(pointer_return_attributes(false, None, false, 0)),
    );
    for rejected in [
        pointer_return_attributes(true, None, false, 0),
        pointer_return_attributes(
            false,
            Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            false,
            0,
        ),
        pointer_return_attributes(false, None, true, 0),
        pointer_return_attributes(false, None, false, 4),
    ] {
        assert_invalid(vec![pointer_type(), pointee_type()], return_abi(rejected));
    }
}

#[test]
fn shared_reference_argument_attributes_require_exact_rustc_pointee_evidence() {
    let pointer = SemanticTypeIdV1::from_index(0);
    let pointee = SemanticTypeIdV1::from_index(1);
    let unit = SemanticTypeIdV1::from_index(2);
    let argument_abi = |attributes| {
        c_abi(
            Some(SemanticAbiValueV1::new(
                pointer,
                SemanticAbiPassModeV1::Direct(attributes),
            )),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
    };
    let pointer_type = || reference_pointer_type(1, pointee);
    let pointee_type = || integer_type(2, 32, true);
    let unit_type = || unit_type(3);
    let exact = attributes(
        true,
        Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
        true,
        true,
        true,
        SemanticAbiExtensionV1::None,
        4,
        Some(4),
    );
    assert_valid(
        vec![pointer_type(), pointee_type(), unit_type()],
        argument_abi(exact),
    );

    for rejected in [
        attributes(
            false,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            true,
            SemanticAbiExtensionV1::None,
            4,
            Some(4),
        ),
        attributes(
            true,
            None,
            true,
            true,
            true,
            SemanticAbiExtensionV1::None,
            4,
            Some(4),
        ),
        attributes(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            false,
            true,
            SemanticAbiExtensionV1::None,
            4,
            Some(4),
        ),
        attributes(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            true,
            SemanticAbiExtensionV1::None,
            0,
            Some(4),
        ),
    ] {
        assert_invalid(
            vec![pointer_type(), pointee_type(), unit_type()],
            argument_abi(rejected),
        );
    }
}

#[test]
fn gpu_kernel_rejects_cast_and_on_stack_argument_modes() {
    let argument = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
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
    let gpu_abi = |mode| {
        function_abi(
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            Some(SemanticAbiValueV1::new(argument, mode)),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
    };

    for rejected in [cast, indirect_attributes(true)] {
        assert_invalid(vec![memory_type(1), unit_type(2)], gpu_abi(rejected));
    }
}

fn shared_slice_type(
    tag: u8,
    pointee: SemanticTypeIdV1,
    pointee_alignment: u64,
    guaranteed_size: u64,
) -> SemanticTypeDeclV1 {
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        full_range(64),
    );
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, length),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
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
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    guaranteed_size,
                    pointee_alignment,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn shared_slice_pair_mode(pointee_alignment: u64, is_return: bool) -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Pair {
        first: attributes(
            !is_return,
            (!is_return).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            !is_return,
            true,
            SemanticAbiExtensionV1::None,
            0,
            (pointee_alignment > 1).then_some(pointee_alignment),
        ),
        second: initialized_integer_attributes(SemanticAbiExtensionV1::None),
    }
}

#[test]
fn fat_slice_references_use_the_unsized_zero_byte_lower_bound() {
    let element = SemanticTypeIdV1::from_index(0);
    let slice = SemanticTypeIdV1::from_index(1);
    let unit = SemanticTypeIdV1::from_index(2);
    let argument = SemanticAbiValueV1::new(slice, shared_slice_pair_mode(4, false));
    let return_value = SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore);
    assert_valid(
        vec![
            integer_type(1, 32, true),
            shared_slice_type(2, element, 4, 0),
            unit_type(3),
        ],
        function_abi(
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            Some(argument),
            return_value,
        ),
    );

    let forged_argument = SemanticAbiValueV1::new(slice, shared_slice_pair_mode(4, false));
    assert_invalid(
        vec![
            integer_type(1, 32, true),
            shared_slice_type(2, element, 4, 4),
            unit_type(3),
        ],
        function_abi(
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            Some(forged_argument),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        ),
    );

    let zst = SemanticTypeIdV1::from_index(0);
    let zst_slice = SemanticTypeIdV1::from_index(1);
    assert_valid(
        vec![unit_type(1), shared_slice_type(2, zst, 1, 0)],
        function_abi(
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            Some(SemanticAbiValueV1::new(
                zst_slice,
                shared_slice_pair_mode(1, false),
            )),
            SemanticAbiValueV1::new(zst, SemanticAbiPassModeV1::Ignore),
        ),
    );

    assert_valid(
        vec![
            integer_type(1, 32, true),
            shared_slice_type(2, element, 4, 0),
        ],
        function_abi(
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            None,
            SemanticAbiValueV1::new(slice, shared_slice_pair_mode(4, true)),
        ),
    );
}
