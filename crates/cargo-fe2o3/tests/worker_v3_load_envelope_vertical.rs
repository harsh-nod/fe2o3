#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use std::{convert::Infallible, sync::OnceLock};

use fe2o3_artifact_transaction::retire_worker_v3_publication_intent_after_load_readiness_v1;
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{
    __hardware_test::application_handoff_observed_context_fixture_v1,
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKernelProfileV1, CompilerGeneratedSemanticWitnessErrorV1,
    RecoveredWorkerV3AdmissionErrorV1, ValidatedCompilerGeneratedSemanticWitnessV1,
    WorkerV3SafetyPropertiesV1, WorkerV3VerificationAuthenticationErrorV1,
    WorkerV3VerificationDecisionErrorV1, WorkerV3VerificationDecisionV1,
    WorkerV3VerificationRequestV1, WorkerV3VerifierV1, admit_recovered_worker_v3_descriptor_v1,
    semantic_witness_from_backend_v1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_worker_v2_bundle::{
    RecoveredWorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeWireV1,
    recover_worker_v3_load_envelope_v1,
};
use reserved_fe2o3_symbols::{
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
};

#[path = "../../fe2o3-hsaco-finalize/tests/worker_v3_hsaco_admission.rs"]
mod worker_v3_fixture;

const TEST_MARKER_BINDING: [u8; 32] = [0xb1; 32];
const TEST_HOST_CONTRACT: [u8; 32] = [0xb2; 32];

struct WorkerV3VecAddMarker;

fn worker_v3_marker_function() {}

unsafe impl KernelMarkerV1 for WorkerV3VecAddMarker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "vecadd";
    const EXPORT_NAME: &'static str = "vecadd";
    const FUNCTION: Self::Function = worker_v3_marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

unsafe impl CompilerGeneratedKernelExpectationV1 for WorkerV3VecAddMarker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: TEST_HOST_CONTRACT,
        };
    const KERNEL_BINDING_ID_V1: [u8; 32] = TEST_MARKER_BINDING;

    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        static WITNESS: OnceLock<Vec<u8>> = OnceLock::new();
        let bytes = WITNESS.get_or_init(|| {
            let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
            let length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();
            let mut bytes = Vec::with_capacity(length);
            bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
            bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
            bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
            bytes.extend_from_slice(&(length as u32).to_le_bytes());
            bytes.extend_from_slice(&TEST_MARKER_BINDING);
            bytes.extend_from_slice(&TEST_HOST_CONTRACT);
            bytes.extend_from_slice(&(profile.len() as u16).to_le_bytes());
            bytes.extend_from_slice(profile);
            assert_eq!(bytes.len(), length);
            bytes
        });
        // SAFETY: `OnceLock` retains these immutable initialized bytes for the process lifetime.
        unsafe {
            semantic_witness_from_backend_v1(
                bytes.as_ptr(),
                bytes.len(),
                TEST_MARKER_BINDING,
                TEST_HOST_CONTRACT,
            )
        }
    }
}

struct ReviewedTestWorkerV3Verifier {
    substitute_finalized: bool,
}

unsafe impl WorkerV3VerifierV1<WorkerV3VecAddMarker> for ReviewedTestWorkerV3Verifier {
    type Error = Infallible;

    unsafe fn verify(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, WorkerV3VecAddMarker>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        let mut finalized = request.finalized_hsaco_sha256();
        if self.substitute_finalized {
            finalized[0] ^= 0xff;
        }
        Ok(WorkerV3VerificationDecisionV1::new(
            request.challenge_identity(),
            request.lineage_identity(),
            request.descriptor().kernel_id(),
            request.marker_binding_identity(),
            request.generated_host_contract_identity(),
            request.capsule_sha256(),
            request.formal_memory_receipt_sha256(),
            request.proof_binding_receipt_sha256(),
            finalized,
            request.finalized_hsaco_length(),
            request.target(),
            request.code_object_version(),
            [0xc1; 32],
            [0xc2; 32],
            [0xc3; 32],
            [0xc4; 32],
            [0xc5; 32],
            WorkerV3SafetyPropertiesV1::required(),
        ))
    }
}

fn recovered_host_fixture() -> (
    worker_v3_fixture::TestDirectory,
    RecoveredWorkerV3LoadEnvelopeV1,
) {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    let intent = envelope.wire().publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&directory.0)
        .unwrap();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);
    let recovered = recover_worker_v3_load_envelope_v1(&directory.0, attempt).unwrap();
    (directory, recovered)
}

#[test]
fn completed_v3_publication_becomes_restartable_inert_envelope_custody() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let output_dir = directory.0.clone();
    let exact_artifact = published
        .recovered_evidence()
        .exact_finalized_hsaco()
        .to_vec();

    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    assert_eq!(envelope.exact_artifact_bytes(), exact_artifact);
    assert!(!envelope.grants_load_authority());
    assert!(!envelope.grants_launch_authority());

    let canonical = envelope.encode_canonical().unwrap();
    let inert = WorkerV3LoadEnvelopeWireV1::decode_canonical(&canonical).unwrap();
    inert
        .validate_reacquired_publication_lease_v1(envelope.current_publication_lease())
        .unwrap();
    assert_eq!(inert.encode_canonical().unwrap(), canonical);
    assert!(!inert.grants_publication_authority());
    assert!(!inert.grants_load_authority());
    assert!(!inert.grants_launch_authority());

    let intent = inert.publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&output_dir)
        .unwrap();
    assert_eq!(readiness.exact_envelope_bytes(), canonical);
    assert!(!readiness.authenticates_descriptor_source());
    assert!(!readiness.establishes_hsa_readiness());
    assert!(!readiness.grants_load_authority());
    assert!(!readiness.grants_launch_authority());

    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &output_dir,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);

    let recovered = recover_worker_v3_load_envelope_v1(&output_dir, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
    assert_eq!(recovered.wire().encode_canonical().unwrap(), canonical);
    assert_eq!(recovered.exact_artifact_bytes(), exact_artifact);
    assert!(!recovered.authenticates_descriptor_source());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    assert_eq!(admitted.descriptor().entry_name().as_str(), "vecadd");
    assert_eq!(
        admitted.descriptor().descriptor_symbol().as_str(),
        "vecadd.kd"
    );
    assert_eq!(admitted.physical_kernel().name(), "vecadd");
    assert_eq!(admitted.physical_kernel().symbol(), "vecadd.kd");
    assert_eq!(admitted.descriptor_binding().kernel_index(), 0);
    assert_eq!(admitted.target().to_string(), "gfx942:xnack-");
    assert_eq!(admitted.code_object_version().number(), 6);
    assert!(admitted.authenticates_descriptor_source());
    assert!(!admitted.authenticates_compiler_origin());
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
    admitted.revalidate_currentness().unwrap();

    let authenticated = AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
        admitted,
        &mut ReviewedTestWorkerV3Verifier {
            substitute_finalized: false,
        },
    )
    .unwrap();
    assert_eq!(
        authenticated.descriptor().kernel_id().as_bytes(),
        &[0xa1; 32]
    );
    assert_eq!(authenticated.target().to_string(), "gfx942:xnack-");
    assert!(authenticated.authenticates_verification_authority());
    assert!(!authenticated.grants_load_authority());
    assert!(!authenticated.grants_launch_authority());
    authenticated.revalidate_currentness().unwrap();
}

#[test]
fn v3_host_admission_rejects_an_unknown_kernel_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes([0xff; 32]),
            &observed,
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)
    ));
}

#[test]
fn v3_host_admission_rejects_incompatible_observed_target_features() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack+");
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes([0xa1; 32]),
            &observed,
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::ObservedTargetMismatch)
    ));
}

#[test]
fn v3_verification_rejects_a_substituted_finalized_hsaco_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    assert!(matches!(
        AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
            admitted,
            &mut ReviewedTestWorkerV3Verifier {
                substitute_finalized: true,
            },
        ),
        Err(WorkerV3VerificationAuthenticationErrorV1::Decision(
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("finalized HSACO")
        ))
    ));
}
