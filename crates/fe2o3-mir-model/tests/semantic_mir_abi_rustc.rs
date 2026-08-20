use std::collections::BTreeSet;

use fe2o3_mir_model::semantic_mir_v1::*;

fn identity(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn u32_type() -> SemanticTypeDeclV1 {
    u32_type_with_tag(1)
}

fn u32_type_with_tag(tag: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    let scalar = SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn pointer_type() -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let scalar = SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(1)),
        SemanticLayoutIdentityV1::from_sha256(identity(1)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                SemanticTypeIdV1::from_index(0),
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
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    )
}

fn memory_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(1)),
        SemanticLayoutIdentityV1::from_sha256(identity(1)),
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

fn request_with_mode(mode: SemanticAbiPassModeV1) -> InertSemanticMirRequestV1 {
    let argument_type = if matches!(&mode, SemanticAbiPassModeV1::Cast { .. }) {
        memory_type()
    } else {
        u32_type()
    };
    request_with_type_and_mode(argument_type, mode)
}

fn request_with_type_and_mode(
    argument_type: SemanticTypeDeclV1,
    mode: SemanticAbiPassModeV1,
) -> InertSemanticMirRequestV1 {
    let ty = SemanticTypeIdV1::from_index(0);
    let return_type = SemanticTypeIdV1::from_index(1);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(identity(2)),
        SemanticLayoutIdentityV1::from_sha256(identity(1)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(ty, mode)],
        SemanticAbiValueV1::new(
            return_type,
            SemanticAbiPassModeV1::Direct(attributes(SemanticAbiRegularAttributesV1::new(
                false, None, false, false, false, true,
            ))),
        ),
    )
    .unwrap();
    let locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(1)),
            return_type,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(2)),
            ty,
            SemanticLocalRoleV1::Argument(0),
            SemanticSourceProvenanceV1::unavailable(),
        ),
    ];
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity(1)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(1)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(1)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(1)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(1)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(1)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(1))),
        vec![argument_type, u32_type_with_tag(2)],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admitted_with_mode(mode: SemanticAbiPassModeV1) -> AdmittedInertSemanticMirV1 {
    request_with_mode(mode)
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
}

fn attributes(regular: SemanticAbiRegularAttributesV1) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(regular, SemanticAbiExtensionV1::None, 0, None).unwrap()
}

fn integer_register(bytes: u64) -> SemanticAbiRegisterV1 {
    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, bytes).unwrap()
}

fn exact_cast(
    pad_i32: bool,
    prefix: [Option<SemanticAbiRegisterV1>; 8],
    rest: SemanticAbiUniformV1,
    attributes: SemanticAbiValueAttributesV1,
) -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::cast(
        pad_i32,
        SemanticAbiCastV1::new(prefix, None, rest, attributes),
    )
}

#[test]
fn rustc_arg_attribute_bits_are_exact_validated_and_collision_free() {
    let captures = [
        (None, 0b000),
        (Some(SemanticAbiPointerCaptureV1::CapturesReadOnly), 0b100),
        (Some(SemanticAbiPointerCaptureV1::CapturesAddress), 0b110),
        (Some(SemanticAbiPointerCaptureV1::CapturesNone), 0b111),
    ];
    let mut regular_values = BTreeSet::new();
    for (capture, bits) in captures {
        let regular =
            SemanticAbiRegularAttributesV1::new(false, capture, false, false, false, true);
        assert_eq!(regular.rustc_bits(), bits | 0x80);
        assert_eq!(regular.pointer_capture(), capture);
        assert_eq!(
            SemanticAbiRegularAttributesV1::from_rustc_bits(bits | 0x80).unwrap(),
            regular
        );
        regular_values.insert(regular.rustc_bits());

        let admission = request_with_type_and_mode(
            pointer_type(),
            SemanticAbiPassModeV1::Direct(attributes(regular)),
        )
        .admit(SemanticMirLimitsV1::default());
        if capture.is_none() {
            admission.unwrap();
        } else {
            assert!(matches!(
                admission,
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            ));
        }
    }
    assert_eq!(regular_values.len(), 4);

    let no_undef = SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true);
    assert_eq!(no_undef.rustc_bits(), 0x80);
    assert!(no_undef.no_undef());
    request_with_type_and_mode(
        pointer_type(),
        SemanticAbiPassModeV1::Direct(attributes(no_undef)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let all = SemanticAbiRegularAttributesV1::new(
        true,
        Some(SemanticAbiPointerCaptureV1::CapturesNone),
        true,
        true,
        true,
        true,
    );
    assert_eq!(all.rustc_bits(), 0xff);
    assert_eq!(
        SemanticAbiRegularAttributesV1::from_rustc_bits(0xff).unwrap(),
        all
    );

    for malformed_capture in [0b001, 0b010, 0b011, 0b101] {
        assert!(matches!(
            SemanticAbiRegularAttributesV1::from_rustc_bits(malformed_capture | 0xf8),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn cast_target_has_exact_slots_pad_and_uniform_semantics() {
    let i8 = integer_register(1);
    let i32 = integer_register(4);
    let rest = SemanticAbiUniformV1::new(i32, 4).unwrap();
    let cast = SemanticAbiCastV1::new(
        [Some(i8), None, None, None, None, None, None, Some(i32)],
        Some(4),
        rest,
        SemanticAbiValueAttributesV1::plain(),
    );
    assert_eq!(cast.prefix().len(), 8);
    assert_eq!(cast.prefix()[0], Some(i8));
    assert_eq!(cast.prefix()[7], Some(i32));
    assert_eq!(cast.rest_offset_bytes(), Some(4));
    assert_eq!(cast.rest(), rest);
    assert_eq!(cast.rest().unit(), i32);
    assert_eq!(cast.rest_total_bytes(), 4);
    assert!(!cast.rest_consecutive());

    let mut rejected_slots = 0;
    for slot in 0..8 {
        let mut prefix = [None; 8];
        prefix[slot] = Some(i8);
        assert!(matches!(
            request_with_mode(exact_cast(
                false,
                prefix,
                rest,
                SemanticAbiValueAttributesV1::plain(),
            ))
            .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
        rejected_slots += 1;
    }
    assert_eq!(rejected_slots, 8);

    let without_pad = admitted_with_mode(exact_cast(
        false,
        [None; 8],
        rest,
        SemanticAbiValueAttributesV1::plain(),
    ));
    assert!(matches!(
        request_with_mode(exact_cast(
            true,
            [None; 8],
            rest,
            SemanticAbiValueAttributesV1::plain(),
        ))
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert_ne!(without_pad.semantic_sha256().as_bytes(), &[0; 32]);

    let exact =
        SemanticAbiCastV1::new([None; 8], None, rest, SemanticAbiValueAttributesV1::plain());
    assert_eq!(
        exact.prefix(),
        &[None, None, None, None, None, None, None, None]
    );
}

#[test]
fn uniform_zero_integer_tail_and_hostile_casts_are_handled() {
    let i32 = integer_register(4);
    let f32 = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Float, 4).unwrap();

    let zero = SemanticAbiUniformV1::new(i32, 0).unwrap();
    assert_eq!(zero.total_bytes(), 0);
    assert!(matches!(
        request_with_mode(exact_cast(
            false,
            [Some(i32), None, None, None, None, None, None, None],
            zero,
            SemanticAbiValueAttributesV1::plain(),
        ))
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let short_integer_tail = SemanticAbiUniformV1::consecutive(i32, 3).unwrap();
    assert_eq!(short_integer_tail.total_bytes(), 3);
    assert!(short_integer_tail.is_consecutive());
    assert!(matches!(
        request_with_mode(exact_cast(
            false,
            [None; 8],
            short_integer_tail,
            SemanticAbiValueAttributesV1::plain(),
        ))
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    assert!(matches!(
        SemanticAbiUniformV1::new(f32, 3),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let empty_target = exact_cast(
        false,
        [None; 8],
        zero,
        SemanticAbiValueAttributesV1::plain(),
    );
    assert!(matches!(
        request_with_mode(empty_target).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let overflowing = SemanticAbiUniformV1::new(integer_register(2), u64::MAX).unwrap();
    assert!(matches!(
        request_with_mode(exact_cast(
            false,
            [None; 8],
            overflowing,
            SemanticAbiValueAttributesV1::plain(),
        ))
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn corrected_rustc_abi_grammar_has_a_golden_encoding() {
    let i32 = integer_register(4);
    let admitted = admitted_with_mode(SemanticAbiPassModeV1::cast(
        false,
        SemanticAbiCastV1::new(
            [None; 8],
            None,
            SemanticAbiUniformV1::new(i32, 4).unwrap(),
            SemanticAbiValueAttributesV1::plain(),
        ),
    ));
    assert!(
        admitted
            .canonical_encoding()
            .starts_with(b"fe2o3.inert-semantic-mir\x01\x00")
    );
    assert_eq!(
        admitted.semantic_sha256().as_bytes(),
        &[
            165, 38, 252, 232, 12, 108, 129, 12, 177, 177, 167, 0, 234, 114, 137, 221, 54, 214,
            135, 163, 42, 149, 185, 65, 43, 202, 87, 144, 93, 25, 0, 102,
        ],
        "update only when the corrected ABI grammar intentionally changes"
    );
}

#[test]
fn rust_aggregate_classification_switches_exactly_at_the_gfx942_pointer_width() {
    let memory16 = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(1)),
        SemanticLayoutIdentityV1::from_sha256(identity(1)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let indirect = SemanticAbiPassModeV1::Indirect {
        attributes: SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesAddress),
                true,
                false,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            16,
            Some(8),
        )
        .unwrap(),
        metadata_attributes: None,
        on_stack: false,
    };
    request_with_type_and_mode(memory16.clone(), indirect)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        request_with_type_and_mode(
            memory16,
            exact_cast(
                false,
                [None; 8],
                SemanticAbiUniformV1::new(integer_register(16), 16).unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let noundef_memory = memory_type().with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
    );
    let exact_noundef = exact_cast(
        false,
        [None; 8],
        SemanticAbiUniformV1::new(integer_register(4), 4).unwrap(),
        attributes(SemanticAbiRegularAttributesV1::new(
            false, None, false, false, false, true,
        )),
    );
    request_with_type_and_mode(noundef_memory.clone(), exact_noundef)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        request_with_type_and_mode(
            noundef_memory,
            exact_cast(
                false,
                [None; 8],
                SemanticAbiUniformV1::new(integer_register(4), 4).unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}
