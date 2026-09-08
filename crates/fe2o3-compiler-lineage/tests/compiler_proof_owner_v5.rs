use fe2o3_compiler_lineage::{
    InertCanonicalKernelIrV13ReceiptErrorV5, InertCanonicalKernelIrV13ReceiptV5,
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertCompilerProofOwnerErrorV5, InertCompilerProofOwnerInputsV5, InertCompilerProofOwnerV5,
    InertKernelIrReceiptV3, InertLineageContentIdentityV3, InertMultiRootProofLineageV3,
    InertStaticCapabilityEvidenceAssociationIdentityV1,
    InertStaticCapabilityEvidenceAssociationInputsV1, InertStaticCapabilityEvidenceAssociationV1,
    MachineRefinementContentIdentityV1, MachineRefinementFamilyV1, MultiRootCanonicalKirVersionV3,
    MultiRootNeutralKirIdentityV3, MultiRootProofRosterInputsV3, MultiRootProofRosterKindV3,
    MultiRootProofRosterRootInputV3, MultiRootProofRosterTranscriptV3,
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

fn digest(seed: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([seed; 32])
}

fn lineage(seed: u8, bytes: u64) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new([seed; 32], bytes).unwrap()
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

fn fixture(epoch: u64) -> InertCompilerProofOwnerV5 {
    let kir_receipt =
        InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(b"canonical-kir-v13".to_vec())
            .unwrap();
    let subject = CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(digest(1)),
        KernelRootIdentityV1::from_untrusted_digest(digest(2)),
        ExecutableKirIdentityV1::from_untrusted_digest(digest(3)),
        epoch,
        TargetModelIdentityV1::from_untrusted_digest(digest(4)),
        LaunchContractIdentityV1::from_untrusted_digest(digest(5)),
    )
    .unwrap();
    let source = machine_receipt(31);
    let machine = machine_receipt(41);
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![CapabilityObligationSpecV1::new(
            CapabilityPropertyIdV1::BOUNDS,
            StatementIdentityV1::from_untrusted_digest(digest(6)),
        )],
    )
    .unwrap();
    let results = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        vec![CapabilityResultSpecV1::new(
            obligations.obligations()[0].identity(),
            CapabilityOutcomeV1::Proven {
                evidence: EvidenceIdentityV1::from_untrusted_digest(digest(7)),
                tool: ExactToolIdentityV1::new(digest(8), digest(9)),
                proof_artifact: ArtifactIdentityV1::new(digest(10), digest(11)),
            },
        )],
    )
    .unwrap();
    let capability = InertStaticCapabilityEvidenceAssociationV1::new(
        InertStaticCapabilityEvidenceAssociationInputsV1::new(
            lineage(10, 10),
            lineage(11, 11),
            lineage(12, 12),
            lineage(13, 13),
            lineage(14, 14),
            lineage(15, 15),
            lineage(16, 16),
            Some(source.identity()),
            Some(machine.identity()),
        ),
        &obligations,
        &results,
    )
    .unwrap();
    InertCompilerProofOwnerV5::new(
        InertCompilerProofOwnerInputsV5::new(
            lineage(17, 17),
            lineage(18, 18),
            [19; 32],
            kir_receipt.identity(),
            kir_receipt.identity().byte_len(),
            subject,
            [21; 32],
            source.identity(),
            machine.identity(),
            capability.identity(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn proof_lineage(subjects: &[CapabilitySubjectV1]) -> InertMultiRootProofLineageV3 {
    let first = subjects[0];
    let neutral = MultiRootNeutralKirIdentityV3::new(
        MultiRootCanonicalKirVersionV3::V13,
        17,
        *first.executable_kir().digest().as_bytes(),
        first.executable_kir_epoch(),
    )
    .unwrap();
    let roster = |kind, payload: &'static [u8]| {
        let roots = [
            MultiRootProofRosterRootInputV3 {
                semantic_root: 0,
                semantic_root_identity: *subjects[0].root().digest().as_bytes(),
                kernel_binding: *subjects[0].kernel().digest().as_bytes(),
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "kernel_a",
                export_symbol: "kernel_a",
                kernel_id: "kernel_a",
                payload,
            },
            MultiRootProofRosterRootInputV3 {
                semantic_root: 1,
                semantic_root_identity: *subjects[1].root().digest().as_bytes(),
                kernel_binding: *subjects[1].kernel().digest().as_bytes(),
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "kernel_b",
                export_symbol: "kernel_b",
                kernel_id: "kernel_b",
                payload,
            },
        ];
        MultiRootProofRosterTranscriptV3::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256: [19; 32],
            neutral_kir: neutral,
            roster_identity: [20; 32],
            canonical_kernel_order: &[0, 1],
            roots: &roots,
        })
        .unwrap()
    };
    InertMultiRootProofLineageV3::new(
        roster(MultiRootProofRosterKindV3::MiddleEnd, b"middle"),
        roster(
            MultiRootProofRosterKindV3::Correspondence,
            b"correspondence",
        ),
        roster(MultiRootProofRosterKindV3::FormalMemory, b"memory"),
        roster(MultiRootProofRosterKindV3::VerusExecution, b"verus"),
    )
    .unwrap()
}

fn capability_association_for_subject(
    subject: CapabilitySubjectV1,
    source: fe2o3_compiler_lineage::InertCapabilityRefinementReceiptIdentityV1,
    machine: fe2o3_compiler_lineage::InertCapabilityRefinementReceiptIdentityV1,
) -> InertStaticCapabilityEvidenceAssociationIdentityV1 {
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![CapabilityObligationSpecV1::new(
            CapabilityPropertyIdV1::BOUNDS,
            StatementIdentityV1::from_untrusted_digest(digest(6)),
        )],
    )
    .unwrap();
    let results = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        vec![CapabilityResultSpecV1::new(
            obligations.obligations()[0].identity(),
            CapabilityOutcomeV1::Proven {
                evidence: EvidenceIdentityV1::from_untrusted_digest(digest(7)),
                tool: ExactToolIdentityV1::new(digest(8), digest(9)),
                proof_artifact: ArtifactIdentityV1::new(digest(10), digest(11)),
            },
        )],
    )
    .unwrap();
    InertStaticCapabilityEvidenceAssociationV1::new(
        InertStaticCapabilityEvidenceAssociationInputsV1::new(
            lineage(10, 10),
            lineage(11, 11),
            lineage(12, 12),
            lineage(13, 13),
            lineage(14, 14),
            lineage(15, 15),
            lineage(16, 16),
            Some(source),
            Some(machine),
        ),
        &obligations,
        &results,
    )
    .unwrap()
    .identity()
}

fn inputs_for_subject(
    inputs: InertCompilerProofOwnerInputsV5,
    subject: CapabilitySubjectV1,
) -> InertCompilerProofOwnerInputsV5 {
    InertCompilerProofOwnerInputsV5::new(
        inputs.legacy_proof_binding(),
        inputs.semantic_mir_receipt(),
        inputs.semantic_mir_identity(),
        inputs.executable_kir_receipt(),
        inputs.executable_kir_bytes(),
        subject,
        inputs.compiler_policy(),
        inputs.source_refinement(),
        inputs.machine_refinement(),
        capability_association_for_subject(
            subject,
            inputs.source_refinement(),
            inputs.machine_refinement(),
        ),
    )
    .unwrap()
}

#[test]
fn v5_kir_receipt_is_domain_separated_from_frozen_v3() {
    let bytes = b"same-kir-preimage".to_vec();
    let v3 = InertKernelIrReceiptV3::from_canonical_preimage(bytes.clone()).unwrap();
    let v5 = InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(bytes).unwrap();
    assert_ne!(&v5.identity().sha256(), v3.identity().sha256());
    assert!(matches!(
        InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(Vec::new()),
        Err(InertCanonicalKernelIrV13ReceiptErrorV5::EmptyPreimage)
    ));
}

#[test]
fn exact_v5_owner_round_trips_every_coordinate() {
    let owner = fixture(7);
    let decoded = InertCompilerProofOwnerV5::decode(owner.canonical_bytes()).unwrap();
    assert_eq!(decoded.inputs(), owner.inputs());
    assert_eq!(decoded.identity(), owner.identity());
    assert_eq!(decoded.inputs().subject().executable_kir_epoch(), 7);

    let inputs = owner.inputs();
    assert!(matches!(
        InertCompilerProofOwnerInputsV5::new(
            inputs.legacy_proof_binding(),
            inputs.semantic_mir_receipt(),
            inputs.semantic_mir_identity(),
            inputs.executable_kir_receipt(),
            inputs.executable_kir_bytes() + 1,
            inputs.subject(),
            inputs.compiler_policy(),
            inputs.source_refinement(),
            inputs.machine_refinement(),
            inputs.capability_association(),
        ),
        Err(InertCompilerProofOwnerErrorV5::ExecutableKirReceiptLengthMismatch)
    ));
}

#[test]
fn downgrade_and_truncation_fail_closed() {
    let owner = fixture(7);
    let mut downgraded = owner.canonical_bytes().to_vec();
    downgraded[8..10].copy_from_slice(&4_u16.to_le_bytes());
    assert!(matches!(
        InertCompilerProofOwnerV5::decode(&downgraded),
        Err(InertCompilerProofOwnerErrorV5::UnsupportedVersion)
    ));
    assert!(InertCompilerProofOwnerV5::decode(&owner.canonical_bytes()[..40]).is_err());
}

#[test]
fn multi_root_owner_binds_exact_lineage_and_rejects_hostile_rosters() {
    let legacy = fixture(7);
    let inputs = legacy.inputs();
    let first = inputs.subject();
    let second = CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(digest(3)),
        KernelRootIdentityV1::from_untrusted_digest(digest(4)),
        first.executable_kir(),
        first.executable_kir_epoch(),
        first.target_model(),
        LaunchContractIdentityV1::from_untrusted_digest(digest(6)),
    )
    .unwrap();
    let lineage = proof_lineage(&[first, second]);
    let owner =
        InertCompilerProofOwnerV5::new_multi_root(inputs, &lineage, vec![first, second]).unwrap();
    let decoded = InertCompilerProofOwnerV5::decode(owner.canonical_bytes()).unwrap();
    assert_eq!(decoded.subjects(), [first, second]);
    assert_eq!(decoded.proof_lineage(), Some(lineage.identity()));
    assert_eq!(decoded.selected_subject_ordinal(), 0);
    assert_eq!(decoded.selected_subject(), first);

    let second_inputs = inputs_for_subject(inputs, second);
    let second_owner = InertCompilerProofOwnerV5::new_multi_root_at_ordinal(
        second_inputs,
        &lineage,
        vec![first, second],
        1,
    )
    .unwrap();
    let second_decoded = InertCompilerProofOwnerV5::decode(second_owner.canonical_bytes()).unwrap();
    assert_eq!(second_decoded.subjects(), [first, second]);
    assert_eq!(second_decoded.selected_subject_ordinal(), 1);
    assert_eq!(second_decoded.selected_subject(), second);
    assert_eq!(second_decoded.inputs().subject(), second);
    assert_eq!(second_decoded.proof_lineage(), Some(lineage.identity()));

    let inferred_second =
        InertCompilerProofOwnerV5::new_multi_root(second_inputs, &lineage, vec![first, second])
            .unwrap();
    assert_eq!(inferred_second.selected_subject_ordinal(), 1);

    assert!(matches!(
        InertCompilerProofOwnerV5::new_multi_root_at_ordinal(
            second_inputs,
            &lineage,
            vec![first, second],
            0,
        ),
        Err(InertCompilerProofOwnerErrorV5::RosterSubjectMismatch)
    ));
    assert!(matches!(
        InertCompilerProofOwnerV5::new_multi_root_at_ordinal(
            second_inputs,
            &lineage,
            vec![first, second],
            2,
        ),
        Err(InertCompilerProofOwnerErrorV5::SelectedSubjectOrdinalOutOfRange)
    ));

    for hostile in [vec![first], vec![second, first], vec![first, first]] {
        assert!(InertCompilerProofOwnerV5::new_multi_root(inputs, &lineage, hostile).is_err());
    }
    let stale = CapabilitySubjectV1::new(
        second.kernel(),
        second.root(),
        second.executable_kir(),
        6,
        second.target_model(),
        second.launch_contract(),
    )
    .unwrap();
    assert!(
        InertCompilerProofOwnerV5::new_multi_root(inputs, &lineage, vec![first, stale]).is_err()
    );

    let mut cross_version = owner.canonical_bytes().to_vec();
    cross_version[8..10].copy_from_slice(&4_u16.to_le_bytes());
    assert!(InertCompilerProofOwnerV5::decode(&cross_version).is_err());
}
