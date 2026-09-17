use super::*;

fn location(block: u32, operation: u64) -> FormalMemoryPublicationLocationV5 {
    FormalMemoryPublicationLocationV5 { block, operation }
}

fn proof(conflicts: bool) -> FormalMemoryStaticPublicationSummaryV5 {
    let ranked_sites = [[1, 2], [1, 3], [2, 1], [2, 2], [2, 3], [2, 4]];
    let events = std::array::from_fn(|index| {
        let (block, operation, ordinal, ranked) = match index {
            0 => (10, 2, 0, 0),
            1 => (10, 4, 1, 1),
            2 => (20, 1, 0, 2),
            3 => (20, 3, 1, 3),
            _ => (20, 8, 2, 5),
        };
        FormalMemoryPublicationEventV5 {
            semantic_block: block,
            semantic_ordinal: ordinal,
            kir: location(block, operation),
            ranked: ranked_sites[ranked],
        }
    });
    let discharges = if conflicts {
        vec![
            FormalMemoryConflictDischargeV5 {
                left: events[0].kir,
                right: events[0].kir,
                allocation: 0,
                reason: FormalMemoryStaticConflictReasonV5::ProducerInjectivity,
            },
            FormalMemoryConflictDischargeV5 {
                left: events[0].kir,
                right: events[4].kir,
                allocation: 0,
                reason: FormalMemoryStaticConflictReasonV5::PublishedReadHappensBefore,
            },
        ]
    } else {
        vec![]
    };
    FormalMemoryStaticPublicationSummaryV5 {
        semantic_function: 1,
        selected_root: 0,
        source_arguments: [0, 1],
        values: [0, 1, 2, 3, 4, 5],
        payload_parameter_index: 0,
        global_extents: [256, 1, 1],
        workgroup_extents: [128, 1, 1],
        events,
        origins: [1, 2],
        ranked_sites,
        ranked_counts: [256, 128, 128, 0],
        discharges: discharges.into_boxed_slice(),
    }
}

// Inert codec fixtures: these byte records do not synthesize live compiler authority.
fn raw_receipt(version: u16, conflicts: bool) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"FE2O3FM\0");
    put16_v5(&mut b, version);
    put16_v5(&mut b, fe2o3_kernel_ir::FORMAL_MEMORY_OBLIGATION_POLICY_V1);
    put32_v5(&mut b, 0);
    put32_v5(&mut b, 0);
    for name in [b"kernel".as_slice(), b"entry".as_slice()] {
        put32_v5(&mut b, name.len() as u32);
        b.extend_from_slice(name);
    }
    b.extend_from_slice(&[2, 1, 0, 0, 1]);
    put64_v5(&mut b, 0);
    put64_v5(&mut b, 256);
    put32_v5(&mut b, if version == 2 { 2 } else { 1 });
    put32_v5(&mut b, 0);
    put32_v5(&mut b, 10);
    b.extend_from_slice(&[2, 3, 2, 0]);
    if version == 2 {
        put32_v5(&mut b, 1);
        put32_v5(&mut b, 11);
        b.extend_from_slice(&[2, 3, 3, 0]);
    }
    put32_v5(&mut b, 2);
    for (site, kind) in [(location(10, 2), 2u8), (location(20, 8), 1)] {
        put_location_v5(&mut b, site);
        put32_v5(&mut b, 0);
        b.extend_from_slice(&[kind, 3, 2, 0, 0, 0]);
        put64_v5(&mut b, 0);
        put64_v5(&mut b, 0);
        put64_v5(&mut b, 4);
        put64_v5(&mut b, 4);
        put64_v5(&mut b, 0);
        put64_v5(&mut b, 256);
    }
    put32_v5(&mut b, 0);
    put32_v5(&mut b, 0);
    put32_v5(&mut b, if conflicts { 2 } else { 0 });
    if conflicts {
        for item in proof(true).discharges {
            put_location_v5(&mut b, item.left);
            put_location_v5(&mut b, item.right);
            put32_v5(&mut b, item.allocation);
        }
    }
    let length = b.len() as u32;
    b[16..20].copy_from_slice(&length.to_le_bytes());
    InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(b.clone()).unwrap();
    b
}

fn fields(receipt: &[u8], conflicts: bool) -> FormalFieldsV5 {
    FormalFieldsV5 {
        source_identity: [1; 32],
        ranked_identity: [2; 32],
        kir: ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
            ProductionCanonicalKernelIrVersionV1::V11,
            [3; 32],
            128,
        ),
        receipt_identity: *InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
            receipt.to_vec(),
        )
        .unwrap()
        .identity()
        .digest(),
        witness: 256,
        raw_conflicts: if conflicts { 2 } else { 0 },
        discharged: if conflicts { 2 } else { 0 },
    }
}

#[test]
fn formal_v5_preserves_both_raw_receipt_versions_and_zero_conflict_publication() {
    for version in [1, 2] {
        for has_publication in [false, true] {
            for conflicts in [false, true] {
                if conflicts && !has_publication {
                    continue;
                }
                let receipt = raw_receipt(version, conflicts);
                let publication = has_publication.then(|| proof(conflicts));
                let bytes =
                    encode_v5(fields(&receipt, conflicts), publication.as_ref(), &receipt).unwrap();
                let decoded =
                    InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).unwrap();
                decoded.revalidate().unwrap();
                assert_eq!(decoded.formal_obligation_receipt_bytes(), receipt);
                assert_eq!(decoded.static_publication().is_some(), has_publication);
                assert_eq!(
                    decoded.raw_inter_invocation_conflict_count(),
                    if conflicts { 2 } else { 0 }
                );
                assert_eq!(
                    decoded.discharged_inter_invocation_conflict_count(),
                    decoded.raw_inter_invocation_conflict_count()
                );
                assert_eq!(decoded.unresolved_inter_invocation_conflict_count(), 0);
                assert!(!decoded.grants_authority());
                assert_eq!(
                    decoded
                        .static_publication()
                        .map(|p| p.ranked_potentially_conflicting_cell_pairs()),
                    has_publication.then_some(128)
                );
            }
        }
    }
}

#[test]
fn formal_v5_rejects_exact_proof_roster_geometry_origin_and_discharge_mutants() {
    let changes: [fn(&mut FormalMemoryStaticPublicationSummaryV5); 12] = [
        |p| p.global_extents[0] = 128,
        |p| p.workgroup_extents[0] = 64,
        |p| p.origins[1] = 1,
        |p| p.source_arguments[1] = 0,
        |p| p.events[1].semantic_ordinal = 0,
        |p| p.events[4].ranked = [2, 3],
        |p| p.ranked_sites[4] = [2, 5],
        |p| p.ranked_counts[1] = 0,
        |p| p.events[3].kir.operation = 0,
        |p| p.discharges[0].allocation = 1,
        |p| p.discharges[0].reason = FormalMemoryStaticConflictReasonV5::PublishedReadHappensBefore,
        |p| p.discharges.swap(0, 1),
    ];
    for change in changes {
        let mut p = proof(true);
        change(&mut p);
        assert!(encode_publication_v5(&p).is_err());
    }
}

#[test]
fn formal_v5_rejects_counter_receipt_and_witness_substitutions() {
    let receipt = raw_receipt(1, true);
    let p = proof(true);
    for change in [
        |f: &mut FormalFieldsV5| f.raw_conflicts = 0,
        |f: &mut FormalFieldsV5| f.discharged = 0,
        |f: &mut FormalFieldsV5| f.witness = 128,
    ] {
        let mut f = fields(&receipt, true);
        change(&mut f);
        assert!(encode_v5(f, Some(&p), &receipt).is_err());
    }
    let mut f = fields(&receipt, true);
    f.receipt_identity = [9; 32];
    assert!(
        InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(
            &encode_v5(f, Some(&p), &receipt).unwrap()
        )
        .is_err()
    );
    let empty = raw_receipt(1, false);
    f = fields(&empty, false);
    f.raw_conflicts = 2;
    f.discharged = 2;
    assert!(
        InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(
            &encode_v5(f, Some(&p), &empty).unwrap()
        )
        .is_err()
    );
    assert!(encode_v5(fields(&receipt, true), None, &receipt).is_err());
}

#[test]
fn formal_v5_rejects_noncanonical_versions_flags_lengths_and_truncation() {
    let receipt = raw_receipt(1, true);
    let bytes = encode_v5(fields(&receipt, true), Some(&proof(true)), &receipt).unwrap();
    for length in 0..bytes.len() {
        assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes[..length]).is_err());
    }
    for offset in [
        0,
        8,
        10,
        12,
        16,
        84,
        86,
        168,
        170,
        172,
        184,
        188,
        192,
        196,
        198,
        196 + 96,
        196 + 97,
        196 + 104 + 4,
        196 + 348 + 12,
    ] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert!(
            InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&changed).is_err(),
            "offset {offset}"
        );
    }
    let mut changed = bytes.clone();
    changed.push(0);
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&changed).is_err());
    let mut changed = bytes;
    changed[20..52].fill(0);
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&changed).is_err());
    assert!(
        InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&vec![
            0;
            MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V5
                + 1
        ])
        .is_err()
    );
}

#[test]
fn formal_v5_cannot_launder_a_changed_raw_conflict_into_a_typed_discharge() {
    let receipt = raw_receipt(1, true);
    let mut p = proof(true);
    // This remains an internally consistent publication roster, but no longer
    // identifies the exact raw receipt's W/R pair.
    p.events[4].kir.operation = 9;
    p.discharges[1].right.operation = 9;
    let bytes = encode_v5(fields(&receipt, true), Some(&p), &receipt).unwrap();
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).is_err());
}
