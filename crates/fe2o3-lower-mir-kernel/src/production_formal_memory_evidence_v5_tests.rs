use super::*;
use crate::production_formal_memory_execution_discharge_v1::tests::{fixture, obligations};
use fe2o3_kernel_ir::{Constant, OperationKind, verify_module_ref};

fn evidence(module: &Module) -> InertCanonicalFormalMemoryAdmissionEvidenceV5 {
    let raw = obligations(module);
    let receipt = InertFormalMemoryReceiptFormatV4::from_current_obligations(&raw).unwrap();
    let rows = derive_execution_discharges_v1(module, &module.kernels[0], &raw).unwrap();
    let kir = canonical_identity(module, ProductionCanonicalKernelIrVersionV1::V8).unwrap();
    // Inert codec test input derived from an actual graph; not a live source owner.
    InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(
        &encode(
            kir,
            0,
            module.kernels[0].id.as_str(),
            module.kernels[0].entry.as_str(),
            [64, 1, 1],
            &receipt,
            &rows,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn canonical_v5_replays_actual_graph_and_preserves_raw_receipt() {
    let module = fixture();
    let evidence = evidence(&module);
    evidence.revalidate().unwrap();
    evidence
        .revalidate_against_verified_module(verify_module_ref(&module).unwrap())
        .unwrap();
    assert_eq!(evidence.discharges().len(), 1);
    assert_eq!(evidence.kernel_id(), "write_once");
    assert_eq!(evidence.entry_id(), "write_once_body");
    assert!(!evidence.grants_authority());
    let facade =
        InertFormalMemoryAdmissionEvidenceFormatV5::decode_current(evidence.canonical_bytes())
            .unwrap();
    assert!(facade.legacy_v4().is_none());
    assert!(facade.execution_discharged_v5().is_some());
    assert_eq!(facade.canonical_bytes(), evidence.canonical_bytes());
    facade
        .revalidate_against_verified_module(verify_module_ref(&module).unwrap())
        .unwrap();
    let raw =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(&obligations(&module)).unwrap();
    assert_eq!(
        evidence.formal_obligation_receipt_bytes(),
        raw.canonical_bytes()
    );
}

#[test]
fn stale_graph_and_stripped_raw_receipt_cannot_replay() {
    let mut module = fixture();
    let evidence = evidence(&module);
    assert!(
        revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
            verify_module_ref(&module).unwrap(),
            0,
            evidence.formal_obligation_receipt_bytes()
        )
        .is_err()
    );
    assert!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(evidence.canonical_bytes()).is_err()
    );
    module.functions[0].blocks[0].operations[1].kind = OperationKind::Constant(Constant::Index(2));
    assert!(
        evidence
            .revalidate_against_verified_module(verify_module_ref(&module).unwrap())
            .is_err()
    );
}

#[test]
fn decoder_rejects_truncation_headers_counts_and_duplicate_rosters() {
    let module = fixture();
    let evidence = evidence(&module);
    let original = evidence.canonical_bytes();
    for end in [0, 7, 19, 151, original.len() - 1] {
        assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&original[..end]).is_err());
    }
    for offset in [
        8, 10, 12, 16, 20, 22, 100, 102, 112, 120, 128, 136, 140, 144, 148,
    ] {
        let mut altered = original.to_vec();
        altered[offset] ^= 128;
        assert!(
            InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&altered).is_err(),
            "offset {offset}"
        );
    }
    let mut altered = original.to_vec();
    altered.push(0);
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&altered).is_err());
    let mut duplicate = original.to_vec();
    duplicate.extend_from_slice(&original[original.len() - ROW_BYTES..]);
    let length = duplicate.len() as u32;
    duplicate[16..20].copy_from_slice(&length.to_le_bytes());
    duplicate[136..140].copy_from_slice(&2_u32.to_le_bytes());
    duplicate[144..148].copy_from_slice(&2_u32.to_le_bytes());
    let second = duplicate.len() - ROW_BYTES;
    duplicate[second..second + 4].copy_from_slice(&1_u32.to_le_bytes());
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&duplicate).is_err());
}

#[test]
fn fresh_inert_digest_cannot_authorize_changed_witness_or_conflict_coordinates() {
    let module = fixture();
    let original = evidence(&module);
    for relative in [4, 8, 12, 16, 20, 32, 36, 40, 48, 52, 56] {
        let mut changed = original.canonical_bytes().to_vec();
        let start = changed.len() - ROW_BYTES;
        changed[start + relative] ^= 1;
        if let Ok(parsed) = InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&changed) {
            assert_ne!(parsed.identity(), original.identity());
            assert!(
                parsed
                    .revalidate_against_verified_module(verify_module_ref(&module).unwrap())
                    .is_err()
            );
        }
    }
}

#[test]
fn legacy_raw_replay_accepts_only_current_conflict_free_bytes() {
    let mut module = fixture();
    let conflict = evidence(&module);
    module.functions[0].blocks[1].operations.clear();
    let raw =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(&obligations(&module)).unwrap();
    let verified = verify_module_ref(&module).unwrap();
    revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
        verified,
        0,
        raw.canonical_bytes(),
    )
    .unwrap();
    assert!(
        revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
            verified,
            0,
            conflict.formal_obligation_receipt_bytes()
        )
        .is_err()
    );
    assert!(
        revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
            verified,
            1,
            raw.canonical_bytes()
        )
        .is_err()
    );
}

#[test]
fn outer_byte_bound_and_empty_discharge_remain_closed() {
    assert!(
        InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&vec![
            0;
            MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4
                + 1
        ])
        .is_err()
    );
    let module = fixture();
    let raw =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(&obligations(&module)).unwrap();
    let kir = canonical_identity(&module, ProductionCanonicalKernelIrVersionV1::V8).unwrap();
    assert!(
        encode(
            kir,
            0,
            "write_once",
            "write_once_body",
            [64, 1, 1],
            &raw,
            &[]
        )
        .is_err()
    );
}

#[test]
fn receipt_width_range_and_legacy_v2_are_not_reinterpreted() {
    let module = fixture();
    let kir = canonical_identity(&module, ProductionCanonicalKernelIrVersionV1::V8).unwrap();
    let rows =
        derive_execution_discharges_v1(&module, &module.kernels[0], &obligations(&module)).unwrap();
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Unknown] {
        let report = derive_kernel_memory_obligations_for_launch(
            &module,
            &module.kernels[0].id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            width,
        )
        .unwrap();
        let raw = InertFormalMemoryReceiptFormatV4::from_current_obligations(report.obligations())
            .unwrap();
        assert!(
            encode(
                kir,
                0,
                "write_once",
                "write_once_body",
                [64, 1, 1],
                &raw,
                &rows
            )
            .is_err()
        );
    }
    let mut write_only = fixture();
    write_only.functions[0].signature.parameters[0] = fe2o3_kernel_ir::Type::pointer(
        fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U32),
        fe2o3_kernel_ir::AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::WriteOnly,
    );
    let raw = InertFormalMemoryReceiptFormatV4::from_current_obligations(&obligations(&write_only))
        .unwrap();
    assert_eq!(
        raw.metadata().encoding(),
        FormalMemoryReceiptEncodingV4::LegacyV2
    );
    assert!(validate_receipt_metadata(&raw, 64).is_err());
    let original = evidence(&module);
    let mut bytes = original.canonical_bytes().to_vec();
    bytes[104..112].copy_from_slice(&63_u64.to_le_bytes());
    bytes[128..136].copy_from_slice(&63_u64.to_le_bytes());
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).is_err());
}

#[test]
fn decode_does_not_claim_raw_pair_coverage_before_replay() {
    let module = fixture();
    let original = evidence(&module);
    let mut bytes = original.canonical_bytes().to_vec();
    let start = bytes.len() - ROW_BYTES;
    bytes[start + 4..start + 8].copy_from_slice(&17_u32.to_le_bytes());
    let syntax_only = InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).unwrap();
    assert!(!syntax_only.grants_authority());
    assert!(
        syntax_only
            .revalidate_against_verified_module(verify_module_ref(&module).unwrap())
            .is_err()
    );
}
