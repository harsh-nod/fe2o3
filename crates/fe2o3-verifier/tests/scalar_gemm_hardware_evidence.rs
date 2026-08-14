use fe2o3_artifacts::{DigestAlgorithm, DigestBytes, PayloadDigest};
use fe2o3_verifier::{
    Digest, SCALAR_GEMM_COV6_IMPLICIT_KERNARG_BYTES_V1, SCALAR_GEMM_COVERAGE_PROFILE_V1,
    SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1, SCALAR_GEMM_KERNARG_ALIGNMENT_V1,
    SCALAR_GEMM_ROOT_SYMBOL_V1, SCALAR_GEMM_TARGET_V1, SCALAR_GEMM_TOTAL_KERNARG_BYTES_V1,
    SCALAR_GEMM_WAVEFRONT_SIZE_V1, ScalarGemmAdjacentCanaryObservationV1,
    ScalarGemmArtifactObservationV1, ScalarGemmDispatchObservationV1,
    ScalarGemmFrontendObservationV1, ScalarGemmHardwareCaseExpectationV1,
    ScalarGemmHardwareCaseObservationV1, ScalarGemmHardwareEvidenceErrorV1,
    ScalarGemmHardwareEvidenceExpectationV1, ScalarGemmHardwareEvidenceRecorderV1,
    ScalarGemmHsaLoadObservationV1, ScalarGemmInputImmutabilityObservationV1,
    ScalarGemmKernelAdmissionObservationV1, ScalarGemmOutputObservationV1,
    ScalarGemmUnloadObservationV1, ScalarGemmWorkerExchangeObservationV1,
};

const CHALLENGE: u8 = 1;
const OBSERVER: u8 = 2;
const PORTABLE_MIR: u8 = 3;
const FRONTEND_AUTHORITY: u8 = 4;
const WORKER_EXCHANGE: u8 = 5;
const WORKER_REQUEST: u8 = 6;
const WORKER_RESPONSE: u8 = 7;
const ARTIFACT: u8 = 8;
const KERNEL_ADMISSION: u8 = 9;
const ABI: u8 = 10;
const HSA_LOAD: u8 = 11;

#[derive(Clone)]
struct Fixture {
    expectation: ScalarGemmHardwareEvidenceExpectationV1,
    frontend: ScalarGemmFrontendObservationV1,
    worker: ScalarGemmWorkerExchangeObservationV1,
    artifact: ScalarGemmArtifactObservationV1,
    admission: ScalarGemmKernelAdmissionObservationV1,
    load: ScalarGemmHsaLoadObservationV1,
    cases: Vec<ScalarGemmHardwareCaseObservationV1>,
    unload: ScalarGemmUnloadObservationV1,
}

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn artifact_digest(seed: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([seed; 32]))
}

fn case_expectations() -> Vec<ScalarGemmHardwareCaseExpectationV1> {
    [
        ("zero-m", [0, 257, 3]),
        ("zero-k", [3, 5, 0]),
        ("wg-plus-one", [1, 257, 3]),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, dimensions))| {
        ScalarGemmHardwareCaseExpectationV1::new(
            name,
            dimensions,
            digest(20 + index as u8),
            digest(30 + index as u8),
        )
        .unwrap()
    })
    .collect()
}

fn expectation_with(challenge: u8) -> ScalarGemmHardwareEvidenceExpectationV1 {
    ScalarGemmHardwareEvidenceExpectationV1::new(
        digest(challenge),
        digest(OBSERVER),
        digest(PORTABLE_MIR),
        digest(FRONTEND_AUTHORITY),
        digest(WORKER_EXCHANGE),
        digest(WORKER_REQUEST),
        digest(WORKER_RESPONSE),
        artifact_digest(ARTIFACT),
        8_600,
        digest(KERNEL_ADMISSION),
        digest(ABI),
        case_expectations(),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn admission(
    root_symbol: &str,
    kernel: Digest,
    abi: Digest,
    explicit: u64,
    implicit: u64,
    total: u64,
    alignment: u64,
    workgroup: [u32; 3],
    wavefront: u32,
) -> ScalarGemmKernelAdmissionObservationV1 {
    ScalarGemmKernelAdmissionObservationV1::new(
        root_symbol,
        kernel,
        abi,
        explicit,
        implicit,
        total,
        alignment,
        workgroup,
        wavefront,
    )
}

fn canonical_admission() -> ScalarGemmKernelAdmissionObservationV1 {
    admission(
        SCALAR_GEMM_ROOT_SYMBOL_V1,
        digest(KERNEL_ADMISSION),
        digest(ABI),
        SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1,
        SCALAR_GEMM_COV6_IMPLICIT_KERNARG_BYTES_V1,
        SCALAR_GEMM_TOTAL_KERNARG_BYTES_V1,
        SCALAR_GEMM_KERNARG_ALIGNMENT_V1,
        [256, 1, 1],
        SCALAR_GEMM_WAVEFRONT_SIZE_V1,
    )
}

#[derive(Clone)]
struct CaseMaterial {
    name: String,
    dimensions: [u32; 3],
    input_profile_identity: Digest,
    oracle_identity: Digest,
    dispatch: ScalarGemmDispatchObservationV1,
    inputs: ScalarGemmInputImmutabilityObservationV1,
    output: ScalarGemmOutputObservationV1,
    canaries: ScalarGemmAdjacentCanaryObservationV1,
}

impl CaseMaterial {
    fn observation(self) -> ScalarGemmHardwareCaseObservationV1 {
        ScalarGemmHardwareCaseObservationV1::new(
            self.name,
            self.dimensions,
            self.input_profile_identity,
            self.oracle_identity,
            self.dispatch,
            self.inputs,
            self.output,
            self.canaries,
        )
    }
}

fn case_material(expected: &ScalarGemmHardwareCaseExpectationV1, index: usize) -> CaseMaterial {
    let input_a = digest(40 + index as u8 * 4);
    let input_b = digest(41 + index as u8 * 4);
    let output = digest(42 + index as u8 * 4);
    let completion = digest(43 + index as u8 * 4);
    let dispatch = match expected.expected_groups() {
        None => ScalarGemmDispatchObservationV1::new(false, None, None, 0, false, None, None),
        Some(groups) => ScalarGemmDispatchObservationV1::new(
            true,
            Some(groups),
            Some([256, 1, 1]),
            0,
            true,
            Some(completion),
            Some(digest(HSA_LOAD)),
        ),
    };
    let left_elements = 32;
    let right_elements = 32;
    let output_offset = left_elements * 4;
    let right_offset = output_offset + expected.c_elements() * 4;
    let allocation_byte_len = right_offset + right_elements * 4;
    CaseMaterial {
        name: expected.name().to_owned(),
        dimensions: expected.dimensions(),
        input_profile_identity: expected.input_profile_identity(),
        oracle_identity: expected.oracle_identity(),
        dispatch,
        inputs: ScalarGemmInputImmutabilityObservationV1::new(
            expected.a_elements(),
            input_a,
            input_a,
            expected.b_elements(),
            input_b,
            input_b,
        ),
        output: ScalarGemmOutputObservationV1::new(
            expected.c_elements(),
            output,
            output,
            if expected.dimensions()[2] == 0 {
                expected.c_elements()
            } else {
                0
            },
        ),
        canaries: ScalarGemmAdjacentCanaryObservationV1::new(
            digest(70 + index as u8),
            allocation_byte_len,
            0,
            left_elements,
            output_offset,
            expected.c_elements(),
            right_offset,
            right_elements,
            digest(80 + index as u8 * 2),
            digest(80 + index as u8 * 2),
            digest(81 + index as u8 * 2),
            digest(81 + index as u8 * 2),
        ),
    }
}

fn fixture() -> Fixture {
    let expectation = expectation_with(CHALLENGE);
    let cases = expectation
        .cases()
        .iter()
        .enumerate()
        .map(|(index, expected)| case_material(expected, index).observation())
        .collect();
    Fixture {
        expectation,
        frontend: ScalarGemmFrontendObservationV1::new(
            digest(PORTABLE_MIR),
            digest(FRONTEND_AUTHORITY),
        ),
        worker: ScalarGemmWorkerExchangeObservationV1::new(
            digest(WORKER_EXCHANGE),
            digest(WORKER_REQUEST),
            digest(WORKER_RESPONSE),
        ),
        artifact: ScalarGemmArtifactObservationV1::new(
            artifact_digest(ARTIFACT),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
        ),
        admission: canonical_admission(),
        load: ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(ARTIFACT),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
        ),
        cases,
        unload: ScalarGemmUnloadObservationV1::new(digest(HSA_LOAD), true),
    }
}

fn record_prefix(fixture: &Fixture) -> ScalarGemmHardwareEvidenceRecorderV1 {
    let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
    recorder.record_frontend(fixture.frontend).unwrap();
    recorder.record_worker_exchange(fixture.worker).unwrap();
    recorder.record_artifact(fixture.artifact.clone()).unwrap();
    recorder
        .record_kernel_admission(fixture.admission.clone())
        .unwrap();
    recorder.record_hsa_load(fixture.load.clone()).unwrap();
    recorder
}

fn record_complete(fixture: &Fixture) -> fe2o3_verifier::ScalarGemmProtectedHardwareEvidenceV1 {
    let mut recorder = record_prefix(fixture);
    for case in &fixture.cases {
        recorder.record_case(case.clone()).unwrap();
    }
    recorder.record_unload(fixture.unload).unwrap();
    recorder.finish().unwrap()
}

#[test]
fn complete_record_binds_every_layer_but_makes_no_formal_claim() {
    let fixture = fixture();
    let first = record_complete(&fixture);
    let second = record_complete(&fixture);

    assert_eq!(first, second);
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.to_canonical_bytes(), second.to_canonical_bytes());
    assert_eq!(first.expectation().artifact_byte_len(), 8_600);
    assert_eq!(
        first.expectation().artifact_digest(),
        artifact_digest(ARTIFACT)
    );
    assert_eq!(first.observed_facts().cases().len(), 3);
    assert!(first.observed_facts().unload().released());
    assert!(first.records_caller_reported_observations());
    assert!(first.formal_claims().is_empty());
    assert!(!first.authenticates_observer());
    assert!(!first.proves_memory_safety());
    assert!(!first.proves_race_freedom());
    assert!(!first.proves_universal_functional_correctness());
    assert!(!first.proves_compiler_to_machine_refinement());
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());
    first.validate_against(&fixture.expectation).unwrap();

    let replay_substitution = expectation_with(99);
    assert_eq!(
        first.validate_against(&replay_substitution),
        Err(ScalarGemmHardwareEvidenceErrorV1::ExpectationMismatch)
    );
    assert_ne!(
        fixture.expectation.identity(),
        replay_substitution.identity()
    );
}

#[test]
fn expectation_rejects_zero_duplicate_and_unbounded_inputs() {
    let cases = case_expectations();
    let result = ScalarGemmHardwareEvidenceExpectationV1::new(
        digest(0),
        digest(OBSERVER),
        digest(PORTABLE_MIR),
        digest(FRONTEND_AUTHORITY),
        digest(WORKER_EXCHANGE),
        digest(WORKER_REQUEST),
        digest(WORKER_RESPONSE),
        artifact_digest(ARTIFACT),
        8_600,
        digest(KERNEL_ADMISSION),
        digest(ABI),
        cases.clone(),
    );
    assert_eq!(
        result,
        Err(ScalarGemmHardwareEvidenceErrorV1::ZeroIdentity {
            field: "attempt challenge"
        })
    );

    let mut duplicate = cases;
    duplicate.push(duplicate[0].clone());
    let result = ScalarGemmHardwareEvidenceExpectationV1::new(
        digest(CHALLENGE),
        digest(OBSERVER),
        digest(PORTABLE_MIR),
        digest(FRONTEND_AUTHORITY),
        digest(WORKER_EXCHANGE),
        digest(WORKER_REQUEST),
        digest(WORKER_RESPONSE),
        artifact_digest(ARTIFACT),
        8_600,
        digest(KERNEL_ADMISSION),
        digest(ABI),
        duplicate,
    );
    assert_eq!(
        result,
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateCase {
            name: "zero-m".into()
        })
    );

    assert_eq!(
        ScalarGemmHardwareCaseExpectationV1::new("bad case", [1, 1, 1], digest(1), digest(2)),
        Err(ScalarGemmHardwareEvidenceErrorV1::InvalidCaseName)
    );
    assert_eq!(
        ScalarGemmHardwareCaseExpectationV1::new(
            "grid-too-large",
            [u32::MAX, u32::MAX, 1],
            digest(1),
            digest(2)
        ),
        Err(ScalarGemmHardwareEvidenceErrorV1::CaseGridTooLarge)
    );
}

#[test]
fn missing_and_duplicate_top_level_fields_fail_closed() {
    let fixture = fixture();

    let recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
    assert!(matches!(
        recorder.finish(),
        Err(ScalarGemmHardwareEvidenceErrorV1::MissingField {
            field: "frontend observation"
        })
    ));

    let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
    recorder.record_frontend(fixture.frontend).unwrap();
    assert_eq!(
        recorder.record_frontend(fixture.frontend),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField {
            field: "frontend observation"
        })
    );
    recorder.record_worker_exchange(fixture.worker).unwrap();
    assert_eq!(
        recorder.record_worker_exchange(fixture.worker),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField {
            field: "Worker V2 exchange observation"
        })
    );
    recorder.record_artifact(fixture.artifact.clone()).unwrap();
    assert_eq!(
        recorder.record_artifact(fixture.artifact.clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField {
            field: "artifact observation"
        })
    );
    recorder
        .record_kernel_admission(fixture.admission.clone())
        .unwrap();
    assert_eq!(
        recorder.record_kernel_admission(fixture.admission.clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField {
            field: "kernel admission observation"
        })
    );
    recorder.record_hsa_load(fixture.load.clone()).unwrap();
    assert_eq!(
        recorder.record_hsa_load(fixture.load.clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField {
            field: "HSA load observation"
        })
    );
    for case in &fixture.cases {
        recorder.record_case(case.clone()).unwrap();
    }
    assert_eq!(
        recorder.record_case(fixture.cases[0].clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateCase {
            name: "zero-m".into()
        })
    );
    recorder.record_unload(fixture.unload).unwrap();
    assert_eq!(
        recorder.record_unload(fixture.unload),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField {
            field: "HSA unload observation"
        })
    );
}

#[test]
fn load_and_unload_require_complete_ordered_lifecycle() {
    let fixture = fixture();
    let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
    assert_eq!(
        recorder.record_hsa_load(fixture.load.clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::MissingField {
            field: "frontend observation"
        })
    );

    let mut recorder = record_prefix(&fixture);
    assert_eq!(
        recorder.record_unload(fixture.unload),
        Err(ScalarGemmHardwareEvidenceErrorV1::MissingHardwareCases {
            expected: 3,
            observed: 0
        })
    );
    recorder.record_case(fixture.cases[0].clone()).unwrap();
    assert_eq!(
        recorder.record_case(fixture.cases[0].clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateCase {
            name: "zero-m".into()
        })
    );

    let mut recorder = record_prefix(&fixture);
    assert_eq!(
        recorder.record_case(fixture.cases[1].clone()),
        Err(ScalarGemmHardwareEvidenceErrorV1::CaseOrderMismatch {
            expected: "zero-m".into(),
            observed: "zero-k".into()
        })
    );
}

#[test]
fn frontend_and_worker_identity_substitution_fail_closed() {
    let fixture = fixture();
    for frontend in [
        ScalarGemmFrontendObservationV1::new(digest(99), digest(FRONTEND_AUTHORITY)),
        ScalarGemmFrontendObservationV1::new(digest(PORTABLE_MIR), digest(99)),
    ] {
        let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
        assert!(matches!(
            recorder.record_frontend(frontend),
            Err(ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch { .. })
        ));
    }
    for worker in [
        ScalarGemmWorkerExchangeObservationV1::new(
            digest(99),
            digest(WORKER_REQUEST),
            digest(WORKER_RESPONSE),
        ),
        ScalarGemmWorkerExchangeObservationV1::new(
            digest(WORKER_EXCHANGE),
            digest(99),
            digest(WORKER_RESPONSE),
        ),
        ScalarGemmWorkerExchangeObservationV1::new(
            digest(WORKER_EXCHANGE),
            digest(WORKER_REQUEST),
            digest(99),
        ),
    ] {
        let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
        assert!(matches!(
            recorder.record_worker_exchange(worker),
            Err(ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch { .. })
        ));
    }
}

#[test]
fn artifact_digest_length_target_and_cov6_are_exact() {
    let fixture = fixture();
    let bad = [
        (
            ScalarGemmArtifactObservationV1::new(
                artifact_digest(99),
                8_600,
                SCALAR_GEMM_TARGET_V1,
                SCALAR_GEMM_COVERAGE_PROFILE_V1,
            ),
            ScalarGemmHardwareEvidenceErrorV1::ArtifactDigestMismatch,
        ),
        (
            ScalarGemmArtifactObservationV1::new(
                artifact_digest(ARTIFACT),
                8_599,
                SCALAR_GEMM_TARGET_V1,
                SCALAR_GEMM_COVERAGE_PROFILE_V1,
            ),
            ScalarGemmHardwareEvidenceErrorV1::ArtifactLengthMismatch,
        ),
        (
            ScalarGemmArtifactObservationV1::new(
                artifact_digest(ARTIFACT),
                8_600,
                "gfx942:xnack+",
                SCALAR_GEMM_COVERAGE_PROFILE_V1,
            ),
            ScalarGemmHardwareEvidenceErrorV1::TextMismatch { field: "target" },
        ),
        (
            ScalarGemmArtifactObservationV1::new(
                artifact_digest(ARTIFACT),
                8_600,
                SCALAR_GEMM_TARGET_V1,
                "COV5",
            ),
            ScalarGemmHardwareEvidenceErrorV1::TextMismatch {
                field: "coverage profile",
            },
        ),
    ];
    for (artifact, expected) in bad {
        let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
        assert_eq!(recorder.record_artifact(artifact), Err(expected));
    }
}

#[test]
fn admitted_symbol_abi_and_machine_profile_are_exact() {
    let fixture = fixture();
    let candidates = [
        admission(
            "scalar_gemm_v2",
            digest(KERNEL_ADMISSION),
            digest(ABI),
            64,
            256,
            320,
            16,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(99),
            digest(ABI),
            64,
            256,
            320,
            16,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(99),
            64,
            256,
            320,
            16,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
            63,
            256,
            320,
            16,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
            64,
            255,
            320,
            16,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
            64,
            256,
            319,
            16,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
            64,
            256,
            320,
            8,
            [256, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
            64,
            256,
            320,
            16,
            [64, 1, 1],
            64,
        ),
        admission(
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
            64,
            256,
            320,
            16,
            [256, 1, 1],
            32,
        ),
    ];
    for candidate in candidates {
        let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
        assert!(recorder.record_kernel_admission(candidate).is_err());
    }
}

#[test]
fn load_rebinds_artifact_target_kernel_and_abi() {
    let fixture = fixture();
    let candidates = [
        ScalarGemmHsaLoadObservationV1::new(
            digest(0),
            artifact_digest(ARTIFACT),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
        ),
        ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(99),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
        ),
        ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(ARTIFACT),
            8_601,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
        ),
        ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(ARTIFACT),
            8_600,
            "gfx942:xnack+",
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(KERNEL_ADMISSION),
            digest(ABI),
        ),
        ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(ARTIFACT),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            "COV5",
            digest(KERNEL_ADMISSION),
            digest(ABI),
        ),
        ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(ARTIFACT),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(99),
            digest(ABI),
        ),
        ScalarGemmHsaLoadObservationV1::new(
            digest(HSA_LOAD),
            artifact_digest(ARTIFACT),
            8_600,
            SCALAR_GEMM_TARGET_V1,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            digest(KERNEL_ADMISSION),
            digest(99),
        ),
    ];
    for load in candidates {
        let mut recorder = ScalarGemmHardwareEvidenceRecorderV1::new(fixture.expectation.clone());
        recorder.record_frontend(fixture.frontend).unwrap();
        recorder.record_worker_exchange(fixture.worker).unwrap();
        recorder.record_artifact(fixture.artifact.clone()).unwrap();
        recorder
            .record_kernel_admission(fixture.admission.clone())
            .unwrap();
        assert!(recorder.record_hsa_load(load).is_err());
    }
}

fn assert_case_rejected(
    fixture: &Fixture,
    index: usize,
    edit: impl FnOnce(&mut CaseMaterial),
    expected: ScalarGemmHardwareEvidenceErrorV1,
) {
    let mut recorder = record_prefix(fixture);
    for earlier in &fixture.cases[..index] {
        recorder.record_case(earlier.clone()).unwrap();
    }
    let mut material = case_material(&fixture.expectation.cases()[index], index);
    edit(&mut material);
    assert_eq!(recorder.record_case(material.observation()), Err(expected));
}

#[test]
fn case_dimensions_profiles_geometry_and_completion_are_exact() {
    let fixture = fixture();
    assert_case_rejected(
        &fixture,
        0,
        |case| case.dimensions = [1, 257, 3],
        ScalarGemmHardwareEvidenceErrorV1::CaseDimensionsMismatch {
            name: "zero-m".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        0,
        |case| case.input_profile_identity = digest(99),
        ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch {
            field: "case input profile",
        },
    );
    assert_case_rejected(
        &fixture,
        0,
        |case| case.oracle_identity = digest(99),
        ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch {
            field: "case bit-exact oracle",
        },
    );
    assert_case_rejected(
        &fixture,
        0,
        |case| {
            case.dispatch = ScalarGemmDispatchObservationV1::new(
                true,
                Some([1, 1, 1]),
                Some([256, 1, 1]),
                0,
                true,
                Some(digest(90)),
                Some(digest(HSA_LOAD)),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::DispatchStateMismatch {
            name: "zero-m".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.dispatch = ScalarGemmDispatchObservationV1::new(
                true,
                Some([1, 1, 1]),
                Some([256, 1, 1]),
                0,
                true,
                Some(digest(90)),
                Some(digest(HSA_LOAD)),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::GeometryMismatch {
            name: "wg-plus-one".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.dispatch = ScalarGemmDispatchObservationV1::new(
                true,
                Some([2, 1, 1]),
                Some([256, 1, 1]),
                0,
                false,
                Some(digest(90)),
                Some(digest(HSA_LOAD)),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::IncompleteDispatch {
            name: "wg-plus-one".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.dispatch = ScalarGemmDispatchObservationV1::new(
                true,
                Some([2, 1, 1]),
                Some([256, 1, 1]),
                0,
                true,
                None,
                Some(digest(HSA_LOAD)),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::MissingCaseField {
            name: "wg-plus-one".into(),
            field: "completion identity",
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.dispatch = ScalarGemmDispatchObservationV1::new(
                true,
                Some([2, 1, 1]),
                Some([256, 1, 1]),
                0,
                true,
                Some(digest(90)),
                Some(digest(99)),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch {
            field: "completed HSA load",
        },
    );
}

#[test]
fn bitwise_input_output_and_zero_k_checks_fail_closed() {
    let fixture = fixture();
    let zero_k = &fixture.expectation.cases()[1];
    assert_case_rejected(
        &fixture,
        1,
        |case| {
            case.inputs = ScalarGemmInputImmutabilityObservationV1::new(
                zero_k.a_elements() + 1,
                digest(40),
                digest(40),
                zero_k.b_elements(),
                digest(41),
                digest(41),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::InputExtentMismatch {
            name: "zero-k".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        1,
        |case| {
            case.inputs = ScalarGemmInputImmutabilityObservationV1::new(
                zero_k.a_elements(),
                digest(40),
                digest(99),
                zero_k.b_elements(),
                digest(41),
                digest(41),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::InputMutation {
            name: "zero-k".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        1,
        |case| {
            case.output = ScalarGemmOutputObservationV1::new(
                zero_k.c_elements(),
                digest(42),
                digest(99),
                zero_k.c_elements(),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::OutputMismatch {
            name: "zero-k".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        1,
        |case| {
            case.output = ScalarGemmOutputObservationV1::new(
                zero_k.c_elements(),
                digest(42),
                digest(42),
                zero_k.c_elements() - 1,
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::ZeroKNotPositiveZero {
            name: "zero-k".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        1,
        |case| {
            case.output = ScalarGemmOutputObservationV1::new(
                zero_k.c_elements(),
                digest(42),
                digest(42),
                zero_k.c_elements() + 1,
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::InvalidPositiveZeroCount {
            name: "zero-k".into(),
        },
    );
}

#[test]
fn canaries_must_be_nonempty_adjacent_same_allocation_observations() {
    let fixture = fixture();
    let expected = &fixture.expectation.cases()[2];
    let base = case_material(expected, 2).canaries;
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.canaries = ScalarGemmAdjacentCanaryObservationV1::new(
                digest(0),
                (64 + expected.c_elements()) * 4,
                0,
                32,
                128,
                expected.c_elements(),
                128 + expected.c_elements() * 4,
                32,
                digest(84),
                digest(84),
                digest(85),
                digest(85),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::ZeroIdentity {
            field: "guarded output allocation",
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.canaries = ScalarGemmAdjacentCanaryObservationV1::new(
                base.allocation_identity(),
                (64 + expected.c_elements()) * 4,
                0,
                0,
                128,
                expected.c_elements(),
                128 + expected.c_elements() * 4,
                32,
                digest(84),
                digest(84),
                digest(85),
                digest(85),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::EmptyCanary {
            name: "wg-plus-one".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.canaries = ScalarGemmAdjacentCanaryObservationV1::new(
                base.allocation_identity(),
                (64 + expected.c_elements()) * 4,
                0,
                32,
                132,
                expected.c_elements(),
                132 + expected.c_elements() * 4,
                32,
                digest(84),
                digest(84),
                digest(85),
                digest(85),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::CanariesNotAdjacent {
            name: "wg-plus-one".into(),
        },
    );
    assert_case_rejected(
        &fixture,
        2,
        |case| {
            case.canaries = ScalarGemmAdjacentCanaryObservationV1::new(
                base.allocation_identity(),
                (64 + expected.c_elements()) * 4,
                0,
                32,
                128,
                expected.c_elements(),
                128 + expected.c_elements() * 4,
                32,
                digest(84),
                digest(99),
                digest(85),
                digest(85),
            )
        },
        ScalarGemmHardwareEvidenceErrorV1::CanaryMutation {
            name: "wg-plus-one".into(),
        },
    );
}

#[test]
fn unload_must_release_the_exact_loaded_object() {
    let fixture = fixture();
    let mut recorder = record_prefix(&fixture);
    for case in &fixture.cases {
        recorder.record_case(case.clone()).unwrap();
    }
    assert_eq!(
        recorder.record_unload(ScalarGemmUnloadObservationV1::new(digest(99), true)),
        Err(ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch {
            field: "unloaded HSA object"
        })
    );
    assert_eq!(
        recorder.record_unload(ScalarGemmUnloadObservationV1::new(digest(HSA_LOAD), false)),
        Err(ScalarGemmHardwareEvidenceErrorV1::UnloadNotReleased)
    );
}
