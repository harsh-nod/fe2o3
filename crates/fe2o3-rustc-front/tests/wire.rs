use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_rustc_front::*;

fn function_id(byte: u8) -> FunctionIdentityV1 {
    FunctionIdentityV1::new([byte; 32]).expect("nonzero function identity")
}

fn type_id(byte: u8) -> StableTypeIdentityV1 {
    StableTypeIdentityV1::new([byte; 32]).expect("nonzero type identity")
}

fn file_id(byte: u8) -> SourceFileIdentityV1 {
    SourceFileIdentityV1::new([byte; 32]).expect("nonzero file identity")
}

fn location(line: u32) -> SourceLocationV1 {
    SourceLocationV1::new(file_id(0x44), line, 5).expect("valid location")
}

fn function(identity: u8, role: FunctionRoleV1) -> MonomorphizedFunctionV1 {
    let (name, parameters, return_type, blocks) = match role {
        FunctionRoleV1::Kernel => (
            "crate::map::<f32>",
            vec![type_id(0x11), type_id(0x11)],
            type_id(0x13),
            vec![
                BasicBlockV1::new(BlockIdV1::new(1), location(12), vec![]).unwrap(),
                BasicBlockV1::new(BlockIdV1::new(0), location(10), vec![BlockIdV1::new(1)])
                    .unwrap(),
            ],
        ),
        FunctionRoleV1::Helper => (
            "crate::twice::<f32>",
            vec![type_id(0x11)],
            type_id(0x11),
            vec![BasicBlockV1::new(BlockIdV1::new(0), location(20), vec![]).unwrap()],
        ),
    };
    MonomorphizedFunctionV1::new(
        function_id(identity),
        role,
        name,
        location(7),
        TypedSignatureV1::new(parameters, return_type).unwrap(),
        BlockIdV1::new(0),
        blocks,
    )
    .expect("valid function")
}

fn fixture() -> FrontendUnitV1 {
    FrontendUnitV1::new(vec![
        function(0x30, FunctionRoleV1::Kernel),
        function(0x20, FunctionRoleV1::Helper),
    ])
    .expect("valid frontend unit")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).expect("valid hex"))
        .collect()
}

fn decode_error(bytes: &[u8]) -> DecodeError {
    decode_frontend_unit_v1(bytes).expect_err("mutated bytes must be rejected")
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn first_function_offsets(bytes: &[u8]) -> (usize, usize, usize) {
    const FUNCTION_START: usize = 24;
    const NAME_LENGTH_OFFSET: usize = FUNCTION_START + 36;
    let source = NAME_LENGTH_OFFSET + 2 + usize::from(u16_at(bytes, NAME_LENGTH_OFFSET));
    let parameter_count = usize::from(u16_at(bytes, source + 40));
    let return_type = source + 44;
    let blocks = return_type + 32 + parameter_count * 32 + 8;
    (source, return_type, blocks)
}

fn kernel_with_blocks(blocks: Vec<BasicBlockV1>) -> FrontendUnitV1 {
    FrontendUnitV1::new(vec![
        MonomorphizedFunctionV1::new(
            function_id(0x30),
            FunctionRoleV1::Kernel,
            "crate::cfg",
            location(1),
            TypedSignatureV1::new(vec![], type_id(1)).unwrap(),
            BlockIdV1::new(0),
            blocks,
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn round_trip_is_byte_identical_and_exposes_typed_data() {
    let original = fixture();
    let encoded = encode_frontend_unit_v1(&original).expect("encode");
    let decoded = decode_frontend_unit_v1(&encoded).expect("decode");
    assert_eq!(decoded, original);
    assert_eq!(encode_frontend_unit_v1(&decoded).unwrap(), encoded);

    let functions = decoded.functions();
    assert_eq!(functions.len(), 2);
    assert_eq!(functions[0].role(), FunctionRoleV1::Helper);
    assert_eq!(functions[1].role(), FunctionRoleV1::Kernel);
    assert_eq!(functions[1].signature().parameters().len(), 2);
    assert_eq!(functions[1].blocks()[0].successors(), &[BlockIdV1::new(1)]);
    assert_eq!(functions[1].blocks()[1].location().line(), 12);
}

#[test]
fn golden_v1_wire_is_stable() {
    const GOLDEN_HEX: &str = "4645324f3352460001000000240200000200000000000000202020202020202020202020202020202020202020202020202020202020202002000000130063726174653a3a74776963653a3a3c6633323e4444444444444444444444444444444444444444444444444444444444444444070000000500000001000000111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111110000000001000000000000004444444444444444444444444444444444444444444444444444444444444444140000000500000000000000303030303030303030303030303030303030303030303030303030303030303001000000110063726174653a3a6d61703a3a3c6633323e444444444444444444444444444444444444444444444444444444444444444407000000050000000200000013131313131313131313131313131313131313131313131313131313131313131111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111100000000020000000000000044444444444444444444444444444444444444444444444444444444444444440a0000000500000001000000010000000100000044444444444444444444444444444444444444444444444444444444444444440c0000000500000000000000";
    let encoded = encode_frontend_unit_v1(&fixture()).expect("encode");
    assert_eq!(hex(&encoded), GOLDEN_HEX);
    assert_eq!(
        decode_frontend_unit_v1(&from_hex(GOLDEN_HEX)),
        Ok(fixture())
    );
}

#[test]
fn constructors_canonicalize_set_order() {
    let canonical = fixture();
    let reordered = FrontendUnitV1::new(vec![
        function(0x20, FunctionRoleV1::Helper),
        function(0x30, FunctionRoleV1::Kernel),
    ])
    .unwrap();
    assert_eq!(canonical, reordered);
    assert_eq!(
        encode_frontend_unit_v1(&canonical).unwrap(),
        encode_frontend_unit_v1(&reordered).unwrap()
    );

    let block = BasicBlockV1::new(
        BlockIdV1::new(0),
        location(1),
        vec![BlockIdV1::new(2), BlockIdV1::new(1)],
    )
    .unwrap();
    assert_eq!(block.successors(), &[BlockIdV1::new(1), BlockIdV1::new(2)]);
}

#[test]
fn typed_constructor_invariants_fail_closed() {
    assert!(matches!(
        FunctionIdentityV1::new([0; 32]),
        Err(ValidationError::ZeroIdentity { .. })
    ));
    assert!(matches!(
        SourceLocationV1::new(file_id(1), 0, 1),
        Err(ValidationError::InvalidSourceLocation)
    ));
    assert!(matches!(
        BasicBlockV1::new(
            BlockIdV1::new(0),
            location(1),
            vec![BlockIdV1::new(1), BlockIdV1::new(1)]
        ),
        Err(ValidationError::Duplicate { .. })
    ));
    assert!(matches!(
        TypedSignatureV1::new(
            vec![type_id(1); MAX_PARAMETERS_PER_FUNCTION_V1 + 1],
            type_id(2)
        ),
        Err(ValidationError::TooMany { .. })
    ));

    let signature = || TypedSignatureV1::new(vec![], type_id(1)).unwrap();
    assert!(matches!(
        MonomorphizedFunctionV1::new(
            function_id(1),
            FunctionRoleV1::Kernel,
            "",
            location(1),
            signature(),
            BlockIdV1::new(0),
            vec![BasicBlockV1::new(BlockIdV1::new(0), location(1), vec![]).unwrap()]
        ),
        Err(ValidationError::Empty { .. })
    ));
    assert!(matches!(
        MonomorphizedFunctionV1::new(
            function_id(1),
            FunctionRoleV1::Kernel,
            "kernel",
            location(1),
            signature(),
            BlockIdV1::new(0),
            vec![BasicBlockV1::new(BlockIdV1::new(1), location(1), vec![]).unwrap()]
        ),
        Err(ValidationError::NonDenseBlockId { .. })
    ));
    assert!(matches!(
        MonomorphizedFunctionV1::new(
            function_id(1),
            FunctionRoleV1::Kernel,
            "kernel",
            location(1),
            signature(),
            BlockIdV1::new(1),
            vec![BasicBlockV1::new(BlockIdV1::new(0), location(1), vec![]).unwrap()]
        ),
        Err(ValidationError::InvalidEntryBlock { .. })
    ));
    assert!(matches!(
        MonomorphizedFunctionV1::new(
            function_id(1),
            FunctionRoleV1::Kernel,
            "kernel",
            location(1),
            signature(),
            BlockIdV1::new(0),
            vec![
                BasicBlockV1::new(BlockIdV1::new(0), location(1), vec![BlockIdV1::new(1)]).unwrap()
            ]
        ),
        Err(ValidationError::InvalidSuccessor { .. })
    ));
    assert_eq!(
        FrontendUnitV1::new(vec![function(1, FunctionRoleV1::Helper)]),
        Err(ValidationError::MissingKernel)
    );
    assert!(matches!(
        FrontendUnitV1::new(vec![
            function(1, FunctionRoleV1::Kernel),
            function(1, FunctionRoleV1::Helper)
        ]),
        Err(ValidationError::Duplicate { .. })
    ));
}

#[test]
fn header_and_bounded_fields_reject_malformed_input() {
    let encoded = encode_frontend_unit_v1(&fixture()).unwrap();

    let mut invalid = encoded.clone();
    invalid[0] ^= 1;
    assert_eq!(decode_error(&invalid), DecodeError::InvalidMagic);

    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(decode_error(&invalid), DecodeError::UnknownVersion(2));

    let mut invalid = encoded.clone();
    invalid[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(decode_error(&invalid), DecodeError::UnsupportedFlags(1));

    let mut invalid = encoded.clone();
    invalid[12..16].copy_from_slice(&23_u32.to_le_bytes());
    assert_eq!(
        decode_error(&invalid),
        DecodeError::InvalidLength { declared: 23 }
    );

    let mut invalid = encoded.clone();
    invalid[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "frontend functions",
            ..
        }
    ));

    let mut invalid = encoded.clone();
    invalid[20] = 1;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::NonzeroReserved { .. }
    ));

    let mut oversized = vec![0_u8; MAX_UNIT_BYTES_V1 + 1];
    oversized[..8].copy_from_slice(&FRONTEND_UNIT_MAGIC_V1);
    assert_eq!(
        decode_error(&oversized),
        DecodeError::TooLarge {
            max: MAX_UNIT_BYTES_V1
        }
    );
}

#[test]
fn malformed_function_fields_are_rejected() {
    let encoded = encode_frontend_unit_v1(
        &FrontendUnitV1::new(vec![function(0x30, FunctionRoleV1::Kernel)]).unwrap(),
    )
    .unwrap();
    const FUNCTION_START: usize = 24;
    const ROLE_OFFSET: usize = FUNCTION_START + 32;
    const NAME_LENGTH_OFFSET: usize = FUNCTION_START + 36;
    const NAME_OFFSET: usize = NAME_LENGTH_OFFSET + 2;

    let mut invalid = encoded.clone();
    invalid[ROLE_OFFSET] = 9;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::UnknownTag {
            kind: "function role",
            tag: 9
        }
    ));

    let mut invalid = encoded.clone();
    invalid[ROLE_OFFSET + 1] = 1;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::NonzeroReserved { .. }
    ));

    let mut invalid = encoded.clone();
    invalid[FUNCTION_START..FUNCTION_START + 32].fill(0);
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::Validation(ValidationError::ZeroIdentity { .. })
    ));

    let mut invalid = encoded.clone();
    invalid[NAME_OFFSET] = 0xff;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::InvalidUtf8 { .. }
    ));

    let mut invalid = encoded.clone();
    invalid[NAME_LENGTH_OFFSET..NAME_LENGTH_OFFSET + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "function diagnostic name",
            ..
        }
    ));

    let (source, return_type, block_start) = first_function_offsets(&encoded);

    let mut invalid = encoded.clone();
    invalid[source..source + 32].fill(0);
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::Validation(ValidationError::ZeroIdentity {
            field: "source file identity"
        })
    ));

    let mut invalid = encoded.clone();
    invalid[source + 32..source + 36].fill(0);
    assert_eq!(
        decode_error(&invalid),
        DecodeError::Validation(ValidationError::InvalidSourceLocation)
    );

    let mut invalid = encoded.clone();
    invalid[return_type..return_type + 32].fill(0);
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::Validation(ValidationError::ZeroIdentity {
            field: "stable type identity"
        })
    ));

    let mut invalid = encoded;
    let first_successor = block_start + 48;
    invalid[first_successor..first_successor + 4].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        decode_error(&invalid),
        DecodeError::Validation(ValidationError::InvalidSuccessor {
            block: 0,
            successor: 99
        })
    );
}

#[test]
fn noncanonical_function_order_is_rejected() {
    let low = encode_frontend_unit_v1(
        &FrontendUnitV1::new(vec![function(0x20, FunctionRoleV1::Kernel)]).unwrap(),
    )
    .unwrap();
    let high = encode_frontend_unit_v1(
        &FrontendUnitV1::new(vec![function(0x30, FunctionRoleV1::Kernel)]).unwrap(),
    )
    .unwrap();
    let mut reversed = low[..24].to_vec();
    reversed[16..20].copy_from_slice(&2_u32.to_le_bytes());
    reversed.extend_from_slice(&high[24..]);
    reversed.extend_from_slice(&low[24..]);
    let length = u32::try_from(reversed.len()).unwrap();
    reversed[12..16].copy_from_slice(&length.to_le_bytes());
    assert_eq!(decode_error(&reversed), DecodeError::NonCanonical);
}

#[test]
fn noncanonical_cfg_order_and_duplicate_edges_are_rejected() {
    let two_blocks = kernel_with_blocks(vec![
        BasicBlockV1::new(BlockIdV1::new(0), location(1), vec![BlockIdV1::new(1)]).unwrap(),
        BasicBlockV1::new(BlockIdV1::new(1), location(2), vec![]).unwrap(),
    ]);
    let canonical = encode_frontend_unit_v1(&two_blocks).unwrap();
    let (_, _, block_start) = first_function_offsets(&canonical);
    let first_block_end = block_start + 52;
    let mut reversed = canonical[..block_start].to_vec();
    reversed.extend_from_slice(&canonical[first_block_end..]);
    reversed.extend_from_slice(&canonical[block_start..first_block_end]);
    assert_eq!(decode_error(&reversed), DecodeError::NonCanonical);

    let three_blocks = kernel_with_blocks(vec![
        BasicBlockV1::new(
            BlockIdV1::new(0),
            location(1),
            vec![BlockIdV1::new(2), BlockIdV1::new(1)],
        )
        .unwrap(),
        BasicBlockV1::new(BlockIdV1::new(1), location(2), vec![]).unwrap(),
        BasicBlockV1::new(BlockIdV1::new(2), location(3), vec![]).unwrap(),
    ]);
    let canonical = encode_frontend_unit_v1(&three_blocks).unwrap();
    let (_, _, block_start) = first_function_offsets(&canonical);
    let first_successor = block_start + 48;

    let mut reversed = canonical.clone();
    reversed[first_successor..first_successor + 4].copy_from_slice(&2_u32.to_le_bytes());
    reversed[first_successor + 4..first_successor + 8].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(decode_error(&reversed), DecodeError::NonCanonical);

    let mut duplicate = canonical;
    duplicate[first_successor + 4..first_successor + 8].copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        decode_error(&duplicate),
        DecodeError::Validation(ValidationError::Duplicate {
            field: "CFG block successors"
        })
    ));
}

#[test]
fn every_truncation_and_trailing_byte_is_rejected() {
    let encoded = encode_frontend_unit_v1(&fixture()).unwrap();
    for length in 0..encoded.len() {
        assert!(
            decode_frontend_unit_v1(&encoded[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(decode_error(&trailing), DecodeError::TrailingBytes);
}

#[test]
fn deterministic_malformed_corpus_never_panics() {
    let encoded = encode_frontend_unit_v1(&fixture()).unwrap();
    for index in 0..encoded.len() {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = encoded.clone();
            mutated[index] ^= mask;
            assert!(
                catch_unwind(AssertUnwindSafe(|| decode_frontend_unit_v1(&mutated))).is_ok(),
                "panic at byte {index}, mask {mask:#x}"
            );
        }
    }

    let mut state = 0x9e37_79b9_u32;
    for _ in 0..4096 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let length = (state as usize) % (encoded.len() + 65);
        let mut bytes = vec![0_u8; length];
        for byte in &mut bytes {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (state >> 24) as u8;
        }
        assert!(catch_unwind(|| decode_frontend_unit_v1(&bytes)).is_ok());
    }
}
