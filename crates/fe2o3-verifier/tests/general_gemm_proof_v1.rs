use std::path::Path;

use fe2o3_verifier::{
    GeneralGemmEvidenceIdentityV1, GeneralGemmProofExecutionErrorV1, GeneralGemmProofRequestV1,
    GeneralGemmProofScheduleV1, execute_general_gemm_schedule_proof_v1,
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

#[test]
fn proof_authority_stays_closed_without_an_authenticated_runtime_closure() {
    let path =
        std::env::var_os("FE2O3_GENERAL_GEMM_VERUS").unwrap_or_else(|| "/does/not/exist".into());
    for schedule in [
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        let result =
            execute_general_gemm_schedule_proof_v1(request(schedule), Path::new(&path), 120);
        assert!(matches!(
            result,
            Err(GeneralGemmProofExecutionErrorV1::AuthenticatedRuntimeClosureUnavailable)
        ));
    }
}
