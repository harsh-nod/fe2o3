#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use std::{
    convert::Infallible,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifact_transaction::retire_worker_v3_publication_intent_after_load_readiness_v1;
use fe2o3_artifacts::{DigestAlgorithm, DigestBytes, PayloadDigest};
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{
    __hardware_test::application_handoff_observed_context_fixture_v1,
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKernelProfileV1, CompilerGeneratedSemanticWitnessErrorV1, HsaAgentIdentityV1,
    HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1, HsaEnvironmentObservationV1,
    HsaExecutableObjectIdentityV1, HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1,
    HsaLaunchGeometryV1, HsaPhysicalDeviceIdentityV1, HsaRuntimeIdentityV1, HsaUnloadObservationV1,
    RecoveredWorkerV3AdmissionErrorV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ValidatedCompilerGeneratedSemanticWitnessV1, WorkerV3SafetyPropertiesV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerificationDecisionErrorV1,
    WorkerV3VerificationDecisionV1, WorkerV3VerificationRequestV1, WorkerV3VerifierV1,
    admit_recovered_worker_v3_descriptor_v1, semantic_witness_from_backend_v1,
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

fn static_host_consumer_application_fixture() -> &'static Path {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = std::env::temp_dir().join(format!(
            "cargo-fe2o3-v3-static-host-consumer-{}",
            std::process::id()
        ));
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let built = Command::new(cargo)
            .current_dir(workspace)
            .env(
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
                "-C target-feature=+crt-static",
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
                "host-consumer-fixture",
                "--bin",
                "cargo-fe2o3-host-consumer-app-fixture",
            ])
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "failed to build static V3 host consumer: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        target.join("x86_64-unknown-linux-gnu/debug/cargo-fe2o3-host-consumer-app-fixture")
    })
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

#[derive(Debug)]
struct ReviewedTestHsaExecutable {
    identity: HsaExecutableObjectIdentityV1,
}

#[derive(Debug)]
struct ReviewedTestHsaKernel;

struct ReviewedTestHsaAdapter {
    environment: HsaEnvironmentObservationV1,
    unloads: Arc<AtomicUsize>,
    substitute_load_digest: bool,
}

impl ReviewedTestHsaAdapter {
    fn new() -> (Self, Arc<AtomicUsize>) {
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
        let unloads = Arc::new(AtomicUsize::new(0));
        (
            Self {
                environment,
                unloads: unloads.clone(),
                substitute_load_digest: false,
            },
            unloads,
        )
    }

    fn with_substituted_load_digest() -> (Self, Arc<AtomicUsize>) {
        let (mut adapter, unloads) = Self::new();
        adapter.substitute_load_digest = true;
        (adapter, unloads)
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
        Ok((
            ReviewedTestHsaKernel,
            HsaKernelResolutionObservationV1::new(
                executable.identity,
                HsaKernelObjectIdentityV1::new([0xd6; 32]).unwrap(),
                export_symbol,
                272,
                16,
            )
            .unwrap(),
        ))
    }

    unsafe fn launch_and_wait(
        &mut self,
        _executable: &Self::Executable,
        _kernel: &Self::Kernel,
        _geometry: HsaLaunchGeometryV1,
        _kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        Err("V3 launch authority is intentionally unavailable")
    }

    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        self.unloads.fetch_add(1, Ordering::SeqCst);
        Ok(HsaUnloadObservationV1::new(
            executable.identity,
            self.environment.runtime().instance(),
            self.environment.agent().agent_handle(),
            true,
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
fn cargo_supervisor_and_static_host_consumer_complete_strict_v3_handoff() {
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

    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let mut owner = b"fe2o3-owned-v1\0".to_vec();
    owner.extend_from_slice(&[0x55; 16]);
    let owner_path = directory.0.join(".fe2o3-owned-v1");
    fs::write(&owner_path, owner).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();

    let kernel = directory.0.join("v3-application.kernel-id");
    let report = directory.0.join("v3-application-report.json");
    fs::write(&kernel, "a1".repeat(32)).unwrap();
    let metadata = fs::metadata(&directory.0).unwrap();
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    let runner_context = "3-test-scheduler-tolerant";
    #[cfg(not(feature = "worker-v2-fault-injection-test-only"))]
    let runner_context = "3";
    let completed = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .arg("__fe2o3-runner-v1")
        .arg(runner_context)
        .arg(lower_hex(directory.0.as_os_str().as_encoded_bytes()))
        .arg(metadata.dev().to_string())
        .arg(metadata.ino().to_string())
        .arg("required")
        .arg("0")
        .arg(static_host_consumer_application_fixture())
        .arg("--worker-v3")
        .arg(&kernel)
        .arg("gfx942:xnack-")
        .arg(&report)
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

    let recovered = recover_worker_v3_load_envelope_v1(&directory.0, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
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

    let (adapter, unloads) = ReviewedTestHsaAdapter::new();
    let authorized = authenticated.authorize_hsa_load(adapter).unwrap();
    assert!(authorized.grants_load_authority());
    assert!(!authorized.grants_launch_authority());
    let loaded = authorized.load().unwrap();
    assert!(!loaded.grants_load_authority());
    assert!(!loaded.grants_launch_authority());
    assert_eq!(loaded.kernel_observation().export_symbol(), "vecadd");
    loaded.revalidate_currentness().unwrap();
    let unloaded = loaded.unload().unwrap();
    assert!(unloaded.unload_observation().released());
    assert!(!unloaded.grants_load_authority());
    assert!(!unloaded.grants_launch_authority());
    assert_eq!(unloads.load(Ordering::SeqCst), 1);
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

#[test]
fn v3_hsa_load_rejects_and_cleans_up_a_substituted_adapter_digest() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    let authenticated = AuthenticatedWorkerV3ExecutableV1::<WorkerV3VecAddMarker>::authenticate(
        admitted,
        &mut ReviewedTestWorkerV3Verifier {
            substitute_finalized: false,
        },
    )
    .unwrap();
    let (adapter, unloads) = ReviewedTestHsaAdapter::with_substituted_load_digest();
    assert!(matches!(
        authenticated.authorize_hsa_load(adapter).unwrap().load(),
        Err(
            fe2o3_host::WorkerV3HsaExecutableLoadErrorV1::LoadObservationMismatch {
                field: "finalized digest"
            }
        )
    ));
    assert_eq!(unloads.load(Ordering::SeqCst), 1);
}
