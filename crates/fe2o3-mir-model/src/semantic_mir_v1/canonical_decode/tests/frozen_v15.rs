//! Full admitted documents for the published BF16 V15 grammar. These synthetic
//! codec fixtures do not claim Rust-source or GPU execution qualification.

use super::*;
use sha2::{Digest, Sha256};

const INDEX: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const VIEW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const FRAGMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const RESULT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const VIEW_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const LANE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const OPERATION_IDENTITY: [u8; 32] = [0xa7; 32];

// Frozen against the published e3c359fb1bf39ec21c4239ac37ce59b7a3a51db9 encoder.
// Literal operation payloads below independently pin the colliding grammar.
const CONSTRUCTOR_SHA256: [u8; 32] = [
    0x29, 0xce, 0x3e, 0xda, 0x9a, 0x5d, 0x06, 0xd1, 0x83, 0x75, 0x4c, 0xc0, 0x73, 0x37, 0x4f, 0x1b,
    0xbf, 0x04, 0xf0, 0xc8, 0x5f, 0xa9, 0x02, 0x6c, 0x7d, 0x26, 0xb3, 0x76, 0x44, 0x88, 0xe6, 0xc5,
];
const LOAD_SHA256: [u8; 32] = [
    0x6f, 0x46, 0xbb, 0xb7, 0xe0, 0xf7, 0xdd, 0x28, 0x13, 0xf8, 0x96, 0x51, 0x0c, 0xa4, 0x0d, 0x28,
    0x44, 0x1e, 0xde, 0x17, 0xe2, 0x57, 0x47, 0x15, 0xa4, 0xe4, 0x87, 0xd9, 0x74, 0xe0, 0xdb, 0xd2,
];

fn declaration(
    index: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(20 + index)),
        SemanticLayoutIdentityV1(identity(30 + index)),
        layout,
        shape,
    )
}

fn reference(index: u8, pointee: SemanticTypeIdV1, size: u64) -> SemanticTypeDeclV1 {
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
                    size,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let (regular, size, alignment) = match ty {
        VIEW_REF | LANE_REF => (
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                true,
                false,
                true,
            ),
            if ty == VIEW_REF { 16 } else { 4 },
            Some(4),
        ),
        RESULT | FRAGMENT => (
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesNone),
                true,
                false,
                false,
                true,
            ),
            if ty == RESULT { 24 } else { 16 },
            Some(4),
        ),
        INDEX => (
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            0,
            None,
        ),
        _ => unreachable!(),
    };
    let attributes =
        SemanticAbiValueAttributesV1::new(regular, SemanticAbiExtensionV1::None, size, alignment)
            .unwrap();
    let mode = if matches!(ty, RESULT | FRAGMENT) {
        SemanticAbiPassModeV1::Indirect {
            attributes,
            metadata_attributes: None,
            on_stack: false,
        }
    } else {
        SemanticAbiPassModeV1::Direct(attributes)
    };
    SemanticAbiValueV1::new(ty, mode)
}

fn operation(constructor: bool) -> SemanticCompilerIntrinsicOperationV1 {
    if constructor {
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewColumnMajor {
            result: RESULT,
            view: VIEW,
            error: INDEX,
        }
    } else {
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
            fragment: FRAGMENT,
            view: VIEW,
            lane: INDEX,
            contract: SemanticMfmaOperandContractV1 {
                role: SemanticMfmaOperandRoleV1::B,
                profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
                register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
                wave_width: 64,
            },
            storage_layout: SemanticMfmaStorageLayoutV1::ColumnMajor,
        }
    }
}

fn request(constructor: bool) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let mut types = request.types.into_vec();
    for index in [1, 2] {
        types.push(declaration(
            index,
            SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
            SemanticTypeShapeV1::Opaque,
        ));
    }
    let variants = [VIEW, INDEX]
        .into_iter()
        .enumerate()
        .map(|(index, field)| {
            SemanticEnumVariantV1::new(
                index as u128,
                SemanticAggregateTypeV1::new(vec![field]).unwrap(),
            )
        })
        .collect();
    let layouts = [20, 8]
        .into_iter()
        .enumerate()
        .map(|(index, end)| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index as u32,
                24,
                4,
                SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticAggregateLayoutV1::new(
                    vec![4],
                    vec![SemanticPaddingV1::new(end, 24 - end).unwrap()],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    types.push(declaration(
        3,
        SemanticTypeLayoutV1::enum_layout(
            24,
            4,
            SemanticEnumLayoutV1::new(
                layouts,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(INDEX, variants).unwrap(),
    ));
    types.push(reference(4, VIEW, 16));
    types.push(reference(5, INDEX, 4));
    request.types = types.into_boxed_slice();
    let inputs = if constructor {
        vec![LANE_REF, INDEX, INDEX, INDEX, INDEX]
    } else {
        vec![VIEW_REF, LANE_REF, INDEX, INDEX]
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(40)),
        SemanticLayoutIdentityV1(identity(41)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs.iter().copied().map(abi_value).collect(),
        abi_value(if constructor { RESULT } else { FRAGMENT }),
    )
    .unwrap();
    let output = abi.source_output_type();
    let source = SemanticSourceProvenanceV1::unavailable();
    let function = &mut request.functions[0];
    function.abi = abi.clone();
    function.abi.identity = SemanticAbiIdentityV1(identity(3));
    function.abi.layout_identity = SemanticLayoutIdentityV1(identity(4));
    function.locals = std::iter::once(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(50)),
        output,
        SemanticLocalRoleV1::Return,
        source,
    ))
    .chain(inputs.iter().enumerate().map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(51 + index as u8)),
            *ty,
            SemanticLocalRoleV1::Argument(index as u32),
            source,
        )
    }))
    .collect();
    // Retain the shared fixture's remaining types in the root's exact closure.
    let mut locals = function.locals.to_vec();
    let retained = if constructor {
        &[FRAGMENT, VIEW_REF][..]
    } else {
        &[RESULT][..]
    };
    locals.extend(retained.iter().enumerate().map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(70 + index as u8)),
            *ty,
            SemanticLocalRoleV1::Temporary,
            source,
        )
    }));
    function.locals = locals.into_boxed_slice();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        inputs
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(index as u32 + 1),
                        vec![],
                        *ty,
                    )
                    .unwrap(),
                )
            })
            .collect(),
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], output).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    function.blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(60)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(61)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    ]
    .into_boxed_slice();
    request.callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1(identity(42)),
                SemanticItemDefinitionIdentityV1(identity(43)),
                SemanticMonomorphizationIdentityV1(identity(44)),
                SemanticGenericTypeArgumentsIdentityV1(identity(45)),
                SemanticConstGenericArgumentsIdentityV1(identity(46)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation: operation(constructor),
            operation_identity: SemanticCompilerIntrinsicIdentityV1(OPERATION_IDENTITY),
        },
    ]
    .into_boxed_slice();
    request
}

fn admitted(constructor: bool) -> AdmittedInertSemanticMirV1 {
    request(constructor)
        .admit_exact_v15(SemanticMirLimitsV1::default())
        .unwrap()
}

fn payload(constructor: bool) -> &'static [u8] {
    if constructor {
        &[68, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]
    } else {
        &[
            36, 2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 64, 0, 0, 0, 2,
        ]
    }
}

fn operation_offset(bytes: &[u8], constructor: bool) -> usize {
    let mut suffix = payload(constructor).to_vec();
    suffix.extend_from_slice(&OPERATION_IDENTITY);
    suffix.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    let offset = bytes.len() - suffix.len();
    assert_eq!(offset, if constructor { 2885 } else { 2758 });
    assert_eq!(&bytes[offset..], suffix);
    offset
}

#[test]
fn full_v15_documents_are_frozen() {
    for (constructor, length, expected_hash) in
        [(true, 2938, CONSTRUCTOR_SHA256), (false, 2819, LOAD_SHA256)]
    {
        let admitted = admitted(constructor);
        let bytes = admitted.canonical_encoding();
        operation_offset(bytes, constructor);
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        assert_eq!(bytes.len(), length);
        assert_eq!(digest, expected_hash);
        assert_eq!(admitted.semantic_sha256().as_bytes(), &digest);
        assert_eq!(
            request(constructor)
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap()
                .canonical_encoding(),
            bytes
        );
        for decoded in [
            AdmittedInertSemanticMirV1::decode_exact_v15_canonical(
                bytes,
                SemanticMirLimitsV1::default(),
            )
            .unwrap(),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                bytes,
                SemanticMirLimitsV1::default(),
            )
            .unwrap(),
        ] {
            assert_eq!(decoded.canonical_encoding(), bytes);
            assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
            assert!(matches!(
                &decoded.callables()[1],
                SemanticCallableDeclV1::CompilerIntrinsic { operation: actual, operation_identity, .. }
                    if *actual == operation(constructor) && operation_identity.as_bytes() == &OPERATION_IDENTITY
            ));
        }
    }
}

#[test]
fn full_v15_documents_reject_truncation_trailing_bytes_and_v14_relabeling() {
    let limits = SemanticMirLimitsV1::default();
    for constructor in [true, false] {
        assert!(request(constructor).admit_exact_v14(limits).is_err());
        let admitted = admitted(constructor);
        let bytes = admitted.canonical_encoding();
        for end in 0..bytes.len() {
            assert!(
                AdmittedInertSemanticMirV1::decode_exact_v15_canonical(&bytes[..end], limits)
                    .is_err(),
                "accepted prefix {end}"
            );
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v15_canonical(&trailing, limits),
            Err(SemanticMirDecodeErrorV1::TrailingBytes { trailing: 1, .. })
        ));
        let mut downgraded = bytes.to_vec();
        let version_offset = b"fe2o3.inert-semantic-mir".len();
        assert_eq!(
            &downgraded[version_offset..version_offset + 2],
            &15u16.to_le_bytes()
        );
        downgraded[version_offset..version_offset + 2].copy_from_slice(&14u16.to_le_bytes());
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v14_canonical(&downgraded, limits),
            Err(SemanticMirDecodeErrorV1::InvalidTag { .. })
        ));
        assert!(
            AdmittedInertSemanticMirV1::decode_current_production_canonical(&downgraded, limits)
                .is_err()
        );
    }
}

#[test]
fn full_v15_documents_obey_the_exact_canonical_byte_budget() {
    for constructor in [true, false] {
        let admitted = admitted(constructor);
        let bytes = admitted.canonical_encoding();
        let actual = bytes.len() as u64;
        let exact = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::CanonicalBytes, actual)
            .unwrap();
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_exact_v15_canonical(bytes, exact)
                .unwrap()
                .canonical_encoding(),
            bytes,
        );
        assert_eq!(
            request(constructor)
                .admit_exact_v15(exact)
                .unwrap()
                .canonical_encoding(),
            bytes
        );
        let short = exact
            .with_limit(SemanticMirResourceV1::CanonicalBytes, actual - 1)
            .unwrap();
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v15_canonical(bytes, short),
            Err(SemanticMirDecodeErrorV1::InputLimitExceeded { actual: observed, max })
                if observed == actual && max == actual - 1
        ));
        assert!(request(constructor).admit_exact_v15(short).is_err());
    }
}

#[test]
fn rust_call_schema_inherits_complete_public_v15_bf16_documents() {
    let limits = SemanticMirLimitsV1::default();
    for constructor in [true, false] {
        let mut expected = admitted(constructor).canonical_encoding().to_vec();
        expected[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&28u16.to_le_bytes());
        let inherited = request(constructor).admit_exact_v28(limits).unwrap();
        assert_eq!(inherited.canonical_encoding(), expected);
        for decoded in [
            AdmittedInertSemanticMirV1::decode_exact_v28_canonical(&expected, limits).unwrap(),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(&expected, limits)
                .unwrap(),
        ] {
            assert_eq!(decoded.canonical_encoding(), expected);
            assert_eq!(decoded.callables(), inherited.callables());
        }
    }
}

#[test]
fn full_v15_documents_reject_future_intrinsics_and_invalid_column_major_contracts() {
    let limits = SemanticMirLimitsV1::default();
    let constructor = admitted(true);
    let offset = operation_offset(constructor.canonical_encoding(), true);
    for tag in 69..=255 {
        let mut bytes = constructor.canonical_encoding().to_vec();
        bytes[offset] = tag;
        for result in [
            AdmittedInertSemanticMirV1::decode_exact_v15_canonical(&bytes, limits),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(&bytes, limits),
        ] {
            assert!(
                matches!(result,
                    Err(SemanticMirDecodeErrorV1::InvalidTag { context: "compiler intrinsic", value, offset: actual })
                        if value == tag && actual == offset
                ),
                "accepted future tag {tag}"
            );
        }
    }
    let load = admitted(false);
    let offset = operation_offset(load.canonical_encoding(), false);
    for (field, value) in [
        (13, 0),
        (14, 1),
        (14, 2),
        (15, 1),
        (16, 32),
        (20, 1),
        (20, 3),
    ] {
        let mut bytes = load.canonical_encoding().to_vec();
        bytes[offset + field] = value;
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v15_canonical(&bytes, limits).is_err(),
            "accepted field {field}={value}"
        );
    }
    for profile in [1, 2] {
        let mut bytes = load.canonical_encoding().to_vec();
        bytes[offset + 14] = profile;
        bytes[offset + 15] = 1;
        assert!(
            matches!(
                AdmittedInertSemanticMirV1::decode_exact_v15_canonical(&bytes, limits),
                Err(SemanticMirDecodeErrorV1::Validation(_))
            ),
            "accepted non-BF16 column-major profile {profile}"
        );
    }
}
