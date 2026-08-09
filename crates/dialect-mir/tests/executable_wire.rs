use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_mir::{
    MAX_EXECUTABLE_TYPE_DEPTH, MAX_EXECUTABLE_WIRE_BYTES, MirAddressSpace, MirBasicBlock,
    MirBlockId, MirBody, MirBodyForm, MirExecutableDecodeError, MirExecutableModule,
    MirExecutableVersion, MirFunction, MirLayout, MirLocalDecl, MirLocalId, MirLocalKind,
    MirMutability, MirOperand, MirPlace, MirRvalue, MirScalarType, MirSemanticType, MirStatement,
    MirStatementKind, MirTerminator, MirTerminatorKind, MirTypeId, MirTypeKind,
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
        types: vec![scalar_u32()],
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
                        name: Some("return".into()),
                        span: None,
                    },
                    MirLocalDecl {
                        ty: MirTypeId(0),
                        kind: MirLocalKind::Argument,
                        mutable: false,
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
    assert_eq!(MirExecutableModule::from_bytes(&bytes).unwrap(), module);
    assert_eq!(module.to_bytes().unwrap(), bytes);
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
