use fe2o3_build_authority::{
    BROKER_V4_BINDING_IDENTITY_DOMAIN, BROKER_V4_BINDING_OFFSET, BROKER_V4_BINDING_WIRE_LEN,
    BROKER_V4_HEADER_LEN, BROKER_V4_MAGIC, BROKER_V4_PROCESS_OFFSET, BROKER_V4_VERSION,
    BrokerFrameKindV4, BrokerFrameV4, BrokerIdentityFieldV4, BrokerProtocolErrorV4, BrokerTargetV4,
    CapabilityBindingV4, HOST_LINK_CLOSURE_OFFSET_V4, HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4,
    HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4, HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4,
    HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4, HOST_LINK_COMMIT_RESERVED_OFFSET_V4,
    HOST_LINK_COMMIT_V4_PAYLOAD_LEN, HOST_LINK_GRANT_OFFSET_V4, HOST_LINK_GRANT_V4_PAYLOAD_LEN,
    HOST_LINK_OUTPUT_MODE_V4, HOST_LINK_PLAN_OFFSET_V4, HOST_LINK_PREPARE_V4_PAYLOAD_LEN,
    HOST_LINK_REQUEST_OFFSET_V4, HostLinkCommitV4, HostLinkGrantV4, HostLinkPrepareV4,
    PROCESS_IDENTITY_V4_WIRE_LEN, ProcessIdentityV4, decode_broker_frame_v4,
    decode_capability_binding_v4,
};
use sha2::{Digest, Sha256};

const HEADER: usize = BROKER_V4_HEADER_LEN;
const GOLDEN_BINDING_IDENTITY: [u8; 32] = [
    0x36, 0xf9, 0xda, 0xaf, 0xa0, 0xaa, 0x12, 0x98, 0x9a, 0xb7, 0xd5, 0x58, 0x74, 0x89, 0x93, 0x87,
    0xd7, 0xf1, 0x83, 0xd1, 0x5d, 0x6e, 0xb8, 0x7b, 0xb1, 0x80, 0x63, 0x4d, 0xaf, 0x05, 0xa0, 0xc1,
];
const GOLDEN_FRAME_SHA256: [[u8; 32]; 3] = [
    [
        0x43, 0x0e, 0xdf, 0xef, 0x8b, 0x24, 0xae, 0x91, 0x81, 0x50, 0xa7, 0x3a, 0x5b, 0x7d, 0x7d,
        0x23, 0x4e, 0x6f, 0x98, 0xa9, 0xfb, 0x76, 0xed, 0x1b, 0x9c, 0x79, 0x60, 0xc6, 0x5e, 0xde,
        0xfe, 0xf1,
    ],
    [
        0x43, 0x11, 0x53, 0x48, 0x44, 0x7c, 0x1d, 0xac, 0x44, 0xad, 0xf4, 0xb1, 0xa1, 0x71, 0x99,
        0x3b, 0x01, 0x57, 0xb2, 0x54, 0x1e, 0x0f, 0x89, 0x0d, 0x97, 0x2a, 0x00, 0xb1, 0xca, 0xc4,
        0x29, 0xf3,
    ],
    [
        0x27, 0xe1, 0x42, 0x3c, 0x5d, 0xf9, 0x44, 0xe0, 0x16, 0xcc, 0x23, 0x3c, 0xb8, 0xca, 0xe4,
        0x3b, 0x33, 0xa2, 0x5e, 0x94, 0x02, 0x1c, 0x0d, 0x46, 0x76, 0x11, 0xde, 0x37, 0xc0, 0x72,
        0x0d, 0x4c,
    ],
];

fn digest(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn patterned_digest(seed: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (index, byte) in value.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(43).wrapping_add(index as u8 + 1);
    }
    value
}

fn binding() -> CapabilityBindingV4 {
    CapabilityBindingV4::new(digest(1), digest(2), digest(3)).unwrap()
}

fn process() -> ProcessIdentityV4 {
    ProcessIdentityV4::new(0x1122_3344, 0x0102_0304_0506_0708).unwrap()
}

fn frames(binding: CapabilityBindingV4) -> [BrokerFrameV4; 3] {
    let process = process();
    let binding_identity = binding.identity_sha256();
    [
        BrokerFrameV4::HostLinkPrepare(
            HostLinkPrepareV4::new(process, binding_identity, digest(4), digest(5), digest(6))
                .unwrap(),
        ),
        BrokerFrameV4::HostLinkGrant(
            HostLinkGrantV4::new(
                process,
                binding_identity,
                digest(4),
                digest(5),
                digest(6),
                digest(7),
            )
            .unwrap(),
        ),
        BrokerFrameV4::HostLinkCommit(
            HostLinkCommitV4::new(
                process,
                binding_identity,
                digest(4),
                digest(5),
                digest(6),
                digest(7),
                digest(8),
                85_597_472,
                HOST_LINK_OUTPUT_MODE_V4,
                digest(9),
            )
            .unwrap(),
        ),
    ]
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn v4_lengths_magic_and_offsets_are_fixed() {
    assert_eq!(BROKER_V4_HEADER_LEN, 24);
    assert_eq!(PROCESS_IDENTITY_V4_WIRE_LEN, 16);
    assert_eq!(BROKER_V4_BINDING_WIRE_LEN, 104);
    assert_eq!(HOST_LINK_PREPARE_V4_PAYLOAD_LEN, 144);
    assert_eq!(HOST_LINK_GRANT_V4_PAYLOAD_LEN, 176);
    assert_eq!(HOST_LINK_COMMIT_V4_PAYLOAD_LEN, 256);
    assert_eq!(BROKER_V4_PROCESS_OFFSET, 0);
    assert_eq!(BROKER_V4_BINDING_OFFSET, 16);
    assert_eq!(HOST_LINK_REQUEST_OFFSET_V4, 48);
    assert_eq!(HOST_LINK_PLAN_OFFSET_V4, 80);
    assert_eq!(HOST_LINK_CLOSURE_OFFSET_V4, 112);
    assert_eq!(HOST_LINK_GRANT_OFFSET_V4, 144);
    assert_eq!(HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4, 176);
    assert_eq!(HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4, 208);
    assert_eq!(HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4, 216);
    assert_eq!(HOST_LINK_COMMIT_RESERVED_OFFSET_V4, 220);
    assert_eq!(HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4, 224);
    assert_eq!(BROKER_V4_MAGIC, *b"F2AUBR4\0");
    assert_eq!(BROKER_V4_VERSION, 4);
}

#[test]
fn binding_layout_identity_target_and_zero_rights_are_stable() {
    let binding = binding();
    let encoded = binding.encode();
    assert_eq!(&encoded[0..32], &digest(1));
    assert_eq!(&encoded[32..64], &digest(2));
    assert_eq!(&encoded[64..96], &digest(3));
    assert_eq!(u16::from_le_bytes(encoded[96..98].try_into().unwrap()), 1);
    assert_eq!(&encoded[98..100], &[0; 2]);
    assert_eq!(&encoded[100..104], &[0; 4]);
    assert_eq!(binding.base_binding_identity(), digest(1));
    assert_eq!(binding.release_contract_identity(), digest(2));
    assert_eq!(binding.static_host_lld_identity(), digest(3));
    assert_eq!(binding.target(), BrokerTargetV4::Gfx942XnackMinus);
    assert_eq!(
        binding.authority(),
        fe2o3_build_authority::BrokerAuthorityV4::None
    );
    assert_eq!(binding.identity_sha256(), GOLDEN_BINDING_IDENTITY);
    assert_eq!(decode_capability_binding_v4(&encoded), Ok(binding));
    assert_eq!(
        BROKER_V4_BINDING_IDENTITY_DOMAIN,
        b"FE2O3/PROTECTED-AUTHORITY-BROKER-V4-BINDING\0"
    );
}

#[test]
fn v4_constructor_binds_the_unmodified_base_identity() {
    let base = digest(10);
    let v4 = CapabilityBindingV4::new(base, digest(20), digest(21)).unwrap();
    assert_eq!(v4.base_binding_identity(), base);
    assert_eq!(v4.release_contract_identity(), digest(20));
    assert_eq!(v4.static_host_lld_identity(), digest(21));
}

#[test]
fn full_frames_match_independent_fixed_sha256_vectors() {
    let actual = frames(binding()).map(|frame| sha256(&frame.encode()));
    assert_eq!(actual, GOLDEN_FRAME_SHA256);
}

#[test]
fn every_frame_has_exact_header_sequence_offsets_and_roundtrip() {
    let expected = [
        (
            BrokerFrameKindV4::HostLinkPrepare,
            HOST_LINK_PREPARE_V4_PAYLOAD_LEN,
            0_u32,
        ),
        (
            BrokerFrameKindV4::HostLinkGrant,
            HOST_LINK_GRANT_V4_PAYLOAD_LEN,
            1,
        ),
        (
            BrokerFrameKindV4::HostLinkCommit,
            HOST_LINK_COMMIT_V4_PAYLOAD_LEN,
            2,
        ),
    ];
    for (frame, (kind, payload_len, sequence)) in frames(binding()).into_iter().zip(expected) {
        let encoded = frame.encode();
        let payload = &encoded[HEADER..];
        assert_eq!(frame.kind(), kind);
        assert_eq!(frame.encoded_len(), HEADER + payload_len);
        assert_eq!(&encoded[0..8], &BROKER_V4_MAGIC);
        assert_eq!(u16::from_le_bytes(encoded[8..10].try_into().unwrap()), 4);
        assert_eq!(
            u16::from_le_bytes(encoded[10..12].try_into().unwrap()),
            kind as u16
        );
        assert_eq!(
            u32::from_le_bytes(encoded[12..16].try_into().unwrap()),
            payload_len as u32
        );
        assert_eq!(
            u32::from_le_bytes(encoded[16..20].try_into().unwrap()),
            sequence
        );
        assert_eq!(&encoded[20..24], &[0; 4]);
        assert_eq!(&payload[0..4], &0x1122_3344_u32.to_le_bytes());
        assert_eq!(&payload[4..8], &[0; 4]);
        assert_eq!(
            &payload[BROKER_V4_BINDING_OFFSET..BROKER_V4_BINDING_OFFSET + 32],
            &binding().identity_sha256()
        );
        assert_eq!(
            &payload[HOST_LINK_REQUEST_OFFSET_V4..HOST_LINK_REQUEST_OFFSET_V4 + 32],
            &digest(4)
        );
        assert_eq!(
            &payload[HOST_LINK_PLAN_OFFSET_V4..HOST_LINK_PLAN_OFFSET_V4 + 32],
            &digest(5)
        );
        assert_eq!(
            &payload[HOST_LINK_CLOSURE_OFFSET_V4..HOST_LINK_CLOSURE_OFFSET_V4 + 32],
            &digest(6)
        );
        assert_eq!(decode_broker_frame_v4(&encoded), Ok(frame));
    }

    let commit = frames(binding())[2].encode();
    let payload = &commit[HEADER..];
    assert_eq!(
        &payload[HOST_LINK_GRANT_OFFSET_V4..HOST_LINK_GRANT_OFFSET_V4 + 32],
        &digest(7)
    );
    assert_eq!(
        &payload[HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4
            ..HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4 + 32],
        &digest(8)
    );
    assert_eq!(
        u64::from_le_bytes(
            payload[HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4
                ..HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4 + 8]
                .try_into()
                .unwrap()
        ),
        85_597_472
    );
    assert_eq!(
        u32::from_le_bytes(
            payload[HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4
                ..HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4 + 4]
                .try_into()
                .unwrap()
        ),
        0o555
    );
    assert_eq!(
        &payload[HOST_LINK_COMMIT_RESERVED_OFFSET_V4..HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4],
        &[0; 4]
    );
    assert_eq!(
        &payload
            [HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4..HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4 + 32],
        &digest(9)
    );
}

#[test]
fn binding_and_header_adversaries_fail_closed() {
    let canonical_binding = binding().encode();
    for length in [0, 1, BROKER_V4_BINDING_WIRE_LEN - 1] {
        assert_eq!(
            decode_capability_binding_v4(&canonical_binding[..length]),
            Err(BrokerProtocolErrorV4::InvalidBindingLength { actual: length })
        );
    }
    let mut trailing = canonical_binding.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_capability_binding_v4(&trailing),
        Err(BrokerProtocolErrorV4::InvalidBindingLength { actual: 105 })
    );
    let mut invalid = canonical_binding;
    set_u16(&mut invalid, 96, 2);
    assert_eq!(
        decode_capability_binding_v4(&invalid),
        Err(BrokerProtocolErrorV4::UnknownTarget { actual: 2 })
    );
    let mut invalid = canonical_binding;
    set_u16(&mut invalid, 98, 1);
    assert_eq!(
        decode_capability_binding_v4(&invalid),
        Err(BrokerProtocolErrorV4::NonzeroBindingReserved)
    );
    for rights in [1, 2, u32::MAX] {
        let mut invalid = canonical_binding;
        set_u32(&mut invalid, 100, rights);
        assert_eq!(
            decode_capability_binding_v4(&invalid),
            Err(BrokerProtocolErrorV4::PublicationAuthorityForbidden { actual: rights })
        );
    }

    let canonical = frames(binding())[0].encode();
    for length in [0, 1, 23] {
        assert_eq!(
            decode_broker_frame_v4(&canonical[..length]),
            Err(BrokerProtocolErrorV4::TruncatedHeader { actual: length })
        );
    }
    let mut invalid = canonical.clone();
    invalid[0] ^= 1;
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::InvalidMagic)
    );
    let mut invalid = canonical.clone();
    set_u16(&mut invalid, 8, 3);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::UnsupportedVersion { actual: 3 })
    );
    let mut invalid = canonical.clone();
    set_u16(&mut invalid, 10, 4);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::UnknownFrameType { actual: 4 })
    );
    let mut invalid = canonical.clone();
    set_u32(&mut invalid, 12, 1);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::InvalidPayloadLength {
            kind: BrokerFrameKindV4::HostLinkPrepare,
            expected: HOST_LINK_PREPARE_V4_PAYLOAD_LEN,
            actual: 1,
        })
    );
    let mut invalid = canonical.clone();
    set_u32(&mut invalid, 16, 1);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::InvalidSequence {
            kind: BrokerFrameKindV4::HostLinkPrepare,
            expected: 0,
            actual: 1,
        })
    );
    let mut invalid = canonical.clone();
    set_u32(&mut invalid, 20, 1);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::UnsupportedFlags { actual: 1 })
    );
    let mut trailing = canonical;
    trailing.push(0);
    assert_eq!(
        decode_broker_frame_v4(&trailing),
        Err(BrokerProtocolErrorV4::InvalidEncodedLength {
            expected: HEADER + HOST_LINK_PREPARE_V4_PAYLOAD_LEN,
            actual: HEADER + HOST_LINK_PREPARE_V4_PAYLOAD_LEN + 1,
        })
    );
}

#[test]
fn process_commit_and_identity_adversaries_fail_closed() {
    let binding = binding();
    let binding_identity = binding.identity_sha256();
    let process = process();
    let prepare = frames(binding)[0].encode();
    let mut invalid = prepare.clone();
    set_u32(&mut invalid, HEADER + BROKER_V4_PROCESS_OFFSET, 0);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::ZeroProcessId)
    );
    let mut invalid = prepare;
    set_u32(&mut invalid, HEADER + BROKER_V4_PROCESS_OFFSET + 4, 1);
    assert_eq!(
        decode_broker_frame_v4(&invalid),
        Err(BrokerProtocolErrorV4::NonzeroProcessReserved)
    );

    let mut commit = frames(binding)[2].encode();
    set_u32(&mut commit, HEADER + HOST_LINK_COMMIT_RESERVED_OFFSET_V4, 1);
    assert_eq!(
        decode_broker_frame_v4(&commit),
        Err(BrokerProtocolErrorV4::NonzeroHostLinkCommitReserved)
    );
    assert_eq!(
        HostLinkCommitV4::new(
            process,
            binding_identity,
            digest(4),
            digest(5),
            digest(6),
            digest(7),
            digest(8),
            0,
            HOST_LINK_OUTPUT_MODE_V4,
            digest(9),
        ),
        Err(BrokerProtocolErrorV4::ZeroHostLinkOutputLength)
    );
    for mode in [0, 0o444, 0o755, u32::MAX] {
        assert_eq!(
            HostLinkCommitV4::new(
                process,
                binding_identity,
                digest(4),
                digest(5),
                digest(6),
                digest(7),
                digest(8),
                1,
                mode,
                digest(9),
            ),
            Err(BrokerProtocolErrorV4::InvalidHostLinkOutputMode { actual: mode })
        );
    }

    for (base, release, lld, field) in [
        (
            [0; 32],
            digest(2),
            digest(3),
            BrokerIdentityFieldV4::BaseBinding,
        ),
        (
            digest(1),
            [0; 32],
            digest(3),
            BrokerIdentityFieldV4::ReleaseContract,
        ),
        (
            digest(1),
            digest(2),
            [0; 32],
            BrokerIdentityFieldV4::StaticHostLld,
        ),
    ] {
        assert_eq!(
            CapabilityBindingV4::new(base, release, lld),
            Err(BrokerProtocolErrorV4::ZeroIdentity { field })
        );
    }
}

#[test]
fn every_frame_identity_constructor_rejects_zero() {
    let binding_identity = binding().identity_sha256();
    let process = process();
    for (binding_value, request, plan, closure, field) in [
        (
            [0; 32],
            digest(4),
            digest(5),
            digest(6),
            BrokerIdentityFieldV4::CapabilityBinding,
        ),
        (
            binding_identity,
            [0; 32],
            digest(5),
            digest(6),
            BrokerIdentityFieldV4::HostLinkRequest,
        ),
        (
            binding_identity,
            digest(4),
            [0; 32],
            digest(6),
            BrokerIdentityFieldV4::HostLinkPlan,
        ),
        (
            binding_identity,
            digest(4),
            digest(5),
            [0; 32],
            BrokerIdentityFieldV4::HostLinkClosure,
        ),
    ] {
        assert_eq!(
            HostLinkPrepareV4::new(process, binding_value, request, plan, closure),
            Err(BrokerProtocolErrorV4::ZeroIdentity { field })
        );
    }
    assert_eq!(
        HostLinkGrantV4::new(
            process,
            binding_identity,
            digest(4),
            digest(5),
            digest(6),
            [0; 32],
        ),
        Err(BrokerProtocolErrorV4::ZeroIdentity {
            field: BrokerIdentityFieldV4::HostLinkGrant,
        })
    );
    for (output, durable, field) in [
        ([0; 32], digest(9), BrokerIdentityFieldV4::HostLinkOutput),
        (
            digest(8),
            [0; 32],
            BrokerIdentityFieldV4::DurableHostLinkPlan,
        ),
    ] {
        assert_eq!(
            HostLinkCommitV4::new(
                process,
                binding_identity,
                digest(4),
                digest(5),
                digest(6),
                digest(7),
                output,
                1,
                HOST_LINK_OUTPUT_MODE_V4,
                durable,
            ),
            Err(BrokerProtocolErrorV4::ZeroIdentity { field })
        );
    }
}

#[test]
fn deterministic_corpus_roundtrips_and_all_bit_mutations_remain_distinct() {
    for seed in 1_u8..=64 {
        let binding = CapabilityBindingV4::new(
            patterned_digest(seed),
            patterned_digest(seed.wrapping_add(1)),
            patterned_digest(seed.wrapping_add(2)),
        )
        .unwrap();
        assert_eq!(decode_capability_binding_v4(&binding.encode()), Ok(binding));
    }

    let canonical_frames = frames(binding());
    let mut mutations = 0_usize;
    for canonical_frame in canonical_frames {
        let canonical = canonical_frame.encode();
        for byte_index in 0..canonical.len() {
            for bit in 0..8 {
                let mut mutated = canonical.clone();
                mutated[byte_index] ^= 1 << bit;
                if let Ok(decoded) = decode_broker_frame_v4(&mutated) {
                    assert_ne!(decoded, canonical_frame);
                    assert_eq!(decoded.encode(), mutated);
                }
                mutations += 1;
            }
        }
    }
    assert_eq!(mutations, 5_184);
}
