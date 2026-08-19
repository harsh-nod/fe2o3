use std::{collections::BTreeSet, path::Path};

use fe2o3_verifier::{
    AuthenticatedGeneralGemmScheduleProofV1, GENERAL_GEMM_PROOF_PROPERTIES_V1,
    GENERAL_GEMM_VERUS_SHA256_V1, GeneralGemmEvidenceIdentityV1,
    GeneralGemmNumericalComparisonPolicyV1, GeneralGemmNumericalPolicyRequestV1,
    GeneralGemmProofExecutionErrorV1, GeneralGemmProofPropertyV1, GeneralGemmProofRequestV1,
    GeneralGemmProofScheduleV1, GeneralGemmPropertyEvidenceBasisV1,
    GeneralGemmPropertyEvidenceStatusV1, execute_general_gemm_numerical_policy_v1,
    execute_general_gemm_schedule_proof_v1, join_general_gemm_proof_and_numerical_evidence_v1,
};

const MODEL: &str = include_str!("../verus/general_gemm_schedule_model_v1.rs");
const REFERENCE: &str = include_str!("../verus/general_gemm_reference_schedule_v1.rs");
const VECTORIZED: &str = include_str!("../verus/general_gemm_vectorized_schedule_v1.rs");

fn identity(seed: u8) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([seed; 32])
}

fn request(schedule: GeneralGemmProofScheduleV1) -> GeneralGemmProofRequestV1 {
    let offset = match schedule {
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1 => 0,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => 32,
    };
    GeneralGemmProofRequestV1::checked(
        schedule,
        identity(offset + 1),
        identity(offset + 2),
        identity(offset + 3),
        identity(offset + 4),
        identity(offset + 5),
        identity(offset + 6),
        identity(offset + 7),
        identity(offset + 8),
        identity(offset + 9),
        identity(offset + 10),
        identity(offset + 11),
    )
    .unwrap()
}

#[test]
fn request_rejects_zero_and_cross_domain_identity_reuse() {
    assert!(
        !request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1)
            .grants_concrete_launch_authority()
    );
    let zero = GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([0; 32]);
    let invalid = GeneralGemmProofRequestV1::checked(
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
        zero,
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
    );
    assert!(matches!(
        invalid,
        Err(GeneralGemmProofExecutionErrorV1::InvalidIdentity)
    ));

    let duplicate = GeneralGemmProofRequestV1::checked(
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
        identity(1),
        identity(1),
        identity(3),
        identity(4),
        identity(5),
        identity(6),
        identity(7),
        identity(8),
        identity(9),
        identity(10),
        identity(11),
    );
    assert!(matches!(
        duplicate,
        Err(GeneralGemmProofExecutionErrorV1::DuplicateIdentity)
    ));
}

#[test]
fn invalid_deadline_is_rejected_before_process_execution() {
    let result = execute_general_gemm_schedule_proof_v1(
        request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
        Path::new("/does/not/exist"),
        0,
    );
    assert!(matches!(
        result,
        Err(GeneralGemmProofExecutionErrorV1::InvalidTimeout)
    ));
}

#[test]
fn proof_sources_have_no_trusted_escape_and_name_a_only_vectorization() {
    for source in [MODEL, REFERENCE, VECTORIZED] {
        for forbidden in ["assume", "admit", "external_body"] {
            assert!(
                !source
                    .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                    .any(|word| word == forbidden),
                "forbidden Verus escape {forbidden}"
            );
        }
    }
    assert!(MODEL.contains("VectorizedAOnlyBf16GlobalTransfer"));
    assert!(!MODEL.contains("vector4_b"));
    assert!(MODEL.contains("machine_refinement_complete_v1() -> bool { false }"));
}

fn assert_property_mapping(proof: &AuthenticatedGeneralGemmScheduleProofV1) {
    assert_eq!(
        proof.properties().len(),
        GENERAL_GEMM_PROOF_PROPERTIES_V1.len()
    );
    let identities: BTreeSet<_> = proof
        .properties()
        .iter()
        .map(|result| *result.identity().as_bytes())
        .collect();
    assert_eq!(identities.len(), GENERAL_GEMM_PROOF_PROPERTIES_V1.len());

    for (expected, result) in GENERAL_GEMM_PROOF_PROPERTIES_V1
        .into_iter()
        .zip(proof.properties())
    {
        assert_eq!(result.property(), expected);
        match expected {
            GeneralGemmProofPropertyV1::MemorySafe
            | GeneralGemmProofPropertyV1::Initialized
            | GeneralGemmProofPropertyV1::RaceFree => {
                assert_eq!(
                    result.status(),
                    GeneralGemmPropertyEvidenceStatusV1::OpenCorrespondenceRequired
                );
                assert!(matches!(
                    result.basis(),
                    GeneralGemmPropertyEvidenceBasisV1::OpenObligation(_)
                ));
            }
            GeneralGemmProofPropertyV1::BarrierConvergent
            | GeneralGemmProofPropertyV1::LdsEpochCorrect => {
                assert_eq!(
                    result.status(),
                    GeneralGemmPropertyEvidenceStatusV1::ModelDefinitionOnly
                );
                assert!(matches!(
                    result.basis(),
                    GeneralGemmPropertyEvidenceBasisV1::ModelDefinition(_)
                ));
            }
            GeneralGemmProofPropertyV1::NumericalContract => {
                assert_eq!(
                    result.status(),
                    GeneralGemmPropertyEvidenceStatusV1::WeakerExactRealTheoremVerified
                );
                assert!(matches!(
                    result.basis(),
                    GeneralGemmPropertyEvidenceBasisV1::VerifiedTheorem(_)
                ));
            }
            GeneralGemmProofPropertyV1::MachineRefinementBoundary => {
                assert_eq!(
                    result.status(),
                    GeneralGemmPropertyEvidenceStatusV1::OpenArtifactRequired
                );
                assert!(matches!(
                    result.basis(),
                    GeneralGemmPropertyEvidenceBasisV1::OpenObligation(_)
                ));
            }
            _ => {
                assert_eq!(
                    result.status(),
                    GeneralGemmPropertyEvidenceStatusV1::ScheduleModelTheoremVerified
                );
                assert!(matches!(
                    result.basis(),
                    GeneralGemmPropertyEvidenceBasisV1::VerifiedTheorem(_)
                ));
            }
        }
    }
    assert!(!proof.can_enter_compiler_proof_gate());
}

#[test]
#[ignore = "requires the exact pinned Verus installation"]
fn pinned_verus_independently_checks_reference_and_a_only_vectorized_schedules() {
    let path = std::env::var_os("FE2O3_GENERAL_GEMM_VERUS")
        .expect("FE2O3_GENERAL_GEMM_VERUS must name the pinned Verus launcher");
    let reference = execute_general_gemm_schedule_proof_v1(
        request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
        Path::new(&path),
        120,
    )
    .unwrap();
    let vectorized = execute_general_gemm_schedule_proof_v1(
        request(GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1),
        Path::new(&path),
        120,
    )
    .unwrap();

    assert_property_mapping(&reference);
    assert_property_mapping(&vectorized);
    assert_eq!(reference.negative_outputs().len(), 2);
    assert_eq!(vectorized.negative_outputs().len(), 3);
    assert_eq!(
        reference.tool_identity().as_bytes(),
        &GENERAL_GEMM_VERUS_SHA256_V1
    );
    assert_eq!(
        vectorized.tool_identity().as_bytes(),
        &GENERAL_GEMM_VERUS_SHA256_V1
    );
    assert_ne!(reference.request(), vectorized.request());
    assert_ne!(
        reference.source_closure_identity(),
        vectorized.source_closure_identity()
    );
    assert_ne!(reference.identity(), vectorized.identity());
    assert!(reference.positive_output().stdout_bytes() > 0);
    assert!(vectorized.positive_output().stdout_bytes() > 0);
}

#[test]
#[ignore = "requires the exact pinned Verus installation"]
fn proof_numerical_join_preserves_all_open_and_weaker_property_records() {
    let path = std::env::var_os("FE2O3_GENERAL_GEMM_VERUS")
        .expect("FE2O3_GENERAL_GEMM_VERUS must name the pinned Verus launcher");
    let request = request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
    let proof = execute_general_gemm_schedule_proof_v1(request, Path::new(&path), 120).unwrap();
    let expected_properties = *proof.properties();
    let numerical_request = GeneralGemmNumericalPolicyRequestV1::checked(
        request.symbolic_compilation_identity(),
        request.symbolic_plan_identity(),
        request.symbolic_kir_identity(),
        request.numerical_policy_identity(),
    )
    .unwrap();
    let numerical = execute_general_gemm_numerical_policy_v1(
        numerical_request,
        GeneralGemmNumericalComparisonPolicyV1::ExactBits,
    )
    .unwrap();
    let evidence = join_general_gemm_proof_and_numerical_evidence_v1(proof, numerical).unwrap();

    assert_eq!(evidence.properties(), &expected_properties);
    assert!(evidence.properties().iter().any(|property| {
        property.status() == GeneralGemmPropertyEvidenceStatusV1::OpenCorrespondenceRequired
    }));
    assert!(evidence.properties().iter().any(|property| {
        property.status() == GeneralGemmPropertyEvidenceStatusV1::WeakerExactRealTheoremVerified
    }));
    assert!(evidence.properties().iter().any(|property| {
        property.status() == GeneralGemmPropertyEvidenceStatusV1::OpenArtifactRequired
    }));
    assert!(!evidence.can_enter_compiler_proof_gate());
    assert!(!evidence.grants_artifact_or_runtime_authority());
}
