use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_mir::{
    MAX_EXECUTABLE_TYPE_DEPTH, MAX_EXECUTABLE_WIRE_BYTES, MirAddressSpace,
    MirAuthorizedDeviceImport, MirBasicBlock, MirBlockId, MirBody, MirBodyForm, MirCallAuthority,
    MirCallReturn, MirCallSignature, MirCallable, MirExecutableDecodeError, MirExecutableModule,
    MirExecutableTarget, MirExecutableVersion, MirExternalCallRegistry, MirExternalCallReturn,
    MirExternalCallSignature, MirFunction, MirLayout, MirLocalDecl, MirLocalId, MirLocalKind,
    MirMutability, MirOperand, MirPlace, MirRvalue, MirScalarType, MirSemanticType, MirStatement,
    MirStatementKind, MirTerminator, MirTerminatorKind, MirTypeId, MirTypeKind,
    ValidatedMirExecutableModule,
};

fn scalar_u32() -> MirSemanticType {
    MirSemanticType {
        layout: MirLayout::sized(4, 4),
        kind: MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
    }
}

fn module() -> MirExecutableModule {
    MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget::gfx942(),
        types: vec![scalar_u32()],
        callables: vec![],
        functions: vec![MirFunction {
            identity: "wire::identity::<u32>".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Places,
                locals: vec![
                    MirLocalDecl {
                        ty: MirTypeId(0),
                        kind: MirLocalKind::Return,
                        mutable: true,
                        storage_address_space: MirAddressSpace::DEFAULT,
                        name: Some("return".into()),
                        span: None,
                    },
                    MirLocalDecl {
                        ty: MirTypeId(0),
                        kind: MirLocalKind::Argument,
                        mutable: false,
                        storage_address_space: MirAddressSpace::DEFAULT,
                        name: Some("input".into()),
                        span: None,
                    },
                ],
                blocks: vec![MirBasicBlock {
                    parameters: vec![],
                    statements: vec![MirStatement {
                        kind: MirStatementKind::Assign {
                            place: MirPlace::local(MirLocalId(0), MirTypeId(0)),
                            value: MirRvalue::Use(MirOperand::Copy(MirPlace::local(
                                MirLocalId(1),
                                MirTypeId(0),
                            ))),
                        },
                        span: None,
                    }],
                    terminator: MirTerminator {
                        kind: MirTerminatorKind::Return,
                        span: None,
                    },
                }],
                entry: MirBlockId(0),
            },
        }],
    }
}

fn set_payload_len(bytes: &mut [u8]) {
    let payload_len = (bytes.len() - 16) as u32;
    bytes[12..16].copy_from_slice(&payload_len.to_le_bytes());
}

fn raw_wire(module: &MirExecutableModule) -> Vec<u8> {
    let payload = serde_json::to_vec(module).unwrap();
    let mut bytes = Vec::with_capacity(16 + payload.len());
    bytes.extend_from_slice(b"F2MEXE01");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

#[test]
fn canonical_wire_roundtrips_with_a_versioned_envelope() {
    let module = module();
    let bytes = module.to_bytes().unwrap();
    assert_eq!(&bytes[..8], b"F2MEXE01");
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1);
    assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 0);
    assert_eq!(
        u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize,
        bytes.len() - 16
    );
    let decoded: ValidatedMirExecutableModule = MirExecutableModule::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, module);
    assert_eq!(module.to_bytes().unwrap(), bytes);
}

#[test]
fn canonical_text_parse_print_and_wire_roundtrip_are_identical() {
    let validated = module().validate().unwrap();
    let first = validated.to_canonical_text().unwrap();
    let second = validated.to_canonical_text().unwrap();
    assert_eq!(first, second);
    assert!(!first.contains('\n'));
    assert_eq!(
        MirExecutableModule::from_canonical_text(&first).unwrap(),
        validated
    );

    let bytes = validated.to_bytes().unwrap();
    let from_wire = MirExecutableModule::from_bytes(&bytes).unwrap();
    assert_eq!(from_wire.to_canonical_text().unwrap(), first);

    let with_whitespace = format!(" {first}");
    assert!(matches!(
        MirExecutableModule::from_canonical_text(&with_whitespace),
        Err(MirExecutableDecodeError::NonCanonical)
    ));
}

#[test]
fn canonical_wire_binds_the_complete_gfx942_target_profile() {
    let canonical = module().to_bytes().unwrap();
    assert!(canonical.len() < MAX_EXECUTABLE_WIRE_BYTES);
    let payload = std::str::from_utf8(&canonical[16..]).unwrap();
    assert!(payload.contains("amdgcn-amd-amdhsa"));
    assert!(payload.contains("gfx942"));
    assert!(payload.contains("-wavefrontsize32,+wavefrontsize64"));
    assert!(payload.contains("p3:32:32"));

    let mut forged_layout = module();
    forged_layout.target.data_layout = "e-p:64:64-p3:64:64".into();
    assert!(matches!(
        MirExecutableModule::from_bytes(&raw_wire(&forged_layout)),
        Err(MirExecutableDecodeError::Validation(_))
    ));

    let mut forged_pointer_map = module();
    forged_pointer_map.target.pointer_abis[3].width_bits = 64;
    forged_pointer_map.target.pointer_abis[3].abi_alignment_bits = 64;
    assert!(matches!(
        MirExecutableModule::from_bytes(&raw_wire(&forged_pointer_map)),
        Err(MirExecutableDecodeError::Validation(_))
    ));

    let mut duplicate_pointer_space = module();
    duplicate_pointer_space.target.pointer_abis[4].address_space = MirAddressSpace(3);
    assert!(matches!(
        MirExecutableModule::from_bytes(&raw_wire(&duplicate_pointer_space)),
        Err(MirExecutableDecodeError::Validation(_))
    ));

    let mut oversized_target = module();
    oversized_target.target.features = "x".repeat(MAX_EXECUTABLE_WIRE_BYTES);
    assert!(matches!(
        MirExecutableModule::from_bytes(&raw_wire(&oversized_target)),
        Err(MirExecutableDecodeError::InputTooLarge)
    ));
}

#[test]
fn serde_data_remains_unvalidated_and_validation_is_owning() {
    let mut untrusted = module();
    let json = serde_json::to_vec(&untrusted).unwrap();
    let decoded: MirExecutableModule = serde_json::from_slice(&json).unwrap();
    let validated = decoded.validate().unwrap();

    untrusted.functions[0].body.blocks[0].statements.clear();
    assert!(untrusted.validate().is_err());
    assert!(validated.to_bytes().is_ok());
    assert_eq!(validated.as_module(), &module());

    let recovered_data = validated.into_unvalidated();
    assert_eq!(recovered_data, module());
}

#[test]
fn wire_device_imports_resolve_only_against_external_authority() {
    let mut module = module();
    let semantic_u32 = module.types[0].clone();
    module.callables.push(MirCallable {
        identity: "wire::trusted".into(),
        authority: MirCallAuthority::DeviceImport {
            contract: "wire::trusted::v1".into(),
        },
        signature: MirCallSignature {
            inputs: vec![MirTypeId(0)],
            output: MirCallReturn::Value(MirTypeId(0)),
            can_unwind: false,
        },
    });
    let registry = MirExternalCallRegistry::try_new(vec![MirAuthorizedDeviceImport {
        identity: "wire::trusted".into(),
        contract: "wire::trusted::v1".into(),
        signature: MirExternalCallSignature {
            inputs: vec![semantic_u32.clone()],
            output: MirExternalCallReturn::Value(semantic_u32),
            can_unwind: false,
        },
    }])
    .unwrap();

    let bytes = module.to_bytes_with_registry(&registry).unwrap();
    assert!(matches!(
        MirExecutableModule::from_bytes(&bytes),
        Err(MirExecutableDecodeError::Validation(_))
    ));
    assert_eq!(
        MirExecutableModule::from_bytes_with_registry(&bytes, &registry).unwrap(),
        module
    );
}

#[test]
fn rejects_header_length_and_payload_mutations() {
    let canonical = module().to_bytes().unwrap();

    let mut bad_magic = canonical.clone();
    bad_magic[0] ^= 1;
    assert!(matches!(
        MirExecutableModule::from_bytes(&bad_magic),
        Err(MirExecutableDecodeError::InvalidMagic)
    ));

    let mut bad_version = canonical.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        MirExecutableModule::from_bytes(&bad_version),
        Err(MirExecutableDecodeError::UnknownVersion(2))
    ));

    let mut bad_flags = canonical.clone();
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        MirExecutableModule::from_bytes(&bad_flags),
        Err(MirExecutableDecodeError::UnsupportedFlags(1))
    ));

    let mut bad_length = canonical.clone();
    bad_length[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        MirExecutableModule::from_bytes(&bad_length),
        Err(MirExecutableDecodeError::LengthMismatch)
    ));

    let mut invalid_json = canonical;
    invalid_json[16] = b'!';
    assert!(matches!(
        MirExecutableModule::from_bytes(&invalid_json),
        Err(MirExecutableDecodeError::InvalidPayload(_))
    ));
}

#[test]
fn rejects_valid_but_noncanonical_json() {
    let canonical = module().to_bytes().unwrap();

    let mut whitespace = canonical.clone();
    whitespace.insert(16, b' ');
    set_payload_len(&mut whitespace);
    assert!(matches!(
        MirExecutableModule::from_bytes(&whitespace),
        Err(MirExecutableDecodeError::NonCanonical)
    ));

    let mut unknown_field = canonical;
    let end = unknown_field.len() - 1;
    unknown_field.splice(end..end, b",\"ignored\":true".iter().copied());
    set_payload_len(&mut unknown_field);
    assert!(matches!(
        MirExecutableModule::from_bytes(&unknown_field),
        Err(MirExecutableDecodeError::NonCanonical)
    ));
}

#[test]
fn decoded_payload_is_structurally_verified() {
    let mut bytes = module().to_bytes().unwrap();
    let needle = b"\"ty\":0";
    let offset = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("fixture contains a type reference");
    bytes[offset + needle.len() - 1] = b'9';

    let error = MirExecutableModule::from_bytes(&bytes).unwrap_err();
    assert!(matches!(error, MirExecutableDecodeError::Validation(_)));
}

#[test]
fn semantic_type_depth_is_bounded_before_recursive_validation() {
    let mut nested = scalar_u32();
    for _ in 0..MAX_EXECUTABLE_TYPE_DEPTH {
        nested = MirSemanticType {
            layout: MirLayout::sized(8, 8),
            kind: MirTypeKind::RawPointer {
                pointee: Box::new(nested),
                mutability: MirMutability::Immutable,
                address_space: MirAddressSpace::DEFAULT,
            },
        };
    }
    let mut module = module();
    module.types = vec![nested];
    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("exceeds depth"));
}

#[test]
fn decoder_is_total_for_truncations_and_single_byte_mutations() {
    let encoded = module().to_bytes().unwrap();
    for end in 0..encoded.len() {
        let result = catch_unwind(|| MirExecutableModule::from_bytes(&encoded[..end]));
        assert!(result.is_ok(), "decoder panicked at truncation {end}");
        assert!(
            result.unwrap().is_err(),
            "decoder accepted truncation {end}"
        );
    }

    for index in 0..encoded.len() {
        let mut mutated = encoded.clone();
        mutated[index] ^= 0x5a;
        let result = catch_unwind(AssertUnwindSafe(|| {
            MirExecutableModule::from_bytes(&mutated)
        }));
        assert!(result.is_ok(), "decoder panicked at mutation {index}");
        if let Ok(decoded) = result.unwrap() {
            assert_eq!(decoded.to_bytes().unwrap(), mutated);
        }
    }
}

#[test]
fn decoder_bounds_input_before_payload_parsing() {
    let bytes = vec![0; MAX_EXECUTABLE_WIRE_BYTES + 1];
    assert!(matches!(
        MirExecutableModule::from_bytes(&bytes),
        Err(MirExecutableDecodeError::InputTooLarge)
    ));
}
