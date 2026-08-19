#![forbid(unsafe_code)]

//! Hostile canonical-wire mutation conformance tests.

use fe2o3_llvm_handoff::{
    DecodeHandoffErrorV1, Gfx942HandoffV1, HandoffDiagnosticV1, WireSectionV1,
};
use fe2o3_llvm_route_conformance::{
    ConformanceExpectationV1, ConformanceSemanticV1, ExpectedRejectionV1,
    GFX942_CONFORMANCE_CORPUS_V1, conformance_case_v1, gfx942_fixture_v1,
};

const EXERCISED_REJECTIONS: [ExpectedRejectionV1; 18] = [
    ExpectedRejectionV1::UnknownTargetFeature,
    ExpectedRejectionV1::ConflictingTargetFeature,
    ExpectedRejectionV1::UnsupportedTargetFeatureState,
    ExpectedRejectionV1::UnknownCallingConvention,
    ExpectedRejectionV1::UnknownAddressSpace,
    ExpectedRejectionV1::ZeroAlignment,
    ExpectedRejectionV1::NonPowerOfTwoAlignment,
    ExpectedRejectionV1::OversizedAlignment,
    ExpectedRejectionV1::UnknownModuleFlag,
    ExpectedRejectionV1::UnknownNamedMetadata,
    ExpectedRejectionV1::DuplicateModuleFlag,
    ExpectedRejectionV1::UnknownOriginKind,
    ExpectedRejectionV1::NonCanonicalOriginIdentity,
    ExpectedRejectionV1::UnknownObligationKind,
    ExpectedRejectionV1::NonCanonicalObligationIdentity,
    ExpectedRejectionV1::UnknownDeviceLibraryKind,
    ExpectedRejectionV1::DuplicateDeviceLibraryKind,
    ExpectedRejectionV1::ZeroDeviceLibraryIdentity,
];

#[test]
fn target_feature_mutations_fail_with_specific_diagnostics() {
    let (canonical, offsets) = fixture_wire();

    let mut unknown = canonical.clone();
    unknown[offsets.target_feature_tags[0]] = 0xff;
    assert_rejection(
        "target-features.unknown-tag",
        ExpectedRejectionV1::UnknownTargetFeature,
        &unknown,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::TargetFeature,
            tag: 0xff,
        },
    );

    let mut conflicting = canonical.clone();
    conflicting[offsets.target_feature_tags[1]] = canonical[offsets.target_feature_tags[0]];
    assert_rejection(
        "target-features.conflicting-state",
        ExpectedRejectionV1::ConflictingTargetFeature,
        &conflicting,
        DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::ConflictingTargetFeature),
    );

    let mut unsupported = canonical;
    assert_eq!(unsupported[offsets.target_feature_states[0]], 0);
    unsupported[offsets.target_feature_states[0]] = 1;
    assert_rejection(
        "target-features.unsupported-state",
        ExpectedRejectionV1::UnsupportedTargetFeatureState,
        &unsupported,
        DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::UnsupportedTargetPolicy),
    );
}

#[test]
fn calling_convention_and_address_space_unknown_tags_fail_closed() {
    let (canonical, offsets) = fixture_wire();

    let mut convention = canonical.clone();
    convention[offsets.calling_convention] = 0xff;
    assert_rejection(
        "calling-convention.unknown-tag",
        ExpectedRejectionV1::UnknownCallingConvention,
        &convention,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::CallingConvention,
            tag: 0xff,
        },
    );

    let mut address_space = canonical;
    address_space[offsets.address_space] = 0xff;
    assert_rejection(
        "address-space.unknown-tag",
        ExpectedRejectionV1::UnknownAddressSpace,
        &address_space,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::AddressSpace,
            tag: 0xff,
        },
    );
}

#[test]
fn hostile_alignments_return_the_bounded_parameter_diagnostic() {
    let (canonical, offsets) = fixture_wire();
    let cases = [
        ("alignment.zero", ExpectedRejectionV1::ZeroAlignment, 0_u16),
        (
            "alignment.non-power-of-two",
            ExpectedRejectionV1::NonPowerOfTwoAlignment,
            3_u16,
        ),
        (
            "alignment.over-maximum",
            ExpectedRejectionV1::OversizedAlignment,
            512_u16,
        ),
    ];

    for (name, rejection, value) in cases {
        let mut hostile = canonical.clone();
        hostile[offsets.alignment..offsets.alignment + 2].copy_from_slice(&value.to_le_bytes());
        assert_rejection(
            name,
            rejection,
            &hostile,
            DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::InvalidParameterAttribute(
                "align",
            )),
        );
    }
}

#[test]
fn module_metadata_mutations_are_typed_and_specific() {
    let (canonical, offsets) = fixture_wire();

    let mut unknown_flag = canonical.clone();
    unknown_flag[offsets.module_flags[0]] = 0xff;
    assert_rejection(
        "module-metadata.unknown-flag",
        ExpectedRejectionV1::UnknownModuleFlag,
        &unknown_flag,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::ModuleFlag,
            tag: 0xff,
        },
    );

    let mut unknown_named = canonical.clone();
    unknown_named[offsets.named_metadata[0]] = 0xff;
    assert_rejection(
        "module-metadata.unknown-named",
        ExpectedRejectionV1::UnknownNamedMetadata,
        &unknown_named,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::NamedMetadata,
            tag: 0xff,
        },
    );

    let mut duplicate_flag = canonical;
    duplicate_flag[offsets.module_flags[2]] = duplicate_flag[offsets.module_flags[1]];
    assert_rejection(
        "module-metadata.duplicate-flag",
        ExpectedRejectionV1::DuplicateModuleFlag,
        &duplicate_flag,
        DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::DuplicateModuleFlag("PIC Level=2")),
    );
}

#[test]
fn origin_and_obligation_mutations_cannot_preserve_canonical_identity() {
    let (canonical, offsets) = fixture_wire();

    let mut origin_kind = canonical.clone();
    origin_kind[offsets.origin_kind] = 0xff;
    assert_rejection(
        "origin.unknown-kind",
        ExpectedRejectionV1::UnknownOriginKind,
        &origin_kind,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::Origin,
            tag: 0xff,
        },
    );

    let mut origin_identity = canonical.clone();
    origin_identity[offsets.origin_identity] ^= 1;
    assert_rejection(
        "origin.identity-mutation",
        ExpectedRejectionV1::NonCanonicalOriginIdentity,
        &origin_identity,
        DecodeHandoffErrorV1::NonCanonical,
    );

    let mut obligation_kind = canonical.clone();
    obligation_kind[offsets.obligation_kind] = 0xff;
    assert_rejection(
        "obligation.unknown-kind",
        ExpectedRejectionV1::UnknownObligationKind,
        &obligation_kind,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::Obligation,
            tag: 0xff,
        },
    );

    let mut obligation_identity = canonical;
    obligation_identity[offsets.obligation_identity] ^= 1;
    assert_rejection(
        "obligation.identity-mutation",
        ExpectedRejectionV1::NonCanonicalObligationIdentity,
        &obligation_identity,
        DecodeHandoffErrorV1::NonCanonical,
    );
}

#[test]
fn device_library_kind_and_zero_identity_mutations_fail_closed() {
    let (canonical, offsets) = fixture_wire();

    let mut unknown_kind = canonical.clone();
    unknown_kind[offsets.device_library_kinds[0]] = 0xff;
    assert_rejection(
        "device-library.unknown-kind",
        ExpectedRejectionV1::UnknownDeviceLibraryKind,
        &unknown_kind,
        DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::DeviceLibrary,
            tag: 0xff,
        },
    );

    let mut duplicate_kind = canonical.clone();
    duplicate_kind[offsets.device_library_kinds[1]] =
        duplicate_kind[offsets.device_library_kinds[0]];
    assert_rejection(
        "device-library.duplicate-kind",
        ExpectedRejectionV1::DuplicateDeviceLibraryKind,
        &duplicate_kind,
        DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::DuplicateDeviceLibrary("ocml")),
    );

    let mut zero_identity = canonical;
    zero_identity[offsets.device_library_sha..offsets.device_library_sha + 32].fill(0);
    assert_rejection(
        "device-library.zero-identity",
        ExpectedRejectionV1::ZeroDeviceLibraryIdentity,
        &zero_identity,
        DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::ZeroIdentity(
            "device-library SHA-256",
        )),
    );
}

#[test]
fn nonzero_device_library_digest_mutation_creates_a_new_canonical_handoff_identity() {
    let case = conformance_case_v1("device-library.identity-mutation-reidentifies-handoff")
        .expect("represented device-library identity case must exist");
    assert_eq!(case.expectation(), ConformanceExpectationV1::Represented);

    let baseline = gfx942_fixture_v1().expect("fixture must remain valid");
    let (mut hostile, offsets) = fixture_wire();
    hostile[offsets.device_library_sha] ^= 1;
    let changed = Gfx942HandoffV1::decode_canonical(&hostile)
        .expect("a nonzero digest is a different canonical handoff input");

    assert_ne!(changed, baseline);
    assert_ne!(changed.identity(), baseline.identity());
    assert_eq!(changed.encode_canonical().as_bytes(), hostile);
}

#[test]
fn every_declared_expected_rejection_has_an_exercised_hostile_case() {
    let declared = GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .filter(|case| case.semantic() != ConformanceSemanticV1::WorkerAdmissionLane)
        .filter_map(|case| match case.expectation() {
            ConformanceExpectationV1::ExpectedRejection(rejection) => Some(rejection),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, EXERCISED_REJECTIONS);
}

fn assert_rejection(
    name: &str,
    rejection: ExpectedRejectionV1,
    bytes: &[u8],
    expected: DecodeHandoffErrorV1,
) {
    let case = conformance_case_v1(name).expect("hostile case must be declared");
    assert_eq!(
        case.expectation(),
        ConformanceExpectationV1::ExpectedRejection(rejection)
    );
    assert_eq!(Gfx942HandoffV1::decode_canonical(bytes), Err(expected));
}

fn fixture_wire() -> (Vec<u8>, WireOffsets) {
    let bytes = gfx942_fixture_v1()
        .expect("fixture must remain valid")
        .encode_canonical()
        .as_bytes()
        .to_vec();
    let offsets = WireOffsets::locate(&bytes);
    (bytes, offsets)
}

#[derive(Debug)]
struct WireOffsets {
    target_feature_tags: Vec<usize>,
    target_feature_states: Vec<usize>,
    calling_convention: usize,
    address_space: usize,
    alignment: usize,
    module_flags: Vec<usize>,
    named_metadata: Vec<usize>,
    device_library_kinds: Vec<usize>,
    device_library_sha: usize,
    origin_identity: usize,
    origin_kind: usize,
    obligation_identity: usize,
    obligation_kind: usize,
}

impl WireOffsets {
    fn locate(bytes: &[u8]) -> Self {
        let mut cursor = Cursor::new(bytes, 16);
        cursor.take(3);
        let feature_count = cursor.u8() as usize;
        let mut target_feature_tags = Vec::with_capacity(feature_count);
        let mut target_feature_states = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            target_feature_tags.push(cursor.take(1));
            target_feature_states.push(cursor.take(1));
        }
        cursor.take(4 + 32 * 3);

        let kernel_count = cursor.u16() as usize;
        let mut calling_convention = None;
        let mut address_space = None;
        let mut alignment = None;
        for _ in 0..kernel_count {
            cursor.string();
            cursor.take(32);
            calling_convention.get_or_insert(cursor.take(1));
            cursor.take(1);
            let parameter_count = cursor.u16() as usize;
            for _ in 0..parameter_count {
                cursor.string();
                match cursor.u8() {
                    1 => {
                        cursor.take(1);
                    }
                    2 => {
                        cursor.take(1);
                        address_space.get_or_insert(cursor.take(1));
                    }
                    tag => panic!("fixture has unexpected value-type tag {tag}"),
                }
                let attribute_count = cursor.u8() as usize;
                for _ in 0..attribute_count {
                    match cursor.u8() {
                        6 => {
                            alignment.get_or_insert(cursor.take(2));
                        }
                        7 => {
                            cursor.take(4);
                        }
                        1..=5 => {}
                        tag => panic!("fixture has unexpected parameter-attribute tag {tag}"),
                    }
                }
            }
            let function_attribute_count = cursor.u8() as usize;
            for _ in 0..function_attribute_count {
                match cursor.u8() {
                    2 => {
                        cursor.take(4);
                    }
                    3 => {
                        cursor.take(2);
                    }
                    1 | 4..=10 => {}
                    tag => panic!("fixture has unexpected function-attribute tag {tag}"),
                }
            }
        }

        let flag_count = cursor.u8() as usize;
        let module_flags = (0..flag_count).map(|_| cursor.take(1)).collect();
        let named_count = cursor.u8() as usize;
        let mut named_metadata = Vec::with_capacity(named_count);
        for _ in 0..named_count {
            let position = cursor.take(1);
            named_metadata.push(position);
            if bytes[position] == 3 {
                cursor.take(32);
            }
        }
        let library_count = cursor.u8() as usize;
        let mut device_library_kinds = Vec::with_capacity(library_count);
        let mut device_library_sha = None;
        for _ in 0..library_count {
            device_library_kinds.push(cursor.take(1));
            device_library_sha.get_or_insert(cursor.take(32));
            cursor.take(8);
        }

        let origin_count = cursor.u16() as usize;
        let mut origin_identity = None;
        let mut origin_kind = None;
        for _ in 0..origin_count {
            origin_identity.get_or_insert(cursor.take(32));
            origin_kind.get_or_insert(cursor.take(1));
            cursor.take(32);
            match cursor.u8() {
                0 => {}
                1 => {
                    cursor.string();
                    cursor.take(16);
                }
                tag => panic!("fixture has unexpected source-span tag {tag}"),
            }
        }

        let obligation_count = cursor.u16() as usize;
        let mut obligation_identity = None;
        let mut obligation_kind = None;
        for _ in 0..obligation_count {
            obligation_identity.get_or_insert(cursor.take(32));
            obligation_kind.get_or_insert(cursor.take(1));
            cursor.take(64);
        }
        assert_eq!(cursor.offset, bytes.len());

        Self {
            target_feature_tags,
            target_feature_states,
            calling_convention: calling_convention.expect("fixture has a kernel"),
            address_space: address_space.expect("fixture has a pointer"),
            alignment: alignment.expect("fixture has an alignment"),
            module_flags,
            named_metadata,
            device_library_kinds,
            device_library_sha: device_library_sha.expect("fixture has a device library"),
            origin_identity: origin_identity.expect("fixture has an origin"),
            origin_kind: origin_kind.expect("fixture has an origin kind"),
            obligation_identity: obligation_identity.expect("fixture has an obligation"),
            obligation_kind: obligation_kind.expect("fixture has an obligation kind"),
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    fn take(&mut self, count: usize) -> usize {
        let start = self.offset;
        self.offset += count;
        assert!(self.offset <= self.bytes.len());
        start
    }

    fn u8(&mut self) -> u8 {
        let position = self.take(1);
        self.bytes[position]
    }

    fn u16(&mut self) -> u16 {
        let position = self.take(2);
        u16::from_le_bytes([self.bytes[position], self.bytes[position + 1]])
    }

    fn string(&mut self) {
        let length = self.u16() as usize;
        self.take(length);
    }
}
