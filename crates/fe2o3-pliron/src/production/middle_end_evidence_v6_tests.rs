use super::*;

const IR: &str = "func @publication {\n  kernel.return\n}\n";

fn summary() -> ProductionMiddleEndPublicationSummaryV6 {
    ProductionMiddleEndPublicationSummaryV6 {
        payload_origin: 1,
        flags_origin: 2,
        sites: [
            ProductionMiddleEndPublicationSiteV6 {
                block: 1,
                operation: 0,
            },
            ProductionMiddleEndPublicationSiteV6 {
                block: 1,
                operation: 1,
            },
            ProductionMiddleEndPublicationSiteV6 {
                block: 2,
                operation: 0,
            },
            ProductionMiddleEndPublicationSiteV6 {
                block: 2,
                operation: 1,
            },
            ProductionMiddleEndPublicationSiteV6 {
                block: 2,
                operation: 2,
            },
            ProductionMiddleEndPublicationSiteV6 {
                block: 2,
                operation: 3,
            },
        ],
        maximum_invocations: 256,
        potentially_conflicting_cell_pairs: 128,
        discharged_cell_pairs: 128,
        unresolved_cell_pairs: 0,
    }
}

fn specimen(publication: bool) -> InertProductionMiddleEndEvidenceV6 {
    encode_record_v6(
        CommonMiddleEndFactsV1::test_fixture(),
        IR,
        [4; 32],
        publication.then(summary),
    )
    .unwrap()
}

fn resign(bytes: &mut [u8]) {
    let terminal = bytes.len() - 32;
    let digest = identity_v6(&bytes[..terminal]);
    bytes[terminal..].copy_from_slice(&digest);
}

#[test]
fn evidence_v6_roundtrips_some_none_with_all_common_commitments_and_no_authority() {
    for present in [false, true] {
        let specimen = specimen(present);
        let decoded =
            InertProductionMiddleEndEvidenceV6::decode(specimen.canonical_bytes()).unwrap();
        assert_eq!(specimen, decoded);
        assert_eq!(decoded.ranked_ir(), IR);
        assert_eq!(decoded.pass_successes().len(), 8);
        assert_eq!(decoded.source_semantic_identity(), &[1; 32]);
        assert_eq!(decoded.ranked_kernel_identity(), &[2; 32]);
        assert_eq!(decoded.live_ranked_graph_identity(), &[4; 32]);
        assert_eq!(
            decoded
                .typed_semantic_reconciliation()
                .ordered_commitments_sha256(),
            &[3; 32]
        );
        assert_eq!(decoded.static_publication().copied(), present.then(summary));
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
        assert!(!decoded.authenticates_producer());
        assert!(!decoded.grants_publication_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_artifact_or_launch_authority());
        assert!(
            super::super::InertProductionMiddleEndEvidenceV5::decode(decoded.canonical_bytes())
                .is_err()
        );
    }
}

#[test]
fn evidence_v6_rejects_resealed_publication_mutants_and_noncanonical_none() {
    let original = specimen(true);
    let offset = original.canonical_bytes().len() - 32 - PUBLICATION_BYTES_V6;
    for field in [0, 1, 8, 16, 24, 32, 40, 48, 56, 64, 72, 76, 80, 84] {
        let mut bytes = original.canonical_bytes().to_vec();
        match field {
            0 => bytes[offset] = 2,
            1 => bytes[offset + 1] = 1,
            8 | 16 => bytes[offset + field..offset + field + 8].fill(0),
            24..=64 => bytes[offset + field..offset + field + 8].fill(255),
            72..=80 => bytes[offset + field..offset + field + 4].fill(0),
            84 => bytes[offset + field] = 1,
            _ => unreachable!(),
        }
        resign(&mut bytes);
        assert!(
            InertProductionMiddleEndEvidenceV6::decode(&bytes).is_err(),
            "field {field}"
        );
    }
    for field in 1..PUBLICATION_BYTES_V6 {
        let mut bytes = specimen(false).canonical_bytes().to_vec();
        bytes[offset + field] = 1;
        resign(&mut bytes);
        assert!(
            InertProductionMiddleEndEvidenceV6::decode(&bytes).is_err(),
            "none field {field}"
        );
    }
}

#[test]
fn evidence_v6_rejects_truncation_trailing_identity_header_and_pass_mutants() {
    let record = specimen(true);
    let original = record.canonical_bytes();
    for length in 0..original.len() {
        assert!(InertProductionMiddleEndEvidenceV6::decode(&original[..length]).is_err());
    }
    let mut trailing = original.to_vec();
    trailing.push(0);
    assert!(InertProductionMiddleEndEvidenceV6::decode(&trailing).is_err());
    for offset in [
        0,
        8,
        10,
        20,
        24,
        26,
        HEADER_BYTES_V6 - 4,
        HEADER_BYTES_V6 - 3,
        HEADER_BYTES_V6 + 68 + IR.len(),
        HEADER_BYTES_V6 + 69 + IR.len(),
        HEADER_BYTES_V6 + 70 + IR.len(),
        HEADER_BYTES_V6 + 75 + IR.len(),
    ] {
        let mut bytes = original.to_vec();
        bytes[offset] ^= 1;
        resign(&mut bytes);
        assert!(
            InertProductionMiddleEndEvidenceV6::decode(&bytes).is_err(),
            "header/pass {offset}"
        );
    }
    let mut bytes = original.to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    assert!(InertProductionMiddleEndEvidenceV6::decode(&bytes).is_err());
}

#[test]
fn evidence_v6_none_cannot_hide_publication_recipe_and_some_requires_full_roster() {
    require_publication_roster_v6([0, 0, 0], false).unwrap();
    require_publication_roster_v6([2, 1, 1], true).unwrap();
    for counts in [[1, 0, 0], [0, 1, 0], [0, 0, 1], [2, 1, 1]] {
        assert!(require_publication_roster_v6(counts, false).is_err());
    }
    for counts in [[0, 0, 0], [1, 1, 1], [2, 2, 1], [2, 1, 2]] {
        assert!(require_publication_roster_v6(counts, true).is_err());
    }
}

#[test]
fn evidence_v6_rejects_wrong_origin_order_and_geometry_before_encoding() {
    for mutant in 0..8 {
        let mut proof = summary();
        match mutant {
            0 => proof.flags_origin = proof.payload_origin,
            1 => proof.sites[1] = proof.sites[0],
            2 => proof.sites[3] = proof.sites[2],
            3 => proof.sites[5].block = 1,
            4 => proof.maximum_invocations = 128,
            5 => proof.potentially_conflicting_cell_pairs = 0,
            6 => proof.discharged_cell_pairs = 127,
            7 => proof.unresolved_cell_pairs = 1,
            _ => unreachable!(),
        }
        assert!(
            encode_record_v6(
                CommonMiddleEndFactsV1::test_fixture(),
                IR,
                [4; 32],
                Some(proof)
            )
            .is_err()
        );
    }
}

#[test]
fn evidence_v6_binds_distinct_live_graph_identity_and_rejects_zero_or_unsealed_mutation() {
    assert!(encode_record_v6(CommonMiddleEndFactsV1::test_fixture(), IR, [0; 32], None).is_err());
    for present in [false, true] {
        let original = specimen(present);
        let offset = original.canonical_bytes().len()
            - 32
            - PUBLICATION_BYTES_V6
            - LIVE_GRAPH_IDENTITY_BYTES_V6;
        let mut bytes = original.canonical_bytes().to_vec();
        bytes[offset..offset + 32].fill(0);
        resign(&mut bytes);
        assert!(InertProductionMiddleEndEvidenceV6::decode(&bytes).is_err());

        let mut bytes = original.canonical_bytes().to_vec();
        bytes[offset] ^= 1;
        assert!(InertProductionMiddleEndEvidenceV6::decode(&bytes).is_err());
        resign(&mut bytes);
        let mutated = InertProductionMiddleEndEvidenceV6::decode(&bytes).unwrap();
        // Resealed bytes remain inert; the external exact graph join must reject
        // this changed claim. It cannot silently replace the old recipe hash.
        assert_eq!(
            mutated.ranked_kernel_identity(),
            original.ranked_kernel_identity()
        );
        assert_ne!(
            mutated.live_ranked_graph_identity(),
            original.live_ranked_graph_identity()
        );
        assert_ne!(mutated.identity(), original.identity());
        assert!(!original.identity().matches_canonical_bytes(&bytes));
        assert!(!mutated.grants_compiler_refinement_authority());
    }
}
