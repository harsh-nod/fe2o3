use fe2o3_build_authority::{
    AuthorityProfileV1, CompilerClosureDigestFieldV1, CompilerClosureErrorV1, CompilerClosureV1,
    POLICY_IDENTITY_DOMAIN_V1, POLICY_V1_ENCODED_LEN, POLICY_V1_FIELD_COUNT, POLICY_V1_HEADER_LEN,
    POLICY_V1_MAGIC, POLICY_V1_TARGET, POLICY_V1_VERSION, PipelineAllowlistV1, PipelineV1,
    PolicyDigestFieldV1, PolicyErrorV1, PolicyV1, PublicationRightsV1, decode_policy_v1,
    policy_identity_sha256_v1,
};

const FIELD_OFFSETS: [usize; 14] = [
    32, 48, 88, 128, 137, 177, 217, 257, 297, 337, 358, 370, 380, 420,
];
const VALUE_OFFSETS: [usize; 14] = [
    40, 56, 96, 136, 145, 185, 225, 265, 305, 345, 366, 378, 388, 428,
];
const FIELD_TAGS: [u16; 14] = [
    0x0001, 0x0002, 0x0003, 0x0004, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0020, 0x0021, 0x0022,
    0x0023, 0x0024,
];
const FIELD_LENGTHS: [u32; 14] = [8, 32, 32, 1, 32, 32, 32, 32, 32, 13, 4, 2, 32, 4];
const GOLDEN_COMPILER_CLOSURE: [u8; 32] = [
    0x1f, 0xea, 0xcf, 0xc5, 0x87, 0x9b, 0x85, 0x3c, 0x7b, 0xa5, 0x5c, 0x34, 0x53, 0x93, 0x98, 0xe8,
    0x57, 0xc0, 0xf9, 0x7d, 0x68, 0x6c, 0xbb, 0x63, 0xcf, 0x99, 0x79, 0x5a, 0x6a, 0xa0, 0x9e, 0xc9,
];
const GOLDEN_POLICY_IDENTITY: [u8; 32] = [
    0x57, 0x17, 0x71, 0x1d, 0x8f, 0x0b, 0xe6, 0x38, 0xe3, 0x83, 0x03, 0x4b, 0x0d, 0x2b, 0x70, 0x51,
    0xbb, 0xb3, 0x4a, 0x1b, 0x72, 0xa2, 0x87, 0x06, 0xae, 0x17, 0x62, 0xfa, 0x0a, 0x78, 0xb6, 0x89,
];

fn golden_compiler() -> CompilerClosureV1 {
    CompilerClosureV1::new([0x05; 32], [0x06; 32], [0x07; 32], [0x08; 32]).unwrap()
}

fn golden_policy() -> PolicyV1 {
    PolicyV1::new(
        0x0102_0304_0506_0708,
        [0x01; 32],
        [0x02; 32],
        golden_compiler(),
        PipelineAllowlistV1::ALL,
        PipelineV1::CollectedTiledGemm,
        [0x09; 32],
    )
    .unwrap()
}

fn golden_bytes() -> [u8; POLICY_V1_ENCODED_LEN] {
    golden_policy().encode()
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn patterned_digest(seed: u8) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = seed
            .wrapping_mul(17)
            .wrapping_add(index as u8)
            .wrapping_add(1);
    }
    digest
}

#[test]
fn exact_wire_layout_and_cross_implementation_golden_identity_are_stable() {
    let policy = golden_policy();
    let bytes = policy.encode();

    assert_eq!(&bytes[0..8], &POLICY_V1_MAGIC);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        POLICY_V1_VERSION
    );
    assert_eq!(
        u16::from_le_bytes(bytes[10..12].try_into().unwrap()),
        POLICY_V1_HEADER_LEN
    );
    assert_eq!(
        u16::from_le_bytes(bytes[12..14].try_into().unwrap()),
        POLICY_V1_FIELD_COUNT
    );
    assert_eq!(&bytes[14..16], &[0; 2]);
    assert_eq!(
        u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
        POLICY_V1_ENCODED_LEN as u32
    );
    assert_eq!(&bytes[20..32], &[0; 12]);

    for index in 0..FIELD_OFFSETS.len() {
        let offset = FIELD_OFFSETS[index];
        assert_eq!(
            u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()),
            FIELD_TAGS[index],
            "tag {index}"
        );
        assert_eq!(&bytes[offset + 2..offset + 4], &[0; 2], "flags {index}");
        assert_eq!(
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
            FIELD_LENGTHS[index],
            "length {index}"
        );
    }

    assert_eq!(&bytes[40..48], &0x0102_0304_0506_0708_u64.to_le_bytes());
    assert_eq!(&bytes[56..88], &[0x01; 32]);
    assert_eq!(&bytes[96..128], &[0x02; 32]);
    assert_eq!(bytes[136], AuthorityProfileV1::StandaloneFoundation as u8);
    assert_eq!(&bytes[145..177], &[0x05; 32]);
    assert_eq!(&bytes[185..217], &[0x06; 32]);
    assert_eq!(&bytes[225..257], &[0x07; 32]);
    assert_eq!(&bytes[265..297], &[0x08; 32]);
    assert_eq!(&bytes[305..337], &GOLDEN_COMPILER_CLOSURE);
    assert_eq!(&bytes[345..358], POLICY_V1_TARGET.as_bytes());
    assert_eq!(&bytes[366..370], &3_u32.to_le_bytes());
    assert_eq!(&bytes[378..380], &2_u16.to_le_bytes());
    assert_eq!(&bytes[388..420], &[0x09; 32]);
    assert_eq!(&bytes[428..432], &0_u32.to_le_bytes());

    assert_eq!(golden_compiler().identity_sha256(), GOLDEN_COMPILER_CLOSURE);
    assert_eq!(policy.identity_sha256(), GOLDEN_POLICY_IDENTITY);
    assert_eq!(
        policy_identity_sha256_v1(&bytes),
        Ok(GOLDEN_POLICY_IDENTITY)
    );
    assert_eq!(
        POLICY_IDENTITY_DOMAIN_V1,
        b"FE2O3/PROTECTED-AUTHORITY-POLICY/V1\0"
    );
}

#[test]
fn every_supported_pipeline_and_allowlist_combination_roundtrips() {
    let cases = [
        (
            PipelineAllowlistV1::ROW_SOFTMAX,
            PipelineV1::CollectedRowSoftmax,
        ),
        (
            PipelineAllowlistV1::TILED_GEMM,
            PipelineV1::CollectedTiledGemm,
        ),
        (PipelineAllowlistV1::ALL, PipelineV1::CollectedRowSoftmax),
        (PipelineAllowlistV1::ALL, PipelineV1::CollectedTiledGemm),
    ];
    for (allowlist, selected) in cases {
        let policy = PolicyV1::new(
            11,
            [1; 32],
            [2; 32],
            golden_compiler(),
            allowlist,
            selected,
            [9; 32],
        )
        .unwrap();
        assert_eq!(decode_policy_v1(&policy.encode()), Ok(policy));
        assert!(
            policy
                .pipeline_allowlist()
                .allows(policy.selected_pipeline())
        );
        assert_eq!(policy.profile(), AuthorityProfileV1::StandaloneFoundation);
        assert_eq!(policy.publication_rights(), PublicationRightsV1::NONE);
        assert_eq!(policy.publication_rights().bits(), 0);
    }
}

#[test]
fn deterministic_policy_corpus_roundtrips_canonically() {
    for seed in 1_u8..=96 {
        let compiler = CompilerClosureV1::new(
            patterned_digest(seed),
            patterned_digest(seed.wrapping_add(1)),
            patterned_digest(seed.wrapping_add(2)),
            patterned_digest(seed.wrapping_add(3)),
        )
        .unwrap();
        let selected = if seed & 1 == 0 {
            PipelineV1::CollectedRowSoftmax
        } else {
            PipelineV1::CollectedTiledGemm
        };
        let policy = PolicyV1::new(
            u64::from(seed),
            patterned_digest(seed.wrapping_add(4)),
            patterned_digest(seed.wrapping_add(5)),
            compiler,
            PipelineAllowlistV1::ALL,
            selected,
            patterned_digest(seed.wrapping_add(6)),
        )
        .unwrap();
        let encoded = policy.encode();
        let decoded = decode_policy_v1(&encoded).unwrap();
        assert_eq!(decoded, policy);
        assert_eq!(decoded.encode(), encoded);
        assert_eq!(
            policy_identity_sha256_v1(&encoded),
            Ok(policy.identity_sha256())
        );
    }
}

#[test]
fn short_and_trailing_documents_fail_before_parsing() {
    let bytes = golden_bytes();
    for length in [0, 1, 31, 32, 431] {
        assert_eq!(
            decode_policy_v1(&bytes[..length]),
            Err(PolicyErrorV1::InvalidEncodedLength { actual: length })
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_policy_v1(&trailing),
        Err(PolicyErrorV1::InvalidEncodedLength { actual: 433 })
    );
    assert!(policy_identity_sha256_v1(&trailing).is_err());
}

#[test]
fn every_header_invariant_fails_closed() {
    let mut bytes = golden_bytes();
    bytes[0] ^= 1;
    assert_eq!(decode_policy_v1(&bytes), Err(PolicyErrorV1::InvalidMagic));

    let mut bytes = golden_bytes();
    set_u16(&mut bytes, 8, 2);
    assert_eq!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::UnsupportedVersion { actual: 2 })
    );

    let mut bytes = golden_bytes();
    set_u16(&mut bytes, 10, 31);
    assert_eq!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::InvalidHeaderLength { actual: 31 })
    );

    let mut bytes = golden_bytes();
    set_u16(&mut bytes, 12, 13);
    assert_eq!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::InvalidFieldCount { actual: 13 })
    );

    for offset in [14, 15, 24, 25, 26, 27, 28, 29, 30, 31] {
        let mut bytes = golden_bytes();
        bytes[offset] = 1;
        assert_eq!(
            decode_policy_v1(&bytes),
            Err(PolicyErrorV1::NonzeroHeaderReserved),
            "reserved byte {offset}"
        );
    }

    let mut bytes = golden_bytes();
    set_u32(&mut bytes, 16, 431);
    assert_eq!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::InvalidTotalLength { actual: 431 })
    );

    let mut bytes = golden_bytes();
    set_u32(&mut bytes, 20, 1);
    assert_eq!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::UnsupportedHeaderFlags { actual: 1 })
    );
}

#[test]
fn unknown_duplicate_reordered_and_missing_tags_fail_closed() {
    let mut bytes = golden_bytes();
    set_u16(&mut bytes, FIELD_OFFSETS[5], 0xffff);
    assert!(matches!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::UnexpectedFieldTag {
            index: 5,
            expected: 0x0011,
            actual: 0xffff
        })
    ));

    let mut bytes = golden_bytes();
    set_u16(&mut bytes, FIELD_OFFSETS[1], FIELD_TAGS[0]);
    assert!(matches!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::UnexpectedFieldTag { index: 1, .. })
    ));

    let mut bytes = golden_bytes();
    let first = bytes[FIELD_OFFSETS[1]..FIELD_OFFSETS[2]].to_vec();
    let second = bytes[FIELD_OFFSETS[2]..FIELD_OFFSETS[3]].to_vec();
    bytes[FIELD_OFFSETS[1]..FIELD_OFFSETS[2]].copy_from_slice(&second);
    bytes[FIELD_OFFSETS[2]..FIELD_OFFSETS[3]].copy_from_slice(&first);
    assert!(matches!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::UnexpectedFieldTag { index: 1, .. })
    ));

    let mut bytes = golden_bytes();
    set_u16(&mut bytes, FIELD_OFFSETS[0], 0);
    assert!(matches!(
        decode_policy_v1(&bytes),
        Err(PolicyErrorV1::UnexpectedFieldTag { index: 0, .. })
    ));
}

#[test]
fn every_tlv_rejects_nonzero_flags_and_wrong_lengths() {
    for index in 0..FIELD_OFFSETS.len() {
        let mut flags = golden_bytes();
        set_u16(&mut flags, FIELD_OFFSETS[index] + 2, 1);
        assert_eq!(
            decode_policy_v1(&flags),
            Err(PolicyErrorV1::UnsupportedFieldFlags {
                tag: FIELD_TAGS[index],
                actual: 1,
            }),
            "flags {index}"
        );

        let mut short = golden_bytes();
        set_u32(
            &mut short,
            FIELD_OFFSETS[index] + 4,
            FIELD_LENGTHS[index] - 1,
        );
        assert_eq!(
            decode_policy_v1(&short),
            Err(PolicyErrorV1::InvalidFieldLength {
                tag: FIELD_TAGS[index],
                expected: FIELD_LENGTHS[index],
                actual: FIELD_LENGTHS[index] - 1,
            }),
            "short length {index}"
        );

        let mut long = golden_bytes();
        set_u32(
            &mut long,
            FIELD_OFFSETS[index] + 4,
            FIELD_LENGTHS[index] + 1,
        );
        assert!(matches!(
            decode_policy_v1(&long),
            Err(PolicyErrorV1::InvalidFieldLength { .. })
        ));
    }
}

#[test]
fn zero_serial_and_top_level_digests_fail_closed() {
    let mut bytes = golden_bytes();
    bytes[VALUE_OFFSETS[0]..VALUE_OFFSETS[0] + 8].fill(0);
    assert_eq!(decode_policy_v1(&bytes), Err(PolicyErrorV1::ZeroSerial));

    for (field_index, field) in [
        (1, PolicyDigestFieldV1::LauncherExecutable),
        (2, PolicyDigestFieldV1::CargoFe2o3Executable),
        (12, PolicyDigestFieldV1::ChildArgv),
    ] {
        let mut bytes = golden_bytes();
        bytes[VALUE_OFFSETS[field_index]..VALUE_OFFSETS[field_index] + 32].fill(0);
        assert_eq!(
            decode_policy_v1(&bytes),
            Err(PolicyErrorV1::ZeroDigest { field })
        );
    }
}

#[test]
fn every_compiler_pin_is_nonzero_and_the_aggregate_is_recomputed() {
    let fields = [
        CompilerClosureDigestFieldV1::CargoExecutable,
        CompilerClosureDigestFieldV1::RustcExecutable,
        CompilerClosureDigestFieldV1::RustcRuntimeTree,
        CompilerClosureDigestFieldV1::CodegenBackend,
        CompilerClosureDigestFieldV1::CompilerClosure,
    ];
    for (field_index, field) in (4..=8).zip(fields) {
        let mut bytes = golden_bytes();
        bytes[VALUE_OFFSETS[field_index]..VALUE_OFFSETS[field_index] + 32].fill(0);
        assert_eq!(
            decode_policy_v1(&bytes),
            Err(PolicyErrorV1::InvalidCompilerClosure(
                CompilerClosureErrorV1::ZeroDigest { field }
            ))
        );
    }

    for field_index in 4..=8 {
        let mut bytes = golden_bytes();
        bytes[VALUE_OFFSETS[field_index]] ^= 1;
        assert_eq!(
            decode_policy_v1(&bytes),
            Err(PolicyErrorV1::InvalidCompilerClosure(
                CompilerClosureErrorV1::IdentityMismatch
            )),
            "compiler field {field_index}"
        );
    }
}

#[test]
fn profile_target_pipeline_and_rights_semantics_fail_closed() {
    let mut reserved_profile = golden_bytes();
    reserved_profile[VALUE_OFFSETS[3]] = 1;
    assert_eq!(
        decode_policy_v1(&reserved_profile),
        Err(PolicyErrorV1::ProfileNotPermitted { value: 1 })
    );

    let mut unknown_profile = golden_bytes();
    unknown_profile[VALUE_OFFSETS[3]] = 2;
    assert_eq!(
        decode_policy_v1(&unknown_profile),
        Err(PolicyErrorV1::UnknownProfile { value: 2 })
    );

    let mut target = golden_bytes();
    target[VALUE_OFFSETS[9]] ^= 1;
    assert_eq!(decode_policy_v1(&target), Err(PolicyErrorV1::InvalidTarget));

    let mut unknown_allowlist = golden_bytes();
    set_u32(&mut unknown_allowlist, VALUE_OFFSETS[10], 1 << 2);
    assert_eq!(
        decode_policy_v1(&unknown_allowlist),
        Err(PolicyErrorV1::UnknownPipelineAllowlistBits { bits: 1 << 2 })
    );

    for value in [0, 3, u16::MAX] {
        let mut unknown_pipeline = golden_bytes();
        set_u16(&mut unknown_pipeline, VALUE_OFFSETS[11], value);
        assert_eq!(
            decode_policy_v1(&unknown_pipeline),
            Err(PolicyErrorV1::UnknownPipeline { value })
        );
    }

    for allowlist in [0, 1] {
        let mut not_allowed = golden_bytes();
        set_u32(&mut not_allowed, VALUE_OFFSETS[10], allowlist);
        assert_eq!(
            decode_policy_v1(&not_allowed),
            Err(PolicyErrorV1::SelectedPipelineNotAllowed {
                selected: PipelineV1::CollectedTiledGemm,
                allowlist_bits: allowlist,
            })
        );
    }

    let mut known_right = golden_bytes();
    set_u32(&mut known_right, VALUE_OFFSETS[13], 1);
    assert_eq!(
        decode_policy_v1(&known_right),
        Err(PolicyErrorV1::PublicationRightsNotPermitted { bits: 1 })
    );

    for bits in [2, 3, u32::MAX] {
        let mut unknown_rights = golden_bytes();
        set_u32(&mut unknown_rights, VALUE_OFFSETS[13], bits);
        assert_eq!(
            decode_policy_v1(&unknown_rights),
            Err(PolicyErrorV1::UnknownPublicationRightsBits { bits })
        );
    }
}

#[test]
fn typed_constructors_reject_invalid_inputs() {
    for field in [
        CompilerClosureDigestFieldV1::CargoExecutable,
        CompilerClosureDigestFieldV1::RustcExecutable,
        CompilerClosureDigestFieldV1::RustcRuntimeTree,
        CompilerClosureDigestFieldV1::CodegenBackend,
    ] {
        let mut pins = [[5; 32], [6; 32], [7; 32], [8; 32]];
        let index = match field {
            CompilerClosureDigestFieldV1::CargoExecutable => 0,
            CompilerClosureDigestFieldV1::RustcExecutable => 1,
            CompilerClosureDigestFieldV1::RustcRuntimeTree => 2,
            CompilerClosureDigestFieldV1::CodegenBackend => 3,
            CompilerClosureDigestFieldV1::CompilerClosure => unreachable!(),
            _ => unreachable!(),
        };
        pins[index] = [0; 32];
        assert_eq!(
            CompilerClosureV1::new(pins[0], pins[1], pins[2], pins[3]),
            Err(CompilerClosureErrorV1::ZeroDigest { field })
        );
    }

    assert_eq!(
        CompilerClosureV1::from_pins_and_identity([5; 32], [6; 32], [7; 32], [8; 32], [9; 32]),
        Err(CompilerClosureErrorV1::IdentityMismatch)
    );
    assert_eq!(
        PipelineAllowlistV1::from_bits(4),
        Err(PolicyErrorV1::UnknownPipelineAllowlistBits { bits: 4 })
    );

    assert_eq!(
        PolicyV1::new(
            0,
            [1; 32],
            [2; 32],
            golden_compiler(),
            PipelineAllowlistV1::ALL,
            PipelineV1::CollectedRowSoftmax,
            [9; 32]
        ),
        Err(PolicyErrorV1::ZeroSerial)
    );
    for (launcher, cargo_fe2o3, argv, field) in [
        (
            [0; 32],
            [2; 32],
            [9; 32],
            PolicyDigestFieldV1::LauncherExecutable,
        ),
        (
            [1; 32],
            [0; 32],
            [9; 32],
            PolicyDigestFieldV1::CargoFe2o3Executable,
        ),
        ([1; 32], [2; 32], [0; 32], PolicyDigestFieldV1::ChildArgv),
    ] {
        assert_eq!(
            PolicyV1::new(
                1,
                launcher,
                cargo_fe2o3,
                golden_compiler(),
                PipelineAllowlistV1::ALL,
                PipelineV1::CollectedRowSoftmax,
                argv,
            ),
            Err(PolicyErrorV1::ZeroDigest { field })
        );
    }
    assert_eq!(
        PolicyV1::new(
            1,
            [1; 32],
            [2; 32],
            golden_compiler(),
            PipelineAllowlistV1::TILED_GEMM,
            PipelineV1::CollectedRowSoftmax,
            [9; 32],
        ),
        Err(PolicyErrorV1::SelectedPipelineNotAllowed {
            selected: PipelineV1::CollectedRowSoftmax,
            allowlist_bits: PipelineAllowlistV1::TILED_GEMM.bits(),
        })
    );
}

#[test]
fn exhaustive_single_bit_mutations_are_rejected_or_remain_canonical_and_distinct() {
    let baseline = golden_bytes();
    let baseline_identity = GOLDEN_POLICY_IDENTITY;
    let mut accepted = 0_usize;
    let mut rejected = 0_usize;

    for byte_index in 0..baseline.len() {
        for bit in 0..8 {
            let mut mutated = baseline;
            mutated[byte_index] ^= 1 << bit;
            match decode_policy_v1(&mutated) {
                Ok(policy) => {
                    accepted += 1;
                    assert_eq!(policy.encode(), mutated, "byte {byte_index}, bit {bit}");
                    let identity = policy_identity_sha256_v1(&mutated).unwrap();
                    assert_eq!(identity, policy.identity_sha256());
                    assert_ne!(identity, baseline_identity);
                }
                Err(_) => {
                    rejected += 1;
                    assert!(policy_identity_sha256_v1(&mutated).is_err());
                }
            }
        }
    }

    assert!(
        accepted > 100,
        "expected semantic content mutations to remain valid"
    );
    assert!(
        rejected > 100,
        "expected structural mutations to fail closed"
    );
    assert_eq!(accepted + rejected, POLICY_V1_ENCODED_LEN * 8);
}

#[test]
fn seeded_multibyte_mutations_never_decode_noncanonically() {
    let baseline = golden_bytes();
    let mut state = 0x7a61_9e37_c4d2_b805_u64;

    for iteration in 0..2_048_u64 {
        let mut mutated = baseline;
        let changes = (iteration as usize % 7) + 1;
        for _ in 0..changes {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let byte_index = state as usize % POLICY_V1_ENCODED_LEN;
            let bit = ((state >> 32) & 7) as u8;
            mutated[byte_index] ^= 1_u8 << bit;
        }
        if mutated == baseline {
            continue;
        }
        if let Ok(policy) = decode_policy_v1(&mutated) {
            assert_eq!(policy.encode(), mutated, "iteration {iteration}");
            assert_ne!(policy.identity_sha256(), GOLDEN_POLICY_IDENTITY);
        }
    }
}
