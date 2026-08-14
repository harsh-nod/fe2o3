use fe2o3_build_authority::{
    ACCEPT_V1_PAYLOAD_LEN, ADMISSION_IDENTITY_DOMAIN_V1, ARGUMENT_VECTOR_IDENTITY_DOMAIN_V1,
    ATTEST_V1_PAYLOAD_LEN, ATTESTATION_IDENTITY_DOMAIN_V1, AcceptV1, ArgvIdentityErrorV1, AttestV1,
    CHALLENGE_V1_PAYLOAD_LEN, ChallengeV1, DENY_V1_PAYLOAD_LEN, DenyReasonV1, DenyV1, FrameKindV1,
    GRANT_V1_PAYLOAD_LEN, GrantV1, IDENTITY_V1_LEN, NONCE_V1_LEN, PROTECTED_AUTHORITY_ARGV0_V1,
    PROTOCOL_V1_HEADER_LEN, PROTOCOL_V1_MAGIC, PROTOCOL_V1_MAX_ARGUMENT_BYTES,
    PROTOCOL_V1_MAX_ARGUMENTS, PROTOCOL_V1_MAX_TOTAL_ARGUMENT_BYTES, PROTOCOL_V1_VERSION,
    PipelineAllowlistV1, PipelineV1, PolicyV1, ProtocolErrorV1, ProtocolFrameV1,
    ProtocolIdentityFieldV1, argv_identity_sha256_v1, decode_protocol_frame_v1,
};
use fe2o3_build_authority::{CompilerClosureV1, PublicationRightsV1};

const HEADER: usize = PROTOCOL_V1_HEADER_LEN;

fn digest(seed: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (index, byte) in value.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(29).wrapping_add(index as u8 + 1);
    }
    value
}

fn compiler(seed: u8) -> CompilerClosureV1 {
    CompilerClosureV1::new(
        digest(seed),
        digest(seed + 1),
        digest(seed + 2),
        digest(seed + 3),
    )
    .unwrap()
}

fn policy(seed: u8, pipeline: PipelineV1) -> PolicyV1 {
    PolicyV1::new(
        0x0102_0304_0506_0708 + u64::from(seed),
        digest(seed + 4),
        digest(seed + 5),
        compiler(seed + 6),
        PipelineAllowlistV1::ALL,
        pipeline,
        digest(seed + 10),
    )
    .unwrap()
}

fn frames() -> [ProtocolFrameV1; 5] {
    let policy = policy(1, PipelineV1::CollectedTiledGemm);
    let challenge = ChallengeV1::for_policy(digest(20), policy).unwrap();
    let attest = AttestV1::for_policy(digest(20), policy).unwrap();
    let grant = GrantV1::for_attestation(attest, digest(21)).unwrap();
    [
        ProtocolFrameV1::Challenge(challenge),
        ProtocolFrameV1::Attest(attest),
        ProtocolFrameV1::Grant(grant),
        ProtocolFrameV1::Deny(DenyV1::for_attestation(
            attest,
            DenyReasonV1::ProtocolViolation,
        )),
        ProtocolFrameV1::Accept(AcceptV1::for_grant(grant)),
    ]
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn layout_lengths_and_domains_are_stable() {
    assert_eq!(PROTOCOL_V1_HEADER_LEN, 24);
    assert_eq!(CHALLENGE_V1_PAYLOAD_LEN, 176);
    assert_eq!(ATTEST_V1_PAYLOAD_LEN, 364);
    assert_eq!(GRANT_V1_PAYLOAD_LEN, 172);
    assert_eq!(DENY_V1_PAYLOAD_LEN, 100);
    assert_eq!(ACCEPT_V1_PAYLOAD_LEN, 32);
    assert_eq!(IDENTITY_V1_LEN, 32);
    assert_eq!(NONCE_V1_LEN, 32);
    assert_eq!(PROTOCOL_V1_MAGIC, *b"F2AUPR1\0");
    assert_eq!(PROTOCOL_V1_VERSION, 1);
    assert_eq!(
        ARGUMENT_VECTOR_IDENTITY_DOMAIN_V1,
        b"FE2O3/PROTECTED-AUTHORITY-ARGV/V1\0"
    );
    assert_eq!(
        ATTESTATION_IDENTITY_DOMAIN_V1,
        b"FE2O3/PROTECTED-AUTHORITY-ATTEST/V1\0"
    );
    assert_eq!(
        ADMISSION_IDENTITY_DOMAIN_V1,
        b"FE2O3/PROTECTED-AUTHORITY-ADMISSION/V1\0"
    );
}

#[test]
fn every_frame_roundtrips_with_an_exact_header() {
    let expected = [
        (FrameKindV1::Challenge, 0_u32, CHALLENGE_V1_PAYLOAD_LEN),
        (FrameKindV1::Attest, 1, ATTEST_V1_PAYLOAD_LEN),
        (FrameKindV1::Grant, 2, GRANT_V1_PAYLOAD_LEN),
        (FrameKindV1::Deny, 2, DENY_V1_PAYLOAD_LEN),
        (FrameKindV1::Accept, 3, ACCEPT_V1_PAYLOAD_LEN),
    ];
    for (frame, (kind, sequence, payload_len)) in frames().into_iter().zip(expected) {
        let bytes = frame.encode();
        assert_eq!(frame.kind(), kind);
        assert_eq!(bytes.len(), HEADER + payload_len);
        assert_eq!(&bytes[..8], &PROTOCOL_V1_MAGIC);
        assert_eq!(u16::from_le_bytes(bytes[8..10].try_into().unwrap()), 1);
        assert_eq!(
            u16::from_le_bytes(bytes[10..12].try_into().unwrap()),
            kind as u16
        );
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            payload_len as u32
        );
        assert_eq!(
            u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            sequence
        );
        assert_eq!(&bytes[20..24], &[0; 4]);
        assert_eq!(decode_protocol_frame_v1(&bytes), Ok(frame));
        assert_eq!(
            decode_protocol_frame_v1(&frame.encode()).unwrap().encode(),
            bytes
        );
    }
}

#[test]
fn typed_payload_fields_use_the_documented_offsets() {
    let [challenge, attest, grant, deny, accept] = frames();

    let ProtocolFrameV1::Challenge(challenge_value) = challenge else {
        unreachable!()
    };
    let bytes = challenge.encode();
    assert_eq!(&bytes[HEADER..HEADER + 32], &challenge_value.nonce());
    assert_eq!(
        &bytes[HEADER + 32..HEADER + 64],
        &challenge_value.policy_identity()
    );
    assert_eq!(
        &bytes[HEADER + 64..HEADER + 96],
        &challenge_value.launcher_executable_identity()
    );
    assert_eq!(
        &bytes[HEADER + 96..HEADER + 128],
        &challenge_value.cargo_fe2o3_executable_identity()
    );
    assert_eq!(
        &bytes[HEADER + 128..HEADER + 160],
        &challenge_value.child_argv_identity()
    );
    assert_eq!(
        u64::from_le_bytes(bytes[HEADER + 160..HEADER + 168].try_into().unwrap()),
        challenge_value.policy_serial()
    );
    assert_eq!(&bytes[HEADER + 168..HEADER + 176], &[0; 8]);

    let ProtocolFrameV1::Attest(attest_value) = attest else {
        unreachable!()
    };
    let bytes = attest.encode();
    assert_eq!(
        &bytes[HEADER + 160..HEADER + 192],
        &attest_value.compiler_closure().cargo_executable_sha256()
    );
    assert_eq!(
        &bytes[HEADER + 192..HEADER + 224],
        &attest_value.compiler_closure().rustc_executable_sha256()
    );
    assert_eq!(
        &bytes[HEADER + 224..HEADER + 256],
        &attest_value.compiler_closure().rustc_runtime_tree_sha256()
    );
    assert_eq!(
        &bytes[HEADER + 256..HEADER + 288],
        &attest_value.compiler_closure().codegen_backend_sha256()
    );
    assert_eq!(
        &bytes[HEADER + 288..HEADER + 320],
        &attest_value.compiler_closure().identity_sha256()
    );
    assert_eq!(
        u16::from_le_bytes(bytes[HEADER + 320..HEADER + 322].try_into().unwrap()),
        1
    );
    assert_eq!(
        u16::from_le_bytes(bytes[HEADER + 322..HEADER + 324].try_into().unwrap()),
        2
    );
    assert_eq!(&bytes[HEADER + 324..HEADER + 332], &[0; 8]);
    assert_eq!(
        &bytes[HEADER + 332..HEADER + 364],
        &attest_value.attestation_identity()
    );

    let ProtocolFrameV1::Grant(grant_value) = grant else {
        unreachable!()
    };
    let bytes = grant.encode();
    assert_eq!(&bytes[HEADER + 96..HEADER + 128], &grant_value.grant_id());
    assert_eq!(
        u16::from_le_bytes(bytes[HEADER + 128..HEADER + 130].try_into().unwrap()),
        1
    );
    assert_eq!(
        u16::from_le_bytes(bytes[HEADER + 130..HEADER + 132].try_into().unwrap()),
        2
    );
    assert_eq!(&bytes[HEADER + 132..HEADER + 140], &[0; 8]);
    assert_eq!(
        &bytes[HEADER + 140..HEADER + 172],
        &grant_value.admission_identity()
    );

    let ProtocolFrameV1::Deny(deny_value) = deny else {
        unreachable!()
    };
    let bytes = deny.encode();
    assert_eq!(
        u16::from_le_bytes(bytes[HEADER + 96..HEADER + 98].try_into().unwrap()),
        8
    );
    assert_eq!(&bytes[HEADER + 98..HEADER + 100], &[0; 2]);
    assert_eq!(deny_value.reason(), DenyReasonV1::ProtocolViolation);

    let ProtocolFrameV1::Accept(accept_value) = accept else {
        unreachable!()
    };
    assert_eq!(
        &accept.encode()[HEADER..],
        &accept_value.admission_identity()
    );
}

#[test]
fn argv_identity_is_bounded_and_order_sensitive() {
    let first = argv_identity_sha256_v1(&[
        PROTECTED_AUTHORITY_ARGV0_V1,
        b"build",
        b"--target",
        b"gfx942:xnack-",
    ])
    .unwrap();
    let repeated = argv_identity_sha256_v1(&[
        PROTECTED_AUTHORITY_ARGV0_V1,
        b"build",
        b"--target",
        b"gfx942:xnack-",
    ])
    .unwrap();
    let reordered = argv_identity_sha256_v1(&[
        PROTECTED_AUTHORITY_ARGV0_V1,
        b"build",
        b"gfx942:xnack-",
        b"--target",
    ])
    .unwrap();
    assert_eq!(first, repeated);
    assert_ne!(first, reordered);

    assert_eq!(
        argv_identity_sha256_v1(&[]),
        Err(ArgvIdentityErrorV1::InvalidArgumentCount { actual: 0 })
    );
    assert_eq!(
        argv_identity_sha256_v1(&[PROTECTED_AUTHORITY_ARGV0_V1]),
        Err(ArgvIdentityErrorV1::InvalidArgumentCount { actual: 1 })
    );
    let too_many = vec![b"x".as_slice(); PROTOCOL_V1_MAX_ARGUMENTS + 1];
    assert_eq!(
        argv_identity_sha256_v1(&too_many),
        Err(ArgvIdentityErrorV1::InvalidArgumentCount {
            actual: PROTOCOL_V1_MAX_ARGUMENTS + 1,
        })
    );
    assert_eq!(
        argv_identity_sha256_v1(&[b"cargo-fe2o3", b"build"]),
        Err(ArgvIdentityErrorV1::InvalidArgv0)
    );
    assert_eq!(
        argv_identity_sha256_v1(&[PROTECTED_AUTHORITY_ARGV0_V1, b""]),
        Err(ArgvIdentityErrorV1::EmptyArgument { index: 1 })
    );
    assert_eq!(
        argv_identity_sha256_v1(&[PROTECTED_AUTHORITY_ARGV0_V1, b"a\0b"]),
        Err(ArgvIdentityErrorV1::InteriorNul { index: 1 })
    );
    let long = vec![b'x'; PROTOCOL_V1_MAX_ARGUMENT_BYTES + 1];
    assert_eq!(
        argv_identity_sha256_v1(&[PROTECTED_AUTHORITY_ARGV0_V1, &long]),
        Err(ArgvIdentityErrorV1::ArgumentTooLong {
            index: 1,
            actual: PROTOCOL_V1_MAX_ARGUMENT_BYTES + 1,
        })
    );
    let chunk = vec![b'x'; PROTOCOL_V1_MAX_ARGUMENT_BYTES];
    let mut large = vec![PROTECTED_AUTHORITY_ARGV0_V1];
    large.extend(std::iter::repeat_n(chunk.as_slice(), 16));
    assert!(matches!(
        argv_identity_sha256_v1(&large),
        Err(ArgvIdentityErrorV1::ArgumentsTooLarge { actual })
            if actual > PROTOCOL_V1_MAX_TOTAL_ARGUMENT_BYTES
    ));
}

#[test]
fn header_adversaries_fail_before_payload_parsing() {
    let bytes = frames()[0].encode();
    for length in 0..HEADER {
        assert_eq!(
            decode_protocol_frame_v1(&bytes[..length]),
            Err(ProtocolErrorV1::TruncatedHeader { actual: length })
        );
    }

    let mut mutated = bytes.clone();
    mutated[0] ^= 1;
    assert_eq!(
        decode_protocol_frame_v1(&mutated),
        Err(ProtocolErrorV1::InvalidMagic)
    );

    let mut mutated = bytes.clone();
    set_u16(&mut mutated, 8, 2);
    assert_eq!(
        decode_protocol_frame_v1(&mutated),
        Err(ProtocolErrorV1::UnsupportedVersion { actual: 2 })
    );

    let mut mutated = bytes.clone();
    set_u16(&mut mutated, 10, 0xffff);
    assert_eq!(
        decode_protocol_frame_v1(&mutated),
        Err(ProtocolErrorV1::UnknownFrameType { actual: 0xffff })
    );

    let mut mutated = bytes.clone();
    set_u32(&mut mutated, 12, (CHALLENGE_V1_PAYLOAD_LEN - 1) as u32);
    assert!(matches!(
        decode_protocol_frame_v1(&mutated),
        Err(ProtocolErrorV1::InvalidPayloadLength {
            kind: FrameKindV1::Challenge,
            ..
        })
    ));

    let mut mutated = bytes.clone();
    set_u32(&mut mutated, 16, 1);
    assert!(matches!(
        decode_protocol_frame_v1(&mutated),
        Err(ProtocolErrorV1::InvalidSequence {
            kind: FrameKindV1::Challenge,
            expected: 0,
            actual: 1,
        })
    ));

    let mut mutated = bytes.clone();
    set_u32(&mut mutated, 20, 1);
    assert_eq!(
        decode_protocol_frame_v1(&mutated),
        Err(ProtocolErrorV1::UnsupportedFlags { actual: 1 })
    );

    let short = &bytes[..bytes.len() - 1];
    assert_eq!(
        decode_protocol_frame_v1(short),
        Err(ProtocolErrorV1::InvalidEncodedLength {
            expected: bytes.len(),
            actual: bytes.len() - 1,
        })
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        decode_protocol_frame_v1(&trailing),
        Err(ProtocolErrorV1::InvalidEncodedLength {
            expected: bytes.len(),
            actual: bytes.len() + 1,
        })
    );
}

#[test]
fn foundation_selectors_and_required_identities_fail_closed() {
    let [challenge, attest, grant, deny, accept] = frames();

    let mut bytes = challenge.encode();
    bytes[HEADER..HEADER + 32].fill(0);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::ZeroIdentity {
            field: ProtocolIdentityFieldV1::Nonce,
        })
    );
    let mut bytes = challenge.encode();
    bytes[HEADER + 160..HEADER + 168].fill(0);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::ZeroPolicySerial)
    );
    let mut bytes = challenge.encode();
    bytes[HEADER + 168] = 1;
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::ProfileNotPermitted { actual: 1 })
    );
    let mut bytes = challenge.encode();
    bytes[HEADER + 169] = 1;
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Challenge,
        })
    );

    let mut bytes = attest.encode();
    set_u16(&mut bytes, HEADER + 320, 2);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::UnknownTarget { actual: 2 })
    );
    let mut bytes = attest.encode();
    set_u16(&mut bytes, HEADER + 322, 3);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::UnknownPipeline { actual: 3 })
    );
    let mut bytes = attest.encode();
    bytes[HEADER + 324] = 1;
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::ProfileNotPermitted { actual: 1 })
    );
    let mut bytes = attest.encode();
    set_u32(&mut bytes, HEADER + 325, 1);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::PublicationRightsNotPermitted { actual: 1 })
    );
    let mut bytes = attest.encode();
    bytes[HEADER + 329] = 1;
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Attest,
        })
    );

    let mut bytes = grant.encode();
    set_u32(&mut bytes, HEADER + 133, 1);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::PublicationRightsNotPermitted { actual: 1 })
    );
    let mut bytes = grant.encode();
    bytes[HEADER + 137] = 1;
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Grant,
        })
    );

    let mut bytes = deny.encode();
    set_u16(&mut bytes, HEADER + 96, 10);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::UnknownDenyReason { actual: 10 })
    );
    let mut bytes = deny.encode();
    bytes[HEADER + 98] = 1;
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::NonzeroReserved {
            kind: FrameKindV1::Deny,
        })
    );

    let mut bytes = accept.encode();
    bytes[HEADER..].fill(0);
    assert_eq!(
        decode_protocol_frame_v1(&bytes),
        Err(ProtocolErrorV1::ZeroIdentity {
            field: ProtocolIdentityFieldV1::Admission,
        })
    );
    assert_eq!(PublicationRightsV1::NONE.bits(), 0);
}

#[test]
fn attest_and_grant_single_bit_mutations_cannot_remain_canonical() {
    for frame in [frames()[1], frames()[2]] {
        let original = frame.encode();
        for byte_index in HEADER..original.len() {
            for bit in 0..8 {
                let mut mutated = original.clone();
                mutated[byte_index] ^= 1 << bit;
                assert!(
                    decode_protocol_frame_v1(&mutated).is_err(),
                    "accepted {frame:?} payload mutation at byte {byte_index}, bit {bit}"
                );
            }
        }
    }
}

#[test]
fn all_typed_deny_reasons_and_pipeline_choices_roundtrip() {
    let reasons = [
        DenyReasonV1::PolicyRejected,
        DenyReasonV1::ExecutableIdentityMismatch,
        DenyReasonV1::ArgumentVectorMismatch,
        DenyReasonV1::CompilerClosureMismatch,
        DenyReasonV1::TargetNotPermitted,
        DenyReasonV1::PipelineNotPermitted,
        DenyReasonV1::RightsNotPermitted,
        DenyReasonV1::ProtocolViolation,
        DenyReasonV1::InternalFailure,
    ];
    for pipeline in [
        PipelineV1::CollectedRowSoftmax,
        PipelineV1::CollectedTiledGemm,
    ] {
        let policy = policy(33, pipeline);
        let attest = AttestV1::for_policy(digest(44), policy).unwrap();
        let grant = GrantV1::for_attestation(attest, digest(45)).unwrap();
        assert_eq!(
            decode_protocol_frame_v1(&ProtocolFrameV1::Grant(grant).encode()),
            Ok(ProtocolFrameV1::Grant(grant))
        );
        for reason in reasons {
            let frame = ProtocolFrameV1::Deny(DenyV1::for_attestation(attest, reason));
            assert_eq!(decode_protocol_frame_v1(&frame.encode()), Ok(frame));
        }
    }
}

#[test]
fn commitment_golden_vectors_are_stable() {
    let argv = argv_identity_sha256_v1(&[
        PROTECTED_AUTHORITY_ARGV0_V1,
        b"build",
        b"--target",
        b"gfx942:xnack-",
    ])
    .unwrap();
    let ProtocolFrameV1::Attest(attest) = frames()[1] else {
        unreachable!()
    };
    let ProtocolFrameV1::Grant(grant) = frames()[2] else {
        unreachable!()
    };

    assert_eq!(
        argv,
        [
            0x19, 0xfd, 0x49, 0x0e, 0xa1, 0xc9, 0xce, 0x23, 0x1c, 0x4f, 0x5a, 0x57, 0x9b, 0x7c,
            0x06, 0x9b, 0xfd, 0x92, 0xe7, 0x18, 0xe7, 0x46, 0xe3, 0x74, 0xa4, 0xa9, 0xdc, 0xc9,
            0xd8, 0xfb, 0x91, 0xa6,
        ]
    );
    assert_eq!(
        attest.attestation_identity(),
        [
            0xd0, 0xda, 0x20, 0xcf, 0x47, 0xba, 0x63, 0x53, 0xb4, 0x84, 0x4b, 0x71, 0x37, 0x97,
            0xc9, 0x78, 0x93, 0x16, 0x6c, 0x88, 0x95, 0x26, 0x93, 0x38, 0x9e, 0x6b, 0xca, 0x28,
            0x93, 0x86, 0xc3, 0xe0,
        ]
    );
    assert_eq!(
        grant.admission_identity(),
        [
            0x39, 0x66, 0x66, 0xad, 0x60, 0x2f, 0x43, 0x3c, 0xf6, 0xd0, 0x26, 0x57, 0xc5, 0x4e,
            0x96, 0x30, 0x21, 0x29, 0x0b, 0xc1, 0x64, 0x12, 0x20, 0x16, 0x10, 0x4a, 0xcd, 0x49,
            0x42, 0x82, 0x1b, 0xca,
        ]
    );
}
