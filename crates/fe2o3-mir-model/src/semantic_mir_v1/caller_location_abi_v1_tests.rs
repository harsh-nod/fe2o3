use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const LOCATION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn attributes(frozen: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            frozen,
            frozen.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            frozen,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        if frozen { 24 } else { 0 },
        Some(8),
    )
    .unwrap()
}

fn fixture(frozen: bool) -> InertSemanticMirRequestV1 {
    let unit = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::Arbitrary {
                source_order_offsets_bytes: Box::new([]),
                memory_order_source_indices: Box::new([]),
            },
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
    );
    let location = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(24),
            8,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
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
                LOCATION,
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
                    SemanticAbiPointeeKindV1::SharedReference { frozen },
                    if frozen { 24 } else { 0 },
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![SemanticAbiArgumentV1::hidden(
            SemanticAbiHiddenArgumentRoleV1::CallerLocation,
            SemanticAbiValueV1::new(REFERENCE, SemanticAbiPassModeV1::Direct(attributes(frozen))),
        )],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([1; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([1; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([1; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([1; 32]),
            UNIT,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([1; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([1; 32])),
        vec![unit, location, reference],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admit(request: InertSemanticMirRequestV1) -> AdmittedInertSemanticMirV1 {
    request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap()
}

fn reject(request: InertSemanticMirRequestV1) {
    let error = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .expect_err("mutated hidden ABI must fail admission");
    assert!(
        matches!(error, SemanticMirErrorV1::InvalidFunctionAbi),
        "{error:?}"
    );
}

fn hidden_value(request: &mut InertSemanticMirRequestV1) -> &mut SemanticAbiValueV1 {
    &mut request.functions[0].abi.arguments[0].value
}

#[test]
fn caller_location_exact_frozen_and_unoptimized_profiles_round_trip() {
    let unoptimized = admit(fixture(false));
    let frozen = admit(fixture(true));
    assert_ne!(
        unoptimized.canonical_encoding(),
        frozen.canonical_encoding()
    );
    for mir in [unoptimized, frozen] {
        let abi = mir.functions()[0].abi();
        assert!(abi.source_input_types().is_empty());
        assert!(abi.fixed_arguments().is_empty());
        assert_eq!(abi.hidden_arguments().len(), 1);
        assert_eq!(mir.functions()[0].locals().len(), 1);
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        assert_eq!(decoded.functions(), mir.functions());
    }
}

#[test]
fn caller_location_profile_attributes_cannot_be_substituted() {
    for frozen in [false, true] {
        let mut request = fixture(frozen);
        hidden_value(&mut request).mode = SemanticAbiPassModeV1::Direct(attributes(!frozen));
        reject(request);
    }
}

#[test]
fn caller_location_every_attribute_remains_exact() {
    for frozen in [false, true] {
        for bit in [
            SemanticAbiRegularAttributesV1::NO_ALIAS,
            SemanticAbiPointerCaptureV1::CapturesReadOnly.rustc_bits(),
            SemanticAbiRegularAttributesV1::NON_NULL,
            SemanticAbiRegularAttributesV1::READ_ONLY,
            SemanticAbiRegularAttributesV1::IN_REGISTER,
            SemanticAbiRegularAttributesV1::NO_UNDEF,
        ] {
            let mut request = fixture(frozen);
            let mut changed = attributes(frozen);
            changed.regular =
                SemanticAbiRegularAttributesV1::from_rustc_bits(changed.regular.rustc_bits() ^ bit)
                    .unwrap();
            hidden_value(&mut request).mode = SemanticAbiPassModeV1::Direct(changed);
            reject(request);
        }
        let exact = attributes(frozen);
        for changed in [
            SemanticAbiValueAttributesV1 {
                pointee_size_bytes: 1,
                ..exact
            },
            SemanticAbiValueAttributesV1 {
                pointee_alignment_bytes: None,
                ..exact
            },
            SemanticAbiValueAttributesV1 {
                pointee_alignment_bytes: Some(4),
                ..exact
            },
            SemanticAbiValueAttributesV1 {
                extension: SemanticAbiExtensionV1::ZeroExtend,
                ..exact
            },
        ] {
            let mut request = fixture(frozen);
            hidden_value(&mut request).mode = SemanticAbiPassModeV1::Direct(changed);
            reject(request);
        }
    }
}

#[test]
fn caller_location_requires_retained_shared_reference_facts() {
    let mut missing = fixture(false);
    missing.types[REFERENCE.0 as usize]
        .abi_properties
        .first_pointee = None;
    reject(missing);
    for replacement in [
        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
            0,
            8,
        )
        .unwrap(),
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            24,
            8,
        )
        .unwrap(),
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            0,
            4,
        )
        .unwrap(),
    ] {
        let mut request = fixture(false);
        request.types[REFERENCE.0 as usize]
            .abi_properties
            .first_pointee = Some(replacement);
        reject(request);
    }
}

#[test]
fn caller_location_stays_hidden_and_direct_without_overrides() {
    let mut request = fixture(false);
    hidden_value(&mut request).mode = SemanticAbiPassModeV1::Ignore;
    reject(request);
    let mut request = fixture(false);
    hidden_value(&mut request).pointee_override = request.types[REFERENCE.0 as usize]
        .abi_properties
        .first_pointee;
    reject(request);
    let mut request = fixture(false);
    request.functions[0].abi.arguments[0].role = SemanticAbiArgumentRoleV1::Source;
    reject(request);
    let mut request = fixture(false);
    request.functions[0].abi.source_signature.inputs = vec![REFERENCE].into_boxed_slice();
    reject(request);
}
