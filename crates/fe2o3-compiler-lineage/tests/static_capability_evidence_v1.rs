use fe2o3_compiler_lineage::{
    INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_MAGIC_V1,
    INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_VERSION_V1,
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertLineageContentIdentityV3, InertMultiRootStaticCapabilityEvidenceAssociationV1,
    InertStaticCapabilityEvidenceAssociationInputsV1, InertStaticCapabilityEvidenceAssociationV1,
    MachineRefinementContentIdentityV1, MachineRefinementFamilyV1,
    TargetMachineRefinementReceiptPartsV1, TargetMachineRefinementReceiptV1,
    TargetMachineRefinementTargetV1,
    decode_compiler_instruction_selection_correspondence_identity_v1,
    decode_exact_compiler_stage_content_identity_v1, decode_post_llvm_stage_custody_identity_v1,
};
use fe2o3_proof_contracts::{
    ArtifactIdentityV1, CapabilityObligationSpecV1, CapabilityOutcomeV1, CapabilityPropertyIdV1,
    CapabilityResultSpecV1, CapabilitySubjectV1, DigestV1, EvidenceIdentityV1, ExactToolIdentityV1,
    ExecutableKirIdentityV1, InertCapabilityObligationSetV1, InertCapabilityResultSetV1,
    KernelIdentityV1, KernelRootIdentityV1, LaunchContractIdentityV1, StatementIdentityV1,
    TargetModelIdentityV1,
};
use sha2::{Digest as _, Sha256};

const ASSOCIATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/CAPABILITY/STATIC-EVIDENCE/IDENTITY/V1\0";
const ASSOCIATION_HEADER_BYTES_V1: usize = 24;
const ASSOCIATION_FIELD_COUNT_V1: usize = 16;
const ASSOCIATION_FIELD_HEADER_BYTES_V1: usize = 8;
const ASSOCIATION_TERMINAL_BYTES_V1: usize = 32;

fn digest(seed: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([seed; 32])
}

fn lineage(seed: u8) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new([seed; 32], u64::from(seed) + 1).unwrap()
}

fn machine_receipt(seed: u8) -> InertCapabilityRefinementReceiptV1 {
    let families = MachineRefinementFamilyV1::ControlFlow.bit();
    let receipt =
        TargetMachineRefinementReceiptV1::from_parts(TargetMachineRefinementReceiptPartsV1 {
            target: TargetMachineRefinementTargetV1::Gfx942,
            post_llvm_custody: decode_post_llvm_stage_custody_identity_v1(
                [seed; 32],
                u64::from(seed),
            )
            .unwrap(),
            instruction_selection:
                decode_compiler_instruction_selection_correspondence_identity_v1(
                    [seed.wrapping_add(1); 32],
                    u64::from(seed) + 1,
                )
                .unwrap(),
            decoded_isa: MachineRefinementContentIdentityV1::new(
                [seed.wrapping_add(2); 32],
                u64::from(seed) + 2,
            )
            .unwrap(),
            final_code_object: decode_exact_compiler_stage_content_identity_v1(
                [seed.wrapping_add(3); 32],
                u64::from(seed) + 3,
            )
            .unwrap(),
            machine_refinement_sha256: [seed.wrapping_add(4); 32],
            required_families: families,
            established_families: families,
        })
        .unwrap();
    InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::Machine,
        receipt.canonical_bytes().to_vec(),
    )
    .unwrap()
}

fn subject(epoch: u64) -> CapabilitySubjectV1 {
    subject_with_coordinates(epoch, 21, 22, 23, 24)
}

fn subject_with_coordinates(
    epoch: u64,
    kernel: u8,
    root: u8,
    kir: u8,
    target: u8,
) -> CapabilitySubjectV1 {
    CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(digest(kernel)),
        KernelRootIdentityV1::from_untrusted_digest(digest(root)),
        ExecutableKirIdentityV1::from_untrusted_digest(digest(kir)),
        epoch,
        TargetModelIdentityV1::from_untrusted_digest(digest(target)),
        LaunchContractIdentityV1::from_untrusted_digest(digest(25)),
    )
    .unwrap()
}

fn sets(
    subject: CapabilitySubjectV1,
) -> (InertCapabilityObligationSetV1, InertCapabilityResultSetV1) {
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        [
            CapabilityPropertyIdV1::BOUNDS,
            CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT,
            CapabilityPropertyIdV1::MACHINE_REFINEMENT,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, property)| {
            CapabilityObligationSpecV1::new(
                property,
                StatementIdentityV1::from_untrusted_digest(digest(30 + index as u8)),
            )
        })
        .collect(),
    )
    .unwrap();
    let results = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        obligations
            .obligations()
            .iter()
            .enumerate()
            .map(|(index, obligation)| {
                CapabilityResultSpecV1::new(
                    obligation.identity(),
                    CapabilityOutcomeV1::Proven {
                        evidence: EvidenceIdentityV1::from_untrusted_digest(digest(
                            40 + index as u8,
                        )),
                        tool: ExactToolIdentityV1::new(
                            digest(50 + index as u8),
                            digest(60 + index as u8),
                        ),
                        proof_artifact: ArtifactIdentityV1::new(
                            digest(70 + index as u8),
                            digest(80 + index as u8),
                        ),
                    },
                )
            })
            .collect(),
    )
    .unwrap();
    (obligations, results)
}

fn association_for(subject: CapabilitySubjectV1) -> InertStaticCapabilityEvidenceAssociationV1 {
    let machine = machine_receipt(91);
    let inputs = InertStaticCapabilityEvidenceAssociationInputsV1::new(
        lineage(1),
        lineage(2),
        lineage(3),
        lineage(4),
        lineage(5),
        lineage(6),
        lineage(7),
        None,
        Some(machine.identity()),
    );
    let (obligations, results) = sets(subject);
    InertStaticCapabilityEvidenceAssociationV1::new(inputs, &obligations, &results).unwrap()
}

fn association(epoch: u64) -> InertStaticCapabilityEvidenceAssociationV1 {
    association_for(subject(epoch))
}

fn field_segments(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut segments = Vec::with_capacity(ASSOCIATION_FIELD_COUNT_V1);
    let mut offset = ASSOCIATION_HEADER_BYTES_V1;
    for _ in 0..ASSOCIATION_FIELD_COUNT_V1 {
        let length = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let end = offset + ASSOCIATION_FIELD_HEADER_BYTES_V1 + length as usize;
        segments.push(offset..end);
        offset = end;
    }
    assert_eq!(offset + ASSOCIATION_TERMINAL_BYTES_V1, bytes.len());
    segments
}

fn reseal_association(mut bytes: Vec<u8>) -> Vec<u8> {
    let terminal = bytes.len() - ASSOCIATION_TERMINAL_BYTES_V1;
    let mut digest = Sha256::new();
    digest.update(ASSOCIATION_IDENTITY_DOMAIN_V1);
    digest.update((terminal as u64).to_le_bytes());
    digest.update(&bytes[..terminal]);
    bytes[terminal..].copy_from_slice(&digest.finalize());
    bytes
}

fn reordered_or_duplicated_fields(bytes: &[u8], order: &[usize]) -> Vec<u8> {
    let segments = field_segments(bytes);
    let mut hostile = Vec::with_capacity(bytes.len());
    hostile.extend_from_slice(&bytes[..ASSOCIATION_HEADER_BYTES_V1]);
    for &index in order {
        hostile.extend_from_slice(&bytes[segments[index].clone()]);
    }
    hostile.extend_from_slice(&[0; ASSOCIATION_TERMINAL_BYTES_V1]);
    assert_eq!(hostile.len(), bytes.len());
    reseal_association(hostile)
}

#[test]
fn side_by_side_association_roundtrips_every_exact_axis() {
    let association = association(9);
    let decoded =
        InertStaticCapabilityEvidenceAssociationV1::decode(association.canonical_bytes()).unwrap();

    assert_eq!(decoded, association);
    assert_eq!(decoded.subject(), subject(9));
    assert_eq!(decoded.inputs().source_refinement(), None);
    assert!(decoded.inputs().machine_refinement().is_some());
    assert_eq!(
        decoded.identity().byte_len(),
        decoded.canonical_bytes().len() as u64
    );
    assert_eq!(
        &decoded.canonical_bytes()[..8],
        &INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_MAGIC_V1
    );
    assert_eq!(
        u16::from_le_bytes(decoded.canonical_bytes()[8..10].try_into().unwrap()),
        INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_VERSION_V1
    );
}

#[test]
fn every_prefix_downgrade_trailing_byte_and_mutated_field_fail_closed() {
    let association = association(9);
    let bytes = association.canonical_bytes();
    for prefix in 0..bytes.len() {
        assert!(
            InertStaticCapabilityEvidenceAssociationV1::decode(&bytes[..prefix]).is_err(),
            "accepted prefix {prefix}"
        );
    }

    let mut downgraded = bytes.to_vec();
    downgraded[8..10].copy_from_slice(&0_u16.to_le_bytes());
    assert!(InertStaticCapabilityEvidenceAssociationV1::decode(&downgraded).is_err());

    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(InertStaticCapabilityEvidenceAssociationV1::decode(&trailing).is_err());

    for offset in [0, 10, 12, 14, 16, 20, 24, bytes.len() - 1] {
        let mut mutated = bytes.to_vec();
        mutated[offset] ^= 0x80;
        assert!(
            InertStaticCapabilityEvidenceAssociationV1::decode(&mutated).is_err(),
            "accepted mutation at {offset}"
        );
    }
}

#[test]
fn canonically_resealed_reordered_and_duplicate_fields_fail_closed() {
    let association = association(9);
    let bytes = association.canonical_bytes();
    let mut reordered: Vec<_> = (0..ASSOCIATION_FIELD_COUNT_V1).collect();
    reordered.swap(2, 3);
    assert!(matches!(
        InertStaticCapabilityEvidenceAssociationV1::decode(&reordered_or_duplicated_fields(
            bytes, &reordered,
        )),
        Err(fe2o3_compiler_lineage::InertStaticCapabilityEvidenceAssociationErrorV1::WrongFieldTag)
    ));

    let mut duplicated: Vec<_> = (0..ASSOCIATION_FIELD_COUNT_V1).collect();
    duplicated[3] = 2;
    assert!(matches!(
        InertStaticCapabilityEvidenceAssociationV1::decode(&reordered_or_duplicated_fields(
            bytes,
            &duplicated,
        )),
        Err(fe2o3_compiler_lineage::InertStaticCapabilityEvidenceAssociationErrorV1::WrongFieldTag)
    ));
}

#[test]
fn epoch_and_refinement_receipt_splicing_change_the_outer_identity() {
    let first = association(9);
    let stale_epoch = association(8);
    assert_ne!(first.identity(), stale_epoch.identity());

    let replacement = machine_receipt(101);
    assert_ne!(
        first.inputs().machine_refinement(),
        Some(replacement.identity())
    );
}

#[test]
fn multi_root_association_roundtrips_complete_canonical_roster() {
    let first = subject_with_coordinates(9, 21, 22, 23, 24);
    let second = subject_with_coordinates(9, 26, 27, 23, 24);
    let association = InertMultiRootStaticCapabilityEvidenceAssociationV1::new(vec![
        association_for(first),
        association_for(second),
    ])
    .unwrap();
    let decoded =
        InertMultiRootStaticCapabilityEvidenceAssociationV1::decode(association.canonical_bytes())
            .unwrap();

    assert_eq!(decoded, association);
    assert_eq!(decoded.subjects().collect::<Vec<_>>(), vec![first, second]);
    assert_eq!(decoded.entries().len(), 2);
}

#[test]
fn multi_root_association_rejects_omission_duplication_permutation_and_substitution() {
    let first = subject_with_coordinates(9, 21, 22, 23, 24);
    let second = subject_with_coordinates(9, 26, 27, 23, 24);
    let valid = InertMultiRootStaticCapabilityEvidenceAssociationV1::new(vec![
        association_for(first),
        association_for(second),
    ])
    .unwrap();

    let mut omitted = valid.canonical_bytes().to_vec();
    omitted[12..16].copy_from_slice(&1_u32.to_le_bytes());
    assert!(InertMultiRootStaticCapabilityEvidenceAssociationV1::decode(&omitted).is_err());
    assert!(
        InertMultiRootStaticCapabilityEvidenceAssociationV1::new(vec![
            association_for(first),
            association_for(first),
        ])
        .is_err()
    );
    assert!(
        InertMultiRootStaticCapabilityEvidenceAssociationV1::new(vec![
            association_for(second),
            association_for(first),
        ])
        .is_err()
    );

    for substituted in [
        subject_with_coordinates(8, 26, 27, 23, 24),
        subject_with_coordinates(9, 26, 27, 28, 24),
        subject_with_coordinates(9, 26, 27, 23, 29),
        subject_with_coordinates(9, 26, 22, 23, 24),
    ] {
        assert!(
            InertMultiRootStaticCapabilityEvidenceAssociationV1::new(vec![
                association_for(first),
                association_for(substituted),
            ])
            .is_err()
        );
    }

    let mut cross_version = valid.canonical_bytes().to_vec();
    cross_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(InertMultiRootStaticCapabilityEvidenceAssociationV1::decode(&cross_version).is_err());
}
