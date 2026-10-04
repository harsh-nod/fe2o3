use super::*;

fn subject() -> InertNativeNeutralSubjectV1 {
    InertNativeNeutralSubjectV1::new([0x31; 32], 101, [0x32; 32], 56).unwrap()
}
fn roots() -> [MultiRootProofRosterRootInputV3<'static>; 2] {
    [
        MultiRootProofRosterRootInputV3 {
            semantic_root: 5,
            semantic_root_identity: [0x21; 32],
            kernel_binding: [0x72; 32],
            source_rank: 1,
            workgroup: [64, 1, 1],
            logical_name: "first source root",
            export_symbol: "zeta",
            kernel_id: "zeta",
            payload: b"first-payload",
        },
        MultiRootProofRosterRootInputV3 {
            semantic_root: 17,
            semantic_root_identity: [0x22; 32],
            kernel_binding: [0x71; 32],
            source_rank: 2,
            workgroup: [8, 8, 1],
            logical_name: "second source root",
            export_symbol: "alpha",
            kernel_id: "alpha",
            payload: b"second-payload",
        },
    ]
}
fn inputs<'a>(
    roots: &'a [MultiRootProofRosterRootInputV3<'a>],
    order: &'a [u32],
) -> MultiRootProofRosterInputsV3<'a> {
    MultiRootProofRosterInputsV3 {
        kind: MultiRootProofRosterKindV3::Correspondence,
        semantic_mir_sha256: [0x11; 32],
        native_neutral_subject: subject(),
        roster_identity: [0x41; 32],
        canonical_kernel_order: order,
        roots,
    }
}

#[test]
fn native_all_four_kinds_roundtrip_singletons_and_multiple_explicit_root_axes() {
    let roots = roots();
    for kind in [
        MultiRootProofRosterKindV3::MiddleEnd,
        MultiRootProofRosterKindV3::Correspondence,
        MultiRootProofRosterKindV3::FormalMemory,
        MultiRootProofRosterKindV3::VerusExecution,
    ] {
        for (selected, order) in [(&roots[..1], &[0][..]), (&roots[..], &[1, 0][..])] {
            let mut input = inputs(selected, order);
            input.kind = kind;
            let framed = MultiRootProofRosterTranscriptV3::new(input).unwrap();
            let decoded =
                MultiRootProofRosterTranscriptV3::decode(framed.canonical_bytes()).unwrap();
            assert_eq!(decoded.canonical_bytes(), framed.canonical_bytes());
            assert_eq!(decoded.kind(), kind);
            assert_eq!(decoded.native_neutral_subject(), &subject());
            assert_eq!(decoded.semantic_mir_sha256(), [0x11; 32]);
            assert_eq!(decoded.roster_identity(), [0x41; 32]);
            assert_eq!(decoded.canonical_kernel_order(), order);
            assert_eq!(decoded.root_count(), selected.len());
            for (ordinal, expected) in selected.iter().enumerate() {
                let actual = decoded.root(ordinal).unwrap();
                assert_eq!(actual.semantic_root(), expected.semantic_root);
                assert_eq!(actual.kernel_binding(), expected.kernel_binding);
                assert_eq!(actual.payload(), expected.payload);
                assert_eq!(actual.source_rank(), expected.source_rank);
                assert_eq!(actual.workgroup(), expected.workgroup);
                assert_eq!(actual.kernel_id(), expected.kernel_id);
            }
            assert!(decoded.root(selected.len()).is_none());
            assert!(!decoded.establishes_compiler_refinement());
        }
    }
}

#[test]
fn native_and_legacy_rosters_do_not_accept_each_others_headers_or_singleton_policy() {
    let roots = roots();
    let legacy = MultiRootProofRosterTranscriptV2::new(MultiRootProofRosterInputsV2 {
        kind: MultiRootProofRosterKindV2::Correspondence,
        semantic_mir_sha256: [0x11; 32],
        neutral_kir: MultiRootNeutralKirIdentityV2::new(
            MultiRootCanonicalKirVersionV2::V11,
            101,
            [0x31; 32],
        )
        .unwrap(),
        roster_identity: [0x41; 32],
        canonical_kernel_order: &[1, 0],
        roots: &roots,
    })
    .unwrap();
    let native = MultiRootProofRosterTranscriptV3::new(inputs(&roots, &[1, 0])).unwrap();
    assert!(MultiRootProofRosterTranscriptV3::decode(legacy.canonical_bytes()).is_err());
    assert!(MultiRootProofRosterTranscriptV2::decode(native.canonical_bytes()).is_err());
    let mut retagged = legacy.canonical_bytes().to_vec();
    retagged[..8].copy_from_slice(b"F2MRCOR3");
    retagged[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert!(MultiRootProofRosterTranscriptV3::decode(&retagged).is_err());
    let mut old_input = MultiRootProofRosterInputsV2 {
        kind: MultiRootProofRosterKindV2::Correspondence,
        semantic_mir_sha256: [0x11; 32],
        neutral_kir: MultiRootNeutralKirIdentityV2::new(
            MultiRootCanonicalKirVersionV2::V11,
            101,
            [0x31; 32],
        )
        .unwrap(),
        roster_identity: [0x41; 32],
        canonical_kernel_order: &[0],
        roots: &roots[..1],
    };
    assert!(MultiRootProofRosterTranscriptV2::new(old_input).is_err());
    old_input.roots = &[];
    old_input.canonical_kernel_order = &[];
    assert!(MultiRootProofRosterTranscriptV2::new(old_input).is_err());
    assert!(MultiRootProofRosterTranscriptV3::new(inputs(&[], &[])).is_err());
}

#[test]
fn native_strict_header_subject_tail_and_permutation_rejections() {
    let roots = roots();
    let native = MultiRootProofRosterTranscriptV3::new(inputs(&roots, &[1, 0])).unwrap();
    let bytes = native.canonical_bytes();
    for end in 0..bytes.len() {
        assert!(MultiRootProofRosterTranscriptV3::decode(&bytes[..end]).is_err());
    }
    for offset in 0..16 {
        let mut changed = bytes.to_vec();
        changed[offset] ^= 1;
        assert!(MultiRootProofRosterTranscriptV3::decode(&changed).is_err());
    }
    for start in [48 + 24, 48 + 64] {
        let mut changed = bytes.to_vec();
        changed[start..start + 32].fill(0);
        assert!(matches!(
            MultiRootProofRosterTranscriptV3::decode(&changed),
            Err(MultiRootProofRosterErrorV3::Subject(_))
        ));
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    let length = extra.len() as u32;
    extra[12..16].copy_from_slice(&length.to_le_bytes());
    assert!(MultiRootProofRosterTranscriptV3::decode(&extra).is_err());
    assert!(MultiRootProofRosterTranscriptV3::new(inputs(&roots, &[0, 1])).is_err());
    assert!(MultiRootProofRosterTranscriptV3::new(inputs(&roots, &[1, 1])).is_err());
    let mut duplicated = roots;
    duplicated[1].semantic_root = duplicated[0].semantic_root;
    assert!(MultiRootProofRosterTranscriptV3::new(inputs(&duplicated, &[1, 0])).is_err());
}

#[test]
fn native_subject_includes_catalog_identity_even_when_the_graph_is_unchanged() {
    let roots = roots();
    let first = MultiRootProofRosterTranscriptV3::new(inputs(&roots[..1], &[0])).unwrap();
    let mut second_input = inputs(&roots[..1], &[0]);
    second_input.native_neutral_subject =
        InertNativeNeutralSubjectV1::new([0x31; 32], 101, [0x33; 32], 56).unwrap();
    let second = MultiRootProofRosterTranscriptV3::new(second_input).unwrap();
    assert_eq!(
        first.native_neutral_subject().graph_digest(),
        second.native_neutral_subject().graph_digest()
    );
    assert_ne!(
        first.native_neutral_subject(),
        second.native_neutral_subject()
    );
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
}
