use super::*;
use crate::{
    MultiRootTargetBindingInputsV2, MultiRootTargetBindingTranscriptV2,
    MultiRootTargetWorkgroupInputV2,
};
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const LLVM_TARGET: &str = "amdgcn-amd-amdhsa";
const CPU: &str = "gfx942";
const FEATURES: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";

fn identity(seed: u8, length: u64) -> TargetLineageIdentityV3 {
    TargetLineageIdentityV3::new([seed; 32], length).unwrap()
}
fn subject() -> InertNativeNeutralSubjectV1 {
    InertNativeNeutralSubjectV1::new([3; 32], 303, [6; 32], 606).unwrap()
}
fn rows() -> [MultiRootTargetWorkgroupInputV3<'static>; 2] {
    [
        MultiRootTargetWorkgroupInputV3 {
            kernel: "crate::alpha",
            workgroup: [64, 1, 1],
        },
        MultiRootTargetWorkgroupInputV3 {
            kernel: "crate::bravo",
            workgroup: [128, 1, 1],
        },
    ]
}
fn inputs<'a>(
    workgroups: &'a [MultiRootTargetWorkgroupInputV3<'a>],
) -> MultiRootTargetBindingInputsV3<'a> {
    MultiRootTargetBindingInputsV3 {
        protected_rustc_invocation: identity(1, 101),
        semantic_mir: identity(2, 202),
        native_neutral_subject: subject(),
        target_bound_kir: identity(4, 404),
        configured_target: TARGET,
        rustc_llvm_target: LLVM_TARGET,
        target_cpu: CPU,
        target_features: FEATURES,
        roster_identity: [5; 32],
        code_object_version: 6,
        wave_width_bits: 64,
        workgroups,
    }
}

#[test]
fn native_roundtrip_preserves_full_subject_and_singleton_or_semantic_row_order() {
    let rows = rows();
    for count in [1, 2] {
        let encoded = MultiRootTargetBindingTranscriptV3::new(inputs(&rows[..count])).unwrap();
        let decoded =
            MultiRootTargetBindingTranscriptV3::decode(encoded.canonical_bytes()).unwrap();
        assert_eq!(encoded, decoded);
        assert_eq!(decoded.native_neutral_subject(), &subject());
        assert_eq!(decoded.root_count(), count);
        // V2's two-root fixture is 364 bytes; replacing one identity by the
        // 96-byte subject adds 56. Each omitted row here removes 4+12+12 bytes.
        assert_eq!(decoded.canonical_bytes().len(), 420 - (2 - count) * 28);
        assert_eq!(
            &decoded.canonical_bytes()[96..192],
            subject().canonical_bytes()
        );
        for (index, row) in rows[..count].iter().enumerate() {
            assert_eq!(decoded.workgroup(index).unwrap().kernel(), row.kernel);
            assert_eq!(decoded.workgroup(index).unwrap().workgroup(), row.workgroup);
        }
        assert!(decoded.workgroup(count).is_none());
        assert_eq!(decoded.protected_rustc_invocation(), identity(1, 101));
        assert_eq!(decoded.semantic_mir(), identity(2, 202));
        assert_eq!(decoded.target_bound_kir(), identity(4, 404));
        assert_eq!(decoded.roster_identity(), [5; 32]);
        assert_eq!(decoded.configured_target(), TARGET);
        assert_eq!(decoded.rustc_llvm_target(), LLVM_TARGET);
        assert_eq!(decoded.target_cpu(), CPU);
        assert_eq!(decoded.target_features(), FEATURES);
        assert_eq!(decoded.code_object_version(), 6);
        assert_eq!(decoded.wave_width_bits(), 64);
        assert!(!decoded.establishes_refinement_proof());
    }
    let reversed = [rows[1], rows[0]];
    let record = MultiRootTargetBindingTranscriptV3::new(inputs(&reversed)).unwrap();
    assert_eq!(record.workgroup(0).unwrap().kernel(), "crate::bravo");
    assert_eq!(record.workgroup(1).unwrap().kernel(), "crate::alpha");
}

#[test]
fn legacy_v2_bytes_remain_frozen_and_cannot_be_retagged_as_native() {
    let rows = [
        MultiRootTargetWorkgroupInputV2 {
            kernel: "crate::alpha",
            workgroup: [64, 1, 1],
        },
        MultiRootTargetWorkgroupInputV2 {
            kernel: "crate::bravo",
            workgroup: [128, 1, 1],
        },
    ];
    let legacy_inputs = MultiRootTargetBindingInputsV2 {
        protected_rustc_invocation: identity(1, 101),
        semantic_mir: identity(2, 202),
        target_neutral_kir: identity(3, 303),
        target_bound_kir: identity(4, 404),
        configured_target: TARGET,
        rustc_llvm_target: LLVM_TARGET,
        target_cpu: CPU,
        target_features: FEATURES,
        roster_identity: [5; 32],
        code_object_version: 6,
        wave_width_bits: 64,
        workgroups: &rows,
    };
    let legacy = MultiRootTargetBindingTranscriptV2::new(legacy_inputs).unwrap();
    assert_eq!(legacy.canonical_bytes().len(), 364);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(legacy.canonical_bytes())),
        [
            0xaf, 0xa6, 0xe7, 0xc4, 0x42, 0x8c, 0x7b, 0xbe, 0x6f, 0xe5, 0xf5, 0xd6, 0x14, 0xc0,
            0x14, 0xb0, 0x84, 0xeb, 0x5a, 0x64, 0x69, 0xaf, 0x46, 0x0a, 0x16, 0x37, 0x7f, 0x29,
            0x22, 0xa3, 0xa2, 0x19,
        ]
    );
    assert_eq!(
        MultiRootTargetBindingTranscriptV2::decode(legacy.canonical_bytes()).unwrap(),
        legacy
    );
    assert!(
        MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
            workgroups: &rows[..1],
            ..legacy_inputs
        })
        .is_err()
    );
    assert!(MultiRootTargetBindingTranscriptV3::decode(legacy.canonical_bytes()).is_err());
    let mut retagged = legacy.canonical_bytes().to_vec();
    retagged[..8].copy_from_slice(&MULTI_ROOT_TARGET_BINDING_MAGIC_V3);
    retagged[8..10].copy_from_slice(&MULTI_ROOT_TARGET_BINDING_VERSION_V3.to_le_bytes());
    assert!(MultiRootTargetBindingTranscriptV3::decode(&retagged).is_err());
    let native_rows = [MultiRootTargetWorkgroupInputV3 {
        kernel: "native",
        workgroup: [64, 1, 1],
    }];
    let native = MultiRootTargetBindingTranscriptV3::new(inputs(&native_rows)).unwrap();
    assert!(MultiRootTargetBindingTranscriptV2::decode(native.canonical_bytes()).is_err());
}

#[test]
fn native_decoder_rejects_every_prefix_headers_reserved_subject_and_trailing_bytes() {
    let rows = rows();
    let record = MultiRootTargetBindingTranscriptV3::new(inputs(&rows)).unwrap();
    let bytes = record.canonical_bytes();
    for end in 0..bytes.len() {
        assert!(MultiRootTargetBindingTranscriptV3::decode(&bytes[..end]).is_err());
    }
    for offset in (0..16).chain(96..112) {
        let mut changed = bytes.to_vec();
        changed[offset] ^= 1;
        assert!(
            MultiRootTargetBindingTranscriptV3::decode(&changed).is_err(),
            "offset {offset}"
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    let length = u32::try_from(trailing.len()).unwrap();
    trailing[12..16].copy_from_slice(&length.to_le_bytes());
    assert!(MultiRootTargetBindingTranscriptV3::decode(&trailing).is_err());
    for (start, end) in [(112, 120), (120, 152), (152, 160), (160, 192), (232, 264)] {
        let mut zero = bytes.to_vec();
        zero[start..end].fill(0);
        assert!(MultiRootTargetBindingTranscriptV3::decode(&zero).is_err());
    }
}

#[test]
fn native_validation_rejects_row_geometry_profile_and_identity_crosses() {
    let rows = rows();
    let valid = inputs(&rows);
    assert!(MultiRootTargetBindingTranscriptV3::new(inputs(&[])).is_err());
    for changed in [
        MultiRootTargetBindingInputsV3 {
            target_cpu: "gfx950",
            ..valid
        },
        MultiRootTargetBindingInputsV3 {
            target_features: "+xnack",
            ..valid
        },
        MultiRootTargetBindingInputsV3 {
            code_object_version: 5,
            ..valid
        },
        MultiRootTargetBindingInputsV3 {
            wave_width_bits: 32,
            ..valid
        },
        MultiRootTargetBindingInputsV3 {
            target_bound_kir: identity(3, 303),
            ..valid
        },
        MultiRootTargetBindingInputsV3 {
            roster_identity: [0; 32],
            ..valid
        },
    ] {
        assert!(MultiRootTargetBindingTranscriptV3::new(changed).is_err());
    }
    let duplicate = [rows[0], rows[0]];
    assert!(MultiRootTargetBindingTranscriptV3::new(inputs(&duplicate)).is_err());
    let zero = [MultiRootTargetWorkgroupInputV3 {
        kernel: "zero",
        workgroup: [64, 0, 1],
    }];
    assert!(MultiRootTargetBindingTranscriptV3::new(inputs(&zero)).is_err());
    let other_catalog = InertNativeNeutralSubjectV1::new([3; 32], 303, [7; 32], 606).unwrap();
    let first = MultiRootTargetBindingTranscriptV3::new(valid).unwrap();
    let second = MultiRootTargetBindingTranscriptV3::new(MultiRootTargetBindingInputsV3 {
        native_neutral_subject: other_catalog,
        ..valid
    })
    .unwrap();
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
    assert_ne!(
        first.native_neutral_subject(),
        second.native_neutral_subject()
    );
}
