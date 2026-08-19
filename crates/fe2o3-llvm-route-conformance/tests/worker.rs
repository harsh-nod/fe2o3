#![forbid(unsafe_code)]

//! Public worker-admission V1 conformance tests.

use fe2o3_llvm_route_conformance::{
    ConformanceExpectationV1, ConformanceSemanticV1, ExpectedRejectionV1,
    FixtureDeviceLibrarySetV1, GFX942_CONFORMANCE_CORPUS_V1, Gfx942FixtureBuilderV1,
    conformance_case_v1, gfx942_fixture_v1,
};
use fe2o3_llvm_worker_handoff::{
    EXACT_LLD_BUILD_IDENTITY_V1, EXACT_LLD_VERSION_V1, EXACT_LLVM_BUILD_IDENTITY_V1,
    EXACT_LLVM_VERSION_V1, MAX_WORKER_BUILD_VERSION_BYTES_V1, MeasuredLlvmLldBuildV1,
    WorkerAdmissionErrorV1, WorkerAdmissionRequestV1, WorkerBuildFieldV1,
};

const EXERCISED_WORKER_REJECTIONS: [ExpectedRejectionV1; 4] = [
    ExpectedRejectionV1::WorkerHandoffIdentityMismatch,
    ExpectedRejectionV1::WorkerBuildIdentitySubstitution,
    ExpectedRejectionV1::WorkerBuildFieldTooLong,
    ExpectedRejectionV1::WorkerUnsupportedDeviceLibrary,
];

#[test]
fn exact_worker_admission_is_inert_and_preserves_canonical_identities() {
    let case = conformance_case_v1("lane.worker-admission.canonical-inert")
        .expect("worker admission case must be declared");
    assert_eq!(case.expectation(), ConformanceExpectationV1::Represented);

    let fixture = worker_fixture();
    let canonical = fixture.encode_canonical();
    let admitted = WorkerAdmissionRequestV1::new(
        canonical.as_bytes(),
        *fixture.identity().as_bytes(),
        MeasuredLlvmLldBuildV1::exact(),
    )
    .admit()
    .expect("exact worker request must be admitted");

    assert_eq!(admitted.handoff(), &fixture);
    assert_eq!(admitted.handoff_identity(), fixture.identity());
    assert_eq!(admitted.handoff().encode_canonical(), canonical);
    assert_ne!(admitted.admission_identity().as_bytes(), &[0; 32]);
    assert!(!admitted.grants_object_authority());
    assert!(!admitted.grants_link_authority());
    assert!(!admitted.grants_publication_authority());
}

#[test]
fn worker_rejects_handoff_identity_substitution() {
    let fixture = worker_fixture();
    let canonical = fixture.encode_canonical();
    assert_worker_rejection(
        "lane.worker-admission.handoff-identity-mismatch",
        ExpectedRejectionV1::WorkerHandoffIdentityMismatch,
        WorkerAdmissionRequestV1::new(
            canonical.as_bytes(),
            [0x99; 32],
            MeasuredLlvmLldBuildV1::exact(),
        )
        .admit(),
        WorkerAdmissionErrorV1::HandoffIdentityMismatch,
    );
}

#[test]
fn worker_rejects_substituted_and_oversized_build_fields() {
    let fixture = worker_fixture();
    let canonical = fixture.encode_canonical();
    let substituted = MeasuredLlvmLldBuildV1::new(
        "22.1.9",
        EXACT_LLVM_BUILD_IDENTITY_V1,
        EXACT_LLD_VERSION_V1,
        EXACT_LLD_BUILD_IDENTITY_V1,
        true,
    );
    assert_worker_rejection(
        "lane.worker-admission.build-identity-substitution",
        ExpectedRejectionV1::WorkerBuildIdentitySubstitution,
        WorkerAdmissionRequestV1::new(
            canonical.as_bytes(),
            *fixture.identity().as_bytes(),
            substituted,
        )
        .admit(),
        WorkerAdmissionErrorV1::BuildIdentitySubstitution(WorkerBuildFieldV1::LlvmVersion),
    );

    let oversized = "v".repeat(MAX_WORKER_BUILD_VERSION_BYTES_V1 + 1);
    let oversized_build = MeasuredLlvmLldBuildV1::new(
        &oversized,
        EXACT_LLVM_BUILD_IDENTITY_V1,
        EXACT_LLD_VERSION_V1,
        EXACT_LLD_BUILD_IDENTITY_V1,
        true,
    );
    assert_worker_rejection(
        "lane.worker-admission.build-field-too-long",
        ExpectedRejectionV1::WorkerBuildFieldTooLong,
        WorkerAdmissionRequestV1::new(
            canonical.as_bytes(),
            *fixture.identity().as_bytes(),
            oversized_build,
        )
        .admit(),
        WorkerAdmissionErrorV1::BuildFieldTooLong {
            field: WorkerBuildFieldV1::LlvmVersion,
            observed: MAX_WORKER_BUILD_VERSION_BYTES_V1 + 1,
            maximum: MAX_WORKER_BUILD_VERSION_BYTES_V1,
        },
    );
}

#[test]
fn worker_rejects_handoff_library_kinds_outside_its_closed_set() {
    let fixture = gfx942_fixture_v1().expect("full handoff fixture must remain valid");
    let canonical = fixture.encode_canonical();
    assert_worker_rejection(
        "lane.worker-admission.unsupported-device-library",
        ExpectedRejectionV1::WorkerUnsupportedDeviceLibrary,
        WorkerAdmissionRequestV1::new(
            canonical.as_bytes(),
            *fixture.identity().as_bytes(),
            MeasuredLlvmLldBuildV1::exact(),
        )
        .admit(),
        WorkerAdmissionErrorV1::UnsupportedDeviceLibrary(
            fe2o3_llvm_handoff::DeviceLibraryKindV1::Ockl,
        ),
    );
}

#[test]
fn every_declared_worker_rejection_has_an_exercised_case() {
    let declared = GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .filter(|case| case.semantic() == ConformanceSemanticV1::WorkerAdmissionLane)
        .filter_map(|case| match case.expectation() {
            ConformanceExpectationV1::ExpectedRejection(rejection) => Some(rejection),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, EXERCISED_WORKER_REJECTIONS);
}

fn worker_fixture() -> fe2o3_llvm_handoff::Gfx942HandoffV1 {
    Gfx942FixtureBuilderV1::new()
        .with_device_libraries(FixtureDeviceLibrarySetV1::WorkerSupportedClosure)
        .build()
        .expect("worker-supported fixture must remain valid")
}

fn assert_worker_rejection(
    name: &str,
    rejection: ExpectedRejectionV1,
    actual: Result<fe2o3_llvm_worker_handoff::AdmittedWorkerRequestV1, WorkerAdmissionErrorV1>,
    expected: WorkerAdmissionErrorV1,
) {
    let case = conformance_case_v1(name).expect("worker hostile case must be declared");
    assert_eq!(
        case.expectation(),
        ConformanceExpectationV1::ExpectedRejection(rejection)
    );
    assert_eq!(actual, Err(expected));
}

#[test]
fn exact_build_observation_matches_the_public_worker_constants() {
    let exact = MeasuredLlvmLldBuildV1::exact();
    assert_eq!(exact.llvm_version(), EXACT_LLVM_VERSION_V1);
    assert_eq!(exact.llvm_build_identity(), EXACT_LLVM_BUILD_IDENTITY_V1);
    assert_eq!(exact.lld_version(), EXACT_LLD_VERSION_V1);
    assert_eq!(exact.lld_build_identity(), EXACT_LLD_BUILD_IDENTITY_V1);
    assert!(exact.in_process_lld());
}
