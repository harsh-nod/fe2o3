//! Focused compatibility and canonical-decoding tests for Policy V2.
#![deny(missing_docs)]

use fe2o3_build_authority::{
    CompilerClosureErrorV1, CompilerClosureV1, PipelineAllowlistV1, PipelineV1, PolicyErrorV1,
    PolicyV1, PublicationRightsV1, decode_policy_v1,
};

#[path = "../src/compiler_closure.rs"]
#[allow(dead_code)]
mod compiler_closure;
/// The pending Pipeline V2 module, compiled before its primary-crate export lands.
#[path = "../src/pipeline_v2.rs"]
pub mod pipeline_v2;
/// The pending Policy V2 module, compiled before its primary-crate export lands.
#[path = "../src/policy_v2.rs"]
pub mod policy_v2;

use compiler_closure::{
    CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1, CompilerClosureDigestFieldV2,
    CompilerClosureErrorV2, CompilerClosureV2,
};
use pipeline_v2::{PipelineAllowlistV2, PipelineErrorV2, PipelineV2};
use policy_v2::{
    AuthorityProfileV2, POLICY_IDENTITY_DOMAIN_V2, POLICY_V2_ENCODED_LEN, POLICY_V2_FIELD_COUNT,
    POLICY_V2_HEADER_LEN, POLICY_V2_MAGIC, POLICY_V2_TARGET, POLICY_V2_VERSION,
    PolicyCompatibilityErrorV2, PolicyErrorV2, PolicyV2, PublicationRightsV2, decode_policy_v2,
    policy_identity_sha256_v2,
};

const PROFILE_VALUE_OFFSET: usize = 136;
const COMPILER_IDENTITY_VALUE_OFFSET: usize = 305;
const TRAMPOLINE_VALUE_OFFSET: usize = 345;
const WRAPPER_VALUE_OFFSET: usize = 385;
const TRANSITION_PROTOCOL_VALUE_OFFSET: usize = 425;
const TARGET_VALUE_OFFSET: usize = 435;
const ALLOWLIST_VALUE_OFFSET: usize = 456;
const SELECTED_VALUE_OFFSET: usize = 468;
const RIGHTS_VALUE_OFFSET: usize = 518;
const V1_ALLOWLIST_VALUE_OFFSET: usize = 366;
const V1_SELECTED_VALUE_OFFSET: usize = 378;
const FIELD_OFFSETS: [usize; 17] = [
    32, 48, 88, 128, 137, 177, 217, 257, 297, 337, 377, 417, 427, 448, 460, 470, 510,
];
const FIELD_TAGS: [u16; 17] = [
    0x0001, 0x0002, 0x0003, 0x0004, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x0017,
    0x0020, 0x0021, 0x0022, 0x0023, 0x0024,
];
const FIELD_LENGTHS: [u32; 17] = [8, 32, 32, 1, 32, 32, 32, 32, 32, 32, 32, 2, 13, 4, 2, 32, 4];

const GOLDEN_POLICY_V1_IDENTITY: [u8; 32] = [
    0x57, 0x17, 0x71, 0x1d, 0x8f, 0x0b, 0xe6, 0x38, 0xe3, 0x83, 0x03, 0x4b, 0x0d, 0x2b, 0x70, 0x51,
    0xbb, 0xb3, 0x4a, 0x1b, 0x72, 0xa2, 0x87, 0x06, 0xae, 0x17, 0x62, 0xfa, 0x0a, 0x78, 0xb6, 0x89,
];
const GOLDEN_POLICY_V2_IDENTITY: [u8; 32] = [
    0x87, 0x36, 0x45, 0x7c, 0x08, 0xc8, 0x44, 0x3b, 0x20, 0xcd, 0xae, 0xf5, 0x68, 0xa8, 0xe0, 0xdf,
    0x57, 0x6a, 0x5a, 0x30, 0xd4, 0x85, 0x8f, 0x78, 0xcc, 0x31, 0x58, 0x4e, 0xa1, 0x3d, 0x6a, 0xbf,
];

fn golden_compiler() -> CompilerClosureV1 {
    CompilerClosureV1::new([0x05; 32], [0x06; 32], [0x07; 32], [0x08; 32]).unwrap()
}

fn golden_compiler_v2() -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [0x05; 32], [0x0a; 32], [0x02; 32], [0x06; 32], [0x07; 32], [0x08; 32],
    )
    .unwrap()
}

fn golden_policy_v1() -> PolicyV1 {
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

fn golden_policy_v2() -> PolicyV2 {
    PolicyV2::new(
        0x0102_0304_0506_0708,
        [0x01; 32],
        [0x02; 32],
        golden_compiler_v2(),
        PipelineAllowlistV2::ALL,
        PipelineV2::ProductionV1,
        [0x09; 32],
    )
    .unwrap()
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn assert_decode_and_identity_error(bytes: &[u8], expected: PolicyErrorV2) {
    assert_eq!(decode_policy_v2(bytes), Err(expected));
    assert_eq!(policy_identity_sha256_v2(bytes), Err(expected));
}

#[test]
fn production_v1_has_stable_wire_assignments_and_roundtrips() {
    assert_eq!(PipelineV2::CollectedRowSoftmax.wire_value(), 1);
    assert_eq!(PipelineV2::CollectedTiledGemm.wire_value(), 2);
    assert_eq!(PipelineV2::ProductionV1.wire_value(), 3);
    assert_eq!(PipelineAllowlistV2::ROW_SOFTMAX.bits(), 1 << 0);
    assert_eq!(PipelineAllowlistV2::TILED_GEMM.bits(), 1 << 1);
    assert_eq!(PipelineAllowlistV2::PRODUCTION_V1.bits(), 1 << 2);
    assert_eq!(PipelineAllowlistV2::POLICY_V1.bits(), 0b011);
    assert_eq!(PipelineAllowlistV2::ALL.bits(), 0b111);

    let cases = [
        (
            PipelineAllowlistV2::ROW_SOFTMAX,
            PipelineV2::CollectedRowSoftmax,
        ),
        (
            PipelineAllowlistV2::TILED_GEMM,
            PipelineV2::CollectedTiledGemm,
        ),
        (PipelineAllowlistV2::PRODUCTION_V1, PipelineV2::ProductionV1),
        (PipelineAllowlistV2::ALL, PipelineV2::CollectedRowSoftmax),
        (PipelineAllowlistV2::ALL, PipelineV2::CollectedTiledGemm),
        (PipelineAllowlistV2::ALL, PipelineV2::ProductionV1),
    ];
    for (allowlist, selected) in cases {
        let policy = PolicyV2::new(
            23,
            [1; 32],
            [2; 32],
            golden_compiler_v2(),
            allowlist,
            selected,
            [9; 32],
        )
        .unwrap();
        let bytes = policy.encode();
        assert_eq!(decode_policy_v2(&bytes), Ok(policy));
        assert_eq!(decode_policy_v2(&bytes).unwrap().encode(), bytes);
        assert!(policy.pipeline_allowlist().allows(selected));
        assert_eq!(policy.profile(), AuthorityProfileV2::StandaloneFoundation);
        assert_eq!(policy.publication_rights(), PublicationRightsV2::NONE);
    }
}

#[test]
fn v2_wire_layout_and_identity_are_versioned_and_stable() {
    let policy = golden_policy_v2();
    let bytes = policy.encode();

    assert_eq!(bytes.len(), POLICY_V2_ENCODED_LEN);
    assert_eq!(bytes[..8], POLICY_V2_MAGIC);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        POLICY_V2_VERSION
    );
    assert_eq!(
        u16::from_le_bytes(bytes[10..12].try_into().unwrap()),
        POLICY_V2_HEADER_LEN
    );
    assert_eq!(
        u16::from_le_bytes(bytes[12..14].try_into().unwrap()),
        POLICY_V2_FIELD_COUNT
    );
    assert_eq!(
        u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
        POLICY_V2_ENCODED_LEN as u32
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
    let compiler = policy.compiler_closure();
    assert_eq!(
        &bytes[COMPILER_IDENTITY_VALUE_OFFSET..COMPILER_IDENTITY_VALUE_OFFSET + 32],
        &compiler.identity_sha256()
    );
    assert_eq!(
        &bytes[TRAMPOLINE_VALUE_OFFSET..TRAMPOLINE_VALUE_OFFSET + 32],
        &compiler.cargo_binding_trampoline_sha256()
    );
    assert_eq!(
        &bytes[WRAPPER_VALUE_OFFSET..WRAPPER_VALUE_OFFSET + 32],
        &compiler.cargo_fe2o3_binding_wrapper_sha256()
    );
    assert_eq!(
        u16::from_le_bytes(
            bytes[TRANSITION_PROTOCOL_VALUE_OFFSET..TRANSITION_PROTOCOL_VALUE_OFFSET + 2]
                .try_into()
                .unwrap()
        ),
        CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1
    );
    assert_eq!(
        &bytes[TARGET_VALUE_OFFSET..ALLOWLIST_VALUE_OFFSET - 8],
        POLICY_V2_TARGET.as_bytes()
    );
    assert_eq!(
        u32::from_le_bytes(
            bytes[ALLOWLIST_VALUE_OFFSET..ALLOWLIST_VALUE_OFFSET + 4]
                .try_into()
                .unwrap()
        ),
        PipelineAllowlistV2::ALL.bits()
    );
    assert_eq!(
        u16::from_le_bytes(
            bytes[SELECTED_VALUE_OFFSET..SELECTED_VALUE_OFFSET + 2]
                .try_into()
                .unwrap()
        ),
        PipelineV2::ProductionV1.wire_value()
    );
    assert_eq!(
        POLICY_IDENTITY_DOMAIN_V2,
        b"FE2O3/PROTECTED-AUTHORITY-POLICY/V2\0"
    );
    assert_eq!(policy.identity_sha256(), GOLDEN_POLICY_V2_IDENTITY);
    assert_eq!(
        policy_identity_sha256_v2(&bytes),
        Ok(GOLDEN_POLICY_V2_IDENTITY)
    );
}

#[test]
fn v1_values_upgrade_and_downgrade_without_changing_v1() {
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
        let v1 = PolicyV1::new(
            19,
            [1; 32],
            [2; 32],
            golden_compiler(),
            allowlist,
            selected,
            [9; 32],
        )
        .unwrap();
        let original_bytes = v1.encode();
        let original_identity = v1.identity_sha256();
        let v2 = PolicyV2::from_policy_v1(v1, [0x0a; 32]).unwrap();

        assert_eq!(v2.selected_pipeline(), PipelineV2::from(selected));
        assert_eq!(
            v2.pipeline_allowlist(),
            PipelineAllowlistV2::from(allowlist)
        );
        assert_eq!(
            v2.compiler_closure().cargo_binding_trampoline_sha256(),
            [0x0a; 32]
        );
        assert_eq!(
            v2.compiler_closure().cargo_fe2o3_binding_wrapper_sha256(),
            v1.cargo_fe2o3_executable_sha256()
        );
        let restored = PolicyV1::try_from(v2).unwrap();
        assert_eq!(restored, v1);
        assert_eq!(restored.encode(), original_bytes);
        assert_eq!(restored.identity_sha256(), original_identity);
    }

    let golden_v1 = golden_policy_v1();
    assert_eq!(golden_v1.identity_sha256(), GOLDEN_POLICY_V1_IDENTITY);
    assert_eq!(decode_policy_v1(&golden_v1.encode()), Ok(golden_v1));
    assert_eq!(golden_v1.publication_rights(), PublicationRightsV1::NONE);
}

#[test]
fn production_v1_cannot_be_silently_downgraded() {
    assert_eq!(
        PipelineV1::try_from(PipelineV2::ProductionV1),
        Err(PipelineErrorV2::PipelineNotRepresentableInV1 {
            pipeline: PipelineV2::ProductionV1,
        })
    );
    assert_eq!(
        PipelineAllowlistV1::try_from(PipelineAllowlistV2::ALL),
        Err(PipelineErrorV2::AllowlistNotRepresentableInV1 { bits: 0b111 })
    );
    assert_eq!(
        PolicyV1::try_from(golden_policy_v2()),
        Err(PolicyCompatibilityErrorV2::InvalidPipeline(
            PipelineErrorV2::AllowlistNotRepresentableInV1 { bits: 0b111 }
        ))
    );
}

#[test]
fn unknown_pipeline_values_fail_closed() {
    for value in [0, 4, u16::MAX] {
        assert_eq!(
            PipelineV2::try_from(value),
            Err(PipelineErrorV2::UnknownPipeline { value })
        );
        let mut bytes = golden_policy_v2().encode();
        set_u16(&mut bytes, SELECTED_VALUE_OFFSET, value);
        assert_decode_and_identity_error(
            &bytes,
            PolicyErrorV2::InvalidPipeline(PipelineErrorV2::UnknownPipeline { value }),
        );
    }

    for bits in [1 << 3, 0b1001, u32::MAX] {
        assert_eq!(
            PipelineAllowlistV2::from_bits(bits),
            Err(PipelineErrorV2::UnknownPipelineAllowlistBits { bits })
        );
        let mut bytes = golden_policy_v2().encode();
        set_u32(&mut bytes, ALLOWLIST_VALUE_OFFSET, bits);
        assert_decode_and_identity_error(
            &bytes,
            PolicyErrorV2::InvalidPipeline(PipelineErrorV2::UnknownPipelineAllowlistBits { bits }),
        );
    }

    let mut not_allowed = golden_policy_v2().encode();
    set_u32(&mut not_allowed, ALLOWLIST_VALUE_OFFSET, 0b011);
    assert_decode_and_identity_error(
        &not_allowed,
        PolicyErrorV2::SelectedPipelineNotAllowed {
            selected: PipelineV2::ProductionV1,
            allowlist_bits: 0b011,
        },
    );
}

#[test]
fn unknown_profile_rights_and_noncanonical_frames_fail_closed() {
    let mut bad_magic = golden_policy_v2().encode();
    bad_magic[0] ^= 1;
    assert_decode_and_identity_error(&bad_magic, PolicyErrorV2::InvalidMagic);

    let mut unknown_version = golden_policy_v2().encode();
    set_u16(&mut unknown_version, 8, 3);
    assert_decode_and_identity_error(
        &unknown_version,
        PolicyErrorV2::UnsupportedVersion { actual: 3 },
    );

    let mut unknown_transition_protocol = golden_policy_v2().encode();
    set_u16(
        &mut unknown_transition_protocol,
        TRANSITION_PROTOCOL_VALUE_OFFSET,
        2,
    );
    assert_decode_and_identity_error(
        &unknown_transition_protocol,
        PolicyErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { version: 2 },
        ),
    );

    let mut mismatched_compiler_identity = golden_policy_v2().encode();
    mismatched_compiler_identity[COMPILER_IDENTITY_VALUE_OFFSET] ^= 1;
    assert_decode_and_identity_error(
        &mismatched_compiler_identity,
        PolicyErrorV2::InvalidCompilerClosure(CompilerClosureErrorV2::IdentityMismatch),
    );

    let mut reserved_profile = golden_policy_v2().encode();
    reserved_profile[PROFILE_VALUE_OFFSET] = 1;
    assert_decode_and_identity_error(
        &reserved_profile,
        PolicyErrorV2::ProfileNotPermitted { value: 1 },
    );

    let mut unknown_profile = golden_policy_v2().encode();
    unknown_profile[PROFILE_VALUE_OFFSET] = 2;
    assert_decode_and_identity_error(&unknown_profile, PolicyErrorV2::UnknownProfile { value: 2 });

    let mut invalid_target = golden_policy_v2().encode();
    invalid_target[TARGET_VALUE_OFFSET] ^= 1;
    assert_decode_and_identity_error(&invalid_target, PolicyErrorV2::InvalidTarget);

    let mut known_right = golden_policy_v2().encode();
    set_u32(&mut known_right, RIGHTS_VALUE_OFFSET, 1);
    assert_decode_and_identity_error(
        &known_right,
        PolicyErrorV2::PublicationRightsNotPermitted { bits: 1 },
    );

    for bits in [2, 3, u32::MAX] {
        let mut unknown_rights = golden_policy_v2().encode();
        set_u32(&mut unknown_rights, RIGHTS_VALUE_OFFSET, bits);
        assert_decode_and_identity_error(
            &unknown_rights,
            PolicyErrorV2::UnknownPublicationRightsBits { bits },
        );
    }

    let mut bad_tag = golden_policy_v2().encode();
    set_u16(&mut bad_tag, 32, 0xffff);
    assert_decode_and_identity_error(
        &bad_tag,
        PolicyErrorV2::UnexpectedFieldTag {
            index: 0,
            expected: 0x0001,
            actual: 0xffff,
        },
    );

    let mut bad_flags = golden_policy_v2().encode();
    set_u16(&mut bad_flags, 34, 1);
    assert_decode_and_identity_error(
        &bad_flags,
        PolicyErrorV2::UnsupportedFieldFlags {
            tag: 0x0001,
            actual: 1,
        },
    );

    let mut trailing = golden_policy_v2().encode().to_vec();
    trailing.push(0);
    assert_decode_and_identity_error(
        &trailing,
        PolicyErrorV2::InvalidEncodedLength {
            actual: POLICY_V2_ENCODED_LEN + 1,
        },
    );
}

#[test]
fn policy_versions_do_not_cross_decode() {
    let v1_bytes = golden_policy_v1().encode();
    let v2_bytes = golden_policy_v2().encode();
    assert_eq!(
        decode_policy_v2(&v1_bytes),
        Err(PolicyErrorV2::InvalidEncodedLength { actual: 432 })
    );
    assert_eq!(
        decode_policy_v1(&v2_bytes),
        Err(PolicyErrorV1::InvalidEncodedLength {
            actual: POLICY_V2_ENCODED_LEN,
        })
    );

    let mut v1_unknown_allowlist = v1_bytes;
    set_u32(&mut v1_unknown_allowlist, V1_ALLOWLIST_VALUE_OFFSET, 1 << 2);
    assert_eq!(
        decode_policy_v1(&v1_unknown_allowlist),
        Err(PolicyErrorV1::UnknownPipelineAllowlistBits { bits: 1 << 2 })
    );

    let mut v1_unknown_selected = golden_policy_v1().encode();
    set_u16(&mut v1_unknown_selected, V1_SELECTED_VALUE_OFFSET, 3);
    assert_eq!(
        decode_policy_v1(&v1_unknown_selected),
        Err(PolicyErrorV1::UnknownPipeline { value: 3 })
    );
}

#[test]
fn invalid_construction_remains_fail_closed() {
    assert_eq!(
        PolicyV2::from_policy_v1(golden_policy_v1(), [0; 32]),
        Err(PolicyErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::ZeroDigest {
                field: CompilerClosureDigestFieldV2::CargoBindingTrampoline,
            }
        ))
    );

    assert_eq!(
        PolicyV2::new(
            1,
            [1; 32],
            [2; 32],
            golden_compiler_v2(),
            PipelineAllowlistV2::ROW_SOFTMAX,
            PipelineV2::ProductionV1,
            [9; 32],
        ),
        Err(PolicyErrorV2::SelectedPipelineNotAllowed {
            selected: PipelineV2::ProductionV1,
            allowlist_bits: 1,
        })
    );
    assert_eq!(
        CompilerClosureV1::from_pins_and_identity([5; 32], [6; 32], [7; 32], [8; 32], [9; 32]),
        Err(CompilerClosureErrorV1::IdentityMismatch)
    );

    let mismatched_wrapper =
        CompilerClosureV2::new([5; 32], [10; 32], [3; 32], [6; 32], [7; 32], [8; 32]).unwrap();
    assert_eq!(
        PolicyV2::new(
            1,
            [1; 32],
            [2; 32],
            mismatched_wrapper,
            PipelineAllowlistV2::PRODUCTION_V1,
            PipelineV2::ProductionV1,
            [9; 32],
        ),
        Err(PolicyErrorV2::CargoFe2o3BindingWrapperMismatch)
    );

    let repeated_transition_image =
        CompilerClosureV2::new([5; 32], [5; 32], [2; 32], [6; 32], [7; 32], [8; 32]).unwrap();
    assert_eq!(
        PolicyV2::new(
            1,
            [1; 32],
            [2; 32],
            repeated_transition_image,
            PipelineAllowlistV2::PRODUCTION_V1,
            PipelineV2::ProductionV1,
            [9; 32],
        ),
        Err(PolicyErrorV2::CargoTransitionImageDigestsNotDistinct)
    );
}
