use fe2o3_kernel_ir::{GeneralGemmKirV1, GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotV1};
use fe2o3_verifier::{
    GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1, GeneralGemmEvidenceIdentityV1,
    GeneralGemmGfx942MfmaMnemonicV1, GeneralGemmKirModelCorrespondenceV1,
    GeneralGemmNumericalCorrespondenceStatusV1, GeneralGemmNumericalCorrespondenceV1,
    GeneralGemmNumericalLateMachineClaimV1, GeneralGemmNumericalLateMachineErrorV1,
    GeneralGemmNumericalLateMachineFieldV1, GeneralGemmNumericalMachineIdentityAxisV1,
    GeneralGemmNumericalMachineJoinV1, GeneralGemmProofRequestV1, GeneralGemmProofScheduleV1,
    check_general_gemm_kir_model_correspondence_v1, check_general_gemm_numerical_correspondence_v1,
    check_general_gemm_numerical_late_machine_binding_v1,
    derive_general_gemm_kir_model_correspondence_claim_v1,
    derive_general_gemm_numerical_correspondence_claim_v1,
    derive_general_gemm_numerical_late_machine_claim_v1,
};

fn identity(seed: u8) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([seed; 32])
}

fn flip(identity: GeneralGemmEvidenceIdentityV1) -> GeneralGemmEvidenceIdentityV1 {
    let mut bytes = *identity.as_bytes();
    bytes[0] ^= 0x80;
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(bytes)
}

fn plan() -> GeneralGemmPlanFieldsV1 {
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [33, 35, 19],
        strides: [23, 41, 43],
        storage_elements: [755, 773, 1411],
        block_counts: [3, 3, 1],
        aql_grid_work_items: [192, 3, 1],
        reduction_phases: 2,
        alpha_bits: 1.25_f32.to_bits(),
        beta_bits: (-0.5_f32).to_bits(),
    })
    .unwrap()
}

fn request(schedule: GeneralGemmProofScheduleV1) -> GeneralGemmProofRequestV1 {
    GeneralGemmProofRequestV1::checked(
        schedule,
        identity(1),
        identity(2),
        identity(3),
        identity(4),
        identity(5),
        identity(6),
        identity(7),
        identity(8),
        identity(9),
        identity(10),
        identity(11),
    )
    .unwrap()
}

fn kir_correspondence(schedule: GeneralGemmProofScheduleV1) -> GeneralGemmKirModelCorrespondenceV1 {
    let kir = GeneralGemmKirV1::canonical(plan());
    let proof_request = request(schedule);
    let claim = derive_general_gemm_kir_model_correspondence_claim_v1(&kir, proof_request).unwrap();
    check_general_gemm_kir_model_correspondence_v1(&kir, proof_request, claim).unwrap()
}

fn numerical_correspondence(
    schedule: GeneralGemmProofScheduleV1,
) -> GeneralGemmNumericalCorrespondenceV1 {
    let kir = kir_correspondence(schedule);
    let claim = derive_general_gemm_numerical_correspondence_claim_v1(&kir).unwrap();
    check_general_gemm_numerical_correspondence_v1(kir, claim).unwrap()
}

fn machine_join(
    numerical_correspondence_identity: GeneralGemmEvidenceIdentityV1,
) -> GeneralGemmNumericalMachineJoinV1 {
    GeneralGemmNumericalMachineJoinV1 {
        numerical_correspondence_identity,
        owner_bound_pliron_graph_serialization_identity: Some(identity(101)),
        direct_llvm_worker_request_response_identity: Some(identity(102)),
        finalizer_post_link_isa_result_identity: Some(identity(103)),
    }
}

fn exact_claim() -> (
    GeneralGemmNumericalCorrespondenceV1,
    GeneralGemmNumericalLateMachineClaimV1,
) {
    let correspondence =
        numerical_correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
    let join = machine_join(correspondence.identity());
    let claim = derive_general_gemm_numerical_late_machine_claim_v1(&correspondence, join).unwrap();
    (correspondence, claim)
}

fn mismatch_field(
    correspondence: GeneralGemmNumericalCorrespondenceV1,
    claim: GeneralGemmNumericalLateMachineClaimV1,
) -> GeneralGemmNumericalLateMachineFieldV1 {
    let join = machine_join(correspondence.identity());
    match check_general_gemm_numerical_late_machine_binding_v1(correspondence, join, claim)
        .unwrap_err()
    {
        GeneralGemmNumericalLateMachineErrorV1::FieldMismatch(field) => field,
        other => panic!("expected field mismatch, got {other:?}"),
    }
}

type ClaimMutation = fn(&mut GeneralGemmNumericalLateMachineClaimV1);
type ErrorMutation = (GeneralGemmNumericalLateMachineErrorV1, ClaimMutation);

#[test]
fn both_schedules_bind_exact_context_without_promoting_numerical_authority() {
    let mut identities = Vec::new();
    for schedule in [
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        let correspondence = numerical_correspondence(schedule);
        let join = machine_join(correspondence.identity());
        let claim =
            derive_general_gemm_numerical_late_machine_claim_v1(&correspondence, join).unwrap();
        assert_eq!(claim.schedule, schedule);
        assert_eq!(claim.mfma_mnemonic.as_str(), "v_mfma_f32_16x16x16_bf16");
        assert!(claim.machine_join.has_all_required_identities());
        assert!(!claim.machine_join.grants_compiler_authority());
        assert_eq!(
            claim
                .properties
                .iter()
                .filter(|fact| {
                    fact.status == GeneralGemmNumericalCorrespondenceStatusV1::Proved
                })
                .count(),
            1
        );
        assert_eq!(
            claim
                .properties
                .iter()
                .filter(|fact| {
                    fact.status == GeneralGemmNumericalCorrespondenceStatusV1::ModelOnly
                })
                .count(),
            2
        );
        assert_eq!(
            claim
                .properties
                .iter()
                .filter(|fact| {
                    fact.status == GeneralGemmNumericalCorrespondenceStatusV1::Contracted
                })
                .count(),
            5
        );
        assert_eq!(
            claim
                .properties
                .iter()
                .filter(|fact| {
                    fact.status == GeneralGemmNumericalCorrespondenceStatusV1::Unsupported
                })
                .count(),
            3
        );
        assert_eq!(
            claim.properties.len(),
            GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1
        );

        let binding =
            check_general_gemm_numerical_late_machine_binding_v1(correspondence, join, claim)
                .unwrap();
        assert_eq!(binding.claim(), claim);
        assert_eq!(binding.machine_join(), join);
        assert!(!binding.grants_compiler_authority());
        assert!(!binding.can_enter_compiler_proof_gate());
        assert!(!binding.grants_artifact_or_runtime_authority());
        assert_ne!(binding.identity().as_bytes(), &[0; 32]);
        identities.push(binding.identity());
    }
    assert_ne!(identities[0], identities[1]);
}

#[test]
fn every_missing_machine_axis_fails_separately_before_binding() {
    use GeneralGemmNumericalMachineIdentityAxisV1 as Axis;

    let cases = [
        (
            Axis::OwnerBoundPlironGraphSerialization,
            GeneralGemmNumericalMachineJoinV1 {
                owner_bound_pliron_graph_serialization_identity: None,
                ..machine_join(identity(200))
            },
        ),
        (
            Axis::DirectLlvmWorkerRequestResponse,
            GeneralGemmNumericalMachineJoinV1 {
                direct_llvm_worker_request_response_identity: None,
                ..machine_join(identity(200))
            },
        ),
        (
            Axis::FinalizerPostLinkIsaResult,
            GeneralGemmNumericalMachineJoinV1 {
                finalizer_post_link_isa_result_identity: None,
                ..machine_join(identity(200))
            },
        ),
    ];
    for (axis, join) in cases {
        let correspondence =
            numerical_correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
        assert_eq!(
            derive_general_gemm_numerical_late_machine_claim_v1(&correspondence, join).unwrap_err(),
            GeneralGemmNumericalLateMachineErrorV1::MissingMachineIdentity(axis)
        );
    }
}

#[test]
fn every_zero_machine_axis_fails_separately_before_binding() {
    use GeneralGemmNumericalMachineIdentityAxisV1 as Axis;

    let zero = Some(identity(0));
    let cases = [
        (
            Axis::OwnerBoundPlironGraphSerialization,
            GeneralGemmNumericalMachineJoinV1 {
                owner_bound_pliron_graph_serialization_identity: zero,
                ..machine_join(identity(200))
            },
        ),
        (
            Axis::DirectLlvmWorkerRequestResponse,
            GeneralGemmNumericalMachineJoinV1 {
                direct_llvm_worker_request_response_identity: zero,
                ..machine_join(identity(200))
            },
        ),
        (
            Axis::FinalizerPostLinkIsaResult,
            GeneralGemmNumericalMachineJoinV1 {
                finalizer_post_link_isa_result_identity: zero,
                ..machine_join(identity(200))
            },
        ),
    ];
    for (axis, join) in cases {
        let correspondence =
            numerical_correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
        assert_eq!(
            derive_general_gemm_numerical_late_machine_claim_v1(&correspondence, join).unwrap_err(),
            GeneralGemmNumericalLateMachineErrorV1::ZeroMachineIdentity(axis)
        );
    }
}

#[test]
fn missing_and_zero_machine_axes_in_transported_claims_fail_before_comparison() {
    use GeneralGemmNumericalMachineIdentityAxisV1 as Axis;

    let mutations: [ErrorMutation; 6] = [
        (
            GeneralGemmNumericalLateMachineErrorV1::MissingMachineIdentity(
                Axis::OwnerBoundPlironGraphSerialization,
            ),
            |claim| {
                claim
                    .machine_join
                    .owner_bound_pliron_graph_serialization_identity = None
            },
        ),
        (
            GeneralGemmNumericalLateMachineErrorV1::MissingMachineIdentity(
                Axis::DirectLlvmWorkerRequestResponse,
            ),
            |claim| {
                claim
                    .machine_join
                    .direct_llvm_worker_request_response_identity = None
            },
        ),
        (
            GeneralGemmNumericalLateMachineErrorV1::MissingMachineIdentity(
                Axis::FinalizerPostLinkIsaResult,
            ),
            |claim| claim.machine_join.finalizer_post_link_isa_result_identity = None,
        ),
        (
            GeneralGemmNumericalLateMachineErrorV1::ZeroMachineIdentity(
                Axis::OwnerBoundPlironGraphSerialization,
            ),
            |claim| {
                claim
                    .machine_join
                    .owner_bound_pliron_graph_serialization_identity = Some(identity(0))
            },
        ),
        (
            GeneralGemmNumericalLateMachineErrorV1::ZeroMachineIdentity(
                Axis::DirectLlvmWorkerRequestResponse,
            ),
            |claim| {
                claim
                    .machine_join
                    .direct_llvm_worker_request_response_identity = Some(identity(0))
            },
        ),
        (
            GeneralGemmNumericalLateMachineErrorV1::ZeroMachineIdentity(
                Axis::FinalizerPostLinkIsaResult,
            ),
            |claim| claim.machine_join.finalizer_post_link_isa_result_identity = Some(identity(0)),
        ),
    ];
    for (error, mutate) in mutations {
        let (correspondence, mut claim) = exact_claim();
        mutate(&mut claim);
        let join = machine_join(correspondence.identity());
        assert_eq!(
            check_general_gemm_numerical_late_machine_binding_v1(correspondence, join, claim,)
                .unwrap_err(),
            error
        );
    }
}

#[test]
fn every_context_claim_property_manifest_and_mfma_substitution_fails_closed() {
    use GeneralGemmNumericalLateMachineFieldV1 as Field;

    let cases: [(Field, ClaimMutation); 17] = [
        (Field::SchemaIdentity, |claim| {
            claim.schema_identity = flip(claim.schema_identity)
        }),
        (Field::NumericalClaimSchemaIdentity, |claim| {
            claim.numerical_claim_schema_identity = flip(claim.numerical_claim_schema_identity)
        }),
        (Field::KirCorrespondenceIdentity, |claim| {
            claim.kir_correspondence_identity = flip(claim.kir_correspondence_identity)
        }),
        (Field::KirIdentity, |claim| {
            claim.kir_identity = flip(claim.kir_identity)
        }),
        (Field::ProofRequestIdentity, |claim| {
            claim.proof_request_identity = flip(claim.proof_request_identity)
        }),
        (Field::Schedule, |claim| {
            claim.schedule = GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
        }),
        (Field::ScheduleIdentity, |claim| {
            claim.schedule_identity = flip(claim.schedule_identity)
        }),
        (Field::ScheduleModelIdentity, |claim| {
            claim.schedule_model_identity = flip(claim.schedule_model_identity)
        }),
        (Field::ScheduleTheoremSetIdentity, |claim| {
            claim.schedule_theorem_set_identity = flip(claim.schedule_theorem_set_identity)
        }),
        (Field::ScheduleSourceClosureIdentity, |claim| {
            claim.schedule_source_closure_identity = flip(claim.schedule_source_closure_identity)
        }),
        (Field::PropertyTheoremManifestIdentity, |claim| {
            claim.property_theorem_manifest_identity =
                flip(claim.property_theorem_manifest_identity)
        }),
        (Field::PropertyTheoremBindingIdentities, |claim| {
            claim.property_theorem_binding_identities[0] =
                flip(claim.property_theorem_binding_identities[0])
        }),
        (Field::NumericalTheoremSetIdentity, |claim| {
            claim.numerical_theorem_set_identity = flip(claim.numerical_theorem_set_identity)
        }),
        (Field::NumericalSourceClosureIdentity, |claim| {
            claim.numerical_source_closure_identity = flip(claim.numerical_source_closure_identity)
        }),
        (Field::Properties, |claim| {
            claim.properties[0].status = GeneralGemmNumericalCorrespondenceStatusV1::Contracted
        }),
        (Field::MfmaContractIdentity, |claim| {
            claim.mfma_contract_identity = flip(claim.mfma_contract_identity)
        }),
        (Field::MfmaMnemonic, |claim| {
            claim.mfma_mnemonic = GeneralGemmGfx942MfmaMnemonicV1::UnsupportedOther
        }),
    ];
    for (field, mutate) in cases {
        let (correspondence, mut claim) = exact_claim();
        mutate(&mut claim);
        assert_eq!(mismatch_field(correspondence, claim), field);
    }

    for index in 0..GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1 {
        let (correspondence, mut claim) = exact_claim();
        claim.property_theorem_binding_identities[index] =
            flip(claim.property_theorem_binding_identities[index]);
        assert_eq!(
            mismatch_field(correspondence, claim),
            Field::PropertyTheoremBindingIdentities
        );
    }
}

#[test]
fn every_machine_identity_substitution_fails_on_its_own_axis() {
    use GeneralGemmNumericalLateMachineFieldV1 as Field;

    let cases: [(Field, ClaimMutation); 3] = [
        (Field::OwnerBoundPlironGraphSerializationIdentity, |claim| {
            claim
                .machine_join
                .owner_bound_pliron_graph_serialization_identity = Some(identity(111));
        }),
        (Field::DirectLlvmWorkerRequestResponseIdentity, |claim| {
            claim
                .machine_join
                .direct_llvm_worker_request_response_identity = Some(identity(112));
        }),
        (Field::FinalizerPostLinkIsaResultIdentity, |claim| {
            claim.machine_join.finalizer_post_link_isa_result_identity = Some(identity(113));
        }),
    ];
    for (field, mutate) in cases {
        let (correspondence, mut claim) = exact_claim();
        mutate(&mut claim);
        assert_eq!(mismatch_field(correspondence, claim), field);
    }
}

#[test]
fn stale_correspondence_and_mismatched_expected_machine_join_fail_distinctly() {
    let reference = numerical_correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
    let reference_join = machine_join(reference.identity());
    let reference_claim =
        derive_general_gemm_numerical_late_machine_claim_v1(&reference, reference_join).unwrap();
    let vector =
        numerical_correspondence(GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1);
    let vector_join = machine_join(vector.identity());
    assert_eq!(
        check_general_gemm_numerical_late_machine_binding_v1(vector, vector_join, reference_claim,)
            .unwrap_err(),
        GeneralGemmNumericalLateMachineErrorV1::StaleOrMismatchedNumericalCorrespondenceIdentity
    );

    let current = numerical_correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
    let stale_join = machine_join(identity(222));
    assert_eq!(
        derive_general_gemm_numerical_late_machine_claim_v1(&current, stale_join).unwrap_err(),
        GeneralGemmNumericalLateMachineErrorV1::StaleOrMismatchedNumericalCorrespondenceIdentity
    );

    let current = numerical_correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
    let zero_context_join = machine_join(identity(0));
    assert_eq!(
        derive_general_gemm_numerical_late_machine_claim_v1(&current, zero_context_join)
            .unwrap_err(),
        GeneralGemmNumericalLateMachineErrorV1::ZeroNumericalCorrespondenceIdentity
    );

    let (correspondence, claim) = exact_claim();
    let correspondence_identity = correspondence.identity();
    let expected = GeneralGemmNumericalMachineJoinV1 {
        owner_bound_pliron_graph_serialization_identity: Some(identity(121)),
        ..machine_join(correspondence_identity)
    };
    assert_eq!(
        check_general_gemm_numerical_late_machine_binding_v1(correspondence, expected, claim)
            .unwrap_err(),
        GeneralGemmNumericalLateMachineErrorV1::FieldMismatch(
            GeneralGemmNumericalLateMachineFieldV1::OwnerBoundPlironGraphSerializationIdentity,
        )
    );
}
