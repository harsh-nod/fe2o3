#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use std::{
    convert::Infallible,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use ed25519_dalek::SigningKey;
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifact_transaction::{
    BuildAttempt, DurablePublishedClaimReacquisitionErrorV3, DurablePublishedHsacoClaimV3,
    InertCompilerExecutionSubjectV1, WorkerV3LoadReadinessReceiptV1,
    reacquire_current_hsaco_publication_lease_v3,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, DigestAlgorithm,
    DigestBytes, Mutability, Name, PayloadDigest, PointerWidth,
};
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::__generated::load_admitted_worker_v3_application_v1;
use fe2o3_host::{
    __hardware_test::{
        application_handoff_observed_context_fixture_v1,
        generated_shared_f32_argument_pair_fixture_v1,
    },
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedArgumentLayoutV1,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
    CompilerGeneratedWorkerV3ArgumentsV1, GeneratedArgumentLayoutError, GeneratedArgumentPackError,
    GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedWorkerV3ArgumentBindingV1,
    GeneratedWorkerV3PrepareErrorV1, HsaAgentIdentityV1, HsaCodeObjectLoadObservationV1,
    HsaDispatchObservationV1, HsaEnvironmentMismatch, HsaEnvironmentObservationV1,
    HsaExecutableObjectIdentityV1, HsaImplicitKernargInitializationObservationV1,
    HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1, HsaLaunchGeometryV1,
    HsaPhysicalDeviceIdentityV1, HsaRuntimeIdentityV1, HsaUnloadObservationV1,
    ProductionWorkerV3ApplicationLoadErrorV1, RecoveredWorkerV3AdmissionErrorV1,
    ReviewedHsaExecutableLifecycleAdapterV1, ReviewedHsaImplicitKernargAdapterV1,
    WorkerV3AuditorV1, WorkerV3CompilerExecutionVerificationV1, WorkerV3GeneratedDispatchErrorV1,
    WorkerV3HsaLoadAuthorizationErrorV1, WorkerV3ProtectedVerificationEvidenceV1,
    WorkerV3ProtectedVerifierAdapterV1, WorkerV3ProtectedVerifierBackendV1,
    WorkerV3SafetyPropertiesV1, WorkerV3SyntheticVerifierAdapterV1, WorkerV3SyntheticVerifierV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerificationDecisionErrorV1,
    WorkerV3VerificationDecisionV1, WorkerV3VerificationRequestV1,
    admit_recovered_worker_v3_descriptor_v1, audit_recovered_worker_v3_verification_v1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_runtime_protocol::{
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationV1,
    RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeWireV2,
    recover_worker_v3_load_envelope_v2,
};
use sha2::{Digest as _, Sha256};

#[path = "fixtures/worker_v3_hsaco_admission.rs"]
mod worker_v3_fixture;

const TEST_MARKER_BINDING: [u8; 32] = [0xa1; 32];
const TEST_HOST_CONTRACT: [u8; 32] = [0xb2; 32];

fn carriage_for_subject(
    subject: &InertCompilerExecutionSubjectV1,
    seed: u8,
) -> CompilerExecutionReceiptCarriageV1 {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let policy = CompilerExecutionIssuerPolicyV1::new(
        u64::from(seed),
        CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 12_345).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 67_890).unwrap(),
        signing_key.verifying_key().to_bytes(),
        SigningKey::from_bytes(&[seed.wrapping_add(1); 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    let challenge =
        CompilerExecutionAttestationChallengeV1::new(&policy, subject, [seed + 3; 32], 1, [0; 32])
            .unwrap();
    let request = CompilerExecutionAttestationRequestV1::new(challenge, subject.clone()).unwrap();
    let receipt =
        CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &signing_key).unwrap();
    let publication =
        CompilerExecutionReceiptPublicationV1::new([seed + 4; 32], [seed + 5; 32], receipt)
            .unwrap();
    let acknowledgment =
        CompilerExecutionReceiptPublicationAckV1::new(&publication, [seed + 6; 32]).unwrap();
    CompilerExecutionReceiptCarriageV1::new(policy, request, publication, acknowledgment).unwrap()
}

struct StaticV3ApplicationFixtures {
    host_consumer: PathBuf,
    hostile: PathBuf,
    no_protocol: PathBuf,
}

fn static_v3_application_fixtures() -> &'static StaticV3ApplicationFixtures {
    static FIXTURES: OnceLock<StaticV3ApplicationFixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = std::env::temp_dir().join(format!(
            "cargo-fe2o3-v3-static-host-consumer-{}",
            std::process::id()
        ));
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut static_rustflags = std::env::var_os("RUSTFLAGS").unwrap_or_default();
        if !static_rustflags.is_empty() {
            static_rustflags.push(" ");
        }
        static_rustflags.push("-C target-feature=+crt-static");
        let built = Command::new(cargo)
            .current_dir(workspace)
            .env_remove("RUSTFLAGS")
            .env(
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
                static_rustflags,
            )
            .env("FE2O3_HIP_SYS_DISABLE", "1")
            .args([
                "build",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--target-dir",
            ])
            .arg(&target)
            .args([
                "-p",
                "cargo-fe2o3",
                "--features",
                "worker-v3-host-consumer-fixture,application-handoff-adversarial-fixture",
                "--bin",
                "cargo-fe2o3-worker-v3-host-consumer-app-fixture",
                "--bin",
                "cargo-fe2o3-runner-app-fixture",
                "--bin",
                "cargo-fe2o3-runner-chain-fixture",
            ])
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "failed to build static V3 host consumer: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let directory = target.join("x86_64-unknown-linux-gnu/debug");
        StaticV3ApplicationFixtures {
            host_consumer: directory.join("cargo-fe2o3-worker-v3-host-consumer-app-fixture"),
            hostile: directory.join("cargo-fe2o3-runner-app-fixture"),
            no_protocol: directory.join("cargo-fe2o3-runner-chain-fixture"),
        }
    })
}

fn static_host_consumer_application_fixture() -> &'static Path {
    &static_v3_application_fixtures().host_consumer
}

fn static_hostile_application_fixture() -> &'static Path {
    &static_v3_application_fixtures().hostile
}

fn static_no_protocol_application_fixture() -> &'static Path {
    &static_v3_application_fixtures().no_protocol
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

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
        CompilerGeneratedKernelProfileV1::new(TEST_HOST_CONTRACT);
    const KERNEL_BINDING_ID_V1: [u8; 32] = TEST_MARKER_BINDING;
}

struct WorkerV3VecAddArguments<'allocation> {
    owner: &'allocation (),
    address: usize,
    length: usize,
}

// SAFETY: this integration fixture mirrors the independently produced descriptor's one exact
// shared-`f32` source argument and retains the inert allocation owner through completion.
unsafe impl<'allocation> CompilerGeneratedWorkerV3ArgumentsV1<'allocation, WorkerV3VecAddMarker>
    for WorkerV3VecAddArguments<'allocation>
{
    fn generated_argument_layout_v1()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        CompilerGeneratedArgumentLayoutV1::new(
            16,
            8,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    Name::new("values").unwrap(),
                    0,
                    16,
                    8,
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4,
                    },
                    Mutability::Immutable,
                    Access::ReadOnly,
                    AddressSpace::Global,
                    <f32 as GeneratedDeviceScalarV1>::shared_slice_type_identity_v1(
                        PointerWidth::Bits64,
                    ),
                    ArgumentOwnership::SharedBorrow,
                    AliasClass::SharedReadOnly,
                )
                .unwrap(),
            ],
        )
    }

    fn bind_arguments_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedWorkerV3ArgumentBindingV1<'allocation>, GeneratedArgumentPackError> {
        // SAFETY: the inert numeric allocation is retained by `self.owner` for this integration
        // test and is never dereferenced by either fake runtime stage.
        let values = unsafe {
            generated_shared_f32_argument_pair_fixture_v1(
                &application_handoff_observed_context_fixture_v1("gfx942:xnack-"),
                self.owner,
                plan,
                0,
                self.address,
                self.length,
            )
        };
        Ok(
            GeneratedWorkerV3ArgumentBindingV1::from_compiler_generated_parts_v1(
                vec![],
                vec![values],
            ),
        )
    }
}

#[derive(Clone, Copy)]
enum ReviewedTestWorkerV3VerifierFault {
    None,
    FinalizedHsaco,
    CompilerSubject,
    CompilerCarriage,
    CompilerPolicy,
    IssuerJournal,
    CompilerOccurrence,
    Receipt,
    Publication,
    Acknowledgment,
    WorkerLedger,
    Sequence,
    PriorRollbackAnchor,
    CurrentRollbackAnchor,
    ZeroCurrentRecordVerification,
    ZeroCurrentRecordAttestation,
    ZeroProtectedPolicyVerification,
    ZeroProtectedWorkerLedgerVerification,
    ZeroExternalRollbackVerification,
}

struct ReviewedTestWorkerV3Verifier {
    fault: ReviewedTestWorkerV3VerifierFault,
}

struct CurrentnessProbingWorkerV3Verifier {
    output_dir: PathBuf,
    claim: DurablePublishedHsacoClaimV3,
    observed_busy: bool,
}

struct ReviewedTestWorkerV3Auditor;

impl<K> WorkerV3AuditorV1<K> for ReviewedTestWorkerV3Auditor
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = Infallible;
    type Evidence = ([u8; 32], u64);

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error> {
        let finalized_sha256: [u8; 32] = Sha256::digest(request.finalized_hsaco_bytes()).into();
        assert_eq!(finalized_sha256, request.finalized_hsaco_sha256());
        assert_eq!(
            u64::try_from(request.finalized_hsaco_bytes().len()).unwrap(),
            request.finalized_hsaco_length()
        );
        Ok((finalized_sha256, request.finalized_hsaco_length()))
    }
}

// SAFETY: this synthetic verifier is confined to test-only fixtures. It mirrors every requested
// identity and must never be used as production proof authority.
unsafe impl<K> WorkerV3SyntheticVerifierV1<K> for ReviewedTestWorkerV3Verifier
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = Infallible;

    unsafe fn verify_synthetic(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        let capsule = request.semantic_compiler_handoff().capsule();
        assert_eq!(*capsule.identity().sha256(), request.capsule_sha256());
        assert_eq!(capsule.canonical_bytes(), request.semantic_capsule_bytes());
        let finalized_sha256: [u8; 32] = Sha256::digest(request.finalized_hsaco_bytes()).into();
        assert_eq!(finalized_sha256, request.finalized_hsaco_sha256());
        assert_eq!(
            u64::try_from(request.finalized_hsaco_bytes().len()).unwrap(),
            request.finalized_hsaco_length()
        );
        assert_eq!(
            *capsule.receipts().formal_memory().identity().sha256(),
            request.formal_memory_receipt_sha256()
        );
        assert_eq!(
            capsule.receipts().formal_memory().canonical_preimage(),
            request.formal_memory_receipt_bytes()
        );
        assert_eq!(
            *capsule.receipts().proof_binding().identity().sha256(),
            request.proof_binding_receipt_sha256()
        );
        assert_eq!(
            capsule.receipts().proof_binding().canonical_preimage(),
            request.proof_binding_receipt_bytes()
        );
        let proof_inputs = request.validate_compiler_proof_inputs_v4().unwrap();
        assert!(proof_inputs.has_exact_decoded_input_association());
        assert!(proof_inputs.has_structural_mir_to_kir_correspondence());
        assert!(proof_inputs.authenticates_signed_verus_receipt_under_embedded_key());
        assert!(!proof_inputs.authenticates_compiler_origin());
        assert!(!proof_inputs.establishes_llvm_or_machine_refinement());
        assert!(!proof_inputs.grants_runtime_authority());
        assert_eq!(
            request
                .compiler_execution_receipt_carriage()
                .request()
                .subject(),
            request.compiler_execution_subject()
        );
        assert!(
            request
                .compiler_execution_subject()
                .identity()
                .matches_canonical_bytes(request.compiler_execution_subject_bytes())
        );
        let decoded_carriage =
            CompilerExecutionReceiptCarriageV1::decode(request.compiler_execution_receipt_bytes())
                .unwrap();
        assert!(decoded_carriage == *request.compiler_execution_receipt_carriage());
        let mut finalized = request.finalized_hsaco_sha256();
        let mut subject = request.compiler_execution_subject_sha256();
        let mut carriage = request.compiler_execution_carriage_sha256();
        let mut policy = request.compiler_execution_policy_sha256();
        let mut issuer_journal = request.compiler_execution_issuer_journal_sha256();
        let mut compiler_occurrence = request.compiler_occurrence_sha256();
        let mut receipt = request.compiler_execution_receipt_sha256();
        let mut publication = request.compiler_execution_publication_sha256();
        let mut acknowledgment = request.compiler_execution_acknowledgment_sha256();
        let mut worker_ledger = request.compiler_execution_worker_ledger_record_sha256();
        let mut sequence = request.compiler_execution_sequence();
        let mut prior_rollback = request.compiler_execution_prior_rollback_anchor();
        let mut current_rollback = request.compiler_execution_current_rollback_anchor();
        let mut current_record_verification = [0xd4; 32];
        let mut current_record_attestation = [0xd5; 32];
        let mut protected_policy_verification = [0xd1; 32];
        let mut protected_worker_ledger_verification = [0xd2; 32];
        let mut external_rollback_verification = [0xd3; 32];
        match self.fault {
            ReviewedTestWorkerV3VerifierFault::None => {}
            ReviewedTestWorkerV3VerifierFault::FinalizedHsaco => finalized[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::CompilerSubject => subject[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::CompilerCarriage => carriage[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::CompilerPolicy => policy[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::IssuerJournal => issuer_journal[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::CompilerOccurrence => {
                compiler_occurrence[0] ^= 0xff;
            }
            ReviewedTestWorkerV3VerifierFault::Receipt => receipt[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::Publication => publication[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::Acknowledgment => acknowledgment[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::WorkerLedger => worker_ledger[0] ^= 0xff,
            ReviewedTestWorkerV3VerifierFault::Sequence => sequence = sequence.wrapping_add(1),
            ReviewedTestWorkerV3VerifierFault::PriorRollbackAnchor => {
                prior_rollback[0] ^= 0xff;
            }
            ReviewedTestWorkerV3VerifierFault::CurrentRollbackAnchor => {
                current_rollback[0] ^= 0xff;
            }
            ReviewedTestWorkerV3VerifierFault::ZeroCurrentRecordVerification => {
                current_record_verification = [0; 32];
            }
            ReviewedTestWorkerV3VerifierFault::ZeroCurrentRecordAttestation => {
                current_record_attestation = [0; 32];
            }
            ReviewedTestWorkerV3VerifierFault::ZeroProtectedPolicyVerification => {
                protected_policy_verification = [0; 32];
            }
            ReviewedTestWorkerV3VerifierFault::ZeroProtectedWorkerLedgerVerification => {
                protected_worker_ledger_verification = [0; 32];
            }
            ReviewedTestWorkerV3VerifierFault::ZeroExternalRollbackVerification => {
                external_rollback_verification = [0; 32];
            }
        }
        let compiler_execution = WorkerV3CompilerExecutionVerificationV1::synthetic_for_test_only(
            subject,
            carriage,
            policy,
            issuer_journal,
            compiler_occurrence,
            receipt,
            publication,
            acknowledgment,
            worker_ledger,
            sequence,
            prior_rollback,
            current_rollback,
            current_record_verification,
            current_record_attestation,
            protected_policy_verification,
            protected_worker_ledger_verification,
            external_rollback_verification,
        );
        Ok(WorkerV3VerificationDecisionV1::synthetic_for_test_only(
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
            compiler_execution,
            [0xc1; 32],
            [0xc2; 32],
            [0xc3; 32],
            [0xc4; 32],
            [0xc5; 32],
            WorkerV3SafetyPropertiesV1::required(),
        ))
    }
}

#[derive(Clone, Copy)]
enum ReviewedTestProtectedVerifierFault {
    None,
    CompilerSubject,
    ZeroVerificationTranscript,
}

struct ReviewedTestProtectedVerifier {
    fault: ReviewedTestProtectedVerifierFault,
}

// SAFETY: this request-echoing backend exists only in the receipt-bearing integration-test
// binary. It deliberately exercises the production adapter's field mapping and rejection paths
// and must never be represented as protected compiler, ledger, rollback, or proof authority.
unsafe impl<K> WorkerV3ProtectedVerifierBackendV1<K> for ReviewedTestProtectedVerifier
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = Infallible;

    unsafe fn verify_protected(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3ProtectedVerificationEvidenceV1, Self::Error> {
        let mut subject = request.compiler_execution_subject_sha256();
        let mut verification_transcript = [0xc2; 32];
        match self.fault {
            ReviewedTestProtectedVerifierFault::None => {}
            ReviewedTestProtectedVerifierFault::CompilerSubject => subject[0] ^= 0xff,
            ReviewedTestProtectedVerifierFault::ZeroVerificationTranscript => {
                verification_transcript = [0; 32];
            }
        }
        let compiler_execution = WorkerV3CompilerExecutionVerificationV1::synthetic_for_test_only(
            subject,
            request.compiler_execution_carriage_sha256(),
            request.compiler_execution_policy_sha256(),
            request.compiler_execution_issuer_journal_sha256(),
            request.compiler_occurrence_sha256(),
            request.compiler_execution_receipt_sha256(),
            request.compiler_execution_publication_sha256(),
            request.compiler_execution_acknowledgment_sha256(),
            request.compiler_execution_worker_ledger_record_sha256(),
            request.compiler_execution_sequence(),
            request.compiler_execution_prior_rollback_anchor(),
            request.compiler_execution_current_rollback_anchor(),
            [0xd1; 32],
            [0xd2; 32],
            [0xd3; 32],
            [0xd4; 32],
            [0xd5; 32],
        );
        let proof_inputs = request
            .validate_compiler_proof_inputs_v4()
            .expect("the integration fixture carries canonical compiler proof inputs");
        // SAFETY: this test-only backend deliberately supplies complete synthetic identities so
        // the sealed adapter's exact mapping and fail-closed validation can be exercised. The
        // proof-input owner was decoded from the exact borrowed request above.
        Ok(unsafe {
            WorkerV3ProtectedVerificationEvidenceV1::new(
                compiler_execution,
                proof_inputs,
                [0xc1; 32],
                verification_transcript,
                [0xc3; 32],
                [0xc4; 32],
                [0xc5; 32],
                WorkerV3SafetyPropertiesV1::required(),
            )
        })
    }
}

// SAFETY: this test-only protected backend probes the cooperative publication lock before
// delegating to the complete protected-evidence fixture above.
unsafe impl<K> WorkerV3ProtectedVerifierBackendV1<K> for CurrentnessProbingWorkerV3Verifier
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = Infallible;

    unsafe fn verify_protected(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3ProtectedVerificationEvidenceV1, Self::Error> {
        self.observed_busy = matches!(
            reacquire_current_hsaco_publication_lease_v3(&self.output_dir, &self.claim),
            Err(DurablePublishedClaimReacquisitionErrorV3::Busy)
        );
        assert!(self.observed_busy);
        // SAFETY: both implementations satisfy the same test-only protected-backend contract.
        unsafe {
            ReviewedTestProtectedVerifier {
                fault: ReviewedTestProtectedVerifierFault::None,
            }
            .verify_protected(request)
        }
    }
}

#[derive(Debug)]
struct ReviewedTestHsaExecutable {
    identity: HsaExecutableObjectIdentityV1,
}

#[derive(Debug)]
struct ReviewedTestHsaKernel {
    identity: HsaKernelObjectIdentityV1,
}

#[derive(Default)]
struct ReviewedTestHsaState {
    unloads: AtomicUsize,
    implicit_initializations: AtomicUsize,
    dispatches: AtomicUsize,
    dispatched_kernarg: Mutex<Option<Vec<u8>>>,
    dispatched_geometry: Mutex<Option<HsaLaunchGeometryV1>>,
    fault: Mutex<ReviewedTestHsaFault>,
}

#[derive(Clone, Copy, Default)]
enum ReviewedTestHsaFault {
    #[default]
    None,
    ImplicitError,
    MutateExplicit,
    ImplicitKernel,
    DispatchError,
    DispatchIncomplete,
}

struct ReviewedTestHsaAdapter {
    environment: HsaEnvironmentObservationV1,
    state: Arc<ReviewedTestHsaState>,
    substitute_load_digest: bool,
}

impl ReviewedTestHsaAdapter {
    fn new() -> (Self, Arc<ReviewedTestHsaState>) {
        let target = AmdTargetId::parse("gfx942:sramecc+:xnack-").unwrap();
        let runtime = HsaRuntimeIdentityV1::new(
            "test-hsa",
            "v1",
            PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([0xd1; 32])),
            [0xd2; 16],
        )
        .unwrap();
        let physical = HsaPhysicalDeviceIdentityV1::new([0xd3; 16], 1, 0, target).unwrap();
        let agent =
            HsaAgentIdentityV1::new(runtime.instance(), 0xd4, physical.uuid(), target).unwrap();
        let environment = HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap();
        let state = Arc::new(ReviewedTestHsaState::default());
        (
            Self {
                environment,
                state: state.clone(),
                substitute_load_digest: false,
            },
            state,
        )
    }

    fn with_substituted_load_digest() -> (Self, Arc<ReviewedTestHsaState>) {
        let (mut adapter, state) = Self::new();
        adapter.substitute_load_digest = true;
        (adapter, state)
    }

    fn executable_identity() -> HsaExecutableObjectIdentityV1 {
        HsaExecutableObjectIdentityV1::new([0xd5; 32]).unwrap()
    }
}

// SAFETY: this test adapter is deterministic, synchronous, and retains no native authority.
unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for ReviewedTestHsaAdapter {
    type Executable = ReviewedTestHsaExecutable;
    type Kernel = ReviewedTestHsaKernel;
    type Error = &'static str;

    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error> {
        Ok(self.environment.clone())
    }

    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        let identity = Self::executable_identity();
        let observed_digest = if self.substitute_load_digest {
            PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([0xdf; 32]))
        } else {
            finalized_digest
        };
        Ok((
            ReviewedTestHsaExecutable { identity },
            HsaCodeObjectLoadObservationV1::new(
                observed_digest,
                u64::try_from(bytes.len()).unwrap(),
                self.environment.runtime().instance(),
                self.environment.agent().agent_handle(),
                identity,
            ),
        ))
    }

    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        let identity = HsaKernelObjectIdentityV1::new([0xd6; 32]).unwrap();
        Ok((
            ReviewedTestHsaKernel { identity },
            HsaKernelResolutionObservationV1::new(
                executable.identity,
                identity,
                export_symbol,
                272,
                16,
                0,
                0,
            )
            .unwrap(),
        ))
    }

    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        self.state.dispatches.fetch_add(1, Ordering::SeqCst);
        let fault = *self.state.fault.lock().unwrap();
        if matches!(fault, ReviewedTestHsaFault::DispatchError) {
            return Err("fixture dispatch failure");
        }
        *self.state.dispatched_kernarg.lock().unwrap() = Some(kernarg.to_vec());
        *self.state.dispatched_geometry.lock().unwrap() = Some(geometry);
        HsaDispatchObservationV1::new(
            [0xd7; 16],
            executable.identity,
            kernel.identity,
            geometry,
            !matches!(fault, ReviewedTestHsaFault::DispatchIncomplete),
        )
        .map_err(|_| "invalid fixture dispatch observation")
    }

    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        self.state.unloads.fetch_add(1, Ordering::SeqCst);
        Ok(HsaUnloadObservationV1::new(
            executable.identity,
            self.environment.runtime().instance(),
            self.environment.agent().agent_handle(),
            true,
        ))
    }
}

// SAFETY: this fake initializer preserves the explicit prefix, initializes the complete supplied
// suffix synchronously, and reports only identities derived from the exact private handles.
unsafe impl ReviewedHsaImplicitKernargAdapterV1 for ReviewedTestHsaAdapter {
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
        self.state
            .implicit_initializations
            .fetch_add(1, Ordering::SeqCst);
        let fault = *self.state.fault.lock().unwrap();
        if matches!(fault, ReviewedTestHsaFault::ImplicitError) {
            return Err("fixture implicit initialization failure");
        }
        kernarg[implicit_byte_offset..implicit_byte_offset + implicit_byte_len].fill(0xa5);
        if matches!(fault, ReviewedTestHsaFault::MutateExplicit) {
            kernarg[0] ^= 0xff;
        }
        let kernel_identity = if matches!(fault, ReviewedTestHsaFault::ImplicitKernel) {
            HsaKernelObjectIdentityV1::new([0xde; 32]).unwrap()
        } else {
            kernel.identity
        };
        Ok(HsaImplicitKernargInitializationObservationV1::new(
            executable.identity,
            kernel_identity,
            geometry,
            u64::try_from(explicit_byte_len).unwrap(),
            u64::try_from(implicit_byte_offset).unwrap(),
            u64::try_from(implicit_byte_len).unwrap(),
            true,
        ))
    }
}

fn recovered_host_fixture() -> (
    worker_v3_fixture::TestDirectory,
    RecoveredWorkerV3LoadEnvelopeV2,
) {
    recover_published_worker_v3_fixture(worker_v3_fixture::published_worker_v3_fixture())
}

fn recover_published_worker_v3_fixture(
    fixture: worker_v3_fixture::PublishedWorkerV3Fixture,
) -> (
    worker_v3_fixture::TestDirectory,
    RecoveredWorkerV3LoadEnvelopeV2,
) {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = fixture;
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        published,
        carriage_for_subject(&subject, 0x71),
    )
    .unwrap();
    let intent = envelope
        .wire()
        .replay()
        .publication_intent_record()
        .identity();
    let readiness = envelope
        .persist_durable_replay_custody_v2(&directory.0)
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
    let recovered = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    (directory, recovered)
}

struct PreparedV3ApplicationFixture {
    directory: worker_v3_fixture::TestDirectory,
    attempt: BuildAttempt,
    readiness: WorkerV3LoadReadinessReceiptV1,
    envelope_path: PathBuf,
    exact_envelope: Vec<u8>,
    kernel: PathBuf,
}

fn prepared_v3_application_fixture() -> PreparedV3ApplicationFixture {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        published,
        carriage_for_subject(&subject, 0x72),
    )
    .unwrap();
    let intent = envelope
        .wire()
        .replay()
        .publication_intent_record()
        .identity();
    let exact_envelope = envelope.encode_canonical().unwrap();
    let readiness = envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    let readiness_receipt = readiness.receipt();
    let envelope_path = readiness.envelope_path().to_path_buf();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness_receipt,
    )
    .unwrap();
    drop(envelope);

    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let mut owner = b"fe2o3-owned-v1\0".to_vec();
    owner.extend_from_slice(&[0x55; 16]);
    let owner_path = directory.0.join(".fe2o3-owned-v1");
    fs::write(&owner_path, owner).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();

    let kernel = directory.0.join("v3-application.kernel-id");
    fs::write(&kernel, "a1".repeat(32)).unwrap();
    PreparedV3ApplicationFixture {
        directory,
        attempt,
        readiness: readiness_receipt,
        envelope_path,
        exact_envelope,
        kernel,
    }
}

fn v3_application_runner_command_for(
    fixture: &PreparedV3ApplicationFixture,
    application: &Path,
) -> Command {
    v3_application_runner_command_for_context(fixture, application, "3-test-envelope-only")
}

fn v3_application_runner_command_for_context(
    fixture: &PreparedV3ApplicationFixture,
    application: &Path,
    runner_context: &str,
) -> Command {
    let metadata = fs::metadata(&fixture.directory.0).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .arg("__fe2o3-runner-v1")
        .arg(runner_context)
        .arg(lower_hex(
            fixture.directory.0.as_os_str().as_encoded_bytes(),
        ))
        .arg(metadata.dev().to_string())
        .arg(metadata.ino().to_string())
        .arg("required")
        .arg("0")
        .arg(application);
    command
}

fn v3_application_runner_command(fixture: &PreparedV3ApplicationFixture, report: &Path) -> Command {
    let mut command =
        v3_application_runner_command_for(fixture, static_host_consumer_application_fixture());
    command
        .arg(&fixture.kernel)
        .arg("gfx942:xnack-")
        .arg(report);
    command
}

fn v3_hostile_runner_command(fixture: &PreparedV3ApplicationFixture, report: &Path) -> Command {
    let mut command =
        v3_application_runner_command_for(fixture, static_hostile_application_fixture());
    command.arg(report).arg("worker-v3-application-payload");
    command
}

fn v3_fast_failure_hostile_runner_command(
    fixture: &PreparedV3ApplicationFixture,
    report: &Path,
) -> Command {
    let mut command = v3_application_runner_command_for_context(
        fixture,
        static_hostile_application_fixture(),
        "3-test-fast-failures",
    );
    command.arg(report).arg("worker-v3-application-payload");
    command
}

#[test]
fn cargo_supervisor_and_static_host_consumer_complete_strict_v3_handoff() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("v3-application-report.json");
    let completed = v3_application_runner_command(&fixture, &report)
        .output()
        .unwrap();
    assert!(
        completed.status.success(),
        "strict V3 application handoff failed: {}; report: {}",
        String::from_utf8_lossy(&completed.stderr),
        fs::read_to_string(&report).unwrap_or_else(|error| format!("unavailable ({error})"))
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(report["host_consumer"], true);
    assert_eq!(report["loader_environment_clear"], true);
    assert_eq!(report["admitted"], true);
    assert_eq!(report["current"], true);

    let recovered =
        recover_worker_v3_load_envelope_v2(&fixture.directory.0, fixture.attempt).unwrap();
    assert_eq!(recovered.receipt(), fixture.readiness);
}

#[test]
fn strict_v3_host_consumer_rejects_substituted_commitment() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("substituted-commitment.json");
    let rejected = v3_application_runner_command(&fixture, &report)
        .arg("--fe2o3-test-substitute-commitment")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let report: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["host_consumer"], true);
    assert_eq!(report["loader_environment_clear"], true);
    assert_eq!(report["admitted"], false);
}

#[test]
fn strict_v3_handoff_rejects_a_symlinked_envelope_before_spawn() {
    use std::os::unix::fs::symlink;

    let fixture = prepared_v3_application_fixture();
    let saved = fixture.envelope_path.with_extension("saved");
    fs::rename(&fixture.envelope_path, &saved).unwrap();
    symlink(saved.file_name().unwrap(), &fixture.envelope_path).unwrap();
    let report = fixture.directory.0.join("symlinked-envelope.json");
    let rejected = v3_application_runner_command(&fixture, &report)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(!report.exists(), "application must not be spawned");
}

#[test]
fn strict_v3_handoff_rejects_truncated_and_extended_envelopes_before_spawn() {
    for trailing_byte in [false, true] {
        let fixture = prepared_v3_application_fixture();
        let bytes = if trailing_byte {
            let mut bytes = fixture.exact_envelope.clone();
            bytes.push(0);
            bytes
        } else {
            fixture.exact_envelope[..fixture.exact_envelope.len() - 1].to_vec()
        };
        fs::write(&fixture.envelope_path, bytes).unwrap();
        let report = fixture.directory.0.join(if trailing_byte {
            "extended-envelope.json"
        } else {
            "truncated-envelope.json"
        });
        let rejected = v3_application_runner_command(&fixture, &report)
            .output()
            .unwrap();
        assert!(!rejected.status.success());
        assert!(!report.exists(), "application must not be spawned");
    }
}

#[test]
fn strict_v3_handoff_closes_unrelated_inheritable_descriptors() {
    use std::fs::File;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    const PROBE_FD: i32 = 199;
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("close-range-report.json");
    let source = File::open("/dev/null").unwrap();
    let source_fd = source.as_raw_fd();
    let mut command = v3_hostile_runner_command(&fixture, &report);
    command
        .arg("--fe2o3-test-probe-fd")
        .arg(PROBE_FD.to_string());
    // SAFETY: the callback creates one intentionally inheritable descriptor in the runner child.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(source_fd, PROBE_FD) != PROBE_FD
                || libc::fcntl(PROBE_FD, libc::F_SETFD, 0) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let completed = command.output().unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["probe_fd_open"], false);
    assert_eq!(report["handoff"]["acknowledged"], true);
}

#[test]
fn strict_v3_public_ack_does_not_claim_child_currentness_authority() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("public-ack-report.json");
    let completed = v3_hostile_runner_command(&fixture, &report)
        .arg("--fe2o3-test-public-ack-without-reacquire")
        .output()
        .unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["handoff"]["acknowledged"], true);
    assert_eq!(report["handoff"]["child_reacquired_currentness"], false);
}

#[test]
fn strict_v3_seccomp_rejects_process_and_session_escape() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("seccomp-process-report.json");
    let escape_marker = fixture.directory.0.join("double-fork-setsid-escaped");
    let completed = v3_hostile_runner_command(&fixture, &report)
        .arg("--fe2o3-test-seccomp-process-probe")
        .arg(&escape_marker)
        .output()
        .unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    for probe in [
        "fork",
        "vfork",
        "clone",
        "clone3",
        "unshare",
        "setns",
        "setsid",
        "io_uring",
        "double_fork_setsid",
    ] {
        assert_eq!(report["handoff"]["process_creation"][probe], "EPERM");
    }
    assert!(!escape_marker.exists());
}

#[test]
fn strict_v3_seccomp_rejects_static_and_dynamic_exec_replacement() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("seccomp-exec-report.json");
    let completed = v3_hostile_runner_command(&fixture, &report)
        .arg("--fe2o3-test-exec-replacement-probe")
        .arg(static_no_protocol_application_fixture())
        .arg("/bin/true")
        .output()
        .unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    for probe in [
        "static_execve",
        "static_execveat",
        "dynamic_execve",
        "dynamic_execveat",
    ] {
        assert_eq!(report["handoff"]["exec_replacement"][probe], "EPERM");
    }
}

#[test]
fn strict_v3_handoff_rejects_child_protocol_substitution_and_omission() {
    let fixture = prepared_v3_application_fixture();
    for probe in [
        "--fe2o3-test-reuse-handoff-fd",
        "--fe2o3-test-reuse-artifact-dir-fd",
        "--fe2o3-test-substitute-commitment",
        "--fe2o3-test-ignore-handoff",
        "--fe2o3-test-premature-close-ack",
        "--fe2o3-test-extra-ack-byte",
    ] {
        let report = fixture.directory.0.join(format!("rejected-{probe}.json"));
        let rejected = v3_fast_failure_hostile_runner_command(&fixture, &report)
            .arg(probe)
            .output()
            .unwrap();
        assert!(
            !rejected.status.success(),
            "{probe} unexpectedly passed: {}",
            String::from_utf8_lossy(&rejected.stderr)
        );
    }

    let report = fixture.directory.0.join("absent-child-protocol.json");
    let rejected = v3_application_runner_command_for_context(
        &fixture,
        static_no_protocol_application_fixture(),
        "3-test-fast-failures",
    )
    .arg(&report)
    .output()
    .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("acknowledgment"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
}

#[test]
fn strict_v3_handoff_rejects_replaced_generation_directory() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("replaced-generation-report.json");
    let mut command = v3_hostile_runner_command(&fixture, &report);
    let original = fixture.directory.0.clone();
    let moved = original.with_extension("original");
    fs::rename(&original, &moved).unwrap();
    fs::create_dir(&original).unwrap();
    let rejected = command.output().unwrap();
    fs::remove_dir(&original).unwrap();
    fs::rename(&moved, &original).unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("identity was substituted"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
}

fn process_cpu_ticks(process: u32) -> u64 {
    let stat = fs::read_to_string(format!("/proc/{process}/stat")).unwrap();
    let fields = stat
        .rsplit_once(')')
        .expect("process stat command field")
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap()
}

#[test]
fn strict_v3_stalled_ack_times_out_without_spinning_and_reaps_application() {
    let fixture = prepared_v3_application_fixture();
    let report = fixture.directory.0.join("stalled-ack-report.json");
    let ready = fixture.directory.0.join("stalled-ack-ready");
    let mut command = v3_application_runner_command_for_context(
        &fixture,
        static_hostile_application_fixture(),
        "3-test-short-timeouts",
    );
    command
        .arg(&report)
        .arg("worker-v3-application-payload")
        .arg("--fe2o3-test-stall-before-ack")
        .arg(&ready)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let child = command.spawn().unwrap();
    let runner = child.id();

    let deadline = Instant::now() + Duration::from_secs(60);
    while !ready.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "application did not reach stalled ACK");
    let ack_started = Instant::now();
    let application = fs::read_to_string(&ready)
        .unwrap()
        .trim()
        .parse::<u32>()
        .unwrap();
    let before = process_cpu_ticks(runner);
    thread::sleep(Duration::from_millis(500));
    let consumed = process_cpu_ticks(runner).saturating_sub(before);
    // SAFETY: `_SC_CLK_TCK` is a scalar process-configuration query.
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    assert!(ticks_per_second > 0);
    assert!(
        consumed <= (ticks_per_second as u64 / 10).max(1),
        "stalled ACK polling consumed {consumed} CPU ticks in 500 ms"
    );

    let rejected = child.wait_with_output().unwrap();
    let ack_elapsed = ack_started.elapsed();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("application handoff acknowledgment timed out"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(ack_elapsed >= Duration::from_secs(1));
    assert!(ack_elapsed < Duration::from_secs(15));
    assert!(started.elapsed() < Duration::from_secs(90));
    assert!(
        !Path::new(&format!("/proc/{application}")).exists(),
        "timed-out application was not killed and reaped"
    );
}

#[test]
fn strict_v3_handoff_rejects_stale_envelope_after_publication_turnover() {
    let directory = worker_v3_fixture::TestDirectory::new();
    let first = worker_v3_fixture::publish_worker_v3_fixture_in_directory(&directory, 0x61);
    let first_subject = first.published.compiler_execution_subject_v1().unwrap();
    let first_envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        first.published,
        carriage_for_subject(&first_subject, 0x74),
    )
    .unwrap();
    let first_intent = first_envelope
        .wire()
        .replay()
        .publication_intent_record()
        .identity();
    let first_exact_envelope = first_envelope.encode_canonical().unwrap();
    let first_readiness = first_envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    let first_path = first_readiness.envelope_path().to_path_buf();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &first.producer,
        first.attempt,
        first_intent,
        first_readiness.receipt(),
    )
    .unwrap();
    drop(first_envelope);

    let second = worker_v3_fixture::publish_worker_v3_fixture_in_directory(&directory, 0x62);
    let second_subject = second.published.compiler_execution_subject_v1().unwrap();
    let second_envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        second.published,
        carriage_for_subject(&second_subject, 0x75),
    )
    .unwrap();
    let second_intent = second_envelope
        .wire()
        .replay()
        .publication_intent_record()
        .identity();
    let exact_envelope = second_envelope.encode_canonical().unwrap();
    let second_readiness = second_envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    let second_path = second_readiness.envelope_path().to_path_buf();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &second.producer,
        second.attempt,
        second_intent,
        second_readiness.receipt(),
    )
    .unwrap();
    drop(second_envelope);
    assert_ne!(first_path, second_path);
    fs::write(&first_path, first_exact_envelope).unwrap();
    fs::set_permissions(&first_path, fs::Permissions::from_mode(0o600)).unwrap();

    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let mut owner = b"fe2o3-owned-v1\0".to_vec();
    owner.extend_from_slice(&[0x55; 16]);
    let owner_path = directory.0.join(".fe2o3-owned-v1");
    fs::write(&owner_path, owner).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let kernel = directory.0.join("v3-application.kernel-id");
    fs::write(&kernel, "a1".repeat(32)).unwrap();
    let fixture = PreparedV3ApplicationFixture {
        directory,
        attempt: second.attempt,
        readiness: second_readiness.receipt(),
        envelope_path: second_path,
        exact_envelope,
        kernel,
    };

    let current_report = fixture.directory.0.join("current-after-turnover.json");
    let current = v3_hostile_runner_command(&fixture, &current_report)
        .output()
        .unwrap();
    assert!(
        current.status.success(),
        "{}",
        String::from_utf8_lossy(&current.stderr)
    );

    fs::remove_file(&fixture.envelope_path).unwrap();
    let stale_report = fixture.directory.0.join("stale-after-turnover.json");
    let stale = v3_hostile_runner_command(&fixture, &stale_report)
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(!stale_report.exists());
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("current"),
        "{}",
        String::from_utf8_lossy(&stale.stderr)
    );
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

    let subject = published.compiler_execution_subject_v1().unwrap();
    let compiler_execution = carriage_for_subject(&subject, 0x73);
    let envelope =
        WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(published, compiler_execution.clone())
            .unwrap();
    assert_eq!(envelope.exact_artifact_bytes(), exact_artifact);
    assert!(!envelope.grants_load_authority());
    assert!(!envelope.grants_launch_authority());

    let canonical = envelope.encode_canonical().unwrap();
    let inert = WorkerV3LoadEnvelopeWireV2::decode_canonical(&canonical).unwrap();
    inert
        .validate_reacquired_publication_lease_v2(envelope.current_publication_lease())
        .unwrap();
    assert_eq!(inert.encode_canonical().unwrap(), canonical);
    assert_eq!(inert.compiler_execution_receipt(), &compiler_execution);
    assert!(!inert.replay().grants_publication_authority());
    assert!(!inert.grants_load_authority());
    assert!(!inert.grants_launch_authority());

    let intent = inert.replay().publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v2(&output_dir)
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

    let recovered = recover_worker_v3_load_envelope_v2(&output_dir, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
    assert_eq!(recovered.wire().encode_canonical().unwrap(), canonical);
    assert_eq!(
        recovered.wire().compiler_execution_receipt(),
        &compiler_execution
    );
    assert_eq!(recovered.exact_artifact_bytes(), exact_artifact);
    assert!(!recovered.authenticates_compiler_origin());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xa1; 32]))
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

    let (adapter, adapter_state) = ReviewedTestHsaAdapter::new();
    let mut loaded = load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
        admitted,
        &observed,
        &mut WorkerV3SyntheticVerifierAdapterV1::new(ReviewedTestWorkerV3Verifier {
            fault: ReviewedTestWorkerV3VerifierFault::None,
        }),
        adapter,
    )
    .unwrap();
    assert!(!loaded.grants_load_authority());
    assert!(!loaded.grants_launch_authority());
    assert_eq!(loaded.kernel_observation().export_symbol(), "vecadd");
    loaded.revalidate_currentness().unwrap();

    let owner = ();
    for rejected_geometry in [
        HsaLaunchGeometryV1::new([0, 1, 1], [64, 1, 1], 0),
        HsaLaunchGeometryV1::new([5, 1, 1], [257, 1, 1], 0),
        HsaLaunchGeometryV1::new([5, 1, 1], [64, 1, 1], 1),
    ] {
        match loaded.prepare_generated_worker_v3_v1(
            &observed,
            rejected_geometry,
            WorkerV3VecAddArguments {
                owner: &owner,
                address: 0x10_000,
                length: 257,
            },
        ) {
            Err(GeneratedWorkerV3PrepareErrorV1::LaunchAuthorization(_)) => {}
            Err(other) => panic!("unexpected rejected-geometry error: {other:?}"),
            Ok(_) => panic!("rejected geometry unexpectedly prepared"),
        }
    }
    assert_eq!(
        adapter_state
            .implicit_initializations
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(adapter_state.dispatches.load(Ordering::SeqCst), 0);

    let geometry = HsaLaunchGeometryV1::new([5, 1, 1], [64, 1, 1], 0);
    for fault in [
        ReviewedTestHsaFault::ImplicitError,
        ReviewedTestHsaFault::MutateExplicit,
        ReviewedTestHsaFault::ImplicitKernel,
        ReviewedTestHsaFault::DispatchError,
        ReviewedTestHsaFault::DispatchIncomplete,
    ] {
        *adapter_state.fault.lock().unwrap() = fault;
        let result = loaded
            .prepare_generated_worker_v3_v1(
                &observed,
                geometry,
                WorkerV3VecAddArguments {
                    owner: &owner,
                    address: 0x10_000,
                    length: 257,
                },
            )
            .unwrap()
            .dispatch();
        let rejected_at_expected_stage = matches!(
            (fault, result),
            (
                ReviewedTestHsaFault::ImplicitError,
                Err(WorkerV3GeneratedDispatchErrorV1::ImplicitAdapter(_)),
            ) | (
                ReviewedTestHsaFault::MutateExplicit,
                Err(WorkerV3GeneratedDispatchErrorV1::ExplicitKernargMutation),
            ) | (
                ReviewedTestHsaFault::ImplicitKernel,
                Err(WorkerV3GeneratedDispatchErrorV1::ImplicitObservationMismatch(_)),
            ) | (
                ReviewedTestHsaFault::DispatchError,
                Err(WorkerV3GeneratedDispatchErrorV1::DispatchAdapter(_)),
            ) | (
                ReviewedTestHsaFault::DispatchIncomplete,
                Err(WorkerV3GeneratedDispatchErrorV1::DispatchObservationMismatch(_)),
            )
        );
        assert!(rejected_at_expected_stage);
    }
    assert_eq!(
        adapter_state
            .implicit_initializations
            .load(Ordering::SeqCst),
        5
    );
    assert_eq!(adapter_state.dispatches.load(Ordering::SeqCst), 2);
    *adapter_state.fault.lock().unwrap() = ReviewedTestHsaFault::None;

    let prepared = loaded
        .prepare_generated_worker_v3_v1(
            &observed,
            geometry,
            WorkerV3VecAddArguments {
                owner: &owner,
                address: 0x10_000,
                length: 257,
            },
        )
        .unwrap();
    assert_eq!(prepared.geometry(), geometry);
    assert_eq!(prepared.explicit_byte_len(), 16);
    assert_eq!(prepared.implicit_byte_len(), 256);
    assert_eq!(prepared.physical_kernarg_byte_len(), 272);
    assert_eq!(prepared.physical_kernarg_alignment(), 16);

    let completed = prepared.dispatch().unwrap();
    assert_eq!(completed.kernel_id().as_bytes(), &[0xa1; 32]);
    assert_eq!(completed.completed_dispatch().geometry(), geometry);
    assert!(completed.completed_dispatch().dispatch().completed());
    loaded.revalidate_currentness().unwrap();
    assert_eq!(
        adapter_state
            .implicit_initializations
            .load(Ordering::SeqCst),
        6
    );
    assert_eq!(adapter_state.dispatches.load(Ordering::SeqCst), 3);
    assert_eq!(
        *adapter_state.dispatched_geometry.lock().unwrap(),
        Some(geometry)
    );
    let kernarg_guard = adapter_state.dispatched_kernarg.lock().unwrap();
    let kernarg = kernarg_guard.as_ref().unwrap();
    assert_eq!(&kernarg[..8], &0x10_000_u64.to_le_bytes());
    assert_eq!(&kernarg[8..16], &257_u64.to_le_bytes());
    assert!(kernarg[16..].iter().all(|byte| *byte == 0xa5));
    drop(kernarg_guard);

    let unloaded = loaded.unload().unwrap();
    assert!(unloaded.unload_observation().released());
    assert!(!unloaded.grants_load_authority());
    assert!(!unloaded.grants_launch_authority());
    assert_eq!(adapter_state.unloads.load(Ordering::SeqCst), 1);
}

#[test]
fn v3_host_admission_rejects_an_unknown_kernel_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xff; 32])),
        Err(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)
    ));
}

#[test]
fn v3_host_load_rejects_incompatible_observed_target_features() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack+");
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xa1; 32]))
            .unwrap();
    assert!(matches!(
        load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
            admitted,
            &observed,
            &mut WorkerV3SyntheticVerifierAdapterV1::new(ReviewedTestWorkerV3Verifier {
                fault: ReviewedTestWorkerV3VerifierFault::None,
            }),
            ReviewedTestHsaAdapter::new().0,
        ),
        Err(ProductionWorkerV3ApplicationLoadErrorV1::LoadAuthorization(
            WorkerV3HsaLoadAuthorizationErrorV1::Environment(HsaEnvironmentMismatch::Target { .. })
        ))
    ));
}

#[test]
fn borrowed_v3_audit_preserves_exact_admission_custody_without_authority() {
    let (_directory, recovered) = recovered_host_fixture();
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xa1; 32]))
            .unwrap();
    let lineage = admitted.lineage_identity();
    let (finalized_sha256, finalized_length) = audit_recovered_worker_v3_verification_v1::<
        WorkerV3VecAddMarker,
        _,
    >(&admitted, &mut ReviewedTestWorkerV3Auditor)
    .unwrap();
    assert_ne!(finalized_sha256, [0; 32]);
    assert_ne!(finalized_length, 0);
    assert_eq!(admitted.lineage_identity(), lineage);
    admitted.revalidate_currentness().unwrap();
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn protected_verifier_adapter_maps_independent_evidence_into_authentication() {
    let (_directory, recovered) = recovered_host_fixture();
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes(TEST_MARKER_BINDING),
    )
    .unwrap();
    let mut verifier = WorkerV3ProtectedVerifierAdapterV1::new(ReviewedTestProtectedVerifier {
        fault: ReviewedTestProtectedVerifierFault::None,
    });
    let authenticated = AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
        admitted,
        &mut verifier,
    )
    .unwrap();
    assert!(authenticated.authenticates_verification_authority());
    let proof_inputs = authenticated
        .verification()
        .validated_compiler_proof_inputs()
        .expect("the protected adapter retains exact V4 proof inputs");
    assert!(proof_inputs.authenticates_signed_verus_receipt_under_embedded_key());
    assert!(!proof_inputs.authenticates_compiler_origin());
    assert!(!proof_inputs.establishes_llvm_or_machine_refinement());
    assert!(!proof_inputs.grants_runtime_authority());
    assert!(
        authenticated
            .verification()
            .retains_current_compiler_and_signed_verus_evidence()
    );
    assert!(!authenticated.grants_load_authority());
    assert!(!authenticated.grants_launch_authority());
}

#[test]
fn protected_verifier_adapter_rejects_substituted_and_zero_evidence() {
    for (fault, expected) in [
        (
            ReviewedTestProtectedVerifierFault::CompilerSubject,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("compiler-execution subject"),
        ),
        (
            ReviewedTestProtectedVerifierFault::ZeroVerificationTranscript,
            WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(
                "verification transcript",
            ),
        ),
    ] {
        let (_directory, recovered) = recovered_host_fixture();
        let admitted = admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes(TEST_MARKER_BINDING),
        )
        .unwrap();
        let mut verifier =
            WorkerV3ProtectedVerifierAdapterV1::new(ReviewedTestProtectedVerifier { fault });
        let error = AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
            admitted,
            &mut verifier,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WorkerV3VerificationAuthenticationErrorV1::Decision(actual) if actual == expected
        ));
    }
}

#[test]
fn authenticated_v3_executable_retains_verifier_entry_currentness_until_drop() {
    let (directory, recovered) = recovered_host_fixture();
    let claim = recovered.wire().published_claim().clone();
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xa1; 32]))
            .unwrap();
    let mut verifier =
        WorkerV3ProtectedVerifierAdapterV1::new(CurrentnessProbingWorkerV3Verifier {
            output_dir: directory.0.clone(),
            claim: claim.clone(),
            observed_busy: false,
        });
    let authenticated = AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
        admitted,
        &mut verifier,
    )
    .unwrap();
    let verifier = verifier.into_inner();
    assert!(verifier.observed_busy);
    authenticated.revalidate_currentness().unwrap();
    assert!(
        authenticated
            .verification()
            .validated_compiler_proof_inputs()
            .is_some()
    );
    assert!(
        authenticated
            .verification()
            .retains_current_compiler_and_signed_verus_evidence()
    );

    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v3(&directory.0, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV3::Busy)
    ));

    drop(authenticated);
    reacquire_current_hsaco_publication_lease_v3(&directory.0, &claim).unwrap();
}

#[test]
fn v3_verification_rejects_a_substituted_finalized_hsaco_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xa1; 32]))
            .unwrap();
    assert!(matches!(
        load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
            admitted,
            &observed,
            &mut WorkerV3SyntheticVerifierAdapterV1::new(ReviewedTestWorkerV3Verifier {
                fault: ReviewedTestWorkerV3VerifierFault::FinalizedHsaco,
            }),
            ReviewedTestHsaAdapter::new().0,
        ),
        Err(ProductionWorkerV3ApplicationLoadErrorV1::Verification(
            WorkerV3VerificationAuthenticationErrorV1::Decision(
                WorkerV3VerificationDecisionErrorV1::IdentityMismatch("finalized HSACO")
            )
        ))
    ));
}

#[test]
fn v3_verification_rejects_every_compiler_execution_substitution_and_missing_authority() {
    for (fault, expected) in [
        (
            ReviewedTestWorkerV3VerifierFault::CompilerSubject,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("compiler-execution subject"),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::CompilerCarriage,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("compiler-execution carriage"),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::CompilerPolicy,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("compiler-execution policy"),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::IssuerJournal,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution issuer journal",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::CompilerOccurrence,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("compiler occurrence"),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::Receipt,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch("compiler-execution receipt"),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::Publication,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution receipt publication",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::Acknowledgment,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution publication acknowledgment",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::WorkerLedger,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution Worker ledger record",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::Sequence,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution rollback sequence",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::PriorRollbackAnchor,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution prior rollback anchor",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::CurrentRollbackAnchor,
            WorkerV3VerificationDecisionErrorV1::IdentityMismatch(
                "compiler-execution current rollback anchor",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::ZeroCurrentRecordVerification,
            WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(
                "compiler current-record verification",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::ZeroCurrentRecordAttestation,
            WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(
                "compiler current-record attestation",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::ZeroProtectedPolicyVerification,
            WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(
                "protected compiler policy verification",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::ZeroProtectedWorkerLedgerVerification,
            WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(
                "protected Worker ledger verification",
            ),
        ),
        (
            ReviewedTestWorkerV3VerifierFault::ZeroExternalRollbackVerification,
            WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(
                "external rollback verification",
            ),
        ),
    ] {
        let (_directory, recovered) = recovered_host_fixture();
        let admitted = admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes(TEST_MARKER_BINDING),
        )
        .unwrap();
        let error = AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
            admitted,
            &mut WorkerV3SyntheticVerifierAdapterV1::new(ReviewedTestWorkerV3Verifier { fault }),
        )
        .unwrap_err();
        match error {
            WorkerV3VerificationAuthenticationErrorV1::Decision(actual) => {
                assert_eq!(actual, expected);
            }
            other => panic!("unexpected verification failure: {other:?}"),
        }
    }
}

#[test]
fn v3_hsa_load_rejects_and_cleans_up_a_substituted_adapter_digest() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes([0xa1; 32]))
            .unwrap();
    let (adapter, adapter_state) = ReviewedTestHsaAdapter::with_substituted_load_digest();
    assert!(matches!(
        load_admitted_worker_v3_application_v1::<WorkerV3VecAddMarker, _, _>(
            admitted,
            &observed,
            &mut WorkerV3SyntheticVerifierAdapterV1::new(ReviewedTestWorkerV3Verifier {
                fault: ReviewedTestWorkerV3VerifierFault::None,
            }),
            adapter,
        ),
        Err(ProductionWorkerV3ApplicationLoadErrorV1::ExecutableLoad(
            fe2o3_host::WorkerV3HsaExecutableLoadErrorV1::LoadObservationMismatch {
                field: "finalized digest"
            }
        ))
    ));
    assert_eq!(adapter_state.unloads.load(Ordering::SeqCst), 1);
}
